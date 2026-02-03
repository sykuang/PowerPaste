#!/bin/bash

# Script to generate all app icons from the logo SVG
# Requires ImageMagick (brew install imagemagick)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
SVG_SOURCE="$PROJECT_ROOT/src/assets/logo.svg"
ICONS_DIR="$PROJECT_ROOT/src-tauri/icons"

if [ ! -f "$SVG_SOURCE" ]; then
  echo "Error: Logo SVG not found at $SVG_SOURCE"
  exit 1
fi

if ! command -v magick &> /dev/null; then
  echo "Error: ImageMagick not found. Install it with: brew install imagemagick"
  exit 1
fi

echo "Generating icons from $SVG_SOURCE..."

# Common options: 8-bit depth for Tauri compatibility, transparent background
# PNG32: forces 32-bit RGBA output (required by Tauri)
OPTS="-background none -depth 8"

# Standard PNG icons
magick $OPTS "$SVG_SOURCE" -resize 32x32 PNG32:"$ICONS_DIR/32x32.png"
magick $OPTS "$SVG_SOURCE" -resize 128x128 PNG32:"$ICONS_DIR/128x128.png"
magick $OPTS "$SVG_SOURCE" -resize 256x256 PNG32:"$ICONS_DIR/256x256.png"
magick $OPTS "$SVG_SOURCE" -resize 512x512 PNG32:"$ICONS_DIR/icon.png"

# Windows Store icons
magick $OPTS "$SVG_SOURCE" -resize 30x30 PNG32:"$ICONS_DIR/Square30x30Logo.png"
magick $OPTS "$SVG_SOURCE" -resize 44x44 PNG32:"$ICONS_DIR/Square44x44Logo.png"
magick $OPTS "$SVG_SOURCE" -resize 71x71 PNG32:"$ICONS_DIR/Square71x71Logo.png"
magick $OPTS "$SVG_SOURCE" -resize 89x89 PNG32:"$ICONS_DIR/Square89x89Logo.png"
magick $OPTS "$SVG_SOURCE" -resize 107x107 PNG32:"$ICONS_DIR/Square107x107Logo.png"
magick $OPTS "$SVG_SOURCE" -resize 142x142 PNG32:"$ICONS_DIR/Square142x142Logo.png"
magick $OPTS "$SVG_SOURCE" -resize 150x150 PNG32:"$ICONS_DIR/Square150x150Logo.png"
magick $OPTS "$SVG_SOURCE" -resize 284x284 PNG32:"$ICONS_DIR/Square284x284Logo.png"
magick $OPTS "$SVG_SOURCE" -resize 310x310 PNG32:"$ICONS_DIR/Square310x310Logo.png"
magick $OPTS "$SVG_SOURCE" -resize 50x50 PNG32:"$ICONS_DIR/StoreLogo.png"

# Windows .ico file
magick "$SVG_SOURCE" -define icon:auto-resize=256,128,64,48,32,16 "$ICONS_DIR/icon.ico"

# macOS .icns file
TEMP_ICONSET=$(mktemp -d)/icon.iconset
mkdir -p "$TEMP_ICONSET"

magick $OPTS "$SVG_SOURCE" -resize 16x16 PNG32:"$TEMP_ICONSET/icon_16x16.png"
magick $OPTS "$SVG_SOURCE" -resize 32x32 PNG32:"$TEMP_ICONSET/icon_16x16@2x.png"
magick $OPTS "$SVG_SOURCE" -resize 32x32 PNG32:"$TEMP_ICONSET/icon_32x32.png"
magick $OPTS "$SVG_SOURCE" -resize 64x64 PNG32:"$TEMP_ICONSET/icon_32x32@2x.png"
magick $OPTS "$SVG_SOURCE" -resize 128x128 PNG32:"$TEMP_ICONSET/icon_128x128.png"
magick $OPTS "$SVG_SOURCE" -resize 256x256 PNG32:"$TEMP_ICONSET/icon_128x128@2x.png"
magick $OPTS "$SVG_SOURCE" -resize 256x256 PNG32:"$TEMP_ICONSET/icon_256x256.png"
magick $OPTS "$SVG_SOURCE" -resize 512x512 PNG32:"$TEMP_ICONSET/icon_256x256@2x.png"
magick $OPTS "$SVG_SOURCE" -resize 512x512 PNG32:"$TEMP_ICONSET/icon_512x512.png"
magick $OPTS "$SVG_SOURCE" -resize 1024x1024 PNG32:"$TEMP_ICONSET/icon_512x512@2x.png"

iconutil -c icns "$TEMP_ICONSET" -o "$ICONS_DIR/icon.icns"
rm -rf "$(dirname "$TEMP_ICONSET")"

# macOS Menu Bar (Tray) Template Icons
# Template icons must be black with alpha for macOS to colorize them properly
TRAY_SVG="$PROJECT_ROOT/src/assets/tray-icon-template.svg"
if [ -f "$TRAY_SVG" ]; then
  echo "Generating tray template icons..."
  # Standard size for macOS menu bar is 22x22 points.
  # We generate a 44x44 pixel image (2x scale) to look crisp on Retina displays.
  # Since we handle loading manually in Rust, we just need one high-quality asset.
  magick $OPTS "$TRAY_SVG" -resize 44x44 PNG32:"$ICONS_DIR/tray-icon.png"
  echo "  - Tray icon: $ICONS_DIR/tray-icon.png"
else
  echo "Warning: Tray icon template SVG not found at $TRAY_SVG"
fi

echo "✓ All icons generated successfully!"
echo "  - PNG icons: $ICONS_DIR/"
echo "  - Windows .ico: $ICONS_DIR/icon.ico"
echo "  - macOS .icns: $ICONS_DIR/icon.icns"
