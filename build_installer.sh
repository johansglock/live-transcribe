#!/bin/bash
set -e

echo "Building Live Transcribe installer..."
echo

# Get version (default to 0.0.0-dev for local builds)
VERSION="${1:-0.0.0-dev}"
echo "Version: $VERSION"
echo

# Build release binaries
echo "==> Building release binaries..."
cargo build --release --bin live-transcribe
cargo build --release --bin generate-icon
echo

# Generate app icon
echo "==> Generating app icon..."
./target/release/generate-icon
echo

# Create .app bundle
echo "==> Creating .app bundle..."
APP_NAME="LiveTranscribe"
BUNDLE_ID="nl.300.live-transcribe"
BUNDLE_DIR="$APP_NAME.app"

rm -rf "$BUNDLE_DIR"
mkdir -p "$BUNDLE_DIR/Contents/MacOS"
mkdir -p "$BUNDLE_DIR/Contents/Resources"

# Copy binary
cp target/release/live-transcribe "$BUNDLE_DIR/Contents/MacOS/$APP_NAME"
chmod +x "$BUNDLE_DIR/Contents/MacOS/$APP_NAME"

# Copy icon
cp AppIcon.icns "$BUNDLE_DIR/Contents/Resources/"

# Create Info.plist
cat > "$BUNDLE_DIR/Contents/Info.plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleExecutable</key>
    <string>$APP_NAME</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>CFBundleIdentifier</key>
    <string>$BUNDLE_ID</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>$APP_NAME</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>$VERSION</string>
    <key>CFBundleVersion</key>
    <string>$VERSION</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.15</string>
    <key>LSUIElement</key>
    <string>1</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSMicrophoneUsageDescription</key>
    <string>Live Transcribe needs microphone access to transcribe your speech in real-time.</string>
    <key>NSPrincipalClass</key>
    <string>NSApplication</string>
    <key>NSSupportsAutomaticGraphicsSwitching</key>
    <true/>
</dict>
</plist>
EOF

echo "✅ App bundle created: $BUNDLE_DIR"
echo

# Sign the app bundle with ad-hoc signature
# This ensures consistent identifier across updates for permissions
echo "==> Signing app bundle..."
codesign --force --deep --sign - \
         --identifier "$BUNDLE_ID" \
         "$BUNDLE_DIR"

if [ $? -eq 0 ]; then
    echo "✅ App bundle signed successfully"
    codesign -dv "$BUNDLE_DIR" 2>&1 | grep "Identifier\|Signature"
else
    echo "⚠️  Warning: Code signing failed, but continuing..."
fi
echo

# Create package structure
echo "==> Creating package structure..."
rm -rf package
mkdir -p package/root/Applications
mkdir -p package/scripts

# Copy .app bundle to a temporary location with the full path
cp -R "$BUNDLE_DIR" package/root/Applications/

# Create LaunchAgent template
mkdir -p package/root/Library/LaunchAgents
cat > package/root/Library/LaunchAgents/nl.300.live-transcribe.plist << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>nl.300.live-transcribe</string>

    <key>ProgramArguments</key>
    <array>
        <string>/Applications/LiveTranscribe.app/Contents/MacOS/LiveTranscribe</string>
    </array>

    <key>RunAtLoad</key>
    <true/>

    <key>KeepAlive</key>
    <true/>

    <key>EnvironmentVariables</key>
    <dict>
        <key>PATH</key>
        <string>/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin</string>
    </dict>

    <key>StandardOutPath</key>
    <string>{{HOME}}/.live-transcribe/logs/stdout.log</string>

    <key>StandardErrorPath</key>
    <string>{{HOME}}/.live-transcribe/logs/stderr.log</string>

    <key>ProcessType</key>
    <string>Interactive</string>
</dict>
</plist>
EOF

# Create postinstall script
cat > package/scripts/postinstall << 'EOF'
#!/bin/bash

# Get the user who invoked the installer (not root)
CURRENT_USER="${USER}"
if [ "$CURRENT_USER" = "root" ]; then
    CURRENT_USER=$(stat -f "%Su" /dev/console)
