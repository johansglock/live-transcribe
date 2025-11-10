// Hybrid VAD + Live Preview Streaming Simulation
// This module simulates the hybrid streaming approach for testing

use crate::transcription::Transcriber;

#[derive(Debug, Clone)]
pub struct KeyboardAction {
    pub delete_count: usize,
    pub type_text: String,
}

pub struct HybridVadResult {
    pub vad_transcriptions: Vec<String>,  // Ground truth from VAD commits
    pub live_transcriptions: Vec<String>, // Live preview outputs
    pub final_text: String,               // What would be typed
    pub chunks_processed: usize,
    pub keyboard_actions: Vec<KeyboardAction>, // All keyboard actions taken
    pub simulated_screen_text: String,    // What should actually appear on screen
}

pub fn simulate_hybrid_vad(
    audio_data: &[f32],
    transcriber: &Transcriber,
    chunk_duration_ms: u64,
    silence_threshold: f32,
) -> HybridVadResult {
    let samples_per_chunk = (16000 * chunk_duration_ms / 1000) as usize;
    let window_duration_ms = 5000; // 5 second sliding window
    let max_window_samples = (16000 * window_duration_ms / 1000) as usize;

    let mut vad_buffer: Vec<f32> = Vec::new();
    let mut vad_committed_text = String::new();
    let mut vad_transcriptions = Vec::new();
    let mut live_transcriptions = Vec::new();
    let mut keyboard_actions = Vec::new();

    // State management (matches main.rs logic)
    let mut live_preview_text = String::new();

    let mut sliding_window: Vec<f32> = Vec::new();
    let mut silence_streak = 0;
    let mut chunks_since_vad_commit = 0;
    let mut chunk_num = 0;

    println!("\n🔄 Simulating hybrid VAD streaming");
    println!("   ({}ms chunks with {}s sliding window)", chunk_duration_ms, window_duration_ms / 1000);
    println!();

    for chunk_start in (0..audio_data.len()).step_by(samples_per_chunk) {
        let chunk_end = (chunk_start + samples_per_chunk).min(audio_data.len());
        let new_audio = &audio_data[chunk_start..chunk_end];

        // Add to sliding window
        sliding_window.extend_from_slice(new_audio);
        if sliding_window.len() > max_window_samples {
            let excess = sliding_window.len() - max_window_samples;
            sliding_window.drain(0..excess);
        }

        // Pad window to at least 1 second
        let min_samples = 16000;
        let mut padded_window = sliding_window.clone();
        if padded_window.len() < min_samples {
            padded_window.resize(min_samples, 0.0);
        }

        chunk_num += 1;

        // Check for silence
        let is_silence = is_silence_chunk(new_audio, silence_threshold);
        let rms = calculate_rms(new_audio);

        if is_silence {
            silence_streak += 1;
            println!("Chunk {}: 🔇 Silence (streak: {}, RMS: {:.4})", chunk_num, silence_streak, rms);

            // VAD commit after 3 silent chunks
            if silence_streak >= 3 && !vad_buffer.is_empty() {
                let buffer_duration = vad_buffer.len() as f32 / 16000.0;
                println!("  💾 VAD: Committing {:.1}s of speech ({} samples)", buffer_duration, vad_buffer.len());

                // Pad VAD buffer to at least 1.5 seconds (whisper.cpp seems to round down)
                let min_samples = 24000; // 1.5 seconds to be safe
                if vad_buffer.len() < min_samples {
                    println!("  ⚠️  Padding buffer from {} to {} samples ({:.1}s)", vad_buffer.len(), min_samples, min_samples as f32 / 16000.0);
                    vad_buffer.resize(min_samples, 0.0);
                }

                println!("  📤 Transcribing {} samples", vad_buffer.len());
                match transcriber.transcribe(&vad_buffer) {
                    Ok(vad_text) => {
                        let vad_text = vad_text.trim().to_string();
                        if !vad_text.is_empty() {
                            println!("  ✅ VAD: \"{}\"", vad_text);
                            vad_transcriptions.push(vad_text.clone());

                            // Simulate the keyboard action (matches main.rs logic)
                            let current_char_count = live_preview_text.chars().count();
                            let new_vad_committed = vad_committed_text.clone() + &vad_text + " ";

                            keyboard_actions.push(KeyboardAction {
                                delete_count: current_char_count,
                                type_text: new_vad_committed.clone(),
                            });

                            vad_committed_text = new_vad_committed.clone();
                            live_preview_text = new_vad_committed;
                        }
                    }
                    Err(e) => {
                        println!("  ❌ VAD error: {}", e);
                    }
                }

                vad_buffer.clear();
                chunks_since_vad_commit = 0;
            }
            continue;
        }

        // Speech detected
        if silence_streak > 0 {
            println!("Chunk {}: 🔊 Speech after {} silent chunks (RMS: {:.4})", chunk_num, silence_streak, rms);
        } else {
            println!("Chunk {}: 🔊 Speech (RMS: {:.4})", chunk_num, rms);
        }
        silence_streak = 0;
        chunks_since_vad_commit += 1;

        // VAD: Accumulate new audio
        vad_buffer.extend_from_slice(new_audio);

        // Live preview after 3 chunks
        if chunks_since_vad_commit >= 3 {
            match transcriber.transcribe(&padded_window) {
                Ok(live_text) => {
                    let live_text = live_text.trim();
                    if !live_text.is_empty() {
                        println!("Chunk {}: 👁️  Live: \"{}\"", chunk_num, live_text);
                        live_transcriptions.push(live_text.to_string());

                        // Simulate the keyboard action (matches main.rs logic)
                        let current_char_count = live_preview_text.chars().count();
                        let full_live_text = vad_committed_text.clone() + live_text;

                        keyboard_actions.push(KeyboardAction {
                            delete_count: current_char_count,
                            type_text: full_live_text.clone(),
                        });

                        live_preview_text = full_live_text;
                    }
                }
                Err(e) => {
                    println!("Chunk {}: ❌ Live error: {}", chunk_num, e);
                }
            }
        }
    }

    // Final VAD commit if there's remaining audio
    if !vad_buffer.is_empty() {
        println!("\n💾 Final VAD commit ({:.1}s remaining)", vad_buffer.len() as f32 / 16000.0);
        let min_samples = 24000; // 1.5 seconds to be safe
        if vad_buffer.len() < min_samples {
            println!("  ⚠️  Padding buffer from {} to {} samples ({:.1}s)", vad_buffer.len(), min_samples, min_samples as f32 / 16000.0);
            vad_buffer.resize(min_samples, 0.0);
        }
        if let Ok(vad_text) = transcriber.transcribe(&vad_buffer) {
            let vad_text = vad_text.trim().to_string();
            if !vad_text.is_empty() {
                println!("✅ Final VAD: \"{}\"", vad_text);
                vad_transcriptions.push(vad_text.clone());

                // Simulate the keyboard action
                let current_char_count = live_preview_text.chars().count();
                let new_vad_committed = vad_committed_text.clone() + &vad_text;

                keyboard_actions.push(KeyboardAction {
                    delete_count: current_char_count,
                    type_text: new_vad_committed.clone(),
                });

                vad_committed_text = new_vad_committed.clone();
                // live_preview_text would be updated to new_vad_committed here,
                // but we're at the end of the loop so we don't need to
            }
        }
    }

    // Simulate what would actually appear on screen by replaying keyboard actions
    let simulated_screen_text = replay_keyboard_actions(&keyboard_actions);

    HybridVadResult {
        vad_transcriptions,
        live_transcriptions,
        final_text: vad_committed_text.trim().to_string(),
        chunks_processed: chunk_num,
        keyboard_actions,
        simulated_screen_text,
    }
}

fn replay_keyboard_actions(actions: &[KeyboardAction]) -> String {
    let mut screen = String::new();

    for action in actions {
        // Delete characters from the end
        if action.delete_count > 0 {
            let chars: Vec<char> = screen.chars().collect();
            let keep_count = chars.len().saturating_sub(action.delete_count);
            screen = chars.iter().take(keep_count).collect();
        }

        // Type new text
        screen.push_str(&action.type_text);
    }

    screen
}

fn calculate_rms(audio: &[f32]) -> f32 {
    if audio.is_empty() {
        return 0.0;
    }
    let sum_squares: f32 = audio.iter().map(|&x| x * x).sum();
    (sum_squares / audio.len() as f32).sqrt()
}

fn is_silence_chunk(audio: &[f32], threshold: f32) -> bool {
    calculate_rms(audio) < threshold
}
