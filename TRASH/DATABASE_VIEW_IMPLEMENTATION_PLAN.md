# Database Management View - Detailed Implementation Plan

## Overview

This plan implements a database management interface accessible via a new sidebar button. The implementation is broken into **3 phases** (MVP → Enhanced → Complete) with clear checkpoints.

---

## Phase 1: MVP - Read-Only Statistics View (4-6 hours)

Goal: Display database statistics in a modal without any write operations.

### Step 1.1: Add Database Icon (15 min)

**File**: `src/style.rs`

**Location**: Line 24-57 (Icon enum)

**Change**:
```rust
pub enum Icon {
    Locked,
    Unlocked,
    // ... existing icons ...
    Folder,
    ExternalLink,
    Database,  // ← ADD THIS
}
```

**Location**: Line 59-100 (Icon::from implementation)

**Change**:
```rust
impl From<Icon> for char {
    fn from(icon: Icon) -> Self {
        match icon {
            // ... existing mappings ...
            Icon::Folder => '\u{E81C}',
            Icon::ExternalLink => '\u{E81D}',
            Icon::Database => '\u{E81E}',  // ← ADD THIS (choose unused unicode)
        }
    }
}
```

**Testing**: Compile check - `cargo check`

---

### Step 1.2: Add Menu::Database Variant (15 min)

**File**: `data/src/config/sidebar.rs`

**Location**: Line 66-72

**Change**:
```rust
#[derive(Debug, Copy, Clone, PartialEq, Deserialize, Serialize)]
pub enum Menu {
    Layout,
    Settings,
    Audio,
    ThemeEditor,
    Database,  // ← ADD THIS
}
```

**Testing**: Compile check - `cargo check --package data`

---

### Step 1.3: Create DatabaseManager Component (2-3 hours)

**New File**: `src/modal/database_manager.rs`

