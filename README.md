# Live Transcribe

A macOS system tray application for live audio transcription using OpenAI's Whisper. Runs 100% offline with GPU acceleration via CoreML Neural Engine.

## Quick Start

```bash
# Install Rust if you haven't already
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and install
git clone https://github.com/johansglock/live-transcribe.git
cd live-transcribe
cargo install --path .

# Download Whisper model (one-time setup)
live-transcribe download-model

# Run the app
live-transcribe
```

Then press **Cmd+Shift+T** to start recording, **Cmd+Shift+S** to stop and transcribe!

## Features

- 🎙️ **Live audio transcription** - Record and transcribe audio in real-time
- ⚡ **GPU acceleration** - Uses Metal for fast inference on macOS
- 🔐 **100% offline** - No internet connection required
- 🛡️ **Sandboxed security** - Restricted file system and network access for safety
- ⌨️ **Global hotkeys** - Control transcription from anywhere
- 📋 **Auto-copy to clipboard** - Transcriptions automatically copied
- ⚙️ **YAML configuration** - Easy to customize settings
- 🎨 **System tray icon** - Minimal, unobtrusive interface

## Installation

**See [INSTALL.md](INSTALL.md) for detailed installation instructions.**

### Quick Install Options

**Option 1: macOS Installer (Recommended)**

Download the latest `.pkg` installer from [Releases](https://github.com/johansglock/live-transcribe/releases) and double-click to install. The app will automatically start and run in the background.

**Option 2: Build from Source**

```bash
# Prerequisites: macOS with Rust installed (https://rustup.rs)

git clone https://github.com/johansglock/live-transcribe.git
cd live-transcribe
cargo build --release
sudo cp target/release/live-transcribe /usr/local/bin/
live-transcribe download-model
live-transcribe
```

### Download Whisper Model

Use the built-in download command (easiest):

```bash
# Download the default medium.en model
cargo run -- download-model

# Or download a specific model
cargo run -- download-model tiny.en     # Fastest
cargo run -- download-model base.en     # Faster, good quality
cargo run -- download-model small.en    # Better quality
cargo run -- download-model base        # Multilingual
```

The download command automatically:
- Downloads the model file
- Downloads the CoreML encoder for Neural Engine acceleration (for `.en` models)
- Extracts and sets up everything correctly

**Available models**:
- `tiny.en` - Fastest, good quality (~75MB)
- `base.en` - Fast, good quality (~142MB)
- `small.en` - Better quality, slower (~466MB)
- `medium.en` - **Default** - best balance of quality and speed (~1.5GB)
- `tiny`, `base`, `small`, `medium` - Multilingual versions (slower, but support 99+ languages)

## Usage

### Running the Application

```bash
# Start the system tray app
cargo run --release

# Or run the binary directly
./target/release/live-transcribe
```

### CLI Commands

```bash
# Download a model
live-transcribe download-model [MODEL_NAME]

# Record and test streaming transcription (debugging)
live-transcribe test-record [NAME] --duration [SECONDS]

# Show help
live-transcribe --help

# Check version
live-transcribe --version
```

### Testing & Debugging

The `test-record` command helps debug and iterate on the streaming transcription algorithm:

```bash
# Record a 10-second test clip
cargo run -- test-record my-test --duration 10

# The command will:
# 1. Show suggested test phrases to read
# 2. Record audio for the specified duration
# 3. Save the audio to ~/.live-transcribe/test_recordings/
# 4. Simulate streaming transcription (300ms chunks with 2s sliding window)
# 5. Show what would be typed/deleted at each step
# 6. Display the final transcription

# This is invaluable for:
# - Testing the sliding window algorithm
# - Debugging correction logic
# - Identifying hallucinations on silence
# - Iterating faster without manual testing
```

### Uninstalling

```bash
cargo uninstall live-transcribe
```

### Default Hotkeys

- **Cmd+Shift+T** - Start transcription
- **Cmd+Shift+S** - Stop transcription and get result

### System Tray Menu

Click the tray icon to access:
- Start Transcription
- Stop Transcription
- Settings (opens config file)
- Quit

## Configuration

Settings are stored in `~/.live-transcribe/settings.yaml`:

```yaml
hotkeys:
  start_transcription: "Cmd+Shift+T"
  stop_transcription: "Cmd+Shift+S"

transcription:
  model: "medium.en"  # Use .en suffix for English-only CoreML models (default)
  language: "en"
  use_gpu: true        # Enables CoreML Neural Engine acceleration
  silence_threshold: 0.003  # Lower = more sensitive to quiet speech
```

### Hotkey Format

Combine modifiers with `+`:
- Modifiers: `Cmd`, `Ctrl`, `Alt`, `Shift`
- Keys: Letters (A-Z), numbers (0-9), function keys (F1-F12), etc.
- Examples: `"Cmd+Shift+R"`, `"Ctrl+Alt+T"`, `"F9"`

### Supported Languages

Set `language` to any supported Whisper language code:
- `en` - English
- `es` - Spanish
- `fr` - French
- `de` - German
- `ja` - Japanese
- And many more...

Or use `"auto"` for automatic detection.

## How It Works

1. Press the start hotkey or use the menu
2. Speak into your microphone
3. Press the stop hotkey when done
4. The transcription is automatically copied to your clipboard
5. Paste anywhere with Cmd+V

## Technical Details

- **Audio capture**: 16kHz mono via cpal
- **Transcription**: whisper.cpp with CoreML Neural Engine acceleration
- **Global hotkeys**: global-hotkey crate
- **System tray**: tray-icon crate
- **Performance**: On Apple Silicon (M1/M2/M3/M4), expect ~5-10x realtime with medium.en model (e.g., 10 seconds of audio transcribes in 1-2 seconds)

## Troubleshooting

### "Failed to build input stream" error

This means the app doesn't have microphone permissions. Fix it:

1. Open **System Settings** (or System Preferences)
2. Go to **Privacy & Security** → **Microphone**
3. Enable access for **Terminal** (or your terminal app like iTerm2, Warp, etc.)
4. Restart the app

**Note:** If you build a standalone app bundle later, you'll need to grant permissions to the app itself instead of Terminal.

### "Model file not found" error

Download a model:
```bash
live-transcribe download-model
```

### Hotkeys not working

Make sure the app has Accessibility permissions:
1. Go to **System Settings** → **Privacy & Security** → **Accessibility**
2. Enable access for **Terminal** (or your terminal app)
3. Restart the app

### No audio captured

Check your default input device in System Preferences → Sound → Input.

### Transcription is slow

Try a smaller model like `tiny` or `base`, or ensure `use_gpu: true` is set in the config.

## Security

This application uses macOS sandboxing to restrict its capabilities during transcription:

- ✅ **File access**: Only `~/.live-transcribe/` directory (config and models)
- ❌ **Network**: Completely blocked (fully offline operation)
- ❌ **Other files**: Cannot access Documents, Desktop, Downloads, etc.

The sandbox is automatically enabled when running the main app (not for `download-model` or test commands). See [SANDBOX.md](SANDBOX.md) for detailed security documentation.

## License

MIT

## Credits

Built with:
- [whisper.cpp](https://github.com/ggerganov/whisper.cpp)
- [whisper-rs](https://github.com/tazz4843/whisper-rs)
