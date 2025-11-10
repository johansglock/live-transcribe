use std::fs;
use std::path::PathBuf;
use live_transcribe::hybrid_vad::simulate_hybrid_vad;
use live_transcribe::transcription::Transcriber;
use live_transcribe::config::Config;

#[derive(Debug)]
struct TestCase {
    name: String,
    audio_path: PathBuf,
    metadata_path: PathBuf,
}

fn get_test_recordings() -> Vec<TestCase> {
    let test_dir = dirs::home_dir()
        .expect("Could not find home directory")
        .join(".live-transcribe/test_recordings");

    let mut test_cases = Vec::new();

    if let Ok(entries) = fs::read_dir(&test_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("raw") {
                let name = path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                let metadata_path = test_dir.join(format!("{}.txt", name));

                test_cases.push(TestCase {
                    name,
                    audio_path: path,
                    metadata_path,
                });
            }
        }
    }

    test_cases.sort_by(|a, b| a.name.cmp(&b.name));
    test_cases
}

#[test]
fn test_all_recordings() {
    let test_cases = get_test_recordings();

    if test_cases.is_empty() {
        panic!("No test recordings found in ~/.live-transcribe/test_recordings/");
    }

    println!("\n🧪 Running {} test cases:", test_cases.len());
    for (i, test) in test_cases.iter().enumerate() {
        println!("  {}. {}", i + 1, test.name);
    }
    println!();

    for test_case in &test_cases {
        run_test_case(test_case);
    }
}

fn run_test_case(test: &TestCase) {
    println!("\n{}", "=".repeat(60));
    println!("🎯 Test: {}", test.name);
    println!("{}", "=".repeat(60));

    // Load audio
    let audio_data = fs::read(&test.audio_path)
        .expect(&format!("Failed to read audio file: {:?}", test.audio_path));

    // Parse as f32 samples
    let samples: Vec<f32> = audio_data
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect();

    println!("📊 Audio: {} samples ({:.2}s)", samples.len(), samples.len() as f32 / 16000.0);

    // Load metadata if available
    if test.metadata_path.exists() {
        if let Ok(metadata) = fs::read_to_string(&test.metadata_path) {
            println!("📝 Metadata:");
            for line in metadata.lines().take(6) {
                println!("   {}", line);
            }
        }
    }

    // Initialize transcriber
    let config = Config::load_or_create().unwrap_or_default();
    let transcriber = Transcriber::new(config.transcription).expect("Failed to create transcriber");

    // Run hybrid VAD simulation
    let result = simulate_hybrid_vad(
        &samples,
        &transcriber,
        300, // 300ms chunks
        0.02, // silence threshold
    );

    println!("\n📊 Results:");
    println!("   Chunks processed: {}", result.chunks_processed);
    println!("   VAD commits: {}", result.vad_transcriptions.len());
    println!("   Live previews: {}", result.live_transcriptions.len());

    if !result.vad_transcriptions.is_empty() {
        println!("\n💾 VAD transcriptions:");
        for (i, text) in result.vad_transcriptions.iter().enumerate() {
            println!("   {}. \"{}\"", i + 1, text);
        }
    }

    if !result.live_transcriptions.is_empty() {
        println!("\n👁️  Live previews (sample):");
        let sample_size = result.live_transcriptions.len().min(5);
        for (i, text) in result.live_transcriptions.iter().take(sample_size).enumerate() {
            println!("   {}. \"{}\"", i + 1, text);
        }
        if result.live_transcriptions.len() > sample_size {
            println!("   ... and {} more", result.live_transcriptions.len() - sample_size);
        }
    }

    println!("\n✨ Final text: \"{}\"", result.final_text);
    println!("📺 Simulated screen: \"{}\"", result.simulated_screen_text);

    // Verify no duplication
    let screen = &result.simulated_screen_text;
    for (i, vad_text) in result.vad_transcriptions.iter().enumerate() {
        let count = screen.matches(vad_text.as_str()).count();
        if count > 1 {
            panic!("❌ DUPLICATION DETECTED: VAD transcription #{} \"{}\" appears {} times in screen output!\nScreen: \"{}\"",
                   i + 1, vad_text, count, screen);
        }
    }

    println!("✅ No duplication detected");
    println!();
}

#[test]
fn test_simple_phrase() {
    let test_cases = get_test_recordings();
    let simple = test_cases.iter().find(|t| t.name == "simple-phrase");

    if let Some(test) = simple {
        run_test_case(test);
    } else {
        println!("⚠️  simple-phrase test recording not found");
    }
}

#[test]
fn test_multiple_pauses() {
    let test_cases = get_test_recordings();
    let test = test_cases.iter().find(|t| t.name == "multiple-pauses");

    if let Some(test) = test {
        run_test_case(test);
    } else {
        println!("⚠️  multiple-pauses test recording not found");
    }
}

#[test]
fn test_with_silence() {
    let test_cases = get_test_recordings();
    let test = test_cases.iter().find(|t| t.name == "with-silence");

    if let Some(test) = test {
        run_test_case(test);
    } else {
        println!("⚠️  with-silence test recording not found");
    }
}
