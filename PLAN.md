# Historical Data Acquisition Plan

## Overview

**Goal:** Build a data acquisition system for backtesting and analysis with focus on pragmatic implementation.

**Strategy:**
1. **Binance:** Full historical data download via bulk files (easiest, most complete)
2. **Other Exchanges:** Focus on saving real-time data going forward (build historical dataset over time)

---

## Phase 1: Binance Historical Data Downloader

### Why Binance First?

- ✅ **Best bulk download infrastructure** (Binance Vision - `data.binance.vision`)
- ✅ **FREE, no rate limits** for bulk files
- ✅ **Complete historical data** back to 2017-2018 for many symbols
- ✅ **Well-organized** daily/monthly CSV files
- ✅ **All data types available:** trades, klines, funding rates, open interest, mark price, premium index
- ✅ **Easy to implement** - just HTTP downloads + CSV parsing

### Implementation Steps

#### 1.1 Core Download Engine

**Location:** `data/src/download/` (new module)

**Components:**
```rust
// data/src/download/mod.rs
pub mod binance;
pub mod job;
pub mod queue;

pub struct DownloadJob {
    pub exchange: Exchange,
    pub symbol: String,
    pub data_type: DataType,  // Trades, Klines, FundingRate, etc.
    pub date_range: (NaiveDate, NaiveDate),
    pub status: JobStatus,
    pub progress: f32,
    pub records_downloaded: u64,
    pub bytes_downloaded: u64,
    pub speed: f64,  // MB/sec or records/sec
    pub error: Option<String>,
}

pub enum DataType {
    Trades,
    AggTrades,
    Klines(Interval),  // 1m, 5m, 1h, etc.
    FundingRate,
    MarkPrice,
    PremiumIndex,
    OpenInterest,
}

pub enum JobStatus {
    Queued,
    Downloading,
    Paused,
    Completed,
    Failed,
}
```

**File URL Pattern:**
```
https://data.binance.vision/data/{market}/{frequency}/{dataType}/{symbol}/{filename}

Examples:
- Spot trades (daily):
  https://data.binance.vision/data/spot/daily/trades/BTCUSDT/BTCUSDT-trades-2024-10-16.zip

- Futures klines (monthly):
  https://data.binance.vision/data/futures/um/monthly/klines/BTCUSDT/1h/BTCUSDT-1h-2024-10.zip

- Funding rate:
  https://data.binance.vision/data/futures/um/daily/fundingRate/BTCUSDT/BTCUSDT-fundingRate-2024-10-16.zip
```

#### 1.2 Download Process

1. **Generate file URLs** for date range
2. **Download ZIP files** via reqwest
3. **Verify checksum** (optional, SHA256 files available)
4. **Extract CSV** from ZIP in memory
5. **Parse CSV** rows
6. **Batch insert** into DuckDB (optimal batch size ~10,000 rows)
7. **Update progress** in UI

**Parallelization:**
- Download multiple files concurrently (5-10 concurrent downloads)
- No rate limits, so can be aggressive
- Limited by network bandwidth and disk I/O

#### 1.3 Database Integration

**Use existing DuckDB schema:**
- `trades` table (already exists)
- `klines` table (already exists)
- Need to add: `funding_rates`, `mark_prices`, `open_interest` tables

**Batch Insert Strategy:**
```rust
// Collect rows in memory
let mut batch = Vec::new();
for csv_row in csv_reader.records() {
    batch.push(parse_trade(csv_row?));

    if batch.len() >= 10_000 {
        db_manager.insert_trades_batch(&batch)?;
        batch.clear();
        // Update progress
    }
}
// Insert remaining
if !batch.is_empty() {
    db_manager.insert_trades_batch(&batch)?;
}
```

#### 1.4 UI Components (Database View)

**Add to existing `src/modal/database_manager.rs`:**

**New tabs/sections:**
1. **Download Manager** (new view)
2. **Database Stats** (existing, already implemented)

