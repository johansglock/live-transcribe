use anyhow::{Context, Result};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use crate::config::TranscriptionConfig;

pub struct Transcriber {
    ctx: Arc<Mutex<WhisperContext>>,
    config: TranscriptionConfig,
}

pub struct TranscriberWithState {
    pub(crate) ctx: Arc<WhisperContext>,
    state: WhisperState,
    pub(crate) config: TranscriptionConfig,
}

impl Transcriber {
    pub fn new(config: TranscriptionConfig) -> Result<Self> {
        // Get model path
        let model_path = Self::get_model_path(&config.model)?;

        println!("Loading Whisper model from: {}", model_path.display());

        // Create context parameters with GPU acceleration
        let ctx_params = WhisperContextParameters {
            use_gpu: config.use_gpu,
            ..Default::default()
        };

        // Load the whisper model
        let ctx = WhisperContext::new_with_params(&model_path.to_string_lossy(), ctx_params)
            .context("Failed to load Whisper model")?;

        println!("Whisper model loaded successfully (GPU: {})", config.use_gpu);

        Ok(Transcriber {
            ctx: Arc::new(Mutex::new(ctx)),
            config,
        })
    }

    fn get_model_path(model_name: &str) -> Result<PathBuf> {
        // Check in the models directory in the config folder
        let config_dir = dirs::home_dir()
            .context("Failed to get home directory")?
            .join(".live-transcribe")
            .join("models");

        let model_filename = format!("ggml-{}.bin", model_name);
        let model_path = config_dir.join(&model_filename);

        if !model_path.exists() {
            anyhow::bail!(
                "Model file not found: {}\n\
                Please download the model from:\n\
                https://huggingface.co/ggerganov/whisper.cpp/tree/main\n\
                and place it in: {}",
                model_filename,
                config_dir.display()
            );
        }

        Ok(model_path)
    }

    pub fn transcribe(&self, audio_data: &[f32]) -> Result<String> {
        let ctx = self.ctx.lock().unwrap();

        // Create parameters for transcription
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

        // Set language if specified
        if !self.config.language.is_empty() && self.config.language != "auto" {
            params.set_language(Some(&self.config.language));
        }

        // Enable translation to English if needed
        params.set_translate(false);

        // Print progress
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        // Disable special tokens to avoid annotations like [BLANK_AUDIO], (coughs), etc.
        params.set_suppress_blank(true);
        params.set_suppress_non_speech_tokens(true);

        // Run the transcription
        let mut state = ctx.create_state()
            .context("Failed to create Whisper state")?;

        state.full(params, audio_data)
            .context("Failed to run Whisper transcription")?;

        // Get the number of segments
        let num_segments = state.full_n_segments()
            .context("Failed to get number of segments")?;

        // Collect all transcribed text
        let mut result = String::new();
        for i in 0..num_segments {
            let segment = state.full_get_segment_text(i)
                .context("Failed to get segment text")?;
            println!("  📝 Whisper segment {}: {:?}", i, segment);
            result.push_str(&segment);
            result.push(' ');
        }

        let final_result = result.trim().to_string();
        println!("  ✅ Whisper final result ({} segments): {:?}", num_segments, final_result);
        Ok(final_result)
    }
}

impl TranscriberWithState {
    pub fn new(config: TranscriptionConfig) -> Result<Self> {
        // Get model path
        let model_path = Transcriber::get_model_path(&config.model)?;

        println!("Loading Whisper model from: {}", model_path.display());

        // Create context parameters with GPU acceleration
        let ctx_params = WhisperContextParameters {
            use_gpu: config.use_gpu,
            ..Default::default()
        };

        // Load the whisper model
        let ctx = WhisperContext::new_with_params(&model_path.to_string_lossy(), ctx_params)
            .context("Failed to load Whisper model")?;

        println!("Whisper model loaded successfully (GPU: {})", config.use_gpu);

        // Create state once (loads CoreML model once)
        let state = ctx.create_state()
            .context("Failed to create Whisper state")?;

        Ok(TranscriberWithState {
            ctx: Arc::new(ctx),
            state,
            config,
        })
    }

    /// Create a new transcriber with state using a shared context
    /// This allows multiple workers to share the same model, saving memory
    pub fn new_with_shared_context(ctx: Arc<WhisperContext>, config: TranscriptionConfig) -> Result<Self> {
        // Create state for this worker (each worker needs its own state)
        let state = ctx.create_state()
            .context("Failed to create Whisper state")?;

        Ok(TranscriberWithState {
            ctx,
            state,
            config,
        })
    }

    pub fn transcribe(&mut self, audio_data: &[f32]) -> Result<String> {
        // Create parameters for transcription
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

        // Set language if specified
        if !self.config.language.is_empty() && self.config.language != "auto" {
            params.set_language(Some(&self.config.language));
        }

        // Enable translation to English if needed
        params.set_translate(false);

        // Print progress
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        // Disable special tokens to avoid annotations like [BLANK_AUDIO], (coughs), etc.
        params.set_suppress_blank(true);
        params.set_suppress_non_speech_tokens(true);

        // Reduce hallucinations by using greedy decoding (temperature = 0)
        // and stricter probability thresholds
        params.set_temperature(0.0);
        params.set_temperature_inc(0.0);  // Don't increase temperature on failure

        // Filter out low-probability tokens (more conservative = higher threshold)
        // Default is -1.0, using 0.0 to only accept confident predictions
        params.set_logprob_thold(0.0);

        // Reuse the existing state
        self.state.full(params, audio_data)
            .context("Failed to run Whisper transcription")?;

        // Get the number of segments
        let num_segments = self.state.full_n_segments()
            .context("Failed to get number of segments")?;

        // Collect all transcribed text
        let mut result = String::new();
        for i in 0..num_segments {
            let segment = self.state.full_get_segment_text(i)
                .context("Failed to get segment text")?;
            println!("  📝 Whisper segment {}: {:?}", i, segment);
            result.push_str(&segment);
            result.push(' ');
        }

        let final_result = result.trim().to_string();
        println!("  ✅ Whisper final result ({} segments): {:?}", num_segments, final_result);
        Ok(final_result)
    }
}