**Full Implementation**:
```rust
use iced::{
    widget::{column, container, row, text, scrollable, Space},
    Alignment, Element, Length, Task,
};
use data::db::{DatabaseManager as DbManager, TradesCRUD, KlinesCRUD};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct DatabaseManager {
    db_manager: Option<Arc<DbManager>>,
    stats: Option<DatabaseStats>,
    loading: bool,
}

#[derive(Debug, Clone)]
pub struct DatabaseStats {
    pub total_trades: i64,
    pub total_klines: i64,
    pub total_tickers: i64,
    pub total_exchanges: i64,
    pub database_size_mb: f64,
    pub ticker_breakdown: Vec<TickerStats>,
}

#[derive(Debug, Clone)]
pub struct TickerStats {
    pub symbol: String,
    pub exchange: String,
    pub trade_count: i64,
    pub kline_count: i64,
    pub first_trade: Option<String>,
    pub last_trade: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Message {
    Refresh,
    StatsLoaded(Result<DatabaseStats, String>),
}

pub enum Action {
    // Future: actions that need to propagate up to main
}

impl DatabaseManager {
    pub fn new(db_manager: Option<Arc<DbManager>>) -> Self {
        Self {
            db_manager,
            stats: None,
            loading: false,
        }
    }

    pub fn update(&mut self, message: Message) -> (Task<Message>, Option<Action>) {
        match message {
            Message::Refresh => {
                if let Some(db_manager) = self.db_manager.clone() {
                    self.loading = true;

                    let task = Task::perform(
                        async move {
                            Self::fetch_stats(db_manager).await
                        },
                        Message::StatsLoaded,
                    );

                    return (task, None);
                }
            }
            Message::StatsLoaded(result) => {
                self.loading = false;

                match result {
                    Ok(stats) => {
                        self.stats = Some(stats);
                    }
                    Err(e) => {
                        log::error!("Failed to load database stats: {}", e);
                    }
                }
            }
        }

        (Task::none(), None)
    }

    async fn fetch_stats(db_manager: Arc<DbManager>) -> Result<DatabaseStats, String> {
        // This runs in background task - won't block UI

        // Query database statistics
        let stats_result = db_manager.with_conn(|conn| {
            // Total counts
            let total_trades: i64 = conn
                .query_row("SELECT COUNT(*) FROM trades", [], |row| row.get(0))
                .map_err(|e| format!("Query failed: {}", e))?;

            let total_klines: i64 = conn
                .query_row("SELECT COUNT(*) FROM klines", [], |row| row.get(0))
                .map_err(|e| format!("Query failed: {}", e))?;

            let total_tickers: i64 = conn
                .query_row("SELECT COUNT(*) FROM tickers", [], |row| row.get(0))
                .map_err(|e| format!("Query failed: {}", e))?;

            let total_exchanges: i64 = conn
                .query_row("SELECT COUNT(*) FROM exchanges", [], |row| row.get(0))
                .map_err(|e| format!("Query failed: {}", e))?;

            // Database size (in MB)
            let database_size_mb: f64 = conn
                .query_row(
                    "SELECT CAST(database_size AS BIGINT) / 1024.0 / 1024.0 FROM pragma_database_size()",
                    [],
                    |row| row.get(0)
                )
                .unwrap_or(0.0);

            // Per-ticker breakdown
            let mut ticker_stmt = conn
                .prepare(
                    "SELECT
                        t.symbol,
                        e.name as exchange,
                        COALESCE((SELECT COUNT(*) FROM trades tr WHERE tr.ticker_id = t.ticker_id), 0) as trade_count,
                        COALESCE((SELECT COUNT(*) FROM klines k WHERE k.ticker_id = t.ticker_id), 0) as kline_count,
                        (SELECT MIN(timestamp) FROM trades tr WHERE tr.ticker_id = t.ticker_id) as first_trade_ts,
                        (SELECT MAX(timestamp) FROM trades tr WHERE tr.ticker_id = t.ticker_id) as last_trade_ts
                     FROM tickers t
                     JOIN exchanges e ON t.exchange_id = e.exchange_id
                     ORDER BY trade_count DESC"
                )
                .map_err(|e| format!("Failed to prepare query: {}", e))?;

            let ticker_breakdown: Vec<TickerStats> = ticker_stmt
                .query_map([], |row| {
                    let first_trade_ts: Option<i64> = row.get(4)?;
                    let last_trade_ts: Option<i64> = row.get(5)?;

                    Ok(TickerStats {
                        symbol: row.get(0)?,
                        exchange: row.get(1)?,
                        trade_count: row.get(2)?,
                        kline_count: row.get(3)?,
                        first_trade: first_trade_ts.map(|ts| {
                            format!("{}", chrono::DateTime::from_timestamp_millis(ts).unwrap_or_default())
                        }),
                        last_trade: last_trade_ts.map(|ts| {
                            format!("{}", chrono::DateTime::from_timestamp_millis(ts).unwrap_or_default())
                        }),
                    })
                })
                .map_err(|e| format!("Query failed: {}", e))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("Failed to collect results: {}", e))?;

            Ok(DatabaseStats {
                total_trades,
                total_klines,
                total_tickers,
                total_exchanges,
                database_size_mb,
                ticker_breakdown,
            })
        });

        stats_result.map_err(|e: data::db::DatabaseError| e.to_string())
    }

    pub fn view(&self) -> Element<Message> {
        let title = text("Database Manager")
            .size(20)
            .width(Length::Fill);

        let refresh_button = iced::widget::button("Refresh")
            .on_press(Message::Refresh);

        let header = row![title, refresh_button]
            .spacing(10)
            .align_y(Alignment::Center);

        let content = if self.loading {
            column![
                text("Loading statistics...").size(16)
            ]
            .spacing(10)
            .padding(20)
        } else if let Some(stats) = &self.stats {
            self.stats_view(stats)
        } else {
            column![
                text("No database statistics available").size(16),
                text("Click Refresh to load statistics").size(14),
            ]
            .spacing(10)
            .padding(20)
        };

        let main_content = column![
            header,
            iced::widget::horizontal_rule(1),
            scrollable(content),
        ]
        .spacing(10)
        .padding(20)
        .width(Length::Fixed(600.0))
        .height(Length::Fixed(700.0));

        container(main_content)
            .style(|theme| {
                container::Style {
                    background: Some(theme.palette().background.into()),
                    border: iced::border::rounded(8),
                    ..container::Style::default()
                }
            })
            .into()
    }

    fn stats_view(&self, stats: &DatabaseStats) -> iced::widget::Column<Message> {
        // Overview section
        let overview = column![
            text("📊 Overview").size(18),
            row![
                text("Database Size:").width(Length::Fixed(150.0)),
                text(format!("{:.2} MB", stats.database_size_mb)),
            ].spacing(10),
            row![
                text("Total Trades:").width(Length::Fixed(150.0)),
                text(format!("{:,}", stats.total_trades)),
            ].spacing(10),
            row![
                text("Total Klines:").width(Length::Fixed(150.0)),
                text(format!("{:,}", stats.total_klines)),
            ].spacing(10),
            row![
                text("Tickers:").width(Length::Fixed(150.0)),
                text(format!("{}", stats.total_tickers)),
            ].spacing(10),
            row![
                text("Exchanges:").width(Length::Fixed(150.0)),
                text(format!("{}", stats.total_exchanges)),
            ].spacing(10),
        ]
        .spacing(8)
        .padding(15);

        // Per-ticker breakdown
        let mut ticker_rows = column![
            text("📁 Data by Ticker").size(18),
        ]
        .spacing(8)
        .padding(15);

        if stats.ticker_breakdown.is_empty() {
            ticker_rows = ticker_rows.push(text("No tickers found").size(14));
        } else {
            for ticker in &stats.ticker_breakdown {
                let ticker_row = column![
                    row![
                        text(format!("{} ({})", ticker.symbol, ticker.exchange))
                            .size(16)
                            .width(Length::Fill),
                    ],
                    row![
                        text(format!("  Trades: {:,}", ticker.trade_count))
                            .size(14)
                            .width(Length::Fixed(200.0)),
                        text(format!("Klines: {:,}", ticker.kline_count))
                            .size(14),
                    ].spacing(20),
                    if let (Some(first), Some(last)) = (&ticker.first_trade, &ticker.last_trade) {
                        row![
                            text(format!("  Range: {} to {}",
                                first.split('.').next().unwrap_or(first),
                                last.split('.').next().unwrap_or(last)
                            )).size(12)
                        ].into()
                    } else {
                        row![text("  No data").size(12)].into()
                    },
                ]
                .spacing(4);

                ticker_rows = ticker_rows.push(ticker_row);
                ticker_rows = ticker_rows.push(Space::with_height(10));
            }
        }

        column![
            overview,
            iced::widget::horizontal_rule(1),
            ticker_rows,
        ]
        .spacing(15)
    }
}
```