**Download Manager UI:**

```
┌─────────────────────────────────────────────────────┐
│ Database Manager                          [Refresh] │
├─────────────────────────────────────────────────────┤
│ [Statistics] [Download Manager] ◄─ Tabs             │
├─────────────────────────────────────────────────────┤
│                                                      │
│  ┌── New Download Job ──────────────────────┐      │
│  │ Exchange:    [Binance ▼]                 │      │
│  │ Symbol:      [BTCUSDT  ]  [Browse...]    │      │
│  │ Data Type:   ☑ Trades  ☑ Klines         │      │
│  │              ☐ Funding Rates             │      │
│  │              ☐ Open Interest             │      │
│  │ Date Range:  [2024-01-01] to [2024-12-31]│     │
│  │ Interval:    [1h ▼] (for klines)        │      │
│  │                                          │      │
│  │              [Add to Queue]              │      │
│  └──────────────────────────────────────────┘      │
│                                                      │
│  ┌── Download Queue ────────────────────────┐      │
│  │                                           │      │
│  │ BTCUSDT - Trades (2024-01-01 to 2024-12-31)    │
│  │ Status: Downloading...                    │      │
│  │ Progress: [████████░░] 78% (285/365 days)│      │
│  │ Speed: 125 MB/s | ETA: 2m 15s            │      │
│  │ Downloaded: 45.2M trades (18.5 GB)       │      │
│  │         [Pause] [Cancel]                  │      │
│  │                                           │      │
│  │ ETHUSDT - Klines 1h (2024-01-01 to ...)  │      │
│  │ Status: Queued                            │      │
│  │         [▲] [▼] [Remove]                  │      │
│  │                                           │      │
│  │ BTCUSDT - Funding Rate (2024-01-01 ...)  │      │
│  │ Status: Completed ✓                       │      │
│  │ Downloaded: 732 records                   │      │
│  │                                           │      │
│  └───────────────────────────────────────────┘      │
│                                                      │
│  Overall: 2 active, 1 queued, 3 completed          │
│  Total Speed: 125 MB/s                              │
│                                                      │
└─────────────────────────────────────────────────────┘
```

**Message Types:**
```rust
pub enum DownloadMessage {
    AddJob(DownloadJobConfig),
    StartJob(JobId),
    PauseJob(JobId),
    CancelJob(JobId),
    RemoveJob(JobId),
    MoveJobUp(JobId),
    MoveJobDown(JobId),
    JobProgress(JobId, ProgressUpdate),
    JobCompleted(JobId, Stats),
    JobFailed(JobId, String),
}

pub struct ProgressUpdate {
    pub progress: f32,
    pub speed: f64,
    pub records: u64,
    pub bytes: u64,
    pub eta: Option<Duration>,
}
```

#### 1.5 State Management

**Add to `src/main.rs`:**
```rust
struct Flowsurface {
    // ... existing fields ...
    download_manager: DownloadManager,
}
```

**Download Manager:**
```rust
pub struct DownloadManager {
    jobs: Vec<DownloadJob>,
    active_downloads: HashMap<JobId, JoinHandle<()>>,
    max_concurrent: usize,  // e.g., 5
}
```

---

## Phase 2: Real-Time Data Persistence (All Exchanges)

### Current Status

**Already Implemented ✅:**
- Trades → DuckDB (via dual-write in `exchange::fetcher`)
- Klines → DuckDB (via dual-write)

**Verification Needed:**
- Ensure ALL exchanges are properly saving data
- Check if any data types are missed

### Implementation Checklist

#### 2.1 Audit Current Data Flow

**For each exchange (Binance, Bybit, Hyperliquid, Aster):**

1. ✅ **Trades** - Verify being saved via `persist_trades()`
2. ✅ **Klines** - Verify being saved via `persist_klines()`
3. ❓ **Depth Snapshots** - Check if needed for analysis
4. ❓ **Funding Rates** - Check if available and being saved
5. ❓ **Open Interest** - Check if available and being saved

