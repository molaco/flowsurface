#!/usr/bin/env bash
# Simple persistence monitor - shows logs and file size

DB_PATH="$HOME/.local/share/flowsurface/flowsurface.duckdb"
LOG_PATH="$HOME/.local/share/flowsurface/flowsurface-current.log"

echo "=== FlowSurface Persistence Monitor ==="
echo ""

if [ ! -f "$DB_PATH" ]; then
    echo "❌ Database not found at: $DB_PATH"
    echo ""
    echo "Make sure to:"
    echo "  1. export FLOWSURFACE_USE_DUCKDB=1"
    echo "  2. Start FlowSurface"
    echo "  3. Connect to an exchange and add some tickers"
    exit 1
fi

echo "✓ Database found!"
echo ""

# Function to get human-readable file size
get_size() {
    if [ -f "$DB_PATH" ]; then
        ls -lh "$DB_PATH" | awk '{print $5}'
    else
        echo "N/A"
    fi
}

# Function to count recent persistence events
count_recent_events() {
    if [ -f "$LOG_PATH" ]; then
        tail -100 "$LOG_PATH" | grep -c "✓ Persisted" || echo "0"
    else
        echo "0"
    fi
}

echo "Monitoring persistence events (Ctrl+C to exit)..."
echo "Database: $DB_PATH"
echo ""

INITIAL_SIZE=$(get_size)
echo "Initial size: $INITIAL_SIZE"
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# Watch the log for persistence events
tail -f "$LOG_PATH" | grep --line-buffered "✓ Persisted\|ERROR.*persist" | while read -r line; do
    CURRENT_SIZE=$(get_size)
    TIMESTAMP=$(date '+%H:%M:%S')

    if echo "$line" | grep -q "ERROR"; then
        echo "[$TIMESTAMP] [SIZE: $CURRENT_SIZE] ❌ $line"
    else
        echo "[$TIMESTAMP] [SIZE: $CURRENT_SIZE] $line"
    fi
done
