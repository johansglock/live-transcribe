#!/bin/bash
set -e

echo "Building Live Transcribe installer..."
echo

# Get version (default to 0.0.0-dev for local builds)
VERSION="${1:-0.0.0-dev}"
echo "Version: $VERSION"
echo

# Build release binary
echo "==> Building release binary..."
cargo build --release --bin live-transcribe
echo

# Create package structure
echo "==> Creating package structure..."
rm -rf package
mkdir -p package/root/usr/local/bin
mkdir -p package/scripts

# Copy binary
cp target/release/live-transcribe package/root/usr/local/bin/
chmod +x package/root/usr/local/bin/live-transcribe

# Create LaunchAgent template
mkdir -p package/root/Library/LaunchAgents
cat > package/root/Library/LaunchAgents/com.johansglock.live-transcribe.plist << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.johansglock.live-transcribe</string>

    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/live-transcribe</string>
    </array>

    <key>RunAtLoad</key>
    <true/>

    <key>KeepAlive</key>
    <true/>

    <key>StandardOutPath</key>
    <string>/tmp/live-transcribe-stdout.log</string>

    <key>StandardErrorPath</key>
    <string>/tmp/live-transcribe-stderr.log</string>

    <key>EnvironmentVariables</key>
    <dict>
        <key>PATH</key>
        <string>/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin</string>
    </dict>

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

# Create config directory
mkdir -p "$USER_HOME/.live-transcribe/logs"
chown -R "$CURRENT_USER" "$USER_HOME/.live-transcribe"

# Copy LaunchAgent to user's LaunchAgents directory
mkdir -p "$USER_HOME/Library/LaunchAgents"
cp /Library/LaunchAgents/com.johansglock.live-transcribe.plist "$USER_HOME/Library/LaunchAgents/"
chown "$CURRENT_USER" "$USER_HOME/Library/LaunchAgents/com.johansglock.live-transcribe.plist"

# Update the log paths to use the user's home directory
sed -i '' "s|/tmp/live-transcribe-stdout.log|$USER_HOME/.live-transcribe/logs/stdout.log|g" \
    "$USER_HOME/Library/LaunchAgents/com.johansglock.live-transcribe.plist"
sed -i '' "s|/tmp/live-transcribe-stderr.log|$USER_HOME/.live-transcribe/logs/stderr.log|g" \
    "$USER_HOME/Library/LaunchAgents/com.johansglock.live-transcribe.plist"

# Load the LaunchAgent as the user
su - "$CURRENT_USER" -c "launchctl load \"$USER_HOME/Library/LaunchAgents/com.johansglock.live-transcribe.plist\""

echo
echo "✅ Live Transcribe has been installed and started!"
echo
echo "Configuration directory: $USER_HOME/.live-transcribe"
echo "Logs: $USER_HOME/.live-transcribe/logs"
echo
echo "Next steps:"
echo "1. Download a model: live-transcribe download-model"
echo "2. Grant Accessibility permissions:"
echo "   System Settings > Privacy & Security > Accessibility"
echo "   (Add and enable 'live-transcribe')"
echo
echo "Default hotkeys:"
echo "  Cmd+Shift+T - Start transcription"
echo "  Cmd+Shift+S - Stop transcription"
echo

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
if [ -f "$USER_HOME/Library/LaunchAgents/com.johansglock.live-transcribe.plist" ]; then
    echo "Stopping existing service..."
    su - "$CURRENT_USER" -c "launchctl unload \"$USER_HOME/Library/LaunchAgents/com.johansglock.live-transcribe.plist\"" 2>/dev/null || true
fi

exit 0
EOF

chmod +x package/scripts/preinstall

echo "==> Building component package..."
pkgbuild --root package/root \
         --scripts package/scripts \
         --identifier com.johansglock.live-transcribe \
         --version "$VERSION" \
         --install-location / \
         live-transcribe-$VERSION.pkg

echo
echo "==> Creating distribution XML..."
cat > distribution.xml << EOF
<?xml version="1.0" encoding="utf-8"?>
<installer-gui-script minSpecVersion="1">
    <title>Live Transcribe</title>
    <organization>com.johansglock</organization>
    <domains enable_localSystem="true"/>
    <options customize="never" require-scripts="false" hostArchitectures="arm64,x86_64"/>

    <welcome file="welcome.html"/>
    <readme file="readme.html"/>
    <license file="license.html"/>

    <pkg-ref id="com.johansglock.live-transcribe"/>

    <options customize="never" require-scripts="true"/>

    <choices-outline>
        <line choice="default">
            <line choice="com.johansglock.live-transcribe"/>
        </line>
    </choices-outline>

    <choice id="default"/>

    <choice id="com.johansglock.live-transcribe" visible="false">
        <pkg-ref id="com.johansglock.live-transcribe"/>
    </choice>

    <pkg-ref id="com.johansglock.live-transcribe" version="$VERSION">live-transcribe-$VERSION.pkg</pkg-ref>
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
<p>Live Transcribe will be installed to <code>/usr/local/bin/live-transcribe</code></p>

<h2>First Run</h2>
<ol>
    <li>After installation, the app will start automatically</li>
    <li>Download a model: Open Terminal and run <code>live-transcribe download-model</code></li>
    <li>Grant Accessibility permissions in System Settings > Privacy & Security > Accessibility</li>
    <li>Use the hotkeys:
        <ul>
            <li><strong>Cmd+Shift+T</strong> - Start transcription</li>
            <li><strong>Cmd+Shift+S</strong> - Stop transcription</li>
        </ul>
    </li>
</ol>

<h2>Configuration</h2>
<p>Edit <code>~/.live-transcribe/settings.yaml</code> to customize hotkeys and transcription settings.</p>

<h2>Logs</h2>
<p>View logs at <code>~/.live-transcribe/logs/</code></p>
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

echo "==> Building product package..."
productbuild --distribution distribution.xml \
             --resources . \
             --package-path . \
             LiveTranscribe-$VERSION-installer.pkg

# Remove intermediate component package (not needed by users)
rm -f live-transcribe-$VERSION.pkg

echo
echo "==> Creating ZIP of binary..."
cd target/release
zip ../../live-transcribe-$VERSION-macos.zip live-transcribe
cd ../..

echo
echo "==> Generating checksums..."
shasum -a 256 LiveTranscribe-$VERSION-installer.pkg > checksums.txt
shasum -a 256 live-transcribe-$VERSION-macos.zip >> checksums.txt

echo
echo "✅ Build complete!"
echo
echo "Files created:"
echo "  - LiveTranscribe-$VERSION-installer.pkg (macOS installer)"
echo "  - live-transcribe-$VERSION-macos.zip (binary only)"
echo "  - checksums.txt (SHA256 checksums)"
echo
echo "To test the installer:"
echo "  open LiveTranscribe-$VERSION-installer.pkg"
echo
echo "To uninstall after testing:"
echo "  launchctl unload ~/Library/LaunchAgents/com.johansglock.live-transcribe.plist"
echo "  rm ~/Library/LaunchAgents/com.johansglock.live-transcribe.plist"
echo "  sudo rm /usr/local/bin/live-transcribe"
echo