**Dependencies to add** to `Cargo.toml`:
```toml
[dependencies]
chrono = "0.4"  # For timestamp formatting
```

**File**: `src/modal.rs`

**Add at top**:
```rust
pub mod database_manager;
```

**Add to exports**:
```rust
pub use database_manager::DatabaseManager as DbManager;
```

**Testing**:
```bash
cargo check
# Should compile without errors
```

---

### Step 1.4: Add Database Button to Sidebar (30 min)

**File**: `src/screen/dashboard/sidebar.rs`

**Location**: Line 119-195 (nav_buttons function)

**Add new button before settings_modal_button**:
```rust
fn nav_buttons(
    &self,
    is_table_open: bool,
    audio_volume: Option<f32>,
    tooltip_position: TooltipPosition,
) -> iced::widget::Column<'_, Message> {
    // ... existing buttons ...

    let audio_btn = {
        // ... existing audio button code ...
    };

    // ← ADD THIS NEW BUTTON
    let database_button = {
        let is_active = self.is_menu_active(sidebar::Menu::Database);

        button_with_tooltip(
            icon_text(Icon::Database, 14)
                .width(24)
                .align_x(Alignment::Center),
            Message::ToggleSidebarMenu(Some(sidebar::Menu::Database)),
            Some("Database".into()),  // Tooltip text
            tooltip_position,
            move |theme, status| crate::style::button::transparent(theme, status, is_active),
        )
    };

    let settings_modal_button = {
        // ... existing settings button code ...
    };

    column![
        ticker_search_button,
        layout_modal_button,
        audio_btn,
        space::vertical(),
        database_button,        // ← ADD THIS LINE
        settings_modal_button,
    ]
    .width(32)
    .spacing(8)
}
```

**Testing**:
```bash
cargo check
# Button should appear in sidebar (no functionality yet)
```

---

### Step 1.5: Integrate DatabaseManager into Main (1 hour)

**File**: `src/main.rs`

**Step 1.5a: Add to imports** (around line 15):
```rust
use modal::{LayoutManager, ThemeEditor, audio, DbManager};
```

**Step 1.5b: Add field to Flowsurface struct** (around line 40):
```rust
pub struct Flowsurface {
    // ... existing fields ...
    audio_stream: audio::AudioStream,
    database_manager: DbManager,  // ← ADD THIS
}
```

**Step 1.5c: Add Message variant** (around line 85):
```rust
pub enum Message {
    // ... existing variants ...
    AudioStream(modal::audio::Message),
    DatabaseManager(modal::database_manager::Message),  // ← ADD THIS
}
```

**Step 1.5d: Initialize in new() function** (around line 120):
```rust
impl Flowsurface {
    fn new() -> (Self, Task<Message>) {
        // ... existing initialization ...

        let database_manager = DbManager::new(None);  // ← ADD THIS

        let flowsurface = Self {
            // ... existing fields ...
            audio_stream,
            database_manager,  // ← ADD THIS
        };

        (flowsurface, Task::batch(tasks))
    }
}
```

**Step 1.5e: Pass db_manager after initialization** (around line 140):

Find where `initialize_database_manager()` is called and update:
```rust
fn new() -> (Self, Task<Message>) {
    // ... existing code ...

    let flowsurface = Self {
        // ... fields ...
    };

    let db_init_task = if let Some(db_manager) = &flowsurface.db_manager {
        // Set db_manager in database_manager component
        // We'll need to add a setter method
        Task::none()
    } else {
        Task::none()
    };

    (flowsurface, Task::batch(tasks).chain(db_init_task))
}
```

Actually, better approach - update DatabaseManager::new call:
```rust
let database_manager = DbManager::new(db_manager.clone());
```

**Step 1.5f: Add update handler** (around line 400):
```rust
impl Flowsurface {
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            // ... existing cases ...

            // ← ADD THIS CASE
            Message::DatabaseManager(msg) => {
                let (task, action) = self.database_manager.update(msg);

                // Handle actions if any (for future features)
                if let Some(_action) = action {
                    // Future: handle database manager actions
                }

                return task.map(Message::DatabaseManager);
            }
        }
    }
}
```

**Step 1.5g: Add modal rendering** (around line 997, in view_with_modal function):
```rust
fn view_with_modal<'a>(
    &'a self,
    base: Element<'a, Message>,
    dashboard: &'a Dashboard,
    menu: sidebar::Menu,
) -> Element<'a, Message> {
    match menu {
        // ... existing cases ...

        // ← ADD THIS NEW CASE
        sidebar::Menu::Database => {
            let sidebar_pos = self.sidebar.state.position;

            let (align_x, padding) = match sidebar_pos {
                sidebar::Position::Left => (Alignment::Start, padding::left(44).top(76)),
                sidebar::Position::Right => (Alignment::End, padding::right(44).top(76)),
            };

            dashboard_modal(
                base,
                self.database_manager
                    .view()
                    .map(Message::DatabaseManager),
                Message::Sidebar(dashboard::sidebar::Message::ToggleSidebarMenu(None)),
                padding,
                Alignment::Start,
                align_x,
            )
        }
    }
}
```

**Testing**:
```bash
cargo build --release
# Run the app
export FLOWSURFACE_USE_DUCKDB=1
./target/release/flowsurface
```

**Expected behavior**:
1. Database button appears in sidebar
2. Clicking it opens database manager modal
3. Shows "No database statistics available"
4. Click "Refresh" to load stats
5. Should display database statistics
6. Charts continue updating in background

