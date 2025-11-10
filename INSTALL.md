# Installation

## Option 1: macOS Installer (Recommended)

1. Download the latest `LiveTranscribe-*-installer.pkg` from [Releases](https://github.com/johansglock/live-transcribe/releases)
2. Double-click the .pkg file and follow the installer
3. Download a model:
   ```bash
   live-transcribe download-model
   ```
4. Grant Accessibility permissions:
   - Open System Settings > Privacy & Security > Accessibility
   - Add `live-transcribe` and enable it

The app will start automatically and run in the background!

## Option 2: Manual Installation

### Build from source

```bash
# Clone the repository
git clone https://github.com/johansglock/live-transcribe.git
cd live-transcribe

# Build release binary
cargo build --release

# Copy to PATH
sudo cp target/release/live-transcribe /usr/local/bin/

# Create config directory
mkdir -p ~/.live-transcribe/logs

# Download a model
live-transcribe download-model

# Run manually
live-transcribe
```

### Set up auto-start (optional)

```bash
# Copy LaunchAgent plist (update paths if needed)
cp com.johansglock.live-transcribe.plist ~/Library/LaunchAgents/

# Load the service
launchctl load ~/Library/LaunchAgents/com.johansglock.live-transcribe.plist
```

## Configuration

Edit `~/.live-transcribe/settings.yaml` to customize:

```yaml
hotkeys:
  start_transcription: "Cmd+Shift+T"
  stop_transcription: "Cmd+Shift+S"

transcription:
  model: "medium.en"
  language: "en"
  use_gpu: true
  streaming: true
  chunk_duration_ms: 300
  silence_threshold: 0.003
```

## Available Models

- `tiny.en` - Fastest, less accurate (~75MB)
- `base.en` - Fast, good accuracy (~145MB)
- `small.en` - Balanced (~485MB)
- `medium.en` - High accuracy, recommended (~1.5GB)
- `large-v2` - Best accuracy, slowest (~3GB)

Download a model:
```bash
live-transcribe download-model <model-name>
```

## Usage

The app runs in your system tray (menu bar). Use the hotkeys to control it:

- **Cmd+Shift+T** - Start transcription
- **Cmd+Shift+S** - Stop transcription

When transcribing, a blinking red dot appears on the icon.

## Troubleshooting

### Check if service is running
```bash
launchctl list | grep live-transcribe
```

### View logs
```bash
tail -f ~/.live-transcribe/logs/stdout.log
tail -f ~/.live-transcribe/logs/stderr.log
```

### Stop/start service
```bash
# Stop
launchctl unload ~/Library/LaunchAgents/com.johansglock.live-transcribe.plist

# Start
launchctl load ~/Library/LaunchAgents/com.johansglock.live-transcribe.plist
```

### Uninstall

```bash
# Stop service
launchctl unload ~/Library/LaunchAgents/com.johansglock.live-transcribe.plist

# Remove files
rm ~/Library/LaunchAgents/com.johansglock.live-transcribe.plist
sudo rm /usr/local/bin/live-transcribe
rm -rf ~/.live-transcribe
```
