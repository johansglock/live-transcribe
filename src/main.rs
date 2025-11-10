mod audio;
mod config;
mod constants;
mod hotkey;
mod keyboard;
mod model_download;
mod sandbox;
mod text_diff;
mod transcription;
mod transcription_state;
mod transcription_worker;
mod tray;
pub mod hybrid_vad;

use anyhow::Result;
use audio::AudioCapture;
use clap::{Parser, Subcommand};
use config::{Config, TranscriptionConfig};
use hotkey::{HotkeyEvent, HotkeyManager};
use model_download::ModelDownloader;
use transcription::{Transcriber, TranscriberWithState};
use transcription_state::{Action, TranscriptionState};
use transcription_worker::TranscriptionWorker;
use tray::{TrayApp, TrayMenuEvent};
use tao::event_loop::{EventLoop, ControlFlow};
#[cfg(target_os = "macos")]
use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
use std::sync::{Arc, Mutex};

#[derive(Parser)]
#[command(name = "live-transcribe")]
#[command(about = "Live audio transcription system tray app", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Download a Whisper model
    DownloadModel {
        /// Model to download (e.g., base.en, tiny.en, small.en). If not specified, uses the configured model from settings.yaml
        model: Option<String>,
    },
    /// Record test audio for debugging streaming transcription
    TestRecord {
        /// Name for this test case
        #[arg(default_value = "test1")]
        name: String,
        /// Duration to record in seconds
        #[arg(short, long, default_value = "10")]
        duration: u64,
    },
    /// Replay and analyze a saved test recording
    TestReplay {
        /// Name of the test recording to replay
        name: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Handle subcommands
    match cli.command {
        Some(Commands::DownloadModel { model }) => {
            // Don't enable sandbox for download - needs network access
            return download_model_command(&model);
        }
        Some(Commands::TestRecord { name, duration }) => {
            // Don't enable sandbox for test commands - needs file system access
            return test_record_command(&name, duration);
        }
        Some(Commands::TestReplay { name }) => {
            // Don't enable sandbox for test commands
            return test_replay_command(&name);
        }
        None => {
            // Initialize sandbox for main app ONLY
            if let Err(e) = sandbox::macos::init() {
                eprintln!("⚠️  Failed to initialize sandbox: {}", e);
                eprintln!("   Continuing without sandbox (less secure)");
            }

            // Run the main application
            run_app()?;
        }
    }

    Ok(())
}