---

### Step 1.6: Add Setter Method to DatabaseManager (15 min)

**File**: `src/modal/database_manager.rs`

**Add method**:
```rust
impl DatabaseManager {
    // ... existing methods ...

    pub fn set_db_manager(&mut self, db_manager: Option<Arc<DbManager>>) {
        self.db_manager = db_manager;
    }
}
```

**File**: `src/main.rs`

**Update in initialize_database_manager or after dashboard creation**:

Find where db_manager is set on Dashboard, add similar for database_manager:
```rust
if let Some(ref db_manager) = self.db_manager {
    self.database_manager.set_db_manager(Some(db_manager.clone()));
}
```

---

### Phase 1 Checkpoint ✓

**What works**:
- ✅ Database button in sidebar
- ✅ Modal opens/closes
- ✅ Displays database statistics
- ✅ Shows trade/kline counts
- ✅ Shows per-ticker breakdown
- ✅ Charts continue working in background
- ✅ Refresh updates statistics

**Test it**:
```bash
# With database persistence enabled
export FLOWSURFACE_USE_DUCKDB=1
cargo run --release

# 1. Connect to an exchange
# 2. Add tickers (let some trades flow in)
# 3. Click database button (bottom of sidebar)
# 4. Click Refresh
# 5. Should see statistics
# 6. Charts should still update in background
```

---

## Phase 2: Enhanced - Cleanup Operations (4-6 hours)

Goal: Add database cleanup operations (delete old data, vacuum).

### Step 2.1: Add Cleanup Functions to Data Layer (1 hour)

**File**: `data/src/db/mod.rs`

**Add public methods**:
```rust
impl DatabaseManager {
    // ... existing methods ...

    /// Delete all trades for a specific ticker
    pub fn delete_ticker_trades(&self, ticker_id: i32) -> Result<usize> {
        self.with_conn(|conn| {
            let deleted = conn.execute(
                "DELETE FROM trades WHERE ticker_id = ?",
                [ticker_id],
            ).map_err(|e| DatabaseError::Query(format!("Failed to delete trades: {}", e)))?;

            Ok(deleted)
        })
    }

    /// Delete all klines for a specific ticker
    pub fn delete_ticker_klines(&self, ticker_id: i32) -> Result<usize> {
        self.with_conn(|conn| {
            let deleted = conn.execute(
                "DELETE FROM klines WHERE ticker_id = ?",
                [ticker_id],
            ).map_err(|e| DatabaseError::Query(format!("Failed to delete klines: {}", e)))?;

            Ok(deleted)
        })
    }

    /// Get all tickers with their IDs
    pub fn get_all_tickers(&self) -> Result<Vec<(i32, String, String)>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT t.ticker_id, t.symbol, e.name
                 FROM tickers t
                 JOIN exchanges e ON t.exchange_id = e.exchange_id
                 ORDER BY t.symbol, e.name"
            ).map_err(|e| DatabaseError::Query(format!("Failed to prepare query: {}", e)))?;

            let tickers = stmt.query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .map_err(|e| DatabaseError::Query(format!("Query failed: {}", e)))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| DatabaseError::Query(format!("Failed to collect: {}", e)))?;

            Ok(tickers)
        })
    }
}
```

---

### Step 2.2: Update DatabaseManager Component with Cleanup UI (2-3 hours)

**File**: `src/modal/database_manager.rs`

**Update struct**:
```rust
pub struct DatabaseManager {
    db_manager: Option<Arc<DbManager>>,
    stats: Option<DatabaseStats>,
    loading: bool,
    // ← ADD THESE FIELDS
    cleanup_trades_days: String,
    cleanup_klines_days: String,
    selected_ticker: Option<i32>,
    available_tickers: Vec<(i32, String, String)>,  // (id, symbol, exchange)
    vacuum_in_progress: bool,
    delete_in_progress: bool,
}
```

**Update Message enum**:
```rust
#[derive(Debug, Clone)]
pub enum Message {
    Refresh,
    StatsLoaded(Result<DatabaseStats, String>),
    // ← ADD THESE
    CleanupTradesDaysChanged(String),
    CleanupKlinesDaysChanged(String),
    DeleteOldTrades,
    DeleteOldKlines,
    DeleteTradesComplete(Result<usize, String>),
    DeleteKlinesComplete(Result<usize, String>),
    VacuumDatabase,
    VacuumComplete(Result<(), String>),
    TickerSelected(i32),
    DeleteTickerData,
    DeleteTickerComplete(Result<(usize, usize), String>),
    TickersLoaded(Vec<(i32, String, String)>),
}
```

**Update new() method**:
```rust
pub fn new(db_manager: Option<Arc<DbManager>>) -> Self {
    Self {
        db_manager,
        stats: None,
        loading: false,
        cleanup_trades_days: "7".to_string(),
        cleanup_klines_days: "30".to_string(),
        selected_ticker: None,
        available_tickers: Vec::new(),
        vacuum_in_progress: false,
        delete_in_progress: false,
    }
}
```

