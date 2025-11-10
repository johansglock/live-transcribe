#!/bin/bash
set -e

# Create macOS .app bundle from the binary
# This gives the app a proper icon in System Settings

if [ -z "$1" ]; then
    echo "Usage: $0 <version>"
    exit 1
fi

VERSION="$1"
APP_NAME="LiveTranscribe"
BUNDLE_ID="com.johansglock.live-transcribe"
BINARY_PATH="target/release/live-transcribe"

echo "Creating $APP_NAME.app bundle..."

# Create bundle structure
BUNDLE_DIR="$APP_NAME.app"
rm -rf "$BUNDLE_DIR"
mkdir -p "$BUNDLE_DIR/Contents/MacOS"
mkdir -p "$BUNDLE_DIR/Contents/Resources"

# Copy binary
cp "$BINARY_PATH" "$BUNDLE_DIR/Contents/MacOS/$APP_NAME"
chmod +x "$BUNDLE_DIR/Contents/MacOS/$APP_NAME"

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
    <true/>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSMicrophoneUsageDescription</key>
    <string>Live Transcribe needs microphone access to transcribe your speech in real-time.</string>
</dict>
</plist>
EOF

# Generate app icon from tray icon code
# We'll create a simple icon using ImageMagick or sips
if command -v sips &> /dev/null; then
    # Create a simple icon (we'll improve this)
    # For now, create a colored square as a placeholder
    echo "Creating app icon..."

    # Create PNG icon at different sizes
    for size in 16 32 128 256 512; do
        # Create a simple colored circle as icon
        # This is a placeholder - we should use the actual waveform icon from tray.rs
        sips -z $size $size /System/Library/CoreServices/CoreTypes.bundle/Contents/Resources/BookmarkIcon.icns --out "$BUNDLE_DIR/Contents/Resources/icon_${size}.png" 2>/dev/null || true
    done

    # Convert to .icns (if we have iconutil)
    if command -v iconutil &> /dev/null; then
        mkdir -p AppIcon.iconset
        for size in 16 32 128 256 512; do
            size2=$((size * 2))
            cp "$BUNDLE_DIR/Contents/Resources/icon_${size}.png" "AppIcon.iconset/icon_${size}x${size}.png" 2>/dev/null || true
            cp "$BUNDLE_DIR/Contents/Resources/icon_${size}.png" "AppIcon.iconset/icon_${size}x${size}@2x.png" 2>/dev/null || true
        done
        iconutil -c icns AppIcon.iconset -o "$BUNDLE_DIR/Contents/Resources/AppIcon.icns"
        rm -rf AppIcon.iconset
        rm -f "$BUNDLE_DIR/Contents/Resources/icon_"*.png
    fi
else
    echo "Warning: sips not found, skipping icon creation"
fi

echo "✅ App bundle created: $BUNDLE_DIR"
echo "   Binary: $BUNDLE_DIR/Contents/MacOS/$APP_NAME"
echo "   Info.plist: $BUNDLE_DIR/Contents/Info.plist"
