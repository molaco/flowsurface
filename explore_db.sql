-- ============================================
-- FlowSurface Database Explorer
-- Run with: duckdb ~/.local/share/flowsurface/flowsurface.duckdb -readonly < explore_db.sql
-- ============================================

.mode line
.echo on

-- ============================================
-- 1. DATABASE OVERVIEW
-- ============================================
.print
.print "═══════════════════════════════════════════"
.print "DATABASE OVERVIEW"
.print "═══════════════════════════════════════════"

SELECT
    database_size,
    wal_size,
    memory_usage,
    memory_limit
FROM pragma_database_size();

-- ============================================
-- 2. ALL TABLES
-- ============================================
.print
.print "═══════════════════════════════════════════"
.print "TABLES IN DATABASE"
.print "═══════════════════════════════════════════"

.mode box
SHOW TABLES;

-- ============================================
-- 3. OVERALL STATISTICS
-- ============================================
.print
.print "═══════════════════════════════════════════"
.print "OVERALL STATISTICS"
.print "═══════════════════════════════════════════"

.mode line
SELECT
    (SELECT COUNT(*) FROM trades) as total_trades,
    (SELECT COUNT(*) FROM klines) as total_klines,
    (SELECT COUNT(*) FROM depth_snapshots) as total_depth_snapshots,
    (SELECT COUNT(*) FROM tickers) as total_tickers,
    (SELECT COUNT(*) FROM exchanges) as total_exchanges;

-- ============================================
-- 4. EXCHANGES
-- ============================================
.print
.print "═══════════════════════════════════════════"
.print "EXCHANGES"
.print "═══════════════════════════════════════════"

.mode box
SELECT * FROM exchanges ORDER BY exchange_id;

-- ============================================
-- 5. TICKERS
-- ============================================
.print
.print "═══════════════════════════════════════════"
.print "TICKERS"
.print "═══════════════════════════════════════════"

SELECT
    t.ticker_id,
    e.name as exchange,
    t.symbol,
    t.min_ticksize,
    t.min_qty,
    t.market_type
FROM tickers t
JOIN exchanges e ON t.exchange_id = e.exchange_id
ORDER BY t.ticker_id;

-- ============================================
-- 6. TRADES SUMMARY BY TICKER
-- ============================================
.print
.print "═══════════════════════════════════════════"
.print "TRADES SUMMARY BY TICKER"
.print "═══════════════════════════════════════════"

SELECT
    t.symbol,
    e.name as exchange,
    FORMAT('{:,}', COUNT(*)) as total_trades,
    to_timestamp(MIN(tr.timestamp) / 1000) as first_trade,
    to_timestamp(MAX(tr.timestamp) / 1000) as last_trade,
    ROUND(MIN(tr.price), 2) as min_price,
    ROUND(MAX(tr.price), 2) as max_price,
    FORMAT('{:,.2f}', SUM(tr.quantity)) as total_volume
FROM trades tr
JOIN tickers t ON tr.ticker_id = t.ticker_id
JOIN exchanges e ON t.exchange_id = e.exchange_id
GROUP BY t.symbol, e.name
ORDER BY COUNT(*) DESC;

-- ============================================
-- 7. TRADE BUY/SELL DISTRIBUTION
-- ============================================
.print
.print "═══════════════════════════════════════════"
.print "BUY/SELL DISTRIBUTION"
.print "═══════════════════════════════════════════"

SELECT
    t.symbol,
    SUM(CASE WHEN tr.is_buyer_maker THEN 1 ELSE 0 END) as buy_trades,
    SUM(CASE WHEN NOT tr.is_buyer_maker THEN 1 ELSE 0 END) as sell_trades,
    ROUND(100.0 * SUM(CASE WHEN tr.is_buyer_maker THEN 1 ELSE 0 END) / COUNT(*), 2) || '%' as buy_pct
FROM trades tr
JOIN tickers t ON tr.ticker_id = t.ticker_id
GROUP BY t.symbol;

-- ============================================
-- 8. RECENT TRADES (Last 10)
-- ============================================
.print
.print "═══════════════════════════════════════════"
.print "RECENT TRADES (Last 10)"
.print "═══════════════════════════════════════════"