**Update update() method** (add new message handlers):
```rust
pub fn update(&mut self, message: Message) -> (Task<Message>, Option<Action>) {
    match message {
        Message::Refresh => {
            // ... existing refresh code ...

            // Also load available tickers
            if let Some(db_manager) = self.db_manager.clone() {
                let tickers_task = Task::perform(
                    async move {
                        db_manager.get_all_tickers()
                            .unwrap_or_default()
                    },
                    Message::TickersLoaded,
                );

                return (Task::batch([stats_task, tickers_task]), None);
            }
        }

        Message::TickersLoaded(tickers) => {
            self.available_tickers = tickers;
        }

        Message::CleanupTradesDaysChanged(value) => {
            self.cleanup_trades_days = value;
        }

        Message::CleanupKlinesDaysChanged(value) => {
            self.cleanup_klines_days = value;
        }

        Message::DeleteOldTrades => {
            if let (Some(db_manager), Ok(days)) = (
                self.db_manager.clone(),
                self.cleanup_trades_days.parse::<u32>()
            ) {
                self.delete_in_progress = true;

                let task = Task::perform(
                    async move {
                        use data::db::TradesCRUD;
                        let cutoff_time = chrono::Utc::now()
                            .timestamp_millis() as u64
                            - (days as u64 * 24 * 60 * 60 * 1000);

                        db_manager.with_conn(|conn| {
                            conn.execute(
                                "DELETE FROM trades WHERE timestamp < ?",
                                [cutoff_time as i64]
                            ).map_err(|e| format!("Delete failed: {}", e))
                        }).map_err(|e: data::db::DatabaseError| e.to_string())
                    },
                    Message::DeleteTradesComplete,
                );

                return (task, None);
            }
        }

        Message::DeleteTradesComplete(result) => {
            self.delete_in_progress = false;

            match result {
                Ok(count) => {
                    log::info!("Deleted {} old trades", count);
                    // Trigger refresh
                    return self.update(Message::Refresh);
                }
                Err(e) => {
                    log::error!("Failed to delete trades: {}", e);
                }
            }
        }

        Message::DeleteOldKlines => {
            // Similar to DeleteOldTrades
            if let (Some(db_manager), Ok(days)) = (
                self.db_manager.clone(),
                self.cleanup_klines_days.parse::<u32>()
            ) {
                self.delete_in_progress = true;

                let task = Task::perform(
                    async move {
                        let cutoff_time = chrono::Utc::now()
                            .timestamp_millis() as u64
                            - (days as u64 * 24 * 60 * 60 * 1000);

                        db_manager.with_conn(|conn| {
                            conn.execute(
                                "DELETE FROM klines WHERE candle_time < ?",
                                [cutoff_time as i64]
                            ).map_err(|e| format!("Delete failed: {}", e))
                        }).map_err(|e: data::db::DatabaseError| e.to_string())
                    },
                    Message::DeleteKlinesComplete,
                );

                return (task, None);
            }
        }

        Message::DeleteKlinesComplete(result) => {
            self.delete_in_progress = false;

            match result {
                Ok(count) => {
                    log::info!("Deleted {} old klines", count);
                    return self.update(Message::Refresh);
                }
                Err(e) => {
                    log::error!("Failed to delete klines: {}", e);
                }
            }
        }

        Message::VacuumDatabase => {
            if let Some(db_manager) = self.db_manager.clone() {
                self.vacuum_in_progress = true;

                let task = Task::perform(
                    async move {
                        db_manager.vacuum()
                            .map_err(|e| e.to_string())
                    },
                    Message::VacuumComplete,
                );

                return (task, None);
            }
        }

        Message::VacuumComplete(result) => {
            self.vacuum_in_progress = false;

            match result {
                Ok(_) => {
                    log::info!("Database vacuumed successfully");
                    return self.update(Message::Refresh);
                }
                Err(e) => {
                    log::error!("Vacuum failed: {}", e);
                }
            }
        }

        Message::TickerSelected(ticker_id) => {
            self.selected_ticker = Some(ticker_id);
        }

        Message::DeleteTickerData => {
            if let (Some(db_manager), Some(ticker_id)) = (
                self.db_manager.clone(),
                self.selected_ticker
            ) {
                self.delete_in_progress = true;

                let task = Task::perform(
                    async move {
                        let trades_deleted = db_manager.delete_ticker_trades(ticker_id)?;
                        let klines_deleted = db_manager.delete_ticker_klines(ticker_id)?;
                        Ok((trades_deleted, klines_deleted))
                    },
                    Message::DeleteTickerComplete,
                );

                return (task, None);
            }
        }

        Message::DeleteTickerComplete(result) => {
            self.delete_in_progress = false;

            match result {
                Ok((trades, klines)) => {
                    log::info!("Deleted {} trades and {} klines", trades, klines);
                    return self.update(Message::Refresh);
                }
                Err(e) => {
                    log::error!("Failed to delete ticker data: {}", e);
                }
            }
        }

        _ => {} // Existing cases
    }

    (Task::none(), None)
}
```

