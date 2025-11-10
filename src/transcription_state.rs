/// Transcription state machine for managing VAD, live preview, and keyboard updates
///
/// This module encapsulates all the complex state management logic that was previously
/// embedded in the main event loop, making it testable and maintainable.

use crate::constants::{audio::MIN_WHISPER_SAMPLES, vad};
use crate::text_diff::{compute_append, compute_text_diff};
use crate::audio::AudioCapture;

/// Actions that should be performed in response to state changes
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Type new text by appending to existing text
    AppendText(String),

    /// Replace text by deleting characters and typing new text
    ReplaceText {
        chars_to_delete: usize,
        new_text: String,
    },

    /// Submit VAD transcription request for complete utterance
    SubmitVadRequest {
        audio: Vec<f32>,
        request_id: u64,
    },

    /// Submit live preview transcription request
    SubmitLiveRequest {
        audio: Vec<f32>,
        request_id: u64,
    },

    /// Cancel pending live preview request (VAD supersedes it)
    CancelLiveRequest,

    /// No action needed
    NoAction,
}

/// Core transcription state machine
pub struct TranscriptionState {
    /// VAD buffer: accumulates complete utterances
    vad_buffer: Vec<f32>,

    /// VAD committed text: ground truth from complete utterances
    vad_committed_text: String,

    /// Live preview text: what's currently displayed on screen
    live_preview_text: String,

    /// Number of consecutive silence chunks detected
    silence_streak: usize,

    /// Number of chunks processed since last VAD commit
    chunks_since_vad_commit: usize,

    /// ID of pending VAD transcription request
    pending_vad_request: Option<u64>,

    /// ID of pending live preview transcription request
    pending_live_request: Option<u64>,

    /// Counter for generating unique request IDs
    next_request_id: u64,

    /// Silence detection threshold
    silence_threshold: f32,
}

impl TranscriptionState {
    /// Create a new transcription state machine
    pub fn new(silence_threshold: f32) -> Self {
        Self {
            vad_buffer: Vec::new(),
            vad_committed_text: String::new(),
            live_preview_text: String::new(),
            silence_streak: 0,
            chunks_since_vad_commit: 0,
            pending_vad_request: None,
            pending_live_request: None,
            next_request_id: 1,
            silence_threshold,
        }
    }

    /// Reset all state for a new recording session
    pub fn reset(&mut self) {
        self.vad_buffer.clear();
        self.vad_committed_text.clear();
        self.live_preview_text.clear();
        self.silence_streak = 0;
        self.chunks_since_vad_commit = 0;
        self.pending_vad_request = None;
        self.pending_live_request = None;
    }

    /// Process a new audio chunk and return actions to perform
    pub fn process_audio_chunk(&mut self, new_audio: &[f32]) -> Vec<Action> {
        let mut actions = Vec::new();

        let is_silence = AudioCapture::is_silence(new_audio, self.silence_threshold);

        if is_silence {
            self.silence_streak += 1;
            println!("🔇 Silence chunk {}", self.silence_streak);

            // Only send LIMITED trailing silence to Whisper
            // Too much trailing silence causes hallucinations
            if !self.vad_buffer.is_empty() && self.silence_streak <= vad::MAX_TRAILING_SILENCE_CHUNKS {
                self.vad_buffer.extend_from_slice(new_audio);
                println!("   ➕ Added trailing silence chunk {} to VAD buffer", self.silence_streak);
            }

            // After sufficient silence, commit VAD transcription
            if self.silence_streak >= vad::COMMIT_SILENCE_CHUNKS
                && !self.vad_buffer.is_empty()
                && self.pending_vad_request.is_none()
            {
                let buffer_duration = self.vad_buffer.len() as f32 / 16000.0;
                println!("💾 VAD: Silence detected - transcribing {:.1}s of speech + trailing silence", buffer_duration);

                // Debug: calculate RMS of VAD buffer
                let vad_rms = if !self.vad_buffer.is_empty() {
                    let sum_squares: f32 = self.vad_buffer.iter().map(|&x| x * x).sum();
                    (sum_squares / self.vad_buffer.len() as f32).sqrt()
                } else {
                    0.0
                };
                println!("   VAD buffer RMS: {:.4}", vad_rms);

                // Pad VAD buffer to minimum length for Whisper if needed
                if self.vad_buffer.len() < MIN_WHISPER_SAMPLES {
                    println!("   Padding VAD buffer from {:.1}s to {:.1}s",
                             buffer_duration,
                             MIN_WHISPER_SAMPLES as f32 / 16000.0);
                    self.vad_buffer.resize(MIN_WHISPER_SAMPLES, 0.0);
                }

                // Generate request ID and submit VAD transcription
                let request_id = self.generate_request_id();
                println!("   Submitting VAD transcription request for {} samples", self.vad_buffer.len());

                actions.push(Action::SubmitVadRequest {
                    audio: self.vad_buffer.clone(),
                    request_id,
                });

                self.pending_vad_request = Some(request_id);

                // Cancel any pending live preview - VAD commit supersedes it
                if self.pending_live_request.is_some() {
                    actions.push(Action::CancelLiveRequest);
                    self.pending_live_request = None;
                }

                // Reset for next utterance
                self.vad_buffer.clear();
                self.chunks_since_vad_commit = 0;
            }

            return actions;
        }

        // Speech detected
        if self.silence_streak > 0 {
            println!("🔊 Speech after {} silent chunks", self.silence_streak);
        }
        self.silence_streak = 0;
        self.chunks_since_vad_commit += 1;

        // VAD: Accumulate speech audio
        self.vad_buffer.extend_from_slice(new_audio);
        println!("📼 VAD buffer: {:.1}s accumulated", self.vad_buffer.len() as f32 / 16000.0);

        // LIVE PREVIEW: Transcribe VAD buffer for immediate feedback
        if self.chunks_since_vad_commit >= crate::constants::streaming::LIVE_PREVIEW_DELAY_CHUNKS
            && self.pending_live_request.is_none()
        {
            // Pad VAD buffer for transcription if needed
            let mut preview_buffer = self.vad_buffer.clone();
            if preview_buffer.len() < MIN_WHISPER_SAMPLES {
                preview_buffer.resize(MIN_WHISPER_SAMPLES, 0.0);
            }

            // Generate request ID and submit live preview transcription
            let request_id = self.generate_request_id();
            actions.push(Action::SubmitLiveRequest {
                audio: preview_buffer,
                request_id,
            });

            self.pending_live_request = Some(request_id);
        } else if self.chunks_since_vad_commit < crate::constants::streaming::LIVE_PREVIEW_DELAY_CHUNKS {
            println!("⏳ Live preview: Waiting for more audio ({}/{} chunks)",
                     self.chunks_since_vad_commit,
                     crate::constants::streaming::LIVE_PREVIEW_DELAY_CHUNKS);
        }

        actions
    }