SELECT
    t.symbol,
    to_timestamp(tr.timestamp / 1000) as time,
    ROUND(tr.price, 2) as price,
    ROUND(tr.quantity, 4) as qty,
    CASE WHEN tr.is_buyer_maker THEN 'BUY' ELSE 'SELL' END as side
FROM trades tr
JOIN tickers t ON tr.ticker_id = t.ticker_id
ORDER BY tr.timestamp DESC
LIMIT 10;

-- ============================================
-- 9. KLINES SUMMARY
-- ============================================
.print
.print "═══════════════════════════════════════════"
.print "KLINES SUMMARY BY TICKER & TIMEFRAME"
.print "═══════════════════════════════════════════"

SELECT
    t.symbol,
    k.timeframe,
    COUNT(*) as kline_count,
    to_timestamp(MIN(k.candle_time) / 1000) as first_kline,
    to_timestamp(MAX(k.candle_time) / 1000) as last_kline
FROM klines k
JOIN tickers t ON k.ticker_id = t.ticker_id
GROUP BY t.symbol, k.timeframe
ORDER BY t.symbol, k.timeframe;

-- ============================================
-- 10. RECENT KLINES (Last 5 per timeframe)
-- ============================================
.print
.print "═══════════════════════════════════════════"
.print "RECENT KLINES (Last 5 of 1m timeframe)"
.print "═══════════════════════════════════════════"

.mode line
SELECT
    t.symbol,
    to_timestamp(k.candle_time / 1000) as time,
    ROUND(k.open_price, 2) as open,
    ROUND(k.high_price, 2) as high,
    ROUND(k.low_price, 2) as low,
    ROUND(k.close_price, 2) as close,
    ROUND(k.volume, 2) as volume
FROM klines k
JOIN tickers t ON k.ticker_id = t.ticker_id
WHERE k.timeframe = '1m'
ORDER BY k.candle_time DESC
LIMIT 5;

-- ============================================
-- 11. TRADES PER HOUR (Last 24 hours)
-- ============================================
.print
.print "═══════════════════════════════════════════"
.print "TRADES PER HOUR"
.print "═══════════════════════════════════════════"

.mode box
SELECT
    t.symbol,
    to_timestamp(tr.timestamp / 1000)::DATE as date,
    EXTRACT(HOUR FROM to_timestamp(tr.timestamp / 1000)) as hour,
    COUNT(*) as trade_count,
    FORMAT('{:,.2f}', SUM(tr.quantity)) as total_volume
FROM trades tr
JOIN tickers t ON tr.ticker_id = t.ticker_id
GROUP BY t.symbol, date, hour
ORDER BY date DESC, hour DESC
LIMIT 10;

-- ============================================
-- 12. PRICE STATISTICS
-- ============================================
.print
.print "═══════════════════════════════════════════"
.print "PRICE STATISTICS"
.print "═══════════════════════════════════════════"

.mode line
SELECT
    t.symbol,
    ROUND(MIN(tr.price), 2) as min_price,
    ROUND(MAX(tr.price), 2) as max_price,
    ROUND(AVG(tr.price), 2) as avg_price,
    ROUND(MAX(tr.price) - MIN(tr.price), 2) as price_range,
    ROUND(100.0 * (MAX(tr.price) - MIN(tr.price)) / MIN(tr.price), 2) || '%' as range_pct
FROM trades tr
JOIN tickers t ON tr.ticker_id = t.ticker_id
GROUP BY t.symbol;

-- ============================================
-- 13. DEPTH SNAPSHOTS (if any)
-- ============================================
.print
.print "═══════════════════════════════════════════"
.print "DEPTH SNAPSHOTS"
.print "═══════════════════════════════════════════"

SELECT
    t.symbol,
    COUNT(*) as snapshot_count,
    to_timestamp(MIN(d.timestamp) / 1000) as first_snapshot,
    to_timestamp(MAX(d.timestamp) / 1000) as last_snapshot
FROM depth_snapshots d
JOIN tickers t ON d.ticker_id = t.ticker_id
GROUP BY t.symbol;

-- ============================================
-- 14. SCHEMA VERSION
-- ============================================
.print
.print "═══════════════════════════════════════════"
.print "SCHEMA VERSION"
.print "═══════════════════════════════════════════"

SELECT * FROM schema_version;

.print
.print "═══════════════════════════════════════════"
.print "END OF REPORT"
.print "═══════════════════════════════════════════"