**Add cleanup UI to view()**:
```rust
fn view(&self) -> Element<Message> {
    // ... existing header code ...

    let content = if self.loading {
        // ... existing loading view ...
    } else if let Some(stats) = &self.stats {
        column![
            self.stats_view(stats),
            iced::widget::horizontal_rule(1),
            self.cleanup_view(),  // ← ADD THIS
        ].spacing(15)
    } else {
        // ... existing no-stats view ...
    };

    // ... rest of view code ...
}

fn cleanup_view(&self) -> iced::widget::Column<Message> {
    let cleanup_title = text("🗑️ Cleanup Operations").size(18);

    // Delete old trades
    let delete_trades_row = row![
        text("Delete trades older than:").width(Length::Fixed(200.0)),
        iced::widget::text_input("7", &self.cleanup_trades_days)
            .on_input(Message::CleanupTradesDaysChanged)
            .width(Length::Fixed(60.0)),
        text("days"),
        iced::widget::button(
            if self.delete_in_progress {
                text("Deleting...")
            } else {
                text("Delete")
            }
        )
        .on_press_maybe(
            if !self.delete_in_progress {
                Some(Message::DeleteOldTrades)
            } else {
                None
            }
        ),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    // Delete old klines
    let delete_klines_row = row![
        text("Delete klines older than:").width(Length::Fixed(200.0)),
        iced::widget::text_input("30", &self.cleanup_klines_days)
            .on_input(Message::CleanupKlinesDaysChanged)
            .width(Length::Fixed(60.0)),
        text("days"),
        iced::widget::button(
            if self.delete_in_progress {
                text("Deleting...")
            } else {
                text("Delete")
            }
        )
        .on_press_maybe(
            if !self.delete_in_progress {
                Some(Message::DeleteOldKlines)
            } else {
                None
            }
        ),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    // Vacuum
    let vacuum_row = row![
        text("Reclaim disk space (VACUUM):").width(Length::Fixed(200.0)),
        iced::widget::button(
            if self.vacuum_in_progress {
                text("Running...")
            } else {
                text("Run Vacuum")
            }
        )
        .on_press_maybe(
            if !self.vacuum_in_progress {
                Some(Message::VacuumDatabase)
            } else {
                None
            }
        ),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    // Delete ticker data
    let ticker_picker = iced::widget::pick_list(
        self.available_tickers
            .iter()
            .map(|(id, symbol, exchange)| (*id, format!("{} ({})", symbol, exchange)))
            .collect::<Vec<_>>(),
        self.selected_ticker.and_then(|id| {
            self.available_tickers
                .iter()
                .find(|(tid, _, _)| *tid == id)
                .map(|(id, symbol, exchange)| (*id, format!("{} ({})", symbol, exchange)))
        }),
        |(_id, _label)| Message::TickerSelected(_id),
    )
    .placeholder("Select ticker...")
    .width(Length::Fixed(200.0));

    let delete_ticker_row = row![
        text("Delete all data for ticker:").width(Length::Fixed(200.0)),
        ticker_picker,
        iced::widget::button(
            if self.delete_in_progress {
                text("Deleting...")
            } else {
                text("Delete All")
            }
        )
        .on_press_maybe(
            if !self.delete_in_progress && self.selected_ticker.is_some() {
                Some(Message::DeleteTickerData)
            } else {
                None
            }
        ),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    column![
        cleanup_title,
        delete_trades_row,
        delete_klines_row,
        delete_ticker_row,
        vacuum_row,
    ]
    .spacing(12)
    .padding(15)
}
```

---

### Phase 2 Checkpoint ✓

**What works**:
- ✅ All Phase 1 features
- ✅ Delete trades older than N days
- ✅ Delete klines older than N days
- ✅ Delete all data for specific ticker
- ✅ VACUUM database to reclaim space
- ✅ Progress indicators during operations
- ✅ Automatic refresh after cleanup
- ✅ Charts continue working during cleanup

**Test it**:
```bash
export FLOWSURFACE_USE_DUCKDB=1
cargo run --release

# 1. Open database manager
# 2. Set "Delete trades older than: 0 days"
# 3. Click Delete - should clear all trades
# 4. Verify with Refresh
# 5. Run Vacuum
# 6. Check database size decreased
```

---

## Phase 3: Complete - Backup & Export (6-8 hours)

Goal: Add backup/restore and CSV/Parquet export functionality.

### Step 3.1: Add Backup Functions to Data Layer (2-3 hours)

**File**: `data/src/db/backup.rs` (NEW FILE)

```rust
use std::path::{Path, PathBuf};
use std::fs;
use chrono::Utc;
use super::error::{DatabaseError, Result};

pub struct BackupManager {
    db_path: PathBuf,
    backup_dir: PathBuf,
}

impl BackupManager {
    pub fn new(db_path: PathBuf) -> Self {
        let backup_dir = db_path
            .parent()
            .unwrap_or(Path::new("."))
            .join("backups");

        Self {
            db_path,
            backup_dir,
        }
    }

    /// Create a backup of the database
    pub fn create_backup(&self) -> Result<PathBuf> {
        // Ensure backup directory exists
        fs::create_dir_all(&self.backup_dir)
            .map_err(|e| DatabaseError::Io(e))?;

        // Generate backup filename with timestamp
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let backup_name = format!("flowsurface_backup_{}.duckdb", timestamp);
        let backup_path = self.backup_dir.join(&backup_name);

        // Copy database file
        fs::copy(&self.db_path, &backup_path)
            .map_err(|e| DatabaseError::Io(e))?;

        Ok(backup_path)
    }

    /// List all available backups
    pub fn list_backups(&self) -> Result<Vec<BackupInfo>> {
        if !self.backup_dir.exists() {
            return Ok(Vec::new());
        }

        let mut backups = Vec::new();

        for entry in fs::read_dir(&self.backup_dir)
            .map_err(|e| DatabaseError::Io(e))?
        {
            let entry = entry.map_err(|e| DatabaseError::Io(e))?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("duckdb") {
                let metadata = fs::metadata(&path)
                    .map_err(|e| DatabaseError::Io(e))?;

                backups.push(BackupInfo {
                    path: path.clone(),
                    name: path.file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string(),
                    size_bytes: metadata.len(),
                    created: metadata.modified()
                        .map_err(|e| DatabaseError::Io(e))?,
                });
            }
        }

        // Sort by creation time (newest first)
        backups.sort_by(|a, b| b.created.cmp(&a.created));

        Ok(backups)
    }

    /// Restore from a backup
    pub fn restore_backup(&self, backup_path: &Path) -> Result<()> {
        if !backup_path.exists() {
            return Err(DatabaseError::Configuration(
                "Backup file does not exist".to_string()
            ));
        }

        // Create a safety backup of current database before restoring
        let safety_backup = self.db_path
            .parent()
            .unwrap_or(Path::new("."))
            .join(format!(
                "flowsurface_pre_restore_{}.duckdb",
                Utc::now().format("%Y%m%d_%H%M%S")
            ));

        fs::copy(&self.db_path, &safety_backup)
            .map_err(|e| DatabaseError::Io(e))?;

        // Restore from backup
        fs::copy(backup_path, &self.db_path)
            .map_err(|e| DatabaseError::Io(e))?;

        Ok(())
    }

    /// Delete a backup
    pub fn delete_backup(&self, backup_path: &Path) -> Result<()> {
        fs::remove_file(backup_path)
            .map_err(|e| DatabaseError::Io(e))
    }
}

#[derive(Debug, Clone)]
pub struct BackupInfo {
    pub path: PathBuf,
    pub name: String,
    pub size_bytes: u64,
    pub created: std::time::SystemTime,
}
```

