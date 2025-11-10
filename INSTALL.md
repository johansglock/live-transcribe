# Installation

## Option 1: macOS Installer (Recommended)

1. Download the latest `LiveTranscribe-*-installer.pkg` from [Releases](https://github.com/johansglock/live-transcribe/releases)
2. Double-click the .pkg file and follow the installer
3. The installer will automatically:
   - Download the Whisper model (small.en)
   - Start the application
   - Set up auto-start on login
4. Grant Accessibility permissions when prompted:
   - Open System Settings > Privacy & Security > Accessibility
   - Add `LiveTranscribe` and enable it

The app will be running in your menu bar!

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
cp nl.300.live-transcribe.plist ~/Library/LaunchAgents/

# Load the service
launchctl load ~/Library/LaunchAgents/nl.300.live-transcribe.plist
```

## Configuration

The app uses sensible defaults. To customize, create `~/.live-transcribe/settings.yaml`:

```yaml
# Optional: Customize hotkeys (default: Option+Space for toggle)
hotkeys:
  start_transcription: "Cmd+Shift+T"
  stop_transcription: "Cmd+Shift+T"

# Optional: Change model (default: small.en)
transcription:
  model: "medium.en"
```

All settings are optional. Only add settings you want to change from defaults.

## Available Models

- `tiny.en` - Fastest, less accurate (~75MB)
- `base.en` - Fast, good accuracy (~142MB)
- `small.en` - Balanced, recommended (~466MB) - **Default**
- `medium.en` - High accuracy (~1.5GB)

The installer automatically downloads `small.en`. To use a different model:
```bash
live-transcribe download-model <model-name>
```

Then update `~/.live-transcribe/settings.yaml` to use the new model.

## Usage

The app runs in your system tray (menu bar). Use the hotkey to control it:

- **Option+Space** - Toggle transcription (start/stop)

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
launchctl unload ~/Library/LaunchAgents/nl.300.live-transcribe.plist

# Start
launchctl load ~/Library/LaunchAgents/nl.300.live-transcribe.plist
```

### Uninstall

```bash
# Stop service
launchctl unload ~/Library/LaunchAgents/nl.300.live-transcribe.plist

# Remove files
rm ~/Library/LaunchAgents/nl.300.live-transcribe.plist
sudo rm /usr/local/bin/live-transcribe
rm -rf ~/.live-transcribe
```
