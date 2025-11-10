/// Application-wide constants for audio processing, transcription, and keyboard handling

pub mod audio {
    /// Minimum audio samples required for Whisper transcription (1.5 seconds)
    pub const MIN_WHISPER_SAMPLES: usize = 24000; // 1.5s at 16kHz
}

pub mod vad {
    /// Number of consecutive silence chunks required to commit VAD transcription
    /// At 300ms per chunk, 5 chunks = 1.5 seconds of silence
    pub const COMMIT_SILENCE_CHUNKS: usize = 5;

    /// Maximum number of trailing silence chunks to include in VAD buffer
    /// This prevents hallucinations while catching quiet speech
    /// At 300ms per chunk, 2 chunks = 600ms of trailing silence
    pub const MAX_TRAILING_SILENCE_CHUNKS: usize = 2;
}

pub mod streaming {
    /// Number of chunks to accumulate before triggering live preview
    /// At 300ms per chunk, 5 chunks = 1.5 seconds
    pub const LIVE_PREVIEW_DELAY_CHUNKS: usize = 5;
}

pub mod worker {
    /// Maximum number of pending transcription requests in queue
    /// This prevents unbounded memory growth under load
    pub const MAX_PENDING_REQUESTS: usize = 2;
}