**File**: `data/src/db/mod.rs`

Add:
```rust
pub mod backup;
pub use backup::{BackupManager, BackupInfo};
```

Add methods to DatabaseManager:
```rust
impl DatabaseManager {
    // ... existing methods ...

    pub fn get_backup_manager(&self) -> BackupManager {
        let db_path = self.with_conn(|conn| {
            conn.path()
                .map(|p| PathBuf::from(p))
                .ok_or(DatabaseError::Configuration("No database path".into()))
        }).unwrap_or_else(|_| PathBuf::from("flowsurface.duckdb"));

        BackupManager::new(db_path)
    }
}
```

---

### Step 3.2: Add Export Functions (2-3 hours)

**File**: `data/src/db/export.rs` (NEW FILE)

```rust
use super::error::{DatabaseError, Result};
use super::DatabaseManager;
use std::path::Path;

impl DatabaseManager {
    /// Export trades to CSV
    pub fn export_trades_csv(&self, output_path: &Path, ticker_id: Option<i32>) -> Result<usize> {
        self.with_conn(|conn| {
            let query = if let Some(tid) = ticker_id {
                format!(
                    "COPY (
                        SELECT t.symbol, e.name as exchange, tr.timestamp, tr.price, tr.quantity, tr.is_buyer_maker
                        FROM trades tr
                        JOIN tickers t ON tr.ticker_id = t.ticker_id
                        JOIN exchanges e ON t.exchange_id = e.exchange_id
                        WHERE tr.ticker_id = {}
                        ORDER BY tr.timestamp
                    ) TO '{}' (HEADER, DELIMITER ',')",
                    tid,
                    output_path.display()
                )
            } else {
                format!(
                    "COPY (
                        SELECT t.symbol, e.name as exchange, tr.timestamp, tr.price, tr.quantity, tr.is_buyer_maker
                        FROM trades tr
                        JOIN tickers t ON tr.ticker_id = t.ticker_id
                        JOIN exchanges e ON t.exchange_id = e.exchange_id
                        ORDER BY tr.timestamp
                    ) TO '{}' (HEADER, DELIMITER ',')",
                    output_path.display()
                )
            };

            let count = conn.execute(&query, [])
                .map_err(|e| DatabaseError::Query(format!("Export failed: {}", e)))?;

            Ok(count)
        })
    }

    /// Export klines to CSV
    pub fn export_klines_csv(&self, output_path: &Path, ticker_id: Option<i32>) -> Result<usize> {
        self.with_conn(|conn| {
            let query = if let Some(tid) = ticker_id {
                format!(
                    "COPY (
                        SELECT t.symbol, e.name as exchange, k.timeframe, k.candle_time,
                               k.open_price, k.high_price, k.low_price, k.close_price, k.volume
                        FROM klines k
                        JOIN tickers t ON k.ticker_id = t.ticker_id
                        JOIN exchanges e ON t.exchange_id = e.exchange_id
                        WHERE k.ticker_id = {}
                        ORDER BY k.candle_time
                    ) TO '{}' (HEADER, DELIMITER ',')",
                    tid,
                    output_path.display()
                )
            } else {
                format!(
                    "COPY (
                        SELECT t.symbol, e.name as exchange, k.timeframe, k.candle_time,
                               k.open_price, k.high_price, k.low_price, k.close_price, k.volume
                        FROM klines k
                        JOIN tickers t ON k.ticker_id = t.ticker_id
                        JOIN exchanges e ON t.exchange_id = e.exchange_id
                        ORDER BY k.candle_time
                    ) TO '{}' (HEADER, DELIMITER ',')",
                    output_path.display()
                )
            };

            let count = conn.execute(&query, [])
                .map_err(|e| DatabaseError::Query(format!("Export failed: {}", e)))?;

            Ok(count)
        })
    }

    /// Export to Parquet
    pub fn export_trades_parquet(&self, output_path: &Path, ticker_id: Option<i32>) -> Result<usize> {
        self.with_conn(|conn| {
            let query = if let Some(tid) = ticker_id {
                format!(
                    "COPY (
                        SELECT * FROM trades WHERE ticker_id = {}
                    ) TO '{}' (FORMAT PARQUET)",
                    tid,
                    output_path.display()
                )
            } else {
                format!(
                    "COPY trades TO '{}' (FORMAT PARQUET)",
                    output_path.display()
                )
            };

            let count = conn.execute(&query, [])
                .map_err(|e| DatabaseError::Query(format!("Export failed: {}", e)))?;

            Ok(count)
        })
    }

    pub fn export_klines_parquet(&self, output_path: &Path, ticker_id: Option<i32>) -> Result<usize> {
        self.with_conn(|conn| {
            let query = if let Some(tid) = ticker_id {
                format!(
                    "COPY (
                        SELECT * FROM klines WHERE ticker_id = {}
                    ) TO '{}' (FORMAT PARQUET)",
                    tid,
                    output_path.display()
                )
            } else {
                format!(
                    "COPY klines TO '{}' (FORMAT PARQUET)",
                    output_path.display()
                )
            };

            let count = conn.execute(&query, [])
                .map_err(|e| DatabaseError::Query(format!("Export failed: {}", e)))?;

            Ok(count)
        })
    }
}
```

