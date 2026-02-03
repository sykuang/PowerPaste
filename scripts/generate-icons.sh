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
magick "$SVG_SOURCE" $OPTS -resize 32x32 PNG32:"$ICONS_DIR/32x32.png"
magick "$SVG_SOURCE" $OPTS -resize 128x128 PNG32:"$ICONS_DIR/128x128.png"
magick "$SVG_SOURCE" $OPTS -resize 256x256 PNG32:"$ICONS_DIR/128x128@2x.png"
magick "$SVG_SOURCE" $OPTS -resize 512x512 PNG32:"$ICONS_DIR/icon.png"

# Windows Store icons
magick "$SVG_SOURCE" $OPTS -resize 30x30 PNG32:"$ICONS_DIR/Square30x30Logo.png"
magick "$SVG_SOURCE" $OPTS -resize 44x44 PNG32:"$ICONS_DIR/Square44x44Logo.png"
magick "$SVG_SOURCE" $OPTS -resize 71x71 PNG32:"$ICONS_DIR/Square71x71Logo.png"
magick "$SVG_SOURCE" $OPTS -resize 89x89 PNG32:"$ICONS_DIR/Square89x89Logo.png"
magick "$SVG_SOURCE" $OPTS -resize 107x107 PNG32:"$ICONS_DIR/Square107x107Logo.png"
magick "$SVG_SOURCE" $OPTS -resize 142x142 PNG32:"$ICONS_DIR/Square142x142Logo.png"
magick "$SVG_SOURCE" $OPTS -resize 150x150 PNG32:"$ICONS_DIR/Square150x150Logo.png"
magick "$SVG_SOURCE" $OPTS -resize 284x284 PNG32:"$ICONS_DIR/Square284x284Logo.png"
magick "$SVG_SOURCE" $OPTS -resize 310x310 PNG32:"$ICONS_DIR/Square310x310Logo.png"
magick "$SVG_SOURCE" $OPTS -resize 50x50 PNG32:"$ICONS_DIR/StoreLogo.png"

# Windows .ico file
magick "$SVG_SOURCE" -define icon:auto-resize=256,128,64,48,32,16 "$ICONS_DIR/icon.ico"

# macOS .icns file
TEMP_ICONSET=$(mktemp -d)/icon.iconset
mkdir -p "$TEMP_ICONSET"

magick "$SVG_SOURCE" $OPTS -resize 16x16 PNG32:"$TEMP_ICONSET/icon_16x16.png"
magick "$SVG_SOURCE" $OPTS -resize 32x32 PNG32:"$TEMP_ICONSET/icon_16x16@2x.png"
magick "$SVG_SOURCE" $OPTS -resize 32x32 PNG32:"$TEMP_ICONSET/icon_32x32.png"
magick "$SVG_SOURCE" $OPTS -resize 64x64 PNG32:"$TEMP_ICONSET/icon_32x32@2x.png"
magick "$SVG_SOURCE" $OPTS -resize 128x128 PNG32:"$TEMP_ICONSET/icon_128x128.png"
magick "$SVG_SOURCE" $OPTS -resize 256x256 PNG32:"$TEMP_ICONSET/icon_128x128@2x.png"
magick "$SVG_SOURCE" $OPTS -resize 256x256 PNG32:"$TEMP_ICONSET/icon_256x256.png"
magick "$SVG_SOURCE" $OPTS -resize 512x512 PNG32:"$TEMP_ICONSET/icon_256x256@2x.png"
magick "$SVG_SOURCE" $OPTS -resize 512x512 PNG32:"$TEMP_ICONSET/icon_512x512.png"
magick "$SVG_SOURCE" $OPTS -resize 1024x1024 PNG32:"$TEMP_ICONSET/icon_512x512@2x.png"

iconutil -c icns "$TEMP_ICONSET" -o "$ICONS_DIR/icon.icns"
rm -rf "$(dirname "$TEMP_ICONSET")"

echo "✓ All icons generated successfully!"
echo "  - PNG icons: $ICONS_DIR/"
echo "  - Windows .ico: $ICONS_DIR/icon.ico"
echo "  - macOS .icns: $ICONS_DIR/icon.icns"