fn simulate_streaming_transcription(audio_data: &[f32], transcriber: &Transcriber, config: &TranscriptionConfig) {
    println!("🔄 Simulating streaming transcription...");
    println!("   (300ms chunks with 5s sliding window)");
    println!();

    let chunk_duration_ms = config.chunk_duration_ms;
    let samples_per_chunk = (16000 * chunk_duration_ms / 1000) as usize;
    let window_duration_ms = 5000;
    let max_window_samples = (16000 * window_duration_ms / 1000) as usize;

    let mut sliding_window: Vec<f32> = Vec::new();
    let mut committed_words: Vec<String> = Vec::new(); // LOCKED - never delete these
    let mut pending_words: Vec<String> = Vec::new(); // Can still be corrected
    let mut chunk_num = 0;
    let mut silence_streak = 0; // Track consecutive silent chunks
    let mut chunks_since_commit = 0; // Track how long pending words have been stable

    println!("─────────────────────────────────────────────────────");

    for chunk_start in (0..audio_data.len()).step_by(samples_per_chunk) {
        let chunk_end = (chunk_start + samples_per_chunk).min(audio_data.len());
        let chunk = &audio_data[chunk_start..chunk_end];

        // Add to sliding window
        sliding_window.extend_from_slice(chunk);

        // Keep only last 2 seconds
        if sliding_window.len() > max_window_samples {
            let excess = sliding_window.len() - max_window_samples;
            sliding_window.drain(0..excess);
        }

        // Pad to 1 second minimum
        let mut padded_window = sliding_window.clone();
        if padded_window.len() < 16000 {
            padded_window.resize(16000, 0.0);
        }

        chunk_num += 1;
        let time_ms = chunk_start as f32 / 16.0;

        // Check for silence to prevent hallucinations
        if AudioCapture::is_silence(&padded_window, config.silence_threshold) {
            silence_streak += 1;

            // Commit pending words after 2+ silent chunks (600ms pause)
            // This is a natural pause in speech - commit what we have
            if silence_streak >= 2 && !pending_words.is_empty() {
                println!("[{:6.0}ms] Chunk {:2}: (silence - committing {} pending words)",
                         time_ms, chunk_num, pending_words.len());
                committed_words.extend(pending_words.drain(..));
            } else {
                println!("[{:6.0}ms] Chunk {:2}: (silence - skipped)", time_ms, chunk_num);
            }
            continue;
        }

        // Reset silence counter when we have speech
        silence_streak = 0;

        // Transcribe
        match transcriber.transcribe(&padded_window) {
            Ok(current_transcription) => {
                let current_transcription = current_transcription.trim();

                if current_transcription.is_empty() {
                    println!("[{:6.0}ms] Chunk {:2}: (empty)", time_ms, chunk_num);
                    continue;
                }

                let curr_words: Vec<String> = current_transcription
                    .split_whitespace()
                    .map(|s| s.to_string())
                    .collect();

                // VAD-based approach: committed words are LOCKED, pending words can be corrected

                // Helper to strip punctuation for comparison
                let strip_punct = |s: &str| -> String {
                    s.chars().filter(|c| c.is_alphanumeric()).collect()
                };

                // Total words we've output = committed + pending
                let total_output_words = committed_words.len() + pending_words.len();

                // Find how many of our output words match the current transcription
                let mut match_len = 0;
                for i in 0..total_output_words.min(curr_words.len()) {
                    let output_word = if i < committed_words.len() {
                        &committed_words[i]
                    } else {
                        &pending_words[i - committed_words.len()]
                    };

                    if strip_punct(output_word).eq_ignore_ascii_case(&strip_punct(&curr_words[i])) {
                        match_len = i + 1;
                    } else {
                        break;
                    }
                }

                let words_to_delete;
                let words_to_type: Vec<String>;

                // Check if mismatch is in committed words (NEVER delete committed!)
                if match_len < committed_words.len() {
                    // Mismatch in committed region - we CANNOT delete committed words
                    println!("           [Mismatch in committed words - CRITICAL: Whisper lost track]");

                    // We have committed words that don't match current transcription
                    // This means Whisper's sliding window no longer contains the old audio
                    // We MUST keep committed words and NOT delete anything

                    words_to_delete = 0;

                    // Just append whatever Whisper says now (it's probably new speech)
                    // Don't try to find committed words - they're outside the window
                    words_to_type = curr_words.clone();

                    // IMPORTANT: Don't clear pending words! They might still be valid
                    // Only clear pending if we're sure they're obsolete
                    // For now, keep them
                } else {
                    // Match is good through committed words - we can correct pending
                    let pending_match_len = match_len.saturating_sub(committed_words.len());

                    words_to_delete = pending_words.len().saturating_sub(pending_match_len);
                    words_to_type = curr_words[match_len..].to_vec();
                }

                let mut action = String::new();

                if action.is_empty() {
                    // Build action string
                    if words_to_delete > 0 {
                        let delete_start = pending_words.len();
                        let deleted = &pending_words[delete_start.saturating_sub(words_to_delete)..];
                        action.push_str(&format!("⌫ DELETE: \"{}\" | ", deleted.join(" ")));
                    }

                    if !words_to_type.is_empty() {
                        action.push_str(&format!("→ TYPE: \"{}\"", words_to_type.join(" ")));
                    }

                    if action.is_empty() {
                        action = "(no change)".to_string();
                    }
                }

                // Apply the changes to our pending_words buffer (NOT committed!)
                if words_to_delete > 0 {
                    let new_len = pending_words.len().saturating_sub(words_to_delete);
                    pending_words.truncate(new_len);
                    chunks_since_commit = 0; // Reset stability counter on deletions
                } else if !words_to_type.is_empty() {
                    // No deletions - increment stability counter
                    chunks_since_commit += 1;
                }

                for word in &words_to_type {
                    pending_words.push(word.clone());
                }

                // Commit pending words if they've been stable for 10 chunks (3 seconds)
                // OR if we have 8+ pending words (likely end of sentence)
                if !pending_words.is_empty() &&
                   (chunks_since_commit >= 10 || pending_words.len() >= 8) {
                    let commit_count = if chunks_since_commit >= 10 {
                        // Stable - commit all but last 2 words (keep them pending for corrections)
                        pending_words.len().saturating_sub(2)
                    } else {
                        // Many words - commit all but last 3 words
                        pending_words.len().saturating_sub(3)
                    };

                    if commit_count > 0 {
                        println!("           [Committing {} stable words]", commit_count);
                        let to_commit: Vec<String> = pending_words.drain(0..commit_count).collect();
                        committed_words.extend(to_commit);
                        chunks_since_commit = 0;
                    }
                }

                println!("[{:6.0}ms] Chunk {:2}: {}", time_ms, chunk_num, action);
                println!("           Full: \"{}\"", current_transcription);
                println!("           Committed: \"{}\" | Pending: \"{}\"",
                         committed_words.join(" "), pending_words.join(" "));
            }
            Err(e) => {
                println!("[{:6.0}ms] Chunk {:2}: ✗ Error: {}", time_ms, chunk_num, e);
            }
        }
    }

    println!("─────────────────────────────────────────────────────");
    println!();
    println!("📊 Final transcription:");

    // Combine committed + pending for final output
    let mut final_words = committed_words.clone();
    final_words.extend(pending_words);
    println!("   \"{}\"", final_words.join(" "));
    println!();
}