fi
USER_HOME=$(eval echo ~$CURRENT_USER)

echo "Installing for user: $CURRENT_USER"
echo "Home directory: $USER_HOME"

# Create logs directory
mkdir -p "$USER_HOME/.live-transcribe/logs"
chown -R "$CURRENT_USER" "$USER_HOME/.live-transcribe"

# Copy LaunchAgent to user's LaunchAgents directory and substitute HOME path
mkdir -p "$USER_HOME/Library/LaunchAgents"
sed "s|{{HOME}}|$USER_HOME|g" /Library/LaunchAgents/nl.300.live-transcribe.plist > "$USER_HOME/Library/LaunchAgents/nl.300.live-transcribe.plist"
chown "$CURRENT_USER" "$USER_HOME/Library/LaunchAgents/nl.300.live-transcribe.plist"

# Wait for the app to be installed before proceeding
echo "Waiting for app installation to complete..."
APP_PATH="/Applications/LiveTranscribe.app/Contents/MacOS/LiveTranscribe"
for i in {1..30}; do
    if [ -f "$APP_PATH" ]; then
        echo "App found!"
        break
    fi
    sleep 1
done

# Create a setup script that will download the model and start the app
cat > /tmp/livetranscribe-setup.sh << SETUP_EOF
#!/bin/bash
echo "=========================================="
echo "Live Transcribe - First Time Setup"
echo "=========================================="
echo ""

# Use the app path detected during postinstall
APP_PATH="$APP_PATH"

if [ -z "\$APP_PATH" ] || [ ! -f "\$APP_PATH" ]; then
    echo "❌ Installation failed. App not found."
    echo ""
    echo "Press any key to close this window..."
    read -n 1
    exit 1
fi

echo "Downloading Whisper model..."
echo "This will take a few minutes..."
echo ""

# Download the model
"\$APP_PATH" download-model

if [ $? -eq 0 ]; then
    echo ""
    echo "✅ Model downloaded successfully!"
    echo ""
    echo "Starting Live Transcribe..."
    sleep 2

    # Start the app via LaunchAgent
    launchctl load "$HOME/Library/LaunchAgents/nl.300.live-transcribe.plist"

    echo ""
    echo "✅ Live Transcribe is now running in your menu bar!"
    echo ""
    echo "Press any key to close this window..."
    read -n 1
else
    echo ""
    echo "❌ Model download failed. Please try again later."
    echo ""
    echo "You can manually download the model by running:"
    echo "  /Applications/LiveTranscribe.app/Contents/MacOS/LiveTranscribe download-model"
    echo ""
    echo "Press any key to close this window..."
    read -n 1
fi
SETUP_EOF

chmod +x /tmp/livetranscribe-setup.sh
chown "$CURRENT_USER" /tmp/livetranscribe-setup.sh

# Open Terminal to run the setup script as the user
su - "$CURRENT_USER" -c "open -a Terminal /tmp/livetranscribe-setup.sh"

exit 0
EOF

chmod +x package/scripts/postinstall

# Create preinstall script to stop existing service
cat > package/scripts/preinstall << 'EOF'
#!/bin/bash

# Get the user who invoked the installer
CURRENT_USER="${USER}"
if [ "$CURRENT_USER" = "root" ]; then
    CURRENT_USER=$(stat -f "%Su" /dev/console)
fi
USER_HOME=$(eval echo ~$CURRENT_USER)

echo "Checking for existing installation..."

# Unload existing LaunchAgent if it exists
if [ -f "$USER_HOME/Library/LaunchAgents/nl.300.live-transcribe.plist" ]; then
    echo "Stopping existing service..."
    su - "$CURRENT_USER" -c "launchctl unload \"$USER_HOME/Library/LaunchAgents/nl.300.live-transcribe.plist\"" 2>/dev/null || true
    # Give the app time to shut down gracefully
    sleep 1
fi

# Kill any remaining LiveTranscribe processes to ensure clean state
echo "Ensuring all LiveTranscribe processes are stopped..."
pkill -f "LiveTranscribe" 2>/dev/null || true
pkill -f "live-transcribe" 2>/dev/null || true
sleep 1