---

### Step 3.3: Add Backup & Export UI (2-3 hours)

**File**: `src/modal/database_manager.rs`

**Update struct**:
```rust
pub struct DatabaseManager {
    // ... existing fields ...
    backups: Vec<BackupInfo>,
    selected_backup: Option<usize>,
    backup_in_progress: bool,
    restore_in_progress: bool,
    export_in_progress: bool,
    export_format: ExportFormat,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExportFormat {
    CsvTrades,
    CsvKlines,
    ParquetTrades,
    ParquetKlines,
}
```

**Add messages**:
```rust
#[derive(Debug, Clone)]
pub enum Message {
    // ... existing messages ...
    LoadBackups,
    BackupsLoaded(Vec<BackupInfo>),
    CreateBackup,
    BackupCreated(Result<String, String>),
    SelectBackup(usize),
    RestoreBackup,
    RestoreComplete(Result<(), String>),
    DeleteBackup(usize),
    DeleteBackupComplete(Result<(), String>),
    ExportFormatChanged(ExportFormat),
    ExportData,
    ExportComplete(Result<usize, String>),
}
```

**Add handlers and view** (similar pattern to cleanup operations)

---

### Phase 3 Checkpoint ✓

**What works**:
- ✅ All Phase 1 & 2 features
- ✅ Create database backups
- ✅ List available backups
- ✅ Restore from backup
- ✅ Delete backups
- ✅ Export trades to CSV
- ✅ Export klines to CSV
- ✅ Export to Parquet format
- ✅ Export all data or specific ticker

---

## Testing Checklist

### Unit Tests
- [ ] Database button renders in sidebar
- [ ] Modal opens/closes correctly
- [ ] Statistics load correctly
- [ ] Cleanup operations work
- [ ] Backup/restore functions work
- [ ] Export functions work

### Integration Tests
- [ ] Charts continue updating with modal open
- [ ] WebSocket data keeps flowing
- [ ] Dual-write continues during database operations
- [ ] Long operations don't freeze UI
- [ ] Error states display correctly
- [ ] Modal can be closed during operations

### Manual Tests
- [ ] Open database manager
- [ ] Verify statistics display
- [ ] Delete old trades
- [ ] Vacuum database
- [ ] Create backup
- [ ] Restore backup
- [ ] Export to CSV
- [ ] Charts work while modal is open

---

## File Summary

### New Files:
1. `src/modal/database_manager.rs` - Main UI component
2. `data/src/db/backup.rs` - Backup/restore logic
3. `data/src/db/export.rs` - Export functionality

### Modified Files:
1. `src/style.rs` - Add Database icon
2. `data/src/config/sidebar.rs` - Add Menu::Database
3. `src/screen/dashboard/sidebar.rs` - Add database button
4. `src/main.rs` - Integrate DatabaseManager
5. `src/modal.rs` - Export DatabaseManager
6. `data/src/db/mod.rs` - Add new modules and methods
7. `Cargo.toml` - Add chrono dependency

---

## Deployment Plan

### For MVP (Phase 1):
```bash
# Merge Phase 1
git checkout -b feature/database-view-mvp
# Implement steps 1.1-1.6
git commit -m "Add database statistics view"
git push
# Create PR, review, merge
```

### For Enhanced (Phase 2):
```bash
git checkout -b feature/database-view-cleanup
# Implement Phase 2
git commit -m "Add database cleanup operations"
git push
```

### For Complete (Phase 3):
```bash
git checkout -b feature/database-view-complete
# Implement Phase 3
git commit -m "Add backup/restore and export features"
git push
```

---

## Estimated Timeline

| Phase | Description | Time | Cumulative |
|-------|-------------|------|------------|
| 1 | MVP - Read-only stats | 4-6 hours | 4-6 hours |
| 2 | Cleanup operations | 4-6 hours | 8-12 hours |
| 3 | Backup & Export | 6-8 hours | 14-20 hours |

**Total: 14-20 hours** for complete implementation with all features.

---

## Next Steps

Ready to proceed? I can:
1. Start with Phase 1 (MVP) implementation
2. Implement all phases at once
3. Create skeleton code for all phases
4. Focus on specific features first

Which would you prefer?