fn test_replay_command(name: &str) -> Result<()> {
    use std::io::Read;

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║          Live Transcribe - Test Replay                      ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    // Load the test recording
    let config_dir = Config::config_dir()?;
    let test_dir = config_dir.join("test_recordings");
    let audio_file = test_dir.join(format!("{}.raw", name));
    let meta_file = test_dir.join(format!("{}.txt", name));

    if !audio_file.exists() {
        anyhow::bail!("Test recording '{}' not found at {}", name, audio_file.display());
    }

    println!("📂 Loading: {}", audio_file.display());

    // Read metadata
    if meta_file.exists() {
        let meta = std::fs::read_to_string(&meta_file)?;
        println!("📋 Metadata:");
        for line in meta.lines() {
            println!("   {}", line);
        }
    }
    println!();

    // Load audio data
    let mut file = std::fs::File::open(&audio_file)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    // Convert bytes to f32 samples
    let mut audio_data = Vec::new();
    for chunk in buffer.chunks_exact(4) {
        let sample = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        audio_data.push(sample);
    }

    println!("✓ Loaded {} samples ({:.2}s)", audio_data.len(), audio_data.len() as f32 / 16000.0);
    println!();

    // Load config and transcriber
    let config = Config::load_or_create()?;
    let transcriber = Transcriber::new(config.transcription.clone())?;

    // Simulate streaming transcription
    simulate_streaming_transcription(&audio_data, &transcriber, &config.transcription);

    Ok(())
}