# Force remove existing app to ensure clean install (check both possible locations)
if [ -d "/Applications/LiveTranscribe.app" ]; then
    echo "Removing existing app at /Applications/LiveTranscribe.app..."
    rm -rf "/Applications/LiveTranscribe.app" 2>/dev/null || true
fi
if [ -d "/Applications/LiveTranscribe.localized" ]; then
    echo "Removing existing app at /Applications/LiveTranscribe.localized..."
    rm -rf "/Applications/LiveTranscribe.localized" 2>/dev/null || true
fi

exit 0
EOF

chmod +x package/scripts/preinstall

echo "==> Building component package..."
pkgbuild --root package/root \
         --scripts package/scripts \
         --identifier nl.300.live-transcribe \
         --version "$VERSION" \
         --install-location / \
         --filter .DS_Store \
         live-transcribe-$VERSION.pkg

echo
echo "==> Creating distribution XML..."
cat > distribution.xml << EOF
<?xml version="1.0" encoding="utf-8"?>
<installer-gui-script minSpecVersion="1">
    <title>Live Transcribe</title>
    <organization>nl.300</organization>
    <domains enable_localSystem="false" enable_currentUserHome="false" enable_anywhere="false"/>
    <options customize="never" require-scripts="false" hostArchitectures="arm64,x86_64" rootVolumeOnly="true"/>

    <welcome file="welcome.html"/>
    <license file="license.html"/>
    <conclusion file="conclusion.html"/>

    <pkg-ref id="nl.300.live-transcribe"/>

    <options customize="never" require-scripts="true"/>

    <choices-outline>
        <line choice="default">
            <line choice="nl.300.live-transcribe"/>
        </line>
    </choices-outline>

    <choice id="default"/>

    <choice id="nl.300.live-transcribe" visible="false">
        <pkg-ref id="nl.300.live-transcribe"/>
    </choice>

    <pkg-ref id="nl.300.live-transcribe" version="$VERSION">live-transcribe-$VERSION.pkg</pkg-ref>
</installer-gui-script>
EOF

# Create welcome text
cat > welcome.html << 'EOF'
<!DOCTYPE html>
<html>
<body>
<h1>Welcome to Live Transcribe</h1>
<p>This installer will install Live Transcribe, a real-time speech-to-text application that runs entirely on your Mac.</p>
<p><strong>Features:</strong></p>
<ul>
    <li>Real-time transcription with low latency</li>
    <li>Fully offline - no internet required</li>
    <li>System tray integration</li>
    <li>Global hotkeys for start/stop</li>
    <li>Sandboxed for security</li>
</ul>
</body>
</html>
EOF

# Create readme
cat > readme.html << 'EOF'
<!DOCTYPE html>
<html>
<body>
<h1>Installation</h1>
<p>Live Transcribe will be installed to <code>/Applications/LiveTranscribe.app</code></p>

<h2>First Run</h2>
<ol>
    <li>After installation, the app will start automatically in the menu bar</li>
    <li>Click the menu bar icon to download a Whisper model</li>
    <li>Grant Accessibility permissions:
        <br><em>System Settings > Privacy & Security > Accessibility</em>
        <br>(Add and enable 'LiveTranscribe')
    </li>
    <li>Use the hotkeys to transcribe:
        <ul>
            <li><strong>Cmd+Shift+T</strong> - Start transcription</li>
            <li><strong>Cmd+Shift+S</strong> - Stop transcription</li>
        </ul>
    </li>
</ol>

<h2>Configuration</h2>
<p>Edit <code>~/.live-transcribe/settings.yaml</code> to customize hotkeys and transcription settings.</p>

<h2>Logs</h2>
<p>View logs using macOS Console or Terminal:</p>
<pre>log show --predicate 'process == "LiveTranscribe"' --last 1h</pre>
<pre>log stream --predicate 'process == "LiveTranscribe"'</pre>
</body>
</html>
EOF

# Create license
cat > license.html << 'EOF'
<!DOCTYPE html>
<html>
<body>
<h1>License</h1>
<p>MIT License</p>
<p>Copyright (c) 2024</p>
<p>Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:</p>

