use anyhow::{Context, Result};
use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager,
};
use crate::config::HotkeyConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyEvent {
    StartTranscription,
    StopTranscription,
    ToggleTranscription,
}

pub struct HotkeyManager {
    manager: GlobalHotKeyManager,
    start_hotkey: Option<HotKey>,
    stop_hotkey: Option<HotKey>,
    toggle_hotkey: Option<HotKey>,
}

impl HotkeyManager {
    pub fn new(config: &HotkeyConfig) -> Result<Self> {
        let manager = GlobalHotKeyManager::new()
            .context("Failed to create global hotkey manager")?;

        // Check if using toggle mode (same hotkey for start and stop)
        let use_toggle = config.start_transcription == config.stop_transcription;

        let (start_hotkey, stop_hotkey, toggle_hotkey) = if use_toggle {
            // Toggle mode: single hotkey
            let hotkey = Self::parse_hotkey(&config.start_transcription)
                .context("Failed to parse toggle hotkey")?;
            manager.register(hotkey)
                .context("Failed to register toggle hotkey")?;

            println!("Registered global hotkey:");
            println!("  Toggle: {}", config.start_transcription);

            (None, None, Some(hotkey))
        } else {
            // Separate mode: different hotkeys for start and stop
            let start = Self::parse_hotkey(&config.start_transcription)
                .context("Failed to parse start transcription hotkey")?;
            manager.register(start)
                .context("Failed to register start transcription hotkey")?;

            let stop = Self::parse_hotkey(&config.stop_transcription)
                .context("Failed to parse stop transcription hotkey")?;
            manager.register(stop)
                .context("Failed to register stop transcription hotkey")?;

            println!("Registered global hotkeys:");
            println!("  Start: {}", config.start_transcription);
            println!("  Stop: {}", config.stop_transcription);

            (Some(start), Some(stop), None)
        };

        Ok(HotkeyManager {
            manager,
            start_hotkey,
            stop_hotkey,
            toggle_hotkey,
        })
    }

    fn parse_hotkey(hotkey_str: &str) -> Result<HotKey> {
        let parts: Vec<&str> = hotkey_str.split('+').map(|s| s.trim()).collect();

        if parts.is_empty() {
            anyhow::bail!("Hotkey string is empty");
        }

        let mut modifiers = Modifiers::empty();
        let mut key_code = None;

        for part in parts {
            match part.to_lowercase().as_str() {
                "cmd" | "command" | "super" => modifiers |= Modifiers::SUPER,
                "ctrl" | "control" => modifiers |= Modifiers::CONTROL,
                "alt" | "option" => modifiers |= Modifiers::ALT,
                "shift" => modifiers |= Modifiers::SHIFT,
                // Parse the actual key
                key => {
                    key_code = Some(Self::parse_key_code(key)?);
                }
            }
        }

        let code = key_code.context("No key code found in hotkey string")?;
        Ok(HotKey::new(Some(modifiers), code))
    }

    fn parse_key_code(key: &str) -> Result<Code> {
        match key.to_uppercase().as_str() {
            "A" => Ok(Code::KeyA),
            "B" => Ok(Code::KeyB),
            "C" => Ok(Code::KeyC),
            "D" => Ok(Code::KeyD),
            "E" => Ok(Code::KeyE),
            "F" => Ok(Code::KeyF),
            "G" => Ok(Code::KeyG),
            "H" => Ok(Code::KeyH),
            "I" => Ok(Code::KeyI),
            "J" => Ok(Code::KeyJ),
            "K" => Ok(Code::KeyK),
            "L" => Ok(Code::KeyL),
            "M" => Ok(Code::KeyM),
            "N" => Ok(Code::KeyN),
            "O" => Ok(Code::KeyO),
            "P" => Ok(Code::KeyP),
            "Q" => Ok(Code::KeyQ),
            "R" => Ok(Code::KeyR),
            "S" => Ok(Code::KeyS),
            "T" => Ok(Code::KeyT),
            "U" => Ok(Code::KeyU),
            "V" => Ok(Code::KeyV),
            "W" => Ok(Code::KeyW),
            "X" => Ok(Code::KeyX),
            "Y" => Ok(Code::KeyY),
            "Z" => Ok(Code::KeyZ),
            "0" => Ok(Code::Digit0),
            "1" => Ok(Code::Digit1),
            "2" => Ok(Code::Digit2),
            "3" => Ok(Code::Digit3),
            "4" => Ok(Code::Digit4),
            "5" => Ok(Code::Digit5),
            "6" => Ok(Code::Digit6),
            "7" => Ok(Code::Digit7),
            "8" => Ok(Code::Digit8),
            "9" => Ok(Code::Digit9),
            "F1" => Ok(Code::F1),
            "F2" => Ok(Code::F2),
            "F3" => Ok(Code::F3),
            "F4" => Ok(Code::F4),
            "F5" => Ok(Code::F5),
            "F6" => Ok(Code::F6),
            "F7" => Ok(Code::F7),
            "F8" => Ok(Code::F8),
            "F9" => Ok(Code::F9),
            "F10" => Ok(Code::F10),
            "F11" => Ok(Code::F11),
            "F12" => Ok(Code::F12),
            "SPACE" => Ok(Code::Space),
            "ENTER" | "RETURN" => Ok(Code::Enter),
            "TAB" => Ok(Code::Tab),
            "BACKSPACE" => Ok(Code::Backspace),
            "ESCAPE" | "ESC" => Ok(Code::Escape),
            _ => anyhow::bail!("Unknown key code: {}", key),
        }
    }

    pub fn poll_event(&self) -> Option<HotkeyEvent> {
        if let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if let Some(toggle) = &self.toggle_hotkey {
                if event.id == toggle.id() {
                    return Some(HotkeyEvent::ToggleTranscription);
                }
            }
            if let Some(start) = &self.start_hotkey {
                if event.id == start.id() {
                    return Some(HotkeyEvent::StartTranscription);
                }
            }
            if let Some(stop) = &self.stop_hotkey {
                if event.id == stop.id() {
                    return Some(HotkeyEvent::StopTranscription);
                }
            }
        }
        None
    }
}

impl Drop for HotkeyManager {
    fn drop(&mut self) {
        if let Some(hotkey) = self.start_hotkey {
            let _ = self.manager.unregister(hotkey);
        }
        if let Some(hotkey) = self.stop_hotkey {
            let _ = self.manager.unregister(hotkey);
        }
        if let Some(hotkey) = self.toggle_hotkey {
            let _ = self.manager.unregister(hotkey);
        }
    }
}