fn test_record_command(name: &str, _duration: u64) -> Result<()> {
    use std::io::{self, BufRead, Write};

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║          Live Transcribe - Test Recording                   ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    // Determine recording name: use command-line arg if provided, otherwise use test case name
    let recording_name = if name == "test1" {
        // Default arg was used, so we'll use the test case name instead
        // (Will be set later after test case selection)
        String::new()
    } else {
        // Explicit name provided on command line
        name.to_string()
    };

    // Test cases specifically designed to test streaming, corrections, and silence
    let test_cases = vec![
        ("Simple phrase", "The quick brown fox jumps over the lazy dog"),
        ("Correction test", "Hello world this is... wait I mean this was a test"),
        ("Paragraph", "Artificial intelligence is transforming technology. Machine learning models can now understand speech. This enables powerful applications"),
        ("With silence", "Hello there. [PAUSE 2 SECONDS] How are you doing today?"),
        ("Natural speech", "Let me think about this for a moment. [PAUSE 1 SECOND] Okay I've got it now"),
        ("Long silence", "This is the first sentence. [PAUSE 10 SECONDS] And this is after a long pause"),
        ("Multiple pauses", "Hello. [PAUSE 3 SECONDS] World. [PAUSE 5 SECONDS] How are you. [PAUSE 3 SECONDS] Today"),
    ];

    println!("Available test cases:");
    println!();
    for (i, (name, phrase)) in test_cases.iter().enumerate() {
        println!("  {}. {} - \"{}\"", i + 1, name, phrase);
    }
    println!();

    print!("Select test case (1-{}): ", test_cases.len());
    io::stdout().flush()?;

    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    let choice: usize = lines.next()
        .ok_or_else(|| anyhow::anyhow!("No input"))??
        .trim()
        .parse()
        .unwrap_or(1);

    let selected = test_cases.get(choice.saturating_sub(1)).unwrap_or(&test_cases[0]);

    // Generate filename from test case name
    let test_case_filename = selected.0
        .to_lowercase()
        .replace(" ", "-")
        .replace(",", "");

    println!();
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║  Test: {}                                           ", selected.0);
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!("Say:");
    println!();
    println!("  \"{}\"", selected.1);
    println!();
    println!("Instructions:");
    println!("  - [PAUSE X SECONDS] means be silent for that long");
    println!("  - Speak naturally and clearly");
    println!("  - Press ENTER when ready to start recording");
    println!();
    print!("Press ENTER to start recording...");
    io::stdout().flush()?;

    lines.next(); // Wait for Enter

    // Create audio capture
    let mut audio_capture = AudioCapture::new()?;

    println!();
    println!("🔴 RECORDING - Speak now!");
    println!();
    println!("Press ENTER when done...");

    audio_capture.start_recording()?;

    // Wait for user to press Enter to stop
    lines.next();

    let audio_data = audio_capture.stop_recording()?;

    println!();
    println!("✓ Recording complete!");

    // Use test case filename if no explicit name was provided
    let final_name = if recording_name.is_empty() {
        &test_case_filename
    } else {
        &recording_name
    };

    // Save to test directory
    let config_dir = Config::config_dir()?;
    let test_dir = config_dir.join("test_recordings");
    std::fs::create_dir_all(&test_dir)?;

    let audio_file = test_dir.join(format!("{}.raw", final_name));
    let meta_file = test_dir.join(format!("{}.txt", final_name));

    // Save raw audio as f32 samples
    let mut file = std::fs::File::create(&audio_file)?;
    for sample in &audio_data {
        file.write_all(&sample.to_le_bytes())?;
    }

    // Save metadata
    std::fs::write(&meta_file, format!(
        "samples: {}\nduration: {:.2}s\nsample_rate: 16000\nchannels: 1\nformat: f32le\n",
        audio_data.len(),
        audio_data.len() as f32 / 16000.0
    ))?;

    println!("💾 Saved to:");
    println!("   Name:  {}", final_name);
    println!("   Audio: {}", audio_file.display());
    println!("   Meta:  {}", meta_file.display());
    println!();
    println!("To replay: cargo run -- test-replay {}", final_name);
    println!();

    // Now test transcription with the streaming algorithm
    let config = Config::load_or_create()?;
    let transcriber = Transcriber::new(config.transcription.clone())?;

    simulate_streaming_transcription(&audio_data, &transcriber, &config.transcription);

    Ok(())
}

fn download_model_command(model_name: &Option<String>) -> Result<()> {
    println!("Live Transcribe - Model Downloader");
    println!();

    // If no model specified, use the configured model
    let model_to_download = if let Some(name) = model_name {
        name.clone()
    } else {
        let config = Config::load_or_create()?;
        println!("No model specified, using configured model: {}", config.transcription.model);
        println!();
        config.transcription.model
    };

    let config_dir = Config::config_dir()?;
    let models_dir = config_dir.join("models");

    let downloader = ModelDownloader::new(models_dir.clone());

    println!("Available models:");
    for (name, size, desc) in ModelDownloader::list_available_models() {
        let marker = if name == model_to_download { "→" } else { " " };
        println!("  {} {} - {} ({})", marker, name, desc, size);
    }
    println!();

    println!("Models directory: {}", models_dir.display());
    println!();

    downloader.ensure_model_exists(&model_to_download)?;

    println!();
    println!("✓ Model setup complete!");

    // Only show the config update message if they explicitly specified a different model
    if model_name.is_some() {
        println!();
        println!("To use this model, update ~/.live-transcribe/settings.yaml:");
        println!("  transcription:");
        println!("    model: \"{}\"", model_to_download);
    }

    println!();
    println!("Run the app to start using the model.");

    Ok(())
}