<p>The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.</p>

<p>THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.</p>
</body>
</html>
EOF

# Create conclusion screen (shown AFTER installation completes)
cat > conclusion.html << 'EOF'
<!DOCTYPE html>
<html>
<head>
<style>
body {
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
    padding: 20px;
}
h1 {
    color: #1d4ed8;
}
.important {
    background-color: #fef3c7;
    border-left: 4px solid #f59e0b;
    padding: 15px;
    margin: 20px 0;
}
.step {
    background-color: #f0f9ff;
    border-left: 4px solid #3b82f6;
    padding: 12px;
    margin: 15px 0;
}
.step h3 {
    margin-top: 0;
    color: #1e40af;
}
.hotkey {
    font-family: monospace;
    background-color: #e5e7eb;
    padding: 2px 6px;
    border-radius: 3px;
}
</style>
</head>
<body>
<h1>Installation Complete!</h1>
<p>Live Transcribe is now running in your menu bar. Look for the icon at the top-right of your screen.</p>

<div class="important">
<strong>⚠️ IMPORTANT: Complete these steps to use the app</strong>
</div>

<div class="step">
<h3>Step 1: Grant Permissions (Required)</h3>
<p>Go to <strong>System Settings > Privacy & Security</strong> and grant these permissions:</p>
<ul>
    <li><strong>Microphone</strong> - Enable for "LiveTranscribe" to record audio</li>
    <li><strong>Accessibility</strong> - Enable for "LiveTranscribe" to type transcribed text</li>
</ul>
<p><em>Without these permissions, the app cannot function.</em></p>
</div>

<div class="step">
<h3>Step 2: Start Transcribing!</h3>
<p>The <strong>small.en</strong> model has been automatically downloaded and configured.</p>
<p>Use these keyboard shortcuts:</p>
<ul>
    <li><span class="hotkey">Cmd+Shift+T</span> - Start transcription</li>
    <li><span class="hotkey">Cmd+Shift+S</span> - Stop transcription</li>
</ul>
<p>You can customize these shortcuts in <code>~/.live-transcribe/settings.yaml</code></p>
</div>

<div class="step">
<h3>Optional: Try Different Models</h3>
<p>Want to try a different model? Available options:</p>
<ul>
    <li><strong>tiny.en</strong> - Fastest, lower accuracy (~75MB)</li>
    <li><strong>base.en</strong> - Fast, good accuracy (~142MB)</li>
    <li><strong>small.en</strong> - Balanced (default) (~466MB)</li>
    <li><strong>medium.en</strong> - Best accuracy, slower (~1.5GB)</li>
</ul>
<p>To change models, update the model in <code>~/.live-transcribe/settings.yaml</code> and run the app's "Download Model" menu option.</p>
</div>

<p><strong>Need help?</strong> Configuration file: <code>~/.live-transcribe/settings.yaml</code></p>
</body>
</html>
EOF

echo "==> Building product package..."
productbuild --distribution distribution.xml \
             --resources . \
             --package-path . \
             LiveTranscribe-$VERSION-installer.pkg

echo
echo "==> Cleaning up intermediate files..."
# Remove intermediate files created during build
rm -f live-transcribe-$VERSION.pkg  # Component package
rm -rf "$BUNDLE_DIR"                 # .app bundle (already packaged)
rm -rf package                       # Package staging directory
rm -f distribution.html welcome.html readme.html license.html conclusion.html  # Temporary installer resources

echo
echo "✅ Build complete!"
echo
echo "Files created:"
echo "  - LiveTranscribe-$VERSION-installer.pkg (macOS installer)"
echo
echo "To test the installer:"
echo "  open LiveTranscribe-$VERSION-installer.pkg"
echo
echo "To uninstall after testing:"
echo "  launchctl unload ~/Library/LaunchAgents/nl.300.live-transcribe.plist"
echo "  rm ~/Library/LaunchAgents/nl.300.live-transcribe.plist"
echo "  rm -rf /Applications/LiveTranscribe.app"
echo
