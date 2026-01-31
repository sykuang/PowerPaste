#!/bin/bash
# Start WebDriverAgentMac for macOS E2E tests
# This script pre-starts WDA so tests don't have to wait for xcodebuild

set -e

WDA_PORT=${WDA_PORT:-10100}
WDA_PROJECT=~/.appium/node_modules/appium-mac2-driver/WebDriverAgentMac/WebDriverAgentMac.xcodeproj
DERIVED_DATA_DIR=~/Library/Developer/Xcode/DerivedData

echo "🚀 Starting WebDriverAgentMac on port $WDA_PORT..."

# Check if WDA is already running
if curl -s "http://127.0.0.1:$WDA_PORT/status" > /dev/null 2>&1; then
    echo "✅ WebDriverAgentMac is already running on port $WDA_PORT"
    exit 0
fi

# Kill any existing xcodebuild processes for WDA
pkill -f "WebDriverAgentRunner" 2>/dev/null || true

# Build WDA first (if needed)
echo "🔨 Building WebDriverAgentMac..."
cd ~/.appium/node_modules/appium-mac2-driver/WebDriverAgentMac
xcodebuild build-for-testing \
    -project WebDriverAgentMac.xcodeproj \
    -scheme WebDriverAgentRunner \
    COMPILER_INDEX_STORE_ENABLE=NO \
    -quiet 2>&1 | tail -5

# Start the test runner (which launches the WDA server)
echo "🏃 Starting WebDriverAgentRunner..."
xcodebuild test-without-building \
    -project WebDriverAgentMac.xcodeproj \
    -scheme WebDriverAgentRunner \
    COMPILER_INDEX_STORE_ENABLE=NO \
    USE_PORT=$WDA_PORT \
    2>&1 | grep -v "^$" &

WDA_PID=$!
echo "WDA_PID=$WDA_PID"

# Wait for WDA to start
echo "⏳ Waiting for WebDriverAgentMac to start..."
MAX_WAIT=120
WAITED=0
while ! curl -s "http://127.0.0.1:$WDA_PORT/status" > /dev/null 2>&1; do
    sleep 1
    WAITED=$((WAITED + 1))
    if [ $WAITED -ge $MAX_WAIT ]; then
        echo "❌ WebDriverAgentMac failed to start within ${MAX_WAIT}s"
        exit 1
    fi
    if [ $((WAITED % 10)) -eq 0 ]; then
        echo "   Still waiting... ($WAITED seconds)"
    fi
done

echo "✅ WebDriverAgentMac is running on http://127.0.0.1:$WDA_PORT"
echo "   PID: $WDA_PID"

# Save PID for later cleanup
echo $WDA_PID > /tmp/wda-mac-pid.txt