fn run_app() -> Result<()> {
    println!("Live Transcribe - System Tray Application");

    // Load configuration
    let config = Config::load_or_create()?;
    println!("Configuration loaded successfully");

    // Check if models exist, show helpful message if not
    let config_dir = Config::config_dir()?;
    let models_dir = config_dir.join("models");

    // Check VAD model (main model)
    let vad_model_path = models_dir.join(format!("ggml-{}.bin", config.transcription.model));
    if !vad_model_path.exists() {
        eprintln!();
        eprintln!("✗ VAD model not found: {}", config.transcription.model);
        eprintln!();
        eprintln!("Download the model with:");
        eprintln!("  cargo run -- download-model {}", config.transcription.model);
        eprintln!();
        anyhow::bail!("VAD model not found");
    }

    // Load model once and share between VAD and live preview workers
    // This saves 300-600MB of memory compared to loading twice
    println!("Initializing transcriber:");
    println!("  Loading {} model (shared between VAD and live preview)", config.transcription.model);

    let shared_transcriber = TranscriberWithState::new(config.transcription.clone())?;

    // Initialize transcription worker threads with shared model
    let (transcription_worker, transcription_results) =
        TranscriptionWorker::new(shared_transcriber)?;

    println!("Transcription workers initialized (sharing model context)");

    // Create audio capture
    let audio_capture = Arc::new(Mutex::new(AudioCapture::new()?));

    // Create event loop
    let mut event_loop = EventLoop::new();

    // Set app to be menu-bar only (no Dock icon) - MUST be before run()
    #[cfg(target_os = "macos")]
    event_loop.set_activation_policy(ActivationPolicy::Accessory);

    // Create tray app
    let mut tray_app = TrayApp::new()?;
    println!("System tray initialized");

    // Create hotkey manager
    let hotkey_manager = HotkeyManager::new(&config.hotkeys)?;

    let streaming_mode = config.transcription.streaming;
    let chunk_duration = config.transcription.chunk_duration_ms;
    let silence_threshold = config.transcription.silence_threshold;

    // Create transcription state machine
    let mut transcription_state = TranscriptionState::new(silence_threshold);

    // Blink timer for recording indicator (blink every 500ms)
    let mut last_blink = std::time::Instant::now();
    let blink_interval = std::time::Duration::from_millis(500);

    // Main event loop
    event_loop.run(move |_event, _, control_flow| {
        // Use WaitUntil with a short timeout for responsive polling
        *control_flow = ControlFlow::WaitUntil(
            std::time::Instant::now() + std::time::Duration::from_millis(16) // ~60fps
        );

        // Blink recording indicator if recording
        if streaming_mode {
            let capture_guard = audio_capture.lock().unwrap();
            let is_recording = capture_guard.is_recording();
            drop(capture_guard);

            if is_recording && last_blink.elapsed() >= blink_interval {
                tray_app.blink_recording_indicator();
                last_blink = std::time::Instant::now();
            }
        }

        // Poll transcription results (non-blocking)
        while let Ok(result) = transcription_results.try_recv() {
            use transcription_worker::TranscriptionResult;

            // Process result through state machine and get keyboard action
            let action = match result {
                TranscriptionResult::VadCommit { text, request_id } => {
                    transcription_state.process_vad_result(text, request_id)
                }
                TranscriptionResult::LivePreview { text, request_id } => {
                    transcription_state.process_live_result(text, request_id)
                }
                TranscriptionResult::Error { error, request_id } => {
                    eprintln!("❌ Transcription error (request {}): {}", request_id, error);
                    transcription_state.process_error(request_id);
                    Action::NoAction
                }
            };

            // Execute keyboard action
            match action {
                Action::AppendText(text) => {
                    keyboard::macos::append_text(&text);
                }
                Action::ReplaceText { chars_to_delete, new_text } => {
                    keyboard::macos::replace_text_with_backspace(chars_to_delete, &new_text);
                }
                Action::NoAction => {}
                _ => {} // SubmitVadRequest, SubmitLiveRequest handled in audio processing
            }
        }

        // Poll hotkey events
        if let Some(event) = hotkey_manager.poll_event() {
            match event {
                HotkeyEvent::StartTranscription => {
                    println!("Hotkey: Starting transcription...");
                    start_transcription(&audio_capture, &mut tray_app);
                    transcription_state.reset();
                }
                HotkeyEvent::StopTranscription => {
                    println!("Hotkey: Stopping transcription...");
                    stop_transcription(&audio_capture, &mut tray_app, streaming_mode);
                }
                HotkeyEvent::ToggleTranscription => {
                    let is_recording = audio_capture.lock().unwrap().is_recording();
                    if is_recording {
                        println!("Hotkey: Toggle - stopping transcription...");
                        stop_transcription(&audio_capture, &mut tray_app, streaming_mode);
                    } else {
                        println!("Hotkey: Toggle - starting transcription...");
                        start_transcription(&audio_capture, &mut tray_app);
                        transcription_state.reset();
                    }
                }
            }
        }

        // Poll tray events
        if let Some(event) = tray_app.poll_event() {
            match event {
                TrayMenuEvent::StartTranscription => {
                    println!("Menu: Starting transcription...");
                    start_transcription(&audio_capture, &mut tray_app);
                    transcription_state.reset();
                }
                TrayMenuEvent::StopTranscription => {
                    println!("Menu: Stopping transcription...");
                    stop_transcription(&audio_capture, &mut tray_app, streaming_mode);
                }
                TrayMenuEvent::Settings => {
                    println!("Opening settings...");
                    if let Ok(config_path) = Config::config_path() {
                        println!("Settings file: {}", config_path.display());
                        // Try to open with default editor
                        #[cfg(target_os = "macos")]
                        {
                            let _ = std::process::Command::new("open")
                                .arg(config_path)
                                .spawn();
                        }
                    }
                }
                TrayMenuEvent::Quit => {
                    println!("Quitting application...");
                    *control_flow = ControlFlow::Exit;
                }
            }
        }

        // Hybrid VAD + live preview streaming
        if streaming_mode {
            let capture_guard = audio_capture.lock().unwrap();
            if capture_guard.is_recording() {
                if let Some((audio_window, new_samples_count)) = capture_guard.get_chunk_if_ready(chunk_duration) {
                    drop(capture_guard); // Release lock before transcribing

                    // Extract only the NEW audio from the sliding window
                    let window_len = audio_window.len();
                    let new_audio = if new_samples_count > 0 && new_samples_count <= window_len {
                        &audio_window[window_len - new_samples_count..]
                    } else {
                        &audio_window[..]
                    };

                    // Process audio chunk through state machine
                    let actions = transcription_state.process_audio_chunk(new_audio);

                    // Execute transcription actions
                    for action in actions {
                        match action {
                            Action::SubmitVadRequest { audio, request_id } => {
                                transcription_worker.transcribe_vad_commit_with_id(audio, request_id);
                            }
                            Action::SubmitLiveRequest { audio, request_id } => {
                                transcription_worker.transcribe_live_preview_with_id(audio, request_id);
                            }
                            Action::CancelLiveRequest => {
                                // Cancel any pending live preview - VAD commit supersedes it
                                transcription_worker.cancel_all_live_before(u64::MAX);
                            }
                            _ => {} // Keyboard actions handled in result processing
                        }
                    }
                }
            }
        }
    });
}

