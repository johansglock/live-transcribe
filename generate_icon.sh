#!/bin/bash

# This script generates the PNG icon from SVG
# Uses sips (built-in macOS tool) or qlmanage

# First, try to convert SVG to PNG using qlmanage (renders SVG)
qlmanage -t -s 64 -o assets/ assets/icon.svg 2>/dev/null

# Rename the output
if [ -f "assets/icon.svg.png" ]; then
    mv assets/icon.svg.png assets/icon@2x.png
    # Create smaller version
    sips -z 32 32 assets/icon@2x.png --out assets/icon.png >/dev/null 2>&1
    echo "Icons generated successfully!"
else
    echo "Warning: Could not generate PNG from SVG. You may need to install librsvg (brew install librsvg) and update this script."
    echo "For now, the SVG will be used directly (may not work with all tray implementations)."
fi
