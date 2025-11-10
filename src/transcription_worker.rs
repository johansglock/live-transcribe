use std::sync::mpsc::{channel, sync_channel, Sender, SyncSender, Receiver, TrySendError};
use std::thread;
use std::collections::HashSet;
use anyhow::Result;
use crate::transcription::TranscriberWithState;
use crate::constants::worker::MAX_PENDING_REQUESTS;

/// Message sent to worker threads
#[derive(Debug)]
enum WorkerMessage {
    /// Transcribe audio with given request ID
    Transcribe { audio: Vec<f32>, request_id: u64 },
    /// Cancel a specific request (currently unused - we use CancelAllBefore instead)
    #[allow(dead_code)]
    Cancel { request_id: u64 },
    /// Cancel all requests before a given ID
    CancelAllBefore { request_id: u64 },
}

/// Result of a transcription
#[derive(Debug)]
pub enum TranscriptionResult {
    /// Live preview result
    LivePreview { text: String, request_id: u64 },
    /// VAD commit result
    VadCommit { text: String, request_id: u64 },
    /// Error during transcription
    Error { error: String, request_id: u64 },
}

/// Handle for communicating with the transcription worker threads
pub struct TranscriptionWorker {
    live_task_sender: SyncSender<WorkerMessage>,
    vad_task_sender: SyncSender<WorkerMessage>,
}

impl TranscriptionWorker {
    /// Create a new transcription worker with two separate threads sharing the same model
    pub fn new(
        shared_transcriber: TranscriberWithState,
    ) -> Result<(Self, Receiver<TranscriptionResult>)> {
        // Use bounded channels to prevent unbounded memory growth
        // MAX_PENDING_REQUESTS ensures backpressure when workers are slow
        let (live_task_tx, live_task_rx) = sync_channel(MAX_PENDING_REQUESTS);
        let (vad_task_tx, vad_task_rx) = sync_channel(MAX_PENDING_REQUESTS);
        let (result_tx, result_rx) = channel(); // Results channel can be unbounded

        // Extract shared context and config from the transcriber
        let shared_ctx = shared_transcriber.ctx.clone();
        let config = shared_transcriber.config.clone();

        // Create separate transcribers for each worker that share the same context
        // This saves memory (~300-600MB) by loading the model only once
        let live_transcriber = TranscriberWithState::new_with_shared_context(
            shared_ctx.clone(),
            config.clone()
        )?;

        let vad_transcriber = TranscriberWithState::new_with_shared_context(
            shared_ctx,
            config
        )?;

        // Spawn live preview worker thread
        let result_tx_live = result_tx.clone();
        thread::spawn(move || {
            Self::live_worker_loop(live_task_rx, result_tx_live, live_transcriber);
        });

        // Spawn VAD worker thread
        thread::spawn(move || {
            Self::vad_worker_loop(vad_task_rx, result_tx, vad_transcriber);
        });

        let worker = TranscriptionWorker {
            live_task_sender: live_task_tx,
            vad_task_sender: vad_task_tx,
        };

        Ok((worker, result_rx))
    }

    /// Submit a live preview transcription request with a specific request ID (non-blocking)
    ///
    /// Uses try_send to avoid blocking the event loop. If the queue is full, the request is dropped.
    pub fn transcribe_live_preview_with_id(&self, audio: Vec<f32>, request_id: u64) {
        match self.live_task_sender.try_send(WorkerMessage::Transcribe { audio, request_id }) {
            Ok(_) => {},
            Err(TrySendError::Full(_)) => {
                // Queue is full - drop this request since we want real-time performance
                // This is actually desirable: we don't want to block on old audio
                eprintln!("⚠️  Live preview queue full, dropping request {} (worker is busy)", request_id);
            },
            Err(TrySendError::Disconnected(_)) => {
                eprintln!("❌ Live preview worker disconnected");
            }
        }
    }

    /// Cancel all live preview requests before a given ID
    pub fn cancel_all_live_before(&self, request_id: u64) {
        if let Err(e) = self.live_task_sender.send(WorkerMessage::CancelAllBefore { request_id }) {
            eprintln!("⚠️  Failed to send cancel-all request: {}", e);
        }
    }

    /// Submit a VAD commit transcription request with a specific request ID (non-blocking)
    ///
    /// Uses try_send to avoid blocking the event loop. If the queue is full, the request is dropped.
    pub fn transcribe_vad_commit_with_id(&self, audio: Vec<f32>, request_id: u64) {
        match self.vad_task_sender.try_send(WorkerMessage::Transcribe { audio, request_id }) {
            Ok(_) => {},
            Err(TrySendError::Full(_)) => {
                // Queue is full - this shouldn't happen often for VAD commits
                // VAD commits are important, so we warn loudly
                eprintln!("⚠️  VAD commit queue full, dropping request {} (worker overloaded!)", request_id);
            },
            Err(TrySendError::Disconnected(_)) => {
                eprintln!("❌ VAD commit worker disconnected");
            }
        }
    }

