-- ============================================
-- Browse Database Rows
-- Run with: duckdb ~/.local/share/flowsurface/flowsurface.duckdb -readonly < browse_rows.sql
-- ============================================

.mode box

-- ============================================
-- EXCHANGES TABLE
-- ============================================
.print "════════════════════════════════════════════════════════════════"
.print "EXCHANGES TABLE (all rows)"
.print "════════════════════════════════════════════════════════════════"
SELECT * FROM exchanges ORDER BY exchange_id;

-- ============================================
-- TICKERS TABLE
-- ============================================
.print
.print "════════════════════════════════════════════════════════════════"
.print "TICKERS TABLE (all rows)"
.print "════════════════════════════════════════════════════════════════"
SELECT * FROM tickers ORDER BY ticker_id;

-- ============================================
-- TRADES TABLE - First 20 rows
-- ============================================
.print
.print "════════════════════════════════════════════════════════════════"
.print "TRADES TABLE (First 20 rows, chronologically)"
.print "════════════════════════════════════════════════════════════════"
SELECT * FROM trades ORDER BY timestamp ASC LIMIT 20;

-- ============================================
-- TRADES TABLE - Last 20 rows
-- ============================================
.print
.print "════════════════════════════════════════════════════════════════"
.print "TRADES TABLE (Last 20 rows, most recent)"
.print "════════════════════════════════════════════════════════════════"
SELECT * FROM trades ORDER BY timestamp DESC LIMIT 20;

-- ============================================
-- KLINES TABLE - 1m timeframe
-- ============================================
.print
.print "════════════════════════════════════════════════════════════════"
.print "KLINES TABLE (1m timeframe, last 10)"
.print "════════════════════════════════════════════════════════════════"
SELECT * FROM klines WHERE timeframe = '1m' ORDER BY candle_time DESC LIMIT 10;

-- ============================================
-- KLINES TABLE - 5m timeframe
-- ============================================
.print
.print "════════════════════════════════════════════════════════════════"
.print "KLINES TABLE (5m timeframe, last 10)"
.print "════════════════════════════════════════════════════════════════"
SELECT * FROM klines WHERE timeframe = '5m' ORDER BY candle_time DESC LIMIT 10;

-- ============================================
-- KLINES TABLE - 1h timeframe
-- ============================================
.print
.print "════════════════════════════════════════════════════════════════"
.print "KLINES TABLE (1h timeframe, last 10)"
.print "════════════════════════════════════════════════════════════════"
SELECT * FROM klines WHERE timeframe = '1h' ORDER BY candle_time DESC LIMIT 10;

-- ============================================
-- DEPTH SNAPSHOTS (if any)
-- ============================================
.print
.print "════════════════════════════════════════════════════════════════"
.print "DEPTH SNAPSHOTS TABLE"
.print "════════════════════════════════════════════════════════════════"
SELECT
    snapshot_id,
    ticker_id,
    timestamp,
    LENGTH(bids) as bids_json_length,
    LENGTH(asks) as asks_json_length,
    SUBSTR(bids, 1, 100) as bids_preview
FROM depth_snapshots
ORDER BY timestamp DESC
LIMIT 5;

-- ============================================
-- SCHEMA VERSION
-- ============================================
.print
.print "════════════════════════════════════════════════════════════════"
.print "SCHEMA VERSION"
.print "════════════════════════════════════════════════════════════════"
SELECT * FROM schema_version;

-- ============================================
-- TABLE SCHEMAS
-- ============================================
.print
.print "════════════════════════════════════════════════════════════════"
.print "TABLE SCHEMAS"
.print "════════════════════════════════════════════════════════════════"

.print
.print "--- TRADES TABLE SCHEMA ---"
DESCRIBE trades;

.print
.print "--- KLINES TABLE SCHEMA ---"
DESCRIBE klines;

.print
.print "--- TICKERS TABLE SCHEMA ---"
DESCRIBE tickers;