fn start_transcription(audio_capture: &Arc<Mutex<AudioCapture>>, tray_app: &mut TrayApp) {
    let mut capture = audio_capture.lock().unwrap();
    if !capture.is_recording() {
        match capture.start_recording() {
            Ok(_) => {
                println!("✓ Recording started");
                tray_app.set_transcribing(true);
            }
            Err(e) => {
                eprintln!("✗ Failed to start recording: {}", e);
            }
        }
    }
}

fn stop_transcription(
    audio_capture: &Arc<Mutex<AudioCapture>>,
    tray_app: &mut TrayApp,
    streaming_mode: bool,
) {
    let mut capture = audio_capture.lock().unwrap();
    if capture.is_recording() {
        match capture.stop_recording() {
            Ok(_audio_data) => {
                println!("✓ Recording stopped");
                tray_app.set_transcribing(false);

                // In streaming mode, we already typed everything, so just finish
                println!("Streaming transcription complete");

                // TODO: Non-streaming mode would need access to transcriber
                // For now, only streaming mode is fully supported with the threaded architecture
                if !streaming_mode {
                    println!("⚠️  Non-streaming mode not yet supported with threaded transcription");
                }
            }
            Err(e) => {
                eprintln!("✗ Failed to stop recording: {}", e);
                tray_app.set_transcribing(false);
            }
        }
    }
}
