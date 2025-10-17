# FlowSurface: Comprehensive Project Documentation

## Table of Contents

1. [Project Overview](#project-overview)
2. [Architecture](#architecture)
3. [Core Domain](#core-domain)
4. [UI Architecture](#ui-architecture)
5. [Real-Time Data Flow](#real-time-data-flow)
6. [Chart System](#chart-system)
7. [Rendering & Performance](#rendering--performance)
8. [Build & Deployment](#build--deployment)
9. [Rust Patterns](#rust-patterns)
10. [Limitations & Technical Debt](#limitations--technical-debt)

---

## Project Overview

**FlowSurface** is an experimental open-source desktop charting application for cryptocurrency trading data visualization, built with Rust using the iced GUI framework.

### Key Statistics
- **~79 Rust files**
- **~20,802 lines of code**
- **3 crates**: main, exchange, data
- **4 exchanges supported**: Binance, Bybit, Hyperliquid, OKX
- **5 chart types**: Heatmap, Candlestick, Footprint, Time & Sales, DOM/Ladder
- **3 platforms**: macOS (Universal), Windows, Linux

### Technology Stack
- **Language**: Rust (Edition 2024)
- **GUI Framework**: iced 0.14.0-dev (from git)
- **Async Runtime**: tokio 1.43
- **HTTP Client**: reqwest 0.12 (rustls-tls)
- **WebSocket**: fastwebsockets 0.9
- **JSON**: sonic-rs 0.5 (SIMD-accelerated)
- **Audio**: rodio 0.20

---

## Architecture

### Workspace Structure

FlowSurface uses a 3-crate workspace design with clear separation of concerns:

```
flowsurface/
├── Cargo.toml              (main binary crate)
├── data/                   (library crate)
└── exchange/              (library crate)
```

### Crate Responsibilities

#### 1. **flowsurface** (Main Binary)
**Purpose**: GUI frontend and application orchestration

**Key Modules**:
- `chart/` - Chart rendering components
- `layout/` - Layout and window management
- `modal/` - Modal dialogs (settings, theme editor, audio)
- `screen/dashboard.rs` - Main dashboard screen
- `widget/` - Custom UI widgets
- `style/` - Visual theming

**Dependencies**: data, exchange, iced, iced_futures, iced_core

#### 2. **exchange** (Library)
**Purpose**: Exchange integration and market data streaming

**Key Modules**:
- `adapter/` - Exchange-specific implementations:
  - `binance.rs` - Binance integration
  - `bybit.rs` - Bybit integration
  - `hyperliquid.rs` - Hyperliquid integration
  - `okex.rs` - OKX integration
- `connect.rs` - WebSocket connection management
- `depth.rs` - Order book depth processing
- `fetcher.rs` - Historical data fetching
- `limiter.rs` - Rate limiting logic
- `util.rs` - Utility types (Price, TickerInfo)

**Key Dependencies**: tokio, reqwest, fastwebsockets, sonic-rs

#### 3. **data** (Library)
**Purpose**: Data processing, state management, and business logic

**Key Modules**:
- `aggr/` - Data aggregation (time, ticks)
- `chart/` - Chart data structures
- `config/` - Configuration management
- `layout/` - Layout and pane management
- `audio.rs` - Audio stream for trade sounds
- `tickers_table.rs` - Ticker information table

**Key Dependencies**: serde, serde_json, chrono, rodio

### Dependency Graph

```
flowsurface (binary)
    ├── data (lib)
    │   └── exchange (lib)
    └── exchange (lib)
```

### Why iced from git?

The project uses iced 0.14.0-dev from git (specific revision) instead of crates.io because it requires unreleased features:

1. **`sipper`** - Async task streaming for progress updates
2. **`unconditional-rendering`** - Rendering optimization
3. **`daemon`** - Application lifecycle management
4. **Bug fixes** - Latest fixes for button flickering and other issues

---

## Core Domain

### Exchange Integration Layer

#### WebSocket Connection Management

**Connection Flow**: TCP → TLS → WebSocket Upgrade

```rust
// Location: exchange/src/connect.rs
pub enum State {
    Disconnected,
    Connected(FragmentCollector<TokioIo<Upgraded>>),
}
```

**Features**:
- Uses `fastwebsockets` for efficient frame handling
- `FragmentCollector` for reassembling fragmented messages
- TLS via `tokio-rustls` and `webpki-roots`

#### Event Types

```rust
pub enum Event {
    Connected(Exchange),
    Disconnected(Exchange, String),
    DepthReceived(StreamKind, u64, Depth, Box<[Trade]>),
    KlineReceived(StreamKind, Kline),
}
```

#### Stream Types

```rust
pub enum StreamKind {
    Kline {
        ticker_info: TickerInfo,
        timeframe: Timeframe
    },
    DepthAndTrades {
        ticker_info: TickerInfo,
        depth_aggr: StreamTicksize,
        push_freq: PushFrequency
    },
}
```

#### Exchange Abstraction

All exchanges implement the same pattern:

**Market Data Streams**:
```rust
fn connect_market_stream(ticker_info, push_freq) -> impl Stream<Item = Event>
fn connect_kline_stream(streams, market_type) -> impl Stream<Item = Event>
```

**Data Fetchers**:
```rust
async fn fetch_ticksize(market_type) -> Result<HashMap<Ticker, Option<TickerInfo>>>
async fn fetch_ticker_prices(market_type) -> Result<HashMap<Ticker, TickerStats>>
async fn fetch_klines(ticker_info, timeframe, range) -> Result<Vec<Kline>>
async fn fetch_historical_oi(ticker, range, timeframe) -> Result<Vec<OpenInterest>>
```

#### Rate Limiting

**Per-Exchange Limits**:
- **Binance**: 6000/min (Spot), 2400/min (Perps), with header tracking
- **Bybit**: 600/5s with fixed window
- **Hyperliquid**: 1200/min (conservative)
- **OKX**: 20/2s with fixed window

**Implementation**:
```rust
pub trait RateLimiter: Send + Sync {
    fn prepare_request(&mut self, weight: usize) -> Option<Duration>;
    fn update_from_response(&mut self, response: &Response, weight: usize);
    fn should_exit_on_response(&self, response: &Response) -> bool;
}
```

**Strategies**:
1. **Fixed Window Bucket** - Bybit, OKX, Hyperliquid
2. **Dynamic Bucket** - Binance (adapts to server-reported usage)

### Data Management & Persistence

#### Core Data Structures

**TimeSeries** (Time-Based Aggregation):
```rust
pub struct TimeSeries<D: DataPoint> {
    datapoints: BTreeMap<u64, D>,  // timestamp → data
    interval: Timeframe,
    tick_size: PriceStep,
}
```

**TickAggr** (Tick-Based Aggregation):
```rust
pub struct TickAggr {
    datapoints: Vec<TickAccumulation>,
    interval: TickCount,
    tick_size: PriceStep,
}
```

#### State Persistence

**Application State**:
```rust
pub struct State {
    pub layout_manager: Layouts,
    pub selected_theme: Theme,
    pub custom_theme: Option<Theme>,
    pub main_window: Option<WindowSpec>,
    pub timezone: UserTimezone,
    pub sidebar: Sidebar,
    pub scale_factor: ScaleFactor,
    pub audio_cfg: AudioStream,
    pub trade_fetch_enabled: bool,
    pub size_in_quote_currency: bool,
}
```

**Persistence Mechanism**:
- Saved to: `saved-state.json` in data directory
- Uses serde_json for serialization
- Automatic backup on parsing failure
- Custom deserializer: `ok_or_default` for backward compatibility

#### Theme System

**Default "Flowsurface" Theme**:
```rust
background: Color::from_rgb8(24, 22, 22)
text: Color::from_rgb8(197, 201, 197)
primary: Color::from_rgb8(200, 200, 200)
success: Color::from_rgb8(81, 205, 160)   // Green
danger: Color::from_rgb8(192, 80, 77)     // Red
warning: Color::from_rgb8(238, 216, 139)  // Yellow
```

**Supported Themes** (24 total):
- Built-in: Dark, Light, Dracula, Nord, Gruvbox, Solarized
- Modern: Catppuccin (4 variants), Tokyo Night (3 variants), Kanagawa (3 variants)

#### Layout Management

**Layout Structure**:
```rust
pub struct Layout {
    pub name: String,
    pub dashboard: Dashboard,
}

pub struct Dashboard {
    pub pane: Pane,                      // Root pane (recursive)
    pub popout: Vec<(Pane, WindowSpec)>, // Detached windows
}
```

**Pane Types**:
```rust
pub enum Pane {
    Split { axis: Axis, ratio: f32, a: Box<Pane>, b: Box<Pane> },
    Starter { link_group: Option<LinkGroup> },
    HeatmapChart { /* ... */ },
    KlineChart { /* ... */ },
    TimeAndSales { /* ... */ },
    Ladder { /* ... */ },
}
```

---

## UI Architecture

### Dashboard Component

**Core Structure**:
```rust
pub struct Dashboard {
    pub panes: pane_grid::State<pane::State>,
    pub focus: Option<(window::Id, pane_grid::Pane)>,
    pub popout: HashMap<window::Id, (pane_grid::State<pane::State>, WindowSpec)>,
    pub streams: UniqueStreams,
    layout_id: uuid::Uuid,
}
```

**Pane Operations**:
- **Split**: Divides pane horizontally or vertically
- **Close**: Removes pane and returns focus
- **Maximize/Restore**: Full-screen mode
- **Replace**: Clears content and creates new state
- **Drag & Drop**: Reorder panes
- **Resize**: Adjust split ratios
- **Pop-out**: Extract to separate window
- **Merge**: Return pop-out to main window

### Link Groups

**Purpose**: Synchronized ticker switching across multiple panes

**Implementation**:
```rust
pub enum LinkGroup {
    A, B, C, D, E, F, G, H, I  // 9 groups (displayed as 1-9)
}
```

**Behavior**:
- Panes in same group share ticker symbols
- Changing ticker in one pane updates all linked panes
- Works across main window and pop-out windows
- Preserves individual pane content types

### Sidebar System

**Structure**:
```rust
pub struct Sidebar {
    pub position: Position,              // Left or Right
    pub active_menu: Option<Menu>,       // Currently open menu
    pub tickers_table: Option<tickers_table::Settings>,
}

pub enum Menu {
    Layout, Settings, Audio, ThemeEditor
}
```

**Ticker Discovery**:
1. Parallel fetch from all exchanges on startup
2. Filters: PERPETUAL contracts, USDT/USD quote, TRADING/HALT status
3. Extracts: tickSize, minQty, contractSize
4. Updates: Every 13s (visible) or 300s (hidden)

**Filtering System**:
- Multi-dimensional: Exchange, Market Type, Search Query, Favorites
- Sorting: Volume (Asc/Desc), Price Change (Asc/Desc)
- Grouping: Favorites displayed at top

### Modal System

**Layered Architecture**:
1. **Base Layer**: Main dashboard with pane grid
2. **Toast Notifications**: Positioned at pane/dashboard edges
3. **Pane Modals**: Settings, indicators, stream modifiers (within pane)
4. **Sidebar Modals**: Layout, audio, theme editor (sidebar level)
5. **Dialog Modals**: Confirmation dialogs (full-screen overlay)

**Modal Types**:
- `main_dialog_modal()` - Full-screen with dark backdrop
- `dashboard_modal()` - Positioned overlay without backdrop
- `stack_modal()` - Pane-level overlays

**Dismissal**: Click-outside-to-close + Escape key

---

## Real-Time Data Flow

### WebSocket to UI Pipeline

```
┌─────────────────────────────────────────────┐
│ 1. EXCHANGE ADAPTERS                        │
│    - WebSocket connection                   │
│    - JSON parsing (sonic_rs)                │
│    - Orderbook management                   │
│    - Trade buffering                        │
│    └──> Event::DepthReceived                │
│    └──> Event::KlineReceived                │
└─────────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────┐
│ 2. SUBSCRIPTION SYSTEM (main.rs)            │
│    - Aggregates active streams              │
│    - Maps exchange::Event → Message         │
│    - Tick timer: 100ms interval             │
└─────────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────┐
│ 3. EVENT ROUTING (main.rs update())         │
│    Message::MarketWsEvent(event)            │
│         ↓                                   │
│    Dashboard::update_depth_and_trades()     │
│    Dashboard::update_latest_klines()        │
└─────────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────┐
│ 4. DASHBOARD ROUTING                        │
│    - Matches stream to panes                │
│    - Routes to specific content types       │
│    - Iterates all panes (main + popouts)    │
└─────────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────┐
│ 5. CONTENT-SPECIFIC PROCESSING              │
│    - Heatmap: Aggregates by time interval   │
│    - Kline: Updates latest candle           │
│    - Time & Sales: Appends trades           │
│    - Ladder: Updates depth + trades         │
└─────────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────┐
│ 6. RENDERING INVALIDATION                   │
│    - Tick timer triggers invalidation       │
│    - Clears rendering caches                │
│    - Iced redraws charts                    │
└─────────────────────────────────────────────┘
```

### Tick System

**Tick Timer**: Every 100ms

**Pane Update Intervals**:
- **Kline**: 1000ms (1 second)
- **Heatmap**: 100ms-1000ms (based on timeframe)
- **Time & Sales**: 100ms
- **Ladder**: 100ms

### Stream Management

**UniqueStreams**:
- Deduplicates streams by (Exchange, Ticker, StreamType)
- Single WebSocket per unique stream
- Multiple panes share single stream efficiently

**Stream Lifecycle**:
1. **Initialization**: Pane requests stream
2. **Activation**: Dashboard aggregates all pane streams
3. **Subscription**: Creates WebSocket connections
4. **Distribution**: Routes data to matching panes
5. **Cleanup**: Unused streams unsubscribed automatically

---

## Chart System

### Chart Types Overview

#### 1. **Heatmap Chart**
**Purpose**: Historical order book depth + trade execution visualization

**Features**:
- Historical depth tracking as time-based "runs"
- Trade visualization (circles proportional to volume)
- Coalescing strategy (Average/First/Max)
- Volume profile study
- Pause buffer for scroll-away

**Data**: `HeatmapDataPoint` with `grouped_trades` and `buy_sell` totals

#### 2. **Candlestick/Kline Chart**
**Purpose**: Traditional OHLC visualization

**Modes**:
- **Candles**: Standard candlestick
- **Footprint**: Trade distribution within candles
  - Cluster types: BidAsk, VolumeProfile, DeltaProfile
  - Cluster scaling: VisibleRange, Datapoint, Hybrid
  - Studies: NPoC (Naked POC), Imbalance Detection

**Data**: `KlineDataPoint` with `kline` (OHLC) and `footprint` (trades by price)

#### 3. **DOM/Ladder**
**Purpose**: Real-time order book depth centered on best bid/ask

**Layout** (5 columns):
```
[BidOrderQty] [SellTrades] [Price] [BuyTrades] [AskOrderQty]
     30%          10%        20%       10%          30%
```

**Features**:
- Infinite vertical scrolling (virtualized)
- Spread row highlighting
- Trade overlays at each price
- Volume bars scaled by visible max

#### 4. **Time & Sales**
**Purpose**: Chronological trade tape with metrics

**Features**:
- Stacked bar metrics (Count/Volume/Average Size)
- Scrollable trade list
- Auto-pause on scroll
- Configurable retention (default: 2 minutes)
- Size filtering

#### 5. **Footprint Chart**
**Purpose**: Advanced order flow visualization

**Features**:
- Price-level trade clustering
- Imbalance detection (diagonal buy/sell dominance)
- Delta analysis (buy - sell)
- Volume profile
- POC tracking with naked POC detection

### Footprint Implementation Deep Dive

#### Trade Clustering

**GroupedTrades Structure**:
```rust
pub struct GroupedTrades {
    pub buy_qty: f32,
    pub sell_qty: f32,
    pub first_time: u64,
    pub last_time: u64,
    pub buy_count: usize,
    pub sell_count: usize,
}
```

**Binning Strategies**:

1. **Nearest-Step Binning** (Footprint/OHLC):
```rust
let price = trade.price.round_to_step(step);
// Ties round UP to higher multiple
```

2. **Side-Aware Binning** (Order Book):
```rust
if is_sell {
    price.floor_to_step(step)  // Sells round DOWN
} else {
    price.ceil_to_step(step)   // Buys round UP
}
```

#### Imbalance Detection

**Algorithm**:
```rust
// Compare current price SELL vs next price BUY
if diagonal_buy_qty > sell_qty * (1 + threshold/100) {
    // Draw BUY imbalance marker at higher_price
}

// Compare current price BUY vs previous price SELL
if sell_qty > diagonal_buy_qty * (1 + threshold/100) {
    // Draw SELL imbalance marker at current price
}
```

**Color Intensity** (optional):
```rust
alpha = 0.2 + 0.8 * ((ratio - 1.0) / divisor).min(1.0)
```

#### Naked POC (Point of Control)

**POC Calculation**:
```rust
// Find price with highest total volume
for (price, group) in &trades {
    let total = group.buy_qty + group.sell_qty;
    if total > max_volume {
        max_volume = total;
        poc_price = price;
    }
}
```

**NPoc Status**:
```rust
pub enum NPoc {
    None,              // Not analyzed
    Naked,             // Never revisited
    Filled { at: u64 } // Revisited at timestamp
}
```

**Status Update**:
- Scan all subsequent candles
- Check if candle's high/low range includes POC price
- Mark as "Filled" when price revisits level

**Visual Rendering**:
- **Naked**: Yellow/orange line from origin to current
- **Filled**: Gray line from origin to fill point

### Scale Systems

#### Linear Scale (Y-axis - Price)
- Optimal tick calculation based on visible range
- Logarithmic step sizing (0.1, 0.2, 0.5, 1.0, 2.0, 5.0 × base)
- Dynamic label density (avoids overlap)
- Decimal and abbreviated formats

#### Timeseries Scale (X-axis - Time)
- Adaptive label spacing based on timeframe
- Multi-tier labeling for daily+ (day/month/year labels)
- Sub-daily: Hour/minute labels
- Timezone support (UTC or Local)

#### Autoscaling Modes
- **CenterLatest**: Keep latest candle centered
- **FitToVisible**: Fit price range to visible data

### Indicators

**Kline Indicators**:
- Volume
- Open Interest (Perpetuals only)

**Heatmap Indicators**:
- Volume Profile (FixedWindow or VisibleRange)

**Implementation**:
```rust
trait KlineIndicatorImpl {
    fn element(&self, chart: &ViewState, range: RangeInclusive<u64>) -> Element<Message>;
    fn rebuild_from_source(&mut self, source: &PlotData<KlineDataPoint>);
    fn on_insert_klines(&mut self, klines: &[Kline]);
    fn on_insert_trades(&mut self, trades: &[Trade], ...);
}
```

---

## Rendering & Performance

### Canvas Rendering

**Program Trait**:
```rust
pub trait canvas::Program<Message> {
    type State;

    fn update(&self, state: &mut State, event: &Event, bounds: Rectangle,
              cursor: mouse::Cursor) -> Option<canvas::Action<Message>>;

    fn draw(&self, state: &State, renderer: &Renderer, theme: &Theme,
            bounds: Rectangle, cursor: mouse::Cursor) -> Vec<Geometry>;
}
```

**Cache Strategy**:
```rust
struct Caches {
    main: Cache,        // Chart content (heavy)
    crosshair: Cache,   // Crosshair overlay (updates frequently)
    y_labels: Cache,    // Y-axis labels
    x_labels: Cache,    // X-axis labels
}
```

**Invalidation Rules**:
- Data change → Clear all caches
- Cursor move → Clear crosshair only
- Resize → Clear all
- Scroll/zoom → Clear main + crosshair

### Coordinate System

**Transform Stack**:
```rust
frame.translate(center);           // Center at viewport middle
frame.scale(chart.scaling);        // Apply zoom level
frame.translate(chart.translation); // Apply pan offset
```

**Coordinate Mappings**:
```rust
// Price → Y position
price_to_y(price) -> y = (base_price - price) / tick_size * cell_height

// Time/Index → X position
interval_to_x(interval) -> x = -(interval_diff / cell_width)

// Reverse mappings for crosshair
x_to_interval(x) -> interval
y_to_price(y) -> price
```

### Performance Optimizations

#### Data Cleanup
- **Heatmap**: Triggers at 4800 datapoints, removes oldest 10%
- **Time & Sales / Ladder**: Time-based retention (default: 2 minutes)

#### Visible Range Culling
```rust
let region = chart.visible_region(bounds.size());
let (earliest, latest) = chart.interval_range(&region);
datapoints.range(earliest..=latest)  // BTreeMap range query
```

#### Virtualized Scrolling
- Ladder & Time&Sales: Only render visible rows ± 2
- Prevents rendering thousands of hidden rows

#### Buffer Pre-allocation
```rust
let capacity = time_offsets.len() * price_offsets.len();
let mut grid: FxHashMap<(u64, Price), (f32, bool)> =
    FxHashMap::with_capacity_and_hasher(capacity, FxBuildHasher);
```

### Custom Widgets

#### 1. **Decorate Widget**
- Powerful wrapper for lifecycle customization
- Builder pattern with type-state
- Hooks: layout, update, draw, mouse_interaction, operate, overlay

#### 2. **Toast Notification**
- Overlay-based with automatic timeout (8s default)
- Diff algorithm for efficient updates
- Status types: Primary, Secondary, Success, Danger, Warning

#### 3. **Color Picker**
- HSV color picker with grid and slider
- Pixel-perfect gradient rendering
- No alpha channel support (simplified from Halloy)

#### 4. **Multi-Split Panel**
- Vertical split pane with draggable dividers
- Proportional sizing (0.0-1.0)
- Min panel height: 40px, drag size: 1px
- Hover/drag states change cursor

#### 5. **Draggable Column**
- Vertical layout with drag-and-drop reordering
- Left 14px strip as drag handle
- Clamping prevents items leaving bounds

---

## Build & Deployment

### Cargo Features

**Feature Flags**:
```toml
[features]
debug = ["iced/hot"]  # Hot reloading for development
```

**Conditional Compilation**:
```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
```

### Platform-Specific Code

#### macOS
```rust
#[cfg(target_os = "macos")]
platform_specific: window::settings::PlatformSpecific {
    title_hidden: true,
    titlebar_transparent: true,
    fullsize_content_view: true,
}
```
- Custom titlebar styling
- Full-size content view
- Minimum OS: macOS 11.0 (Big Sur)

#### Windows
```rust
#[cfg(target_os = "windows")]
// Standard window configuration
// No console window in release builds
```

#### Linux
```rust
#[cfg(target_os = "linux")]
decorations: false
```
- Custom window decorations
- Dependencies: `build-essential pkg-config libasound2-dev`

### CI/CD Pipeline

**Three Workflows**:

1. **Format** (`format.yml`):
   - Triggers: Every push and PR
   - Platform: Ubuntu only
   - Tool: rustfmt (edition 2024)

2. **Clippy** (`lint.yml`):
   - Triggers: Push/PR to main, manual dispatch
   - Excludes: Dependabot
   - Command: `cargo clippy --workspace --all-targets --all-features -- -D warnings`

3. **Release** (`release.yaml`):
   - Trigger: Manual dispatch with tag
   - Multi-platform: macOS (Universal), Windows, Linux
   - Optimization: `lto = "fat"`, `codegen-units = 1`, `opt-level = 3`

**Build Scripts**:
- `scripts/build-macos.sh` - Creates universal binary (x86_64 + arm64)
- `scripts/build-windows.sh` - MSVC toolchain, portable ZIP
- `scripts/package-linux.sh` - x86_64 tar.gz

---

## Rust Patterns

### Type Safety

#### Ticker Type System
```rust
pub struct Ticker {
    bytes: [u8; 28],           // Fixed-size, stack-allocated
    pub exchange: Exchange,     // Compile-time coupling
    display_bytes: [u8; 28],   // UI display symbol
    has_display_symbol: bool,
}
```

#### Price Type (Fixed-Point Arithmetic)
```rust
pub struct Price {
    pub units: i64,  // Atomic units at 10^-8 precision
}
pub const PRICE_SCALE: i32 = 8;
```

- Avoids floating-point errors
- Lossless rounding with integer math
- Type-safe precision with `Power10<MIN, MAX>`

### Enums and Pattern Matching

**Exhaustive Matching**:
```rust
match exchange {
    Exchange::BinanceLinear | Exchange::BinanceInverse | Exchange::BinanceSpot => {
        binance::fetch_ticksize(market_type).await
    }
    Exchange::BybitLinear | Exchange::BybitInverse | Exchange::BybitSpot => {
        bybit::fetch_ticksize(market_type).await
    }
    // ... all 11 variants covered
}
```

**Sum Types for State**:
```rust
pub enum ResolvedStream {
    Waiting(Vec<PersistStreamKind>),
    Ready(Vec<StreamKind>),
}
```

**enum_map for O(1) Lookups**:
```rust
EnumMap<Exchange, Option<StreamSpecs>>
```

### Async/Await Patterns

**Async Stream Processing**:
```rust
pub fn connect_depth_stream(...) -> impl Stream<Item = Event> {
    stream::channel(100, async move |mut output| {
        let mut state = State::Disconnected;
        loop {
            match &mut state {
                State::Disconnected => { /* reconnect */ }
                State::Connected(ws) => { /* read frames */ }
            }
        }
    })
}
```

**Task Spawning**:
```rust
tokio::spawn(async move {
    let result = fetch_depth(&ticker, contract_size).await;
    let _ = tx.send(result);
});
```

### Error Handling

**thiserror for Custom Errors**:
```rust
#[derive(thiserror::Error, Debug)]
pub enum AdapterError {
    #[error("{0}")]
    FetchError(#[from] reqwest::Error),
    #[error("Parsing: {0}")]
    ParseError(String),
}
```

**Error Propagation**:
```rust
.map_err(|e| AdapterError::ParseError(e.to_string()))?
```

### Hash Map Usage

**FxHashMap for Performance**:
```rust
use rustc_hash::{FxHashMap, FxHashSet, FxBuildHasher};

// Hot path: price aggregation
pub trades: FxHashMap<Price, GroupedTrades>,
```

**Where Used**:
- Trade aggregation by price
- Audio stream configs
- Grid quantities for heatmap
- Stream management per ticker

**Rationale**: FxHash faster for small keys (integers, fixed-size types)

**BTreeMap for Ordered Data**:
```rust
pub struct Depth {
    pub bids: BTreeMap<Price, f32>,
    pub asks: BTreeMap<Price, f32>,
}
```

### Performance Optimizations

#### Buffer Management
```rust
// Pre-allocated with capacity
let mut grid = FxHashMap::with_capacity_and_hasher(capacity, FxBuildHasher);
let mut labels = Vec::with_capacity(labels_can_fit + 2);
```

#### Copy Trait
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Timeframe { /* variants */ }
```

#### Inline Hints
```rust
#[inline]
pub fn as_f32(self) -> f32 {
    10f32.powi(self.power as i32)
}
```

#### Atomic Operations
```rust
static TRADE_FETCH_ENABLED: AtomicBool = AtomicBool::new(false);
```

#### LazyLock for Statics
```rust
static HTTP_CLIENT: LazyLock<Client> = LazyLock::new(Client::new);
static SPOT_LIMITER: LazyLock<Mutex<BinanceLimiter>> =
    LazyLock::new(|| Mutex::new(BinanceLimiter::new(SPOT_LIMIT, REFILL_RATE)));
```

---

## Limitations & Technical Debt

### Known TODOs

1. **Tick-Based Trade Fetching** (`chart/kline.rs:420`)
   - Trade fetching not implemented for tick-based charts
   - Affects footprint charts using tick intervals

2. **Tick-Based Clear Trades** (`chart/kline.rs:448`)
   - Cannot clear trades on tick-based charts

3. **Heatmap Tick Basis** (`chart/heatmap.rs:260`)
   - Heatmap doesn't support tick-based intervals
   - Will panic if attempted

4. **Hyperliquid Open Interest** (`exchange/adapter/hyperliquid.rs:144`)
   - Open interest data not captured

### Exchange Limitations

#### Historical Trade Fetching
- **Bybit**: No support (lacks bulk historical API)
- **Hyperliquid**: No support (lacks bulk historical API)
- **OKX**: Work in Progress

**Impact**: Footprint charts cannot backfill historical data for these exchanges

#### Exchange-Specific Workarounds
- **Bybit Volume**: Single-colored bars when buy/sell split unavailable
- **Bybit Depth Levels**: Reduced from 500→200 at 100ms (API change)

### Platform-Specific Issues

#### macOS
- Extra title bar padding (20px)
- Custom window chrome (transparent titlebar)

#### Linux
- System dependencies required: `build-essential`, `pkg-config`, `libasound2-dev`
- Custom window decorations

#### Windows
- Console window suppressed in release builds

### Technical Debt

#### Unsafe Code
```rust
// JSON parsing - performance optimization
let iter: sonic_rs::ObjectJsonIter = unsafe { to_object_iter_unchecked(slice) };
```
- **Purpose**: High-frequency WebSocket message parsing
- **Risk**: Bypasses safety checks
- **Justification**: Critical for real-time performance

#### Panic-Heavy Error Handling
```rust
_ => panic!("Invalid depth limit for Spot market"),
_ => panic!("Unsupported timeframe for open interest: {period}"),
```
- **Count**: 108+ occurrences of `.unwrap()` and `.expect()` across 26 files
- **Impact**: Application crashes on invariant violations
- **Improvement**: Convert to `Result<T, E>` types

#### Temporary Workarounds
```rust
/// Temporary workaround,
/// represents any indicator type in the UI
pub enum UiIndicator {
    Heatmap(HeatmapIndicator),
    Kline(KlineIndicator),
}
```

#### Data Cleanup Logic
- **Heatmap**: Hard-coded threshold (4800 datapoints)
- **Historical Files**: Fixed 4-day retention
- **Improvement**: Make configurable

### Architectural Decisions & Trade-offs

#### Performance-First Design
- **SIMD JSON** (sonic-rs): Trades safety for speed
- **Fixed-Point Price**: Avoids floating-point errors (complexity vs precision)
- **BTreeMap for Orderbook**: Sorted order (slower insertion, faster queries)

#### Async Runtime
- **Lightweight tokio**: Only `rt` and `macros` features
- **iced-tokio Bridge**: WebSocket streams → iced Subscriptions

#### Rate Limiting Strategy
- **Exit on Violation**: Hard exit vs graceful degradation
- **Justification**: Prevents IP bans

#### GUI Framework
- **iced from git**: Latest features vs stability
- **wgpu Backend**: GPU-accelerated rendering

#### Data Persistence
- **JSON State Files**: Simplicity vs performance
- **No Database**: In-memory only, on-demand downloads

#### Memory Management
- **Aggressive Cleanup**: Prevents unbounded growth
- **VecDeque**: Circular buffers for streaming data

---

## Key Takeaways

### Strengths

1. **Clean Architecture**: Clear separation of concerns (workspace structure)
2. **Type Safety**: Leverages Rust's type system extensively
3. **Performance**: SIMD JSON, fixed-point math, strategic data structures
4. **Cross-Platform**: macOS (Universal), Windows, Linux with native UX
5. **Real-Time**: Low-latency WebSocket → UI pipeline
6. **Extensibility**: Trait-based design, adapter pattern

### Areas for Improvement

1. **Error Handling**: Convert panics to Result types
2. **Tick-Based Charts**: Complete implementation
3. **Exchange Coverage**: Complete OKX, add Hyperliquid OI
4. **Configurability**: Make thresholds and retention periods configurable
5. **Resilience**: Add WebSocket reconnection logic
6. **Documentation**: Complete TODO items

### Technical Debt Priority

1. **High**: Panic calls → Result types
2. **Medium**: Unsafe blocks (review justification)
3. **Medium**: Tick-based chart support
4. **Low**: Temporary workarounds

---

## Conclusion

FlowSurface is a well-architected, performance-focused cryptocurrency charting application that demonstrates production-grade Rust patterns. The codebase balances correctness and performance with thoughtful architectural decisions, while maintaining clarity through its 3-crate workspace design. The acknowledged experimental status reflects active development and areas for improvement, particularly in error handling robustness and complete feature coverage across all chart types and exchanges.

**Total Analysis Coverage**: 12 phases across architecture, domain, UI, data flow, rendering, build, patterns, and limitations.

---

*Document generated from comprehensive codebase analysis covering ~20,802 lines across 79 Rust files.*
