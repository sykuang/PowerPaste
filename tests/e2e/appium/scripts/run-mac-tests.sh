#!/bin/bash
# One-click macOS E2E test runner
# Starts WDA, runs tests, cleans up

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../../../.." && pwd)"
WDA_PORT=10100
APPIUM_PORT=4723
WDA_PID_FILE="/tmp/wda-mac-pid.txt"
APPIUM_PID_FILE="/tmp/appium-pid.txt"

cleanup() {
    echo "[run-mac-tests] Cleaning up..."
    
    # Kill Appium
    if [ -f "$APPIUM_PID_FILE" ]; then
        APPIUM_PID=$(cat "$APPIUM_PID_FILE")
        if kill -0 "$APPIUM_PID" 2>/dev/null; then
            kill "$APPIUM_PID" 2>/dev/null || true
        fi
        rm -f "$APPIUM_PID_FILE"
    fi
    
    # Kill WDA
    if [ -f "$WDA_PID_FILE" ]; then
        WDA_PID=$(cat "$WDA_PID_FILE")
        if kill -0 "$WDA_PID" 2>/dev/null; then
            kill "$WDA_PID" 2>/dev/null || true
        fi
        rm -f "$WDA_PID_FILE"
    fi
    
    # Kill any orphaned processes
    pkill -f "WebDriverAgentRunner" 2>/dev/null || true
    pkill -f "appium.*--port $APPIUM_PORT" 2>/dev/null || true
}

trap cleanup EXIT

echo "[run-mac-tests] 🚀 Starting macOS E2E tests..."

# Pre-authenticate sudo to avoid GUI password prompts later
# This keeps sudo credentials cached for the duration of the script
if ! sudo -n true 2>/dev/null; then
    echo "[run-mac-tests] 🔐 Please enter your password to authorize Accessibility access:"
    sudo -v
fi
# Keep sudo alive in background
while true; do sudo -n true; sleep 50; kill -0 "$$" || exit; done 2>/dev/null &

# Check if WDA is already running
if curl -s "http://localhost:$WDA_PORT/status" | grep -q "ready.*true"; then
    echo "[run-mac-tests] ✓ WDA already running on port $WDA_PORT"
    WDA_ALREADY_RUNNING=true
else
    WDA_ALREADY_RUNNING=false
    echo "[run-mac-tests] Starting WebDriverAgentMac..."
    
    # Start WDA in background
    cd "$PROJECT_ROOT"
    WDA_PROJECT="$HOME/.appium/node_modules/appium-mac2-driver/WebDriverAgentMac/WebDriverAgentMac.xcodeproj"
    
    if [ ! -d "$WDA_PROJECT" ]; then
        echo "[run-mac-tests] ✗ WebDriverAgentMac not found at: $WDA_PROJECT"
        echo "[run-mac-tests] Please ensure appium-mac2-driver is installed: appium driver install mac2"
        exit 1
    fi
    
    xcodebuild \
        -project "$WDA_PROJECT" \
        -scheme WebDriverAgentRunner \
        -destination 'platform=macOS' \
        -derivedDataPath /tmp/wda-mac \
        test \
        USE_PORT=$WDA_PORT \
        > /tmp/wda-mac.log 2>&1 &
    
    echo $! > "$WDA_PID_FILE"
    
    # Wait for WDA to be ready
    echo "[run-mac-tests] Waiting for WDA to start (this may take a moment on first run)..."
    MAX_WAIT=120
    WAITED=0
    while [ $WAITED -lt $MAX_WAIT ]; do
        if curl -s "http://localhost:$WDA_PORT/status" | grep -q "ready.*true"; then
            echo "[run-mac-tests] ✓ WDA ready after ${WAITED}s"
            break
        fi
        sleep 2
        WAITED=$((WAITED + 2))
        echo -n "."
    done
    echo ""
    
    if [ $WAITED -ge $MAX_WAIT ]; then
        echo "[run-mac-tests] ✗ WDA failed to start within ${MAX_WAIT}s"
        exit 1
    fi
fi

# Check if Appium is already running
if curl -s "http://localhost:$APPIUM_PORT/status" | grep -q "ready.*true"; then
    echo "[run-mac-tests] ✓ Appium already running on port $APPIUM_PORT"
    APPIUM_ALREADY_RUNNING=true
else
    APPIUM_ALREADY_RUNNING=false
    echo "[run-mac-tests] Starting Appium server..."
    
    appium --relaxed-security --port $APPIUM_PORT > /tmp/appium.log 2>&1 &
    echo $! > "$APPIUM_PID_FILE"
    
    # Wait for Appium
    sleep 3
    MAX_WAIT=30
    WAITED=0
    while [ $WAITED -lt $MAX_WAIT ]; do
        if curl -s "http://localhost:$APPIUM_PORT/status" | grep -q "ready.*true"; then
            echo "[run-mac-tests] ✓ Appium ready"
            break
        fi
        sleep 1
        WAITED=$((WAITED + 1))
    done
    
    if [ $WAITED -ge $MAX_WAIT ]; then
        echo "[run-mac-tests] ✗ Appium failed to start"
        exit 1
    fi
fi

# Run tests
echo "[run-mac-tests] 🧪 Running tests..."
cd "$PROJECT_ROOT"

POWERPASTE_WDA_PRESTARTED=true \
POWERPASTE_TEST_WORKERS="${POWERPASTE_TEST_WORKERS:-1}" \
npx wdio run tests/e2e/appium/wdio.mac.conf.ts "$@"

TEST_EXIT_CODE=$?

echo "[run-mac-tests] Tests completed with exit code: $TEST_EXIT_CODE"

# Only cleanup processes we started
if [ "$WDA_ALREADY_RUNNING" = true ]; then
    rm -f "$WDA_PID_FILE"  # Don't kill pre-existing WDA
fi
if [ "$APPIUM_ALREADY_RUNNING" = true ]; then
    rm -f "$APPIUM_PID_FILE"  # Don't kill pre-existing Appium
fi

exit $TEST_EXIT_CODE
