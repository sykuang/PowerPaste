#!/bin/bash
# Stop WebDriverAgentMac if running
# Used after E2E tests complete

WDA_PORT=${WDA_PORT:-10100}

echo "🛑 Stopping WebDriverAgentMac..."

# Stop by saved PID
if [ -f /tmp/wda-mac-pid.txt ]; then
    WDA_PID=$(cat /tmp/wda-mac-pid.txt)
    if kill -0 $WDA_PID 2>/dev/null; then
        echo "   Killing WDA process $WDA_PID"
        kill $WDA_PID 2>/dev/null || true
    fi
    rm /tmp/wda-mac-pid.txt
fi

# Also kill any remaining xcodebuild processes for WDA
pkill -f "WebDriverAgentRunner" 2>/dev/null || true

# Verify it's stopped
sleep 1
if curl -s "http://127.0.0.1:$WDA_PORT/status" > /dev/null 2>&1; then
    echo "⚠️  WebDriverAgentMac is still running, force killing..."
    pkill -9 -f "WebDriverAgentRunner" 2>/dev/null || true
else
    echo "✅ WebDriverAgentMac stopped"
fi
