#!/usr/bin/env bash
# Monitor DuckDB persistence in real-time

DB_PATH="$HOME/.local/share/flowsurface/flowsurface.duckdb"

echo "=== FlowSurface Database Monitor ==="
echo ""

if [ ! -f "$DB_PATH" ]; then
    echo "❌ Database not found at: $DB_PATH"
    echo "Make sure to run: export FLOWSURFACE_USE_DUCKDB=1"
    exit 1
fi

echo "✓ Database found: $DB_PATH"
echo ""

# Check if duckdb CLI is available
if ! command -v duckdb &> /dev/null; then
    echo "⚠️  DuckDB CLI not installed. Showing file size only."
    echo ""
    watch -n 2 "ls -lh $DB_PATH | awk '{print \"Database size: \" \$5}'"
    exit 0
fi

# Full monitoring with DuckDB queries in READ-ONLY mode
while true; do
    clear
    echo "=== FlowSurface Database Monitor ==="
    date
    echo ""

    duckdb "$DB_PATH" -readonly << 'EOF'
-- Database size
.mode line
SELECT
    database_size,
    wal_size,
    memory_usage,
    memory_limit
FROM pragma_database_size();

-- Trades summary
.mode box
SELECT
    t.symbol,
    e.name as exchange,
    COUNT(*) as trade_count,
    to_timestamp(MIN(tr.timestamp) / 1000) as first_trade,
    to_timestamp(MAX(tr.timestamp) / 1000) as latest_trade
FROM trades tr
JOIN tickers t ON tr.ticker_id = t.ticker_id
JOIN exchanges e ON t.exchange_id = e.exchange_id
GROUP BY t.symbol, e.name
ORDER BY trade_count DESC;

-- Klines summary
SELECT
    t.symbol,
    k.timeframe,
    COUNT(*) as kline_count
FROM klines k
JOIN tickers t ON k.ticker_id = t.ticker_id
GROUP BY t.symbol, k.timeframe
ORDER BY t.symbol, k.timeframe;

-- Overall stats
.mode line
SELECT
    (SELECT COUNT(*) FROM trades) as total_trades,
    (SELECT COUNT(*) FROM klines) as total_klines,
    (SELECT COUNT(*) FROM tickers) as total_tickers,
    (SELECT COUNT(*) FROM exchanges) as total_exchanges;
EOF

    echo ""
    echo "Refreshing in 5 seconds... (Ctrl+C to exit)"
    sleep 5
done