#### 2.2 Add Missing Data Types

**If funding rates/open interest are available via WebSocket:**

Add new tables:
```sql
CREATE TABLE funding_rates (
    funding_rate_id INTEGER PRIMARY KEY,
    ticker_id INTEGER REFERENCES tickers(ticker_id),
    timestamp BIGINT NOT NULL,
    funding_rate DOUBLE NOT NULL,
    mark_price DOUBLE,
    INDEX idx_funding_ticker_time (ticker_id, timestamp)
);

CREATE TABLE open_interest (
    oi_id INTEGER PRIMARY KEY,
    ticker_id INTEGER REFERENCES tickers(ticker_id),
    timestamp BIGINT NOT NULL,
    open_interest DOUBLE NOT NULL,
    INDEX idx_oi_ticker_time (ticker_id, timestamp)
);
```

Add persistence methods:
```rust
// data/src/db/crud/funding.rs
pub fn insert_funding_rate(
    conn: &Connection,
    ticker_id: i32,
    timestamp: u64,
    funding_rate: f64,
    mark_price: Option<f64>,
) -> Result<(), DatabaseError>

// data/src/db/crud/open_interest.rs
pub fn insert_open_interest(
    conn: &Connection,
    ticker_id: i32,
    timestamp: u64,
    open_interest: f64,
) -> Result<(), DatabaseError>
```

#### 2.3 WebSocket Message Handling

**Update adapters to extract and persist additional data:**

Example for Binance:
```rust
// In binance adapter WebSocket handler
match channel {
    "trade" => { /* existing */ },
    "kline" => { /* existing */ },
    "markPrice" => {
        // Extract mark price, funding rate
        // Call persist_funding_rate()
    }
    // ... other channels
}
```

---

## Phase 3: Data Validation & Quality

### 3.1 Gap Detection

**Add to Database Manager UI:**

```
┌── Data Quality Report ──────────────────┐
│                                          │
│ BTCUSDT (Binance - Linear)              │
│ ✓ Trades: 45.2M records                 │
│   Range: 2024-01-01 to 2024-12-31       │
│   No gaps detected                       │
│                                          │
│ ⚠ Klines (1h): 8,544 candles            │
│   Range: 2024-01-01 to 2024-12-31       │
│   Gap: 2024-03-15 14:00 - 16:00 (2h)   │
│                                          │
│ ✗ Funding Rate: Missing                 │
│   No data available                      │
│                                          │
└──────────────────────────────────────────┘
```

**Implementation:**
```rust
pub fn check_gaps(
    conn: &Connection,
    ticker_id: i32,
    data_type: DataType,
    expected_interval: Duration,
) -> Vec<Gap>
```

### 3.2 Duplicate Detection

- Already handled via deterministic IDs + `ON CONFLICT DO NOTHING`
- Monitor duplicate rate in stats

---

## Phase 4: Performance Optimization

### 4.1 Batch Size Tuning

Test optimal batch sizes for DuckDB inserts:
- Trades: 10,000 - 50,000 per batch
- Klines: 1,000 - 5,000 per batch

### 4.2 Parallel Downloads

- Binance: Up to 10 concurrent (no rate limit)
- Monitor network bandwidth utilization

### 4.3 Compression

- Keep downloaded ZIPs temporarily for resume capability
- Delete after successful insertion
- Option to keep ZIPs for archival

---

## Implementation Order

### Week 1: Core Infrastructure
1. ✅ Create download job structs
2. ✅ Implement Binance file URL generator
3. ✅ Basic HTTP downloader
4. ✅ CSV parser for trades
5. ✅ Batch insertion into DuckDB

### Week 2: UI & Queue Management
1. ✅ Add Download Manager tab to database view
2. ✅ Job queue display
3. ✅ Progress tracking
4. ✅ Pause/resume/cancel functionality
5. ✅ Error handling and retry logic

