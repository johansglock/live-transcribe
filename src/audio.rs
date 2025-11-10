use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Stream, StreamConfig};
use std::sync::{Arc, Mutex};

const WHISPER_SAMPLE_RATE: u32 = 16000;
const SLIDING_WINDOW_DURATION_MS: u64 = 5000; // Keep 5 seconds of context

pub struct AudioCapture {
    device: Device,
    config: StreamConfig,
    buffer: Arc<Mutex<Vec<f32>>>,
    sliding_window: Arc<Mutex<Vec<f32>>>, // Last 5 seconds for context
    stream: Option<Stream>,
    last_chunk_time: Arc<Mutex<std::time::Instant>>,
}

impl AudioCapture {
    pub fn new() -> Result<Self> {
        let host = cpal::default_host();

        // Get default input device
        let device = host
            .default_input_device()
            .context("No input device available")?;

        println!("Using audio input device: {}", device.name()?);

        // Get the default input config
        let default_config = device
            .default_input_config()
            .context("Failed to get default input config")?;

        println!("Default config: {:?}", default_config);

        // Try to find a supported config close to what we want
        let supported_configs = device
            .supported_input_configs()
            .context("Failed to query supported input configs")?;

        println!("Supported configs:");
        for (i, config) in supported_configs.enumerate() {
            println!("  {}: {:?}", i, config);
        }

        // Use the default config but with our desired sample rate if supported
        let mut config: StreamConfig = default_config.clone().into();

        // Check if 16kHz is supported
        let supported_configs = device.supported_input_configs()?;
        let mut found_16k = false;

        for supported_config in supported_configs {
            if supported_config.min_sample_rate().0 <= WHISPER_SAMPLE_RATE
                && supported_config.max_sample_rate().0 >= WHISPER_SAMPLE_RATE {
                found_16k = true;
                config.sample_rate = cpal::SampleRate(WHISPER_SAMPLE_RATE);
                break;
            }
        }

        if !found_16k {
            println!("Warning: 16kHz not supported, using default sample rate: {}", config.sample_rate.0);
            println!("Audio will be resampled during transcription");
        }

        println!(
            "Final audio config: {} channels, {} Hz, {:?}",
            config.channels, config.sample_rate.0, default_config.sample_format()
        );

        Ok(AudioCapture {
            device,
            config,
            buffer: Arc::new(Mutex::new(Vec::new())),
            sliding_window: Arc::new(Mutex::new(Vec::new())),
            stream: None,
            last_chunk_time: Arc::new(Mutex::new(std::time::Instant::now())),
        })
    }

    pub fn get_chunk_if_ready(&self, chunk_duration_ms: u64) -> Option<(Vec<f32>, usize)> {
        let mut last_time = self.last_chunk_time.lock().unwrap();
        let now = std::time::Instant::now();

        if now.duration_since(*last_time).as_millis() >= chunk_duration_ms as u128 {
            *last_time = now;

            // Minimize time holding the buffer lock - swap out the data instead of cloning
            let new_chunk = {
                let mut buffer = self.buffer.lock().unwrap();
                let buffer_len = buffer.len();
                eprintln!("📦 get_chunk_if_ready: buffer has {} samples", buffer_len);
                if buffer.is_empty() {
                    eprintln!("⚠️  Buffer is empty, returning None");
                    return None;
                }
                // Use mem::take to swap out the buffer without cloning
                // This replaces buffer with an empty Vec and gives us the old contents
                std::mem::take(&mut *buffer)
            }; // Lock released here

            // Resample AFTER releasing the lock to avoid blocking audio thread
            let actual_sample_rate = self.config.sample_rate.0;
            let resampled_new = if actual_sample_rate != WHISPER_SAMPLE_RATE {
                Self::resample(&new_chunk, actual_sample_rate, WHISPER_SAMPLE_RATE)
            } else {
                new_chunk
            };

            let new_samples_count = resampled_new.len();

            // Add to sliding window and return result
            // Note: The clone is necessary here because we need to:
            // 1. Return the audio data to the caller for transcription
            // 2. Keep it in the sliding window for the next chunk
            // At 320KB every 300ms, modern systems handle this fine (~1MB/s allocation rate)
            let result = {
                let mut sliding_window = self.sliding_window.lock().unwrap();

                // Optimization: Reserve capacity before extending to avoid reallocation
                let needed_capacity = sliding_window.len() + resampled_new.len();
                if sliding_window.capacity() < needed_capacity {
                    sliding_window.reserve(resampled_new.len());
                }
                sliding_window.extend_from_slice(&resampled_new);

                // Keep only the last 5 seconds (SLIDING_WINDOW_DURATION_MS)
                let max_samples = (WHISPER_SAMPLE_RATE as u64 * SLIDING_WINDOW_DURATION_MS / 1000) as usize;
                if sliding_window.len() > max_samples {
                    let excess = sliding_window.len() - max_samples;
                    sliding_window.drain(0..excess);
                }

                // Pad to at least 1 second for Whisper
                let min_samples = WHISPER_SAMPLE_RATE as usize;

                // Build output efficiently - clone with known capacity
                if sliding_window.len() >= min_samples {
                    // Clone is necessary but we can optimize it by cloning with the right capacity
                    let mut output = Vec::with_capacity(sliding_window.len());
                    output.extend_from_slice(&sliding_window);
                    (output, new_samples_count)
                } else {
                    // Need padding - create vec with required capacity
                    let mut padded_window = Vec::with_capacity(min_samples);
                    padded_window.extend_from_slice(&sliding_window);
                    padded_window.resize(min_samples, 0.0);
                    (padded_window, new_samples_count)
                }
            };

            Some(result)
        } else {
            None
        }
    }

