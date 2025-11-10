// Test the keyboard output state management logic
// This tests the logic that was buggy: managing vad_committed_text and live_preview_text

#[derive(Debug, PartialEq)]
struct KeyboardAction {
    delete_count: usize,
    type_text: String,
}

#[derive(Debug)]
struct StateManager {
    vad_committed_text: String,
    live_preview_text: String,
    actions: Vec<KeyboardAction>,
}

impl StateManager {
    fn new() -> Self {
        StateManager {
            vad_committed_text: String::new(),
            live_preview_text: String::new(),
            actions: Vec::new(),
        }
    }

    /// Simulate receiving a VAD commit
    fn vad_commit(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }

        // Current state: how many chars are on screen right now
        let current_char_count = self.live_preview_text.chars().count();

        // Build what the full committed text should be: old committed + new VAD result
        let new_vad_committed = self.vad_committed_text.clone() + text + " ";

        // Record the keyboard action: delete everything and retype full VAD committed
        self.actions.push(KeyboardAction {
            delete_count: current_char_count,
            type_text: new_vad_committed.clone(),
        });

        // Update state
        self.vad_committed_text = new_vad_committed.clone();
        self.live_preview_text = new_vad_committed;
    }

    /// Simulate receiving a live preview
    fn live_preview(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }

        // Strategy: Delete all text on screen, then retype VAD committed + live preview
        let current_char_count = self.live_preview_text.chars().count();

        // Build full text: VAD committed + new live preview
        let full_live_text = self.vad_committed_text.clone() + text;

        // Record the keyboard action
        self.actions.push(KeyboardAction {
            delete_count: current_char_count,
            type_text: full_live_text.clone(),
        });

        self.live_preview_text = full_live_text;
    }

    /// Get the final text that should be on screen
    fn get_screen_text(&self) -> String {
        self.live_preview_text.clone()
    }

    /// Simulate what would actually appear on screen by replaying all keyboard actions
    fn replay_actions(&self) -> String {
        let mut screen = String::new();

        for action in &self.actions {
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
}

#[test]
fn test_multiple_vad_commits() {
    let mut state = StateManager::new();

    // First VAD commit
    state.vad_commit("Hello");
    assert_eq!(state.get_screen_text(), "Hello ");
    assert_eq!(state.replay_actions(), "Hello ");

    // Second VAD commit
    state.vad_commit("world");
    assert_eq!(state.get_screen_text(), "Hello world ");
    assert_eq!(state.replay_actions(), "Hello world ");

    // Third VAD commit
    state.vad_commit("how are you");
    assert_eq!(state.get_screen_text(), "Hello world how are you ");
    assert_eq!(state.replay_actions(), "Hello world how are you ");
}

#[test]
fn test_vad_then_live_preview() {
    let mut state = StateManager::new();

    // VAD commit
    state.vad_commit("Hello");
    assert_eq!(state.get_screen_text(), "Hello ");

    // Live preview
    state.live_preview("world");
    assert_eq!(state.get_screen_text(), "Hello world");
    assert_eq!(state.replay_actions(), "Hello world");

    // Another live preview (updating the preview)
    state.live_preview("world there");
    assert_eq!(state.get_screen_text(), "Hello world there");
    assert_eq!(state.replay_actions(), "Hello world there");
}

#[test]
fn test_live_preview_then_vad_commit() {
    let mut state = StateManager::new();

    // VAD commit
    state.vad_commit("Hello");
    assert_eq!(state.get_screen_text(), "Hello ");

    // Live preview
    state.live_preview("world");
    assert_eq!(state.get_screen_text(), "Hello world");

    // VAD commit (should replace live preview)
    state.vad_commit("everyone");
    assert_eq!(state.get_screen_text(), "Hello everyone ");
    assert_eq!(state.replay_actions(), "Hello everyone ");
}

#[test]
fn test_complex_sequence() {
    let mut state = StateManager::new();

    // VAD 1
    state.vad_commit("Okay.");
    assert_eq!(state.replay_actions(), "Okay. ");

    // VAD 2
    state.vad_commit("How are we?");
    assert_eq!(state.replay_actions(), "Okay. How are we? ");

    // Live preview
    state.live_preview("still have to double.");
    assert_eq!(state.replay_actions(), "Okay. How are we? still have to double.");

    // VAD 3 (this was the buggy case!)
    state.vad_commit("have to double if you want to.");
    assert_eq!(state.replay_actions(), "Okay. How are we? have to double if you want to. ");

    // Verify no duplication
    let final_text = state.replay_actions();
    assert!(!final_text.contains("Okay.Okay."), "Should not have duplicated 'Okay.'");
    assert!(!final_text.contains("How are we?How are we?"), "Should not have duplicated 'How are we?'");
}

#[test]
fn test_empty_inputs() {
    let mut state = StateManager::new();

    state.vad_commit("");
    assert_eq!(state.get_screen_text(), "");

    state.live_preview("");
    assert_eq!(state.get_screen_text(), "");

    state.vad_commit("Hello");
    assert_eq!(state.get_screen_text(), "Hello ");

    state.vad_commit("");
    assert_eq!(state.get_screen_text(), "Hello ");

    state.live_preview("");
    assert_eq!(state.get_screen_text(), "Hello ");
}

#[test]
fn test_user_reported_bug_okay_that() {
    // User reported: "Okay. Okay. Okay. that."
    // When they said: "Okay. [PAUSE] That was not good"
    let mut state = StateManager::new();

    // First VAD commit: "Okay."
    state.vad_commit("Okay.");
    let screen1 = state.replay_actions();
    println!("After VAD1: '{}'", screen1);
    assert_eq!(screen1, "Okay. ");

    // Second VAD commit: "that."
    state.vad_commit("that.");
    let screen2 = state.replay_actions();
    println!("After VAD2: '{}'", screen2);
    assert_eq!(screen2, "Okay. that. ");

    // Should NOT see duplication
    assert!(!screen2.contains("Okay. Okay."), "Should not duplicate first VAD");
    assert!(!screen2.contains("that. that."), "Should not duplicate second VAD");

    // Count occurrences
    let okay_count = screen2.matches("Okay.").count();
    let that_count = screen2.matches("that.").count();
    assert_eq!(okay_count, 1, "Should have exactly one 'Okay.'");
    assert_eq!(that_count, 1, "Should have exactly one 'that.'");
}