    /// Process a VAD commit result and return keyboard action
    pub fn process_vad_result(&mut self, text: String, request_id: u64) -> Action {
        // Verify this is the request we're waiting for
        if self.pending_vad_request != Some(request_id) {
            return Action::NoAction;
        }

        self.pending_vad_request = None;
        println!("✅ VAD committed: \"{}\"", text);

        if text.is_empty() {
            return Action::NoAction;
        }

        // Build what the full committed text should be
        let new_vad_committed = self.vad_committed_text.clone() + &text + " ";

        // Determine keyboard action based on relationship between new VAD and current screen text
        let action = if let Some(suffix) = compute_append(&self.live_preview_text, &new_vad_committed) {
            // Just append the new part
            println!("➕ VAD appended: \"{}\"", suffix);
            Action::AppendText(suffix)
        } else if !self.live_preview_text.is_empty() {
            // VAD diverged from live preview - find common prefix to minimize flickering
            let diff = compute_text_diff(&self.live_preview_text, &new_vad_committed);

            if diff.chars_to_delete > 0 || !diff.suffix_to_type.is_empty() {
                println!("🔄 VAD partial update: kept {} bytes, changed ending", diff.common_prefix_bytes);
                Action::ReplaceText {
                    chars_to_delete: diff.chars_to_delete,
                    new_text: diff.suffix_to_type,
                }
            } else {
                Action::NoAction
            }
        } else {
            // No live preview - just type the VAD result
            println!("➕ VAD typed: \"{}\"", new_vad_committed.trim());
            Action::AppendText(new_vad_committed.clone())
        };

        // Update VAD committed state
        self.vad_committed_text = new_vad_committed.clone();
        self.live_preview_text = new_vad_committed;

        println!("   State: {} chars committed", self.vad_committed_text.chars().count());

        action
    }