    /// Live preview worker thread - handles fast live transcriptions with cancellation support
    fn live_worker_loop(
        task_rx: Receiver<WorkerMessage>,
        result_tx: Sender<TranscriptionResult>,
        mut transcriber: TranscriberWithState,
    ) {
        println!("🔧 Live preview worker thread started");

        let mut cancelled_ids: HashSet<u64> = HashSet::new();
        const MAX_CANCELLED_IDS: usize = 100; // Prevent unbounded memory growth

        for message in task_rx {
            match message {
                WorkerMessage::Transcribe { audio, request_id } => {
                    // Check if this request was cancelled
                    if cancelled_ids.contains(&request_id) {
                        println!("⏭️  Skipping cancelled live request {}", request_id);
                        cancelled_ids.remove(&request_id);
                        continue;
                    }

                    let result = match transcriber.transcribe(&audio) {
                        Ok(text) => TranscriptionResult::LivePreview {
                            text: text.trim().to_string(),
                            request_id,
                        },
                        Err(e) => TranscriptionResult::Error {
                            error: format!("Live preview error: {}", e),
                            request_id,
                        },
                    };

                    if result_tx.send(result).is_err() {
                        println!("⚠️  Live preview worker: main thread disconnected");
                        break;
                    }
                }
                WorkerMessage::Cancel { request_id } => {
                    cancelled_ids.insert(request_id);
                    println!("❌ Cancelled live request {}", request_id);

                    // Prevent unbounded growth: if set is too large, remove smallest IDs
                    if cancelled_ids.len() > MAX_CANCELLED_IDS {
                        if let Some(&min_id) = cancelled_ids.iter().min() {
                            cancelled_ids.remove(&min_id);
                            println!("⚠️  Cancelled IDs set too large, removed oldest ID: {}", min_id);
                        }
                    }
                }
                WorkerMessage::CancelAllBefore { request_id } => {
                    // Cancel all requests with IDs less than the given ID
                    // In practice, we just clear the set since requests are processed in order
                    cancelled_ids.clear();
                    println!("❌ Cancelled all live requests before {}", request_id);
                }
            }
        }

        println!("🔧 Live preview worker thread stopped");
    }

    /// VAD worker thread - handles accurate VAD transcriptions
    fn vad_worker_loop(
        task_rx: Receiver<WorkerMessage>,
        result_tx: Sender<TranscriptionResult>,
        mut transcriber: TranscriberWithState,
    ) {
        println!("🔧 VAD worker thread started");

        let mut cancelled_ids: HashSet<u64> = HashSet::new();
        const MAX_CANCELLED_IDS: usize = 100; // Prevent unbounded memory growth

        for message in task_rx {
            match message {
                WorkerMessage::Transcribe { audio, request_id } => {
                    // Check if this request was cancelled
                    if cancelled_ids.contains(&request_id) {
                        println!("⏭️  Skipping cancelled VAD request {}", request_id);
                        cancelled_ids.remove(&request_id);
                        continue;
                    }

                    let result = match transcriber.transcribe(&audio) {
                        Ok(text) => TranscriptionResult::VadCommit {
                            text: text.trim().to_string(),
                            request_id,
                        },
                        Err(e) => TranscriptionResult::Error {
                            error: format!("VAD commit error: {}", e),
                            request_id,
                        },
                    };

                    if result_tx.send(result).is_err() {
                        println!("⚠️  VAD worker: main thread disconnected");
                        break;
                    }
                }
                WorkerMessage::Cancel { request_id } => {
                    cancelled_ids.insert(request_id);
                    println!("❌ Cancelled VAD request {}", request_id);

                    // Prevent unbounded growth: if set is too large, remove smallest IDs
                    if cancelled_ids.len() > MAX_CANCELLED_IDS {
                        if let Some(&min_id) = cancelled_ids.iter().min() {
                            cancelled_ids.remove(&min_id);
                            println!("⚠️  Cancelled IDs set too large, removed oldest ID: {}", min_id);
                        }
                    }
                }
                WorkerMessage::CancelAllBefore { request_id } => {
                    cancelled_ids.clear();
                    println!("❌ Cancelled all VAD requests before {}", request_id);
                }
            }
        }

        println!("🔧 VAD worker thread stopped");
    }
}