### Week 3: Additional Data Types
1. ✅ Klines download
2. ✅ Funding rates download
3. ✅ Open interest download
4. ✅ Multi-symbol support

### Week 4: Polish & Validation
1. ✅ Gap detection
2. ✅ Data quality reports
3. ✅ Resume failed downloads
4. ✅ Testing with real data

---

## Technical Specifications

### Download Performance Targets

- **Download Speed:** 100+ MB/sec (network limited)
- **Parsing Speed:** 500K+ trades/sec
- **Insert Speed:** 100K+ trades/sec (batched)
- **Concurrent Jobs:** 5-10 files simultaneously

### Storage Estimates

**1 year of data for BTCUSDT:**
- Trades: ~18 GB (500K trades/day × 365 days × 100 bytes)
- Klines (all intervals): ~1 GB
- Funding rates: ~100 MB
- **Total per symbol:** ~20 GB/year

**10 major symbols × 1 year:** ~200 GB

### Dependencies

**New Rust crates needed:**
- `zip` - ZIP file extraction
- `csv` - CSV parsing (likely already have)
- `reqwest` - HTTP downloads (already have)
- `tokio::fs` - Async file operations (already have)

---

## Success Criteria

### Phase 1 Complete When:
- ✅ Can download Binance spot trades for any symbol/date range
- ✅ Data correctly inserted into DuckDB
- ✅ UI shows download progress
- ✅ Can pause/resume/cancel downloads
- ✅ Queue multiple downloads

### Phase 2 Complete When:
- ✅ All real-time WebSocket data from all exchanges is being persisted
- ✅ Verified data flow for trades, klines, funding rates (if available)
- ✅ Database statistics show data accumulating

### Phase 3 Complete When:
- ✅ Gap detection working
- ✅ Data quality reports visible in UI
- ✅ Can identify missing data

---

## Future Enhancements (Post-MVP)

### Export Functionality
- Export date range to CSV/Parquet
- Useful for external analysis

### Scheduled Downloads
- Auto-download yesterday's data every day
- Keep database up-to-date with bulk files

### Other Exchanges Bulk Download
- Bybit: `public.bybit.com` (similar to Binance)
- Hyperliquid: S3 bucket access for trade fills
- Aster: API-based (more complex)

### Advanced Features
- Data compression in DuckDB
- Archival to cold storage
- Data retention policies
- Automated backup

---

## Notes

### Why Not Implement Hyperliquid/Bybit/Aster Historical First?

**Hyperliquid:**
- S3 bucket requires AWS credentials setup
- Trade fills format may differ from API
- Klines limited to 5000 most recent
- More complex than Binance

**Bybit:**
- Similar structure to Binance (could add later easily)
- Focus on one first to validate architecture

**Aster:**
- Smaller DEX, less data available
- API-only (no bulk files)
- Lower priority

### Design Decisions

**Single Exchange First:**
- Validate architecture with simplest case (Binance)
- Learn optimal batch sizes, error handling patterns
- Easy to extend to other exchanges later

**Real-time Focus for Others:**
- Start building historical dataset NOW
- By the time you need other exchanges' historical data, you'll have months of data already
- Can add bulk downloads later if needed

**UI in Database View:**
- Natural fit (database management)
- Modal overlay keeps charts working in background
- Consistent with existing database stats view

---

## Risk Mitigation

### Potential Issues:

1. **Network failures during download**
   - Solution: Resume capability, checksum verification

2. **Disk space exhaustion**
   - Solution: Estimate before download, show warning

3. **CSV format changes**
   - Solution: Versioned parsers, graceful error handling

4. **Database write bottleneck**
   - Solution: Batch inserts, benchmark and optimize

5. **UI freezing during operations**
   - Solution: All operations in async tasks, progress updates via channels

---

**Last Updated:** 2025-10-17
**Author:** Claude (Sonnet 4.5)
**Status:** Ready for Implementation