    /// Process a live preview result and return keyboard action
    pub fn process_live_result(&mut self, text: String, request_id: u64) -> Action {
        // Verify this is the request we're waiting for
        if self.pending_live_request != Some(request_id) {
            return Action::NoAction;
        }

        self.pending_live_request = None;

        if text.is_empty() {
            return Action::NoAction;
        }

        println!("👁️  Live preview: \"{}\"", text);

        // Build full text: VAD committed + new live preview
        let full_live_text = self.vad_committed_text.clone() + &text;

        // Determine keyboard action
        let action = if let Some(suffix) = compute_append(&self.live_preview_text, &full_live_text) {
            // Append only the new part
            println!("➕ Live preview appended: \"{}\"", suffix);
            Action::AppendText(suffix)
        } else if self.live_preview_text.starts_with(&full_live_text) {
            // Live preview is showing MORE than what it should
            // This means Whisper changed its mind - delete extra chars
            let chars_to_delete = self.live_preview_text.chars().count() - full_live_text.chars().count();
            println!("🔄 Live correction: deleted {} chars (Whisper shortened)", chars_to_delete);
            Action::ReplaceText {
                chars_to_delete,
                new_text: String::new(),
            }
        } else {
            // Text diverged - find longest common prefix to minimize deletions
            let diff = compute_text_diff(&self.live_preview_text, &full_live_text);

            if diff.chars_to_delete > 0 || !diff.suffix_to_type.is_empty() {
                println!("🔄 Live partial update: kept {} bytes, changed ending", diff.common_prefix_bytes);
                Action::ReplaceText {
                    chars_to_delete: diff.chars_to_delete,
                    new_text: diff.suffix_to_type,
                }
            } else {
                Action::NoAction
            }
        };

        self.live_preview_text = full_live_text;
        println!("   State: {} chars on screen", self.live_preview_text.chars().count());

        action
    }

    /// Process a transcription error
    pub fn process_error(&mut self, request_id: u64) {
        if self.pending_vad_request == Some(request_id) {
            self.pending_vad_request = None;
        }
        if self.pending_live_request == Some(request_id) {
            self.pending_live_request = None;
        }
    }

    /// Generate a unique request ID
    /// Uses wrapping arithmetic to prevent overflow panic (though at 1000 req/s, it would take 584 million years)
    fn generate_request_id(&mut self) -> u64 {
        let id = self.next_request_id;
        self.next_request_id = self.next_request_id.wrapping_add(1);
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_speech_audio(duration_ms: usize) -> Vec<f32> {
        let sample_count = (duration_ms * 16) as usize; // 16 samples per ms at 16kHz
        vec![0.1; sample_count] // Amplitude above silence threshold
    }

    fn create_silence_audio(duration_ms: usize) -> Vec<f32> {
        let sample_count = (duration_ms * 16) as usize;
        vec![0.001; sample_count] // Amplitude below silence threshold
    }

    #[test]
    fn test_reset_clears_all_state() {
        let mut state = TranscriptionState::new(0.01);
        state.vad_buffer = vec![1.0, 2.0, 3.0];
        state.vad_committed_text = "test".to_string();
        state.live_preview_text = "test".to_string();
        state.silence_streak = 5;

        state.reset();

        assert!(state.vad_buffer.is_empty());
        assert!(state.vad_committed_text.is_empty());
        assert!(state.live_preview_text.is_empty());
        assert_eq!(state.silence_streak, 0);
    }

    #[test]
    fn test_vad_commit_append() {
        let mut state = TranscriptionState::new(0.01);
        state.live_preview_text = "Hello".to_string();
        state.vad_committed_text = String::new();
        state.pending_vad_request = Some(1); // Mark as pending

        let action = state.process_vad_result("Hello world".to_string(), 1);

        match action {
            Action::AppendText(text) => {
                assert_eq!(text, " world ");
            }
            _ => panic!("Expected AppendText action, got {:?}", action),
        }

        assert_eq!(state.vad_committed_text, "Hello world ");
        assert_eq!(state.live_preview_text, "Hello world ");
    }

    #[test]
    fn test_vad_commit_replace() {
        let mut state = TranscriptionState::new(0.01);
        state.live_preview_text = "Hello world".to_string();
        state.vad_committed_text = String::new();
        state.pending_vad_request = Some(1); // Mark as pending

        let action = state.process_vad_result("Hello there".to_string(), 1);

        match action {
            Action::ReplaceText { chars_to_delete, new_text } => {
                assert_eq!(chars_to_delete, 5); // "world"
                assert_eq!(new_text, "there ");
            }
            _ => panic!("Expected ReplaceText action, got {:?}", action),
        }
    }

    #[test]
    fn test_live_preview_append() {
        let mut state = TranscriptionState::new(0.01);
        state.vad_committed_text = "Hello".to_string();
        state.live_preview_text = "Hello".to_string();
        state.pending_live_request = Some(1); // Mark as pending

        let action = state.process_live_result(" world".to_string(), 1);

        match action {
            Action::AppendText(text) => {
                assert_eq!(text, " world");
            }
            _ => panic!("Expected AppendText action, got {:?}", action),
        }

        assert_eq!(state.live_preview_text, "Hello world");
    }

    #[test]
    fn test_request_id_ignored_if_not_pending() {
        let mut state = TranscriptionState::new(0.01);
        state.pending_vad_request = Some(1);

        let action = state.process_vad_result("test".to_string(), 2);

        assert_eq!(action, Action::NoAction);
        assert_eq!(state.pending_vad_request, Some(1)); // Unchanged
    }
}