    pub fn start_recording(&mut self) -> Result<()> {
        if self.stream.is_some() {
            return Ok(()); // Already recording
        }

        // Clear the buffers
        self.buffer.lock().unwrap().clear();
        self.sliding_window.lock().unwrap().clear();

        let buffer = Arc::clone(&self.buffer);
        let channels = self.config.channels as usize;

        // Track if we're receiving audio
        let sample_counter = Arc::new(Mutex::new(0usize));
        let counter_clone = Arc::clone(&sample_counter);

        let err_fn = |err| eprintln!("🔴 Audio stream error: {}", err);

        // Build the input stream
        let stream = self
            .device
            .build_input_stream(
                &self.config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    // Debug: log every second of audio received
                    if let Ok(mut counter) = counter_clone.lock() {
                        *counter += data.len();
                        if *counter >= 48000 { // ~1 second at 48kHz
                            eprintln!("🎤 Audio callback: received {}+ samples", *counter);
                            *counter = 0;
                        }
                    }

                    // Handle poisoned mutex gracefully in audio callback
                    let Ok(mut buf) = buffer.lock() else {
                        eprintln!("⚠️  Audio buffer mutex poisoned, dropping audio data");
                        return;
                    };

                    // Convert to mono if needed and store samples
                    if channels == 1 {
                        buf.extend_from_slice(data);
                    } else {
                        // Average channels to get mono
                        for chunk in data.chunks(channels) {
                            let mono_sample: f32 = chunk.iter().sum::<f32>() / channels as f32;
                            buf.push(mono_sample);
                        }
                    }
                },
                err_fn,
                None,
            )
            .context("Failed to build input stream.\n\nThis is likely a microphone permissions issue.\nPlease grant microphone access:\n  1. Open System Settings → Privacy & Security → Microphone\n  2. Enable access for Terminal (or your terminal app)\n  3. Restart the app")?;

        stream.play().context("Failed to start audio stream")?;

        self.stream = Some(stream);
        println!("Recording started");

        Ok(())
    }

    pub fn stop_recording(&mut self) -> Result<Vec<f32>> {
        if let Some(stream) = self.stream.take() {
            drop(stream);
            println!("Recording stopped");
        }

        let buffer = self.buffer.lock().unwrap();
        let audio_data = buffer.clone();
        drop(buffer); // Release lock before processing

        let actual_sample_rate = self.config.sample_rate.0;

        println!("Captured {} samples ({:.2}s of audio at {}Hz)",
            audio_data.len(),
            audio_data.len() as f32 / actual_sample_rate as f32,
            actual_sample_rate
        );

        // Resample if needed
        if actual_sample_rate != WHISPER_SAMPLE_RATE {
            println!("Resampling from {}Hz to {}Hz...", actual_sample_rate, WHISPER_SAMPLE_RATE);
            let resampled = Self::resample(&audio_data, actual_sample_rate, WHISPER_SAMPLE_RATE);
            println!("Resampled to {} samples ({:.2}s)",
                resampled.len(),
                resampled.len() as f32 / WHISPER_SAMPLE_RATE as f32
            );
            Ok(resampled)
        } else {
            Ok(audio_data)
        }
    }

    // Simple linear interpolation resampling
    fn resample(input: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
        if from_rate == to_rate {
            return input.to_vec();
        }

        let ratio = from_rate as f64 / to_rate as f64;
        let output_len = (input.len() as f64 / ratio) as usize;
        let mut output = Vec::with_capacity(output_len);

        for i in 0..output_len {
            let src_idx = i as f64 * ratio;
            let src_idx_floor = src_idx.floor() as usize;
            let src_idx_ceil = (src_idx_floor + 1).min(input.len() - 1);
            let frac = src_idx - src_idx_floor as f64;

            // Linear interpolation
            let sample = input[src_idx_floor] * (1.0 - frac) as f32
                + input[src_idx_ceil] * frac as f32;

            output.push(sample);
        }

        output
    }

    pub fn is_recording(&self) -> bool {
        self.stream.is_some()
    }

    // Simple energy-based silence detection
    pub fn is_silence(audio: &[f32], threshold: f32) -> bool {
        if audio.is_empty() {
            return true;
        }

        // Calculate RMS (Root Mean Square) energy
        let sum_squares: f32 = audio.iter().map(|&x| x * x).sum();
        let rms = (sum_squares / audio.len() as f32).sqrt();

        rms < threshold
    }
}

impl Drop for AudioCapture {
    fn drop(&mut self) {
        let _ = self.stop_recording();
    }
}
