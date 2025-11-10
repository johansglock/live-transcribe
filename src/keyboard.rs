#[cfg(target_os = "macos")]
pub mod macos {
    use core_graphics::event::{CGEvent, CGEventTapLocation, CGKeyCode};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};

    const KEY_DELETE: CGKeyCode = 51; // Backspace key code

    // Global lock to serialize keyboard events (prevent race conditions)
    static KEYBOARD_LOCK: Mutex<()> = Mutex::new(());

    // Track if we've checked accessibility permissions
    static ACCESSIBILITY_CHECKED: AtomicBool = AtomicBool::new(false);
    static ACCESSIBILITY_GRANTED: AtomicBool = AtomicBool::new(false);

    /// Check if accessibility permissions are granted
    /// This uses a test keyboard event to verify permissions
    fn check_accessibility_permissions() -> bool {
        // Cache the result to avoid repeated checks
        if ACCESSIBILITY_CHECKED.load(Ordering::Relaxed) {
            return ACCESSIBILITY_GRANTED.load(Ordering::Relaxed);
        }

        // Try to create an event source - this will fail if permissions are not granted
        let has_permission = match CGEventSource::new(CGEventSourceStateID::HIDSystemState) {
            Ok(source) => {
                // Try to create a simple keyboard event to verify we can actually post events
                match CGEvent::new_keyboard_event(source, 0, true) {
                    Ok(_) => true,
                    Err(_) => false,
                }
            }
            Err(_) => false,
        };

        ACCESSIBILITY_CHECKED.store(true, Ordering::Relaxed);
        ACCESSIBILITY_GRANTED.store(has_permission, Ordering::Relaxed);

        if !has_permission {
            eprintln!("\n⚠️  WARNING: Accessibility permissions not granted!");
            eprintln!("    Keyboard typing will not work.");
            eprintln!("    Please grant accessibility access:");
            eprintln!("    1. Open System Settings → Privacy & Security → Accessibility");
            eprintln!("    2. Enable access for Terminal (or your terminal app)");
            eprintln!("    3. Restart the app\n");
        }

        has_permission
    }

    /// Append text (terminal-friendly - just adds new characters)
    pub fn append_text(text: &str) {
        if text.is_empty() {
            return;
        }

        // Check accessibility permissions before attempting keyboard injection
        if !check_accessibility_permissions() {
            eprintln!("⚠️  Skipping text append - no accessibility permissions");
            return;
        }

        let _lock = KEYBOARD_LOCK.lock().unwrap();

        let source = match CGEventSource::new(CGEventSourceStateID::HIDSystemState) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to create event source: {:?}", e);
                return;
            }
        };

        println!("⌨️  Typing {} chars: {:?}", text.len(), text);
        let utf16: Vec<u16> = text.encode_utf16().collect();
        if let Ok(event) = CGEvent::new_keyboard_event(source, 0, true) {
            event.set_string_from_utf16_unchecked(&utf16);
            event.post(CGEventTapLocation::HID);
            println!("✅ Keyboard event posted successfully");
        } else {
            eprintln!("❌ Failed to create keyboard event");
        }
    }

    /// Replace text by deleting old chars and typing new (safer than Ctrl+U for multi-line)
    pub fn replace_text_with_backspace(delete_count: usize, new_text: &str) {
        if delete_count == 0 && new_text.is_empty() {
            return;
        }

        // Check accessibility permissions before attempting keyboard injection
        if !check_accessibility_permissions() {
            eprintln!("⚠️  Skipping text replacement - no accessibility permissions");
            return;
        }

        let _lock = KEYBOARD_LOCK.lock().unwrap();

        let source = match CGEventSource::new(CGEventSourceStateID::HIDSystemState) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to create event source: {:?}", e);
                return;
            }
        };

        println!("⌨️  Replacing text: deleting {} chars, typing {} chars: {:?}", delete_count, new_text.len(), new_text);

        // Post delete events for the old text
        for _ in 0..delete_count {
            if let Ok(event) = CGEvent::new_keyboard_event(source.clone(), KEY_DELETE, true) {
                event.post(CGEventTapLocation::HID);
            }
            if let Ok(event) = CGEvent::new_keyboard_event(source.clone(), KEY_DELETE, false) {
                event.post(CGEventTapLocation::HID);
            }
        }

        // Delay to ensure deletes are processed
        if delete_count > 0 {
            let delay_ms = (delete_count * 2).max(10).min(200);
            std::thread::sleep(std::time::Duration::from_millis(delay_ms as u64));
        }

        // Type the new text
        if !new_text.is_empty() {
            let utf16: Vec<u16> = new_text.encode_utf16().collect();
            if let Ok(event) = CGEvent::new_keyboard_event(source, 0, true) {
                event.set_string_from_utf16_unchecked(&utf16);
                event.post(CGEventTapLocation::HID);
                println!("✅ Keyboard event posted successfully");
            } else {
                eprintln!("❌ Failed to create keyboard event");
            }
        }
    }

}

#[cfg(not(target_os = "macos"))]
pub mod macos {
    pub fn append_text(_text: &str) {
        eprintln!("Keyboard typing only supported on macOS");
    }

    pub fn replace_text_with_backspace(_delete_count: usize, _new_text: &str) {
        eprintln!("Keyboard replacement only supported on macOS");
    }
}
