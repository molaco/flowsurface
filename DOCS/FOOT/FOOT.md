# Footprint Chart: Complete Implementation Documentation

## Table of Contents

1. [Understanding the Footprint Chart](#understanding-the-footprint-chart)
2. [File Structure](#file-structure)
3. [Methods & Functions](#methods--functions)
4. [Data Flow & Logic](#data-flow--logic)

---

## Understanding the Footprint Chart

### Core Concept

Footprint charts visualize trade execution within each candlestick by showing buy/sell volume at each price level.

### Key Components

**1. Trade Clustering** (`chart/kline.rs`):
- Trades are grouped by price level using `GroupedTrades`
- Tracks: `buy_qty`, `sell_qty`, `buy_count`, `sell_count`, `first_time`, `last_time`
- Uses **nearest-step binning**: `trade.price.round_to_step(step)` (ties round UP)

**2. Cluster Display Types** (3 modes):
- **BidAsk**: Shows buy vs sell quantities at each price
- **VolumeProfile**: Total volume distribution
- **DeltaProfile**: Net delta (buy - sell)

**3. Cluster Scaling** (3 strategies):
- **VisibleRange**: Scale relative to max in visible candles
- **Datapoint**: Scale relative to max within each individual candle
- **Hybrid**: Combination approach

**4. Studies**:
- **Imbalance Detection**: Compares diagonal cells (current SELL vs next BUY, current BUY vs prev SELL) with configurable threshold
- **NPoC (Naked Point of Control)**: Tracks highest-volume price level and whether it's been revisited

### Data Structure

```rust
pub struct KlineDataPoint {
    pub kline: Kline,                              // OHLC data
    pub footprint: FxHashMap<Price, GroupedTrades>, // Price-level trades
}
```

The footprint data lives alongside the kline data and gets updated as trades stream in real-time.

---

## File Structure

### Core Implementation Files

#### Data Layer (`data/` crate)

1. **`data/src/chart/kline.rs`** - Core data structures:
   - `KlineDataPoint` (kline + footprint)
   - `GroupedTrades` (buy/sell qty, counts, timestamps)
   - `KlineTrades` (FxHashMap wrapper)
   - `ClusterKind` (BidAsk, DeltaProfile, VolumeProfile)
   - `ClusterScaling` (VisibleRange, Datapoint, Hybrid)
   - `FootprintStudy` (Imbalance, NPoC)
   - `PointOfControl` and `NPoc` enums

2. **`data/src/chart.rs`** - Chart configuration:
   - `KlineChartKind` enum
   - `ViewConfig`

3. **`data/src/aggr/time.rs`** - Time-based aggregation (`TimeSeries<KlineDataPoint>`)
4. **`data/src/aggr/ticks.rs`** - Tick-based aggregation (`TickAggr`)

#### UI/Rendering Layer (`src/` crate)

5. **`src/chart/kline.rs`** - Main rendering logic:
   - `KlineChart` struct
   - Canvas drawing (footprint clusters, imbalances, NPoC lines)
   - Interaction handling
   - Trade insertion/aggregation

6. **`src/chart/indicator/kline.rs`** - Indicator trait for kline charts
7. **`src/chart/indicator/kline/volume.rs`** - Volume indicator
8. **`src/chart/indicator/kline/open_interest.rs`** - Open interest indicator

#### Supporting Files

9. **`src/chart/scale/linear.rs`** - Y-axis (price) scaling
10. **`src/chart/scale/timeseries.rs`** - X-axis (time) scaling
11. **`src/modal/pane/settings.rs`** - Footprint settings UI (cluster type, scaling, studies)
12. **`src/modal/pane/stream.rs`** - Stream configuration UI
13. **`src/screen/dashboard/pane.rs`** - Pane state management
14. **`src/screen/dashboard.rs`** - Dashboard integration (routes trades to panes)

#### Dependencies (External)

15. **`exchange/src/util.rs`** - `Price`, `PriceStep`, `TickerInfo`
16. **`exchange/src/fetcher.rs`** - Historical trade fetching
17. **`exchange/src/lib.rs`** - `Trade`, `Kline` types

### Key File Relationships

```
data/src/chart/kline.rs (data structures)
         ↓
src/chart/kline.rs (rendering)
         ↓
src/modal/pane/settings.rs (UI controls)
         ↓
src/screen/dashboard.rs (integration)
```

---

## Methods & Functions

### Data Layer (`data/` crate)

#### `data/src/chart/kline.rs`

**KlineDataPoint**:
- `max_cluster_qty(cluster_kind, highest, lowest) -> f32`
- `add_trade(&trade, step)`
- `poc_price() -> Option<Price>`
- `set_poc_status(status)`
- `clear_trades()`
- `calculate_poc()`
- `last_trade_time() -> Option<u64>`
- `first_trade_time() -> Option<u64>`

**GroupedTrades**:
- `new(trade) -> Self`
- `add_trade(&trade)`
- `total_qty() -> f32`
- `delta_qty() -> f32`

**KlineTrades**:
- `new() -> Self`
- `first_trade_t() -> Option<u64>`
- `last_trade_t() -> Option<u64>`
- `add_trade_to_side_bin(&trade, step)` (for ladder/DOM)
- `add_trade_to_nearest_bin(&trade, step)` (for footprint/OHLC)
- `max_qty_by<F>(highest, lowest, f) -> f32`
- `calculate_poc()`
- `set_poc_status(status)`
- `poc_price() -> Option<Price>`
- `clear()`

**KlineChartKind**:
- `min_scaling() -> f32`
- `max_scaling() -> f32`
- `max_cell_width() -> f32`
- `min_cell_width() -> f32`
- `max_cell_height() -> f32`
- `min_cell_height() -> f32`
- `default_cell_width() -> f32`

**FootprintStudy**:
- `is_same_type(&other) -> bool`

**NPoc**:
- `filled(&mut self, at: u64)`
- `unfilled(&mut self)`

---

### UI/Rendering Layer (`src/` crate)

#### `src/chart/kline.rs`

**KlineChart** (main struct):

*Constructor*:
- `new(layout, basis, tick_size, klines_raw, raw_trades, enabled_indicators, ticker_info, kind) -> Self`

*Data Updates*:
- `update_latest_kline(&kline)`
- `insert_trades_buffer(&trades_buffer)`
- `insert_raw_trades(raw_trades, is_batches_done)`
- `insert_new_klines(req_id, klines_raw)`
- `insert_open_interest(req_id, oi_data)`

*Trade Management*:
- `raw_trades() -> Vec<Trade>`
- `clear_trades(clear_raw: bool)`

*Footprint Configuration*:
- `kind() -> &KlineChartKind`
- `set_cluster_kind(new_kind)`
- `set_cluster_scaling(new_scaling)`
- `studies() -> Option<Vec<FootprintStudy>>`
- `set_studies(new_studies)`

*Study Configurator*:
- `study_configurator() -> &study::Configurator<FootprintStudy>`
- `update_study_configurator(message)`

*Basis & Tick Size*:
- `basis() -> Basis`
- `change_tick_size(new_tick_size)`
- `set_tick_basis(tick_basis)`
- `tick_size() -> f32`

*Chart State*:
- `chart_layout() -> ViewConfig`
- `last_update() -> Instant`
- `invalidate(now) -> Option<Action>`

*Request Handling*:
- `missing_data_task() -> Option<Action>`
- `reset_request_handler()`
- `set_handle(handle)`

*Indicators*:
- `toggle_indicator(indicator)`

*Internal*:
- `calc_qty_scales(earliest, latest, highest, lowest, step, cluster_kind) -> f32`

**Chart Trait Implementation**:
- `state() -> &ViewState`
- `mut_state() -> &mut ViewState`
- `invalidate_crosshair()`
- `invalidate_all()`
- `view_indicators(&enabled) -> Vec<Element<Message>>`
- `visible_timerange() -> (u64, u64)`
- `interval_keys() -> Option<Vec<u64>>`
- `autoscaled_coords() -> Vector`
- `supports_fit_autoscaling() -> bool`
- `is_empty() -> bool`

**PlotConstants Trait Implementation**:
- `min_scaling() -> f32`
- `max_scaling() -> f32`
- `max_cell_width() -> f32`
- `min_cell_width() -> f32`
- `max_cell_height() -> f32`
- `min_cell_height() -> f32`
- `default_cell_width() -> f32`

**Canvas::Program Trait Implementation**:
- `update(&interaction, &event, bounds, cursor) -> Option<canvas::Action<Message>>`
- `draw(&interaction, renderer, theme, bounds, cursor) -> Vec<Geometry>`
- `mouse_interaction(&interaction, bounds, cursor) -> mouse::Interaction`

---

#### Rendering Functions (in `src/chart/kline.rs`)

**Main Rendering**:
- `render_data_source<F>(data_source, frame, earliest, latest, interval_to_x, draw_fn)`

**Cluster Drawing**:
- `draw_clusters(frame, price_to_y, x_position, cell_width, cell_height, candle_width, max_cluster_qty, palette, text_size, tick_size, show_text, imbalance, kline, footprint, cluster_kind, spacing)`
- `effective_cluster_qty(scaling, visible_max, footprint, cluster_kind) -> f32`
- `draw_cluster_text(frame, text, position, text_size, color, align_x, align_y)`

**Imbalance**:
- `draw_imbalance_markers(frame, price_to_y, footprint, price, sell_qty, higher_price, threshold, color_scale, ignore_zeros, cell_height, palette, buyside_x, sellside_x, rect_width)`

**NPoC (Naked Point of Control)**:
- `draw_all_npocs(data_source, frame, price_to_y, interval_to_x, candle_width, cell_width, cell_height, palette, studies, visible_earliest, visible_latest, cluster_kind, spacing, imb_study_on)`

**Candle Drawing**:
- `draw_footprint_kline(frame, price_to_y, x_position, candle_width, kline, palette)`
- `draw_candle_dp(frame, price_to_y, candle_width, palette, x_position, kline)`

**Crosshair**:
- `draw_crosshair_tooltip(data, ticker_info, frame, palette, at_interval)`

**Layout Helpers**:
- `ProfileArea::new(content_left, content_right, candle_width, gaps, has_imbalance) -> Self`
- `BidAskArea::new(x_position, content_left, content_right, candle_width, spacing) -> Self`
- `ContentGaps::from_view(candle_width, scaling) -> Self`
- `should_show_text(cell_height_unscaled, cell_width_unscaled, min_w) -> bool`

---

### Summary

**Total Methods**: ~100+

**Data Layer** (15 methods): Core footprint logic, trade aggregation, POC calculation
**UI Layer** (85+ methods): Rendering, interaction, configuration, state management

**Key Method Categories**:
1. Trade aggregation & binning
2. POC calculation & tracking
3. Cluster rendering (BidAsk, VolumeProfile, DeltaProfile)
4. Imbalance detection & visualization
5. NPoC status tracking & rendering
6. Cluster scaling strategies
7. Study management
8. Data updates & synchronization

---

## Data Flow & Logic

This section traces the complete data flow from application entry point to the final rendered footprint chart.

### 1. Application Entry Point

**File**: `src/main.rs`

**Entry Flow**:
```
main()
  → FlowSurface::load()
  → FlowSurface::new(state: State)
  → Dashboard::new() (contains panes)
```

**Key Structs Initialized**:
- `FlowSurface` - Main application state
- `State` - Persisted application configuration (from `saved-state.json`)
- `Dashboard` - Contains `pane_grid::State<pane::State>`

---

### 2. Pane Creation & Chart Initialization

**File**: `src/screen/dashboard/pane.rs`

**Flow**:
```
User creates new footprint pane
  ↓
Dashboard::update(Message::ReplacePane { content: PaneContent::KlineChart { kind: Footprint {...} } })
  ↓
pane::State::new_kline_chart(ticker_info, basis, kind)
  ↓
Creates PaneContent::KlineChart { state: ChartState::Loading }
```

**Structs Involved**:
```rust
pub enum PaneContent {
    KlineChart {
        state: ChartState,
        kind: KlineChartKind,  // Contains Footprint config
        stream: ResolvedStream,
        // ...
    }
}

pub enum KlineChartKind {
    Footprint {
        clusters: ClusterKind,        // BidAsk | VolumeProfile | DeltaProfile
        scaling: ClusterScaling,      // VisibleRange | Hybrid | Datapoint
        studies: Vec<FootprintStudy>, // NPoC, Imbalance
    }
}
```

---

### 3. Historical Data Fetching

**Files**: `exchange/src/fetcher.rs`, `src/screen/dashboard.rs`

**Flow**:
```
pane::State triggers data fetch
  ↓
Task::perform(exchange::fetcher::fetch_klines(...))
  → Returns Vec<Kline>
  ↓
Task::perform(exchange::fetcher::fetch_trades(...))  [if enabled]
  → Returns Vec<Trade>
  ↓
Dashboard receives Message::KlinesFetched(klines, trades)
```

**Data Structures**:
```rust
// From exchange crate
pub struct Kline {
    pub time: u64,
    pub open: Price,
    pub high: Price,
    pub low: Price,
    pub close: Price,
    pub volume: f32,
    // ...
}

pub struct Trade {
    pub price: Price,
    pub qty: f32,
    pub time: u64,
    pub is_sell: bool,
}
```

---

### 4. Chart Construction (Time-Based)

**File**: `src/chart/kline.rs` → `KlineChart::new()`

**Flow**:
```
KlineChart::new(layout, basis, tick_size, klines_raw, raw_trades, ...)
  ↓
[Time-Based Branch]
  ↓
TimeSeries::<KlineDataPoint>::new(interval, step, &raw_trades, klines_raw)
```

**File**: `data/src/aggr/time.rs` → `TimeSeries::new()`

**Detailed Flow**:
```rust
TimeSeries::new(interval, step, trades, klines)
  ↓
1. Create empty BTreeMap<u64, KlineDataPoint>
  ↓
2. Insert klines:
   for kline in klines {
       let timestamp = kline.time;
       datapoints.insert(timestamp, KlineDataPoint {
           kline: *kline,
           footprint: KlineTrades::new(),  // Empty initially
       });
   }
  ↓
3. Insert trades:
   self.insert_trades(trades)
```

**File**: `data/src/aggr/time.rs` → `TimeSeries::insert_trades()`

**Trade Aggregation Flow**:
```rust
insert_trades(trades: &[Trade])
  ↓
for trade in trades {
    // 1. Find which time interval this trade belongs to
    let interval_timestamp = (trade.time / interval_ms) * interval_ms;

    // 2. Get or create KlineDataPoint for this interval
    let dp = datapoints.entry(interval_timestamp)
        .or_insert_with(|| KlineDataPoint {
            kline: Kline::from_trade(trade, interval_timestamp),
            footprint: KlineTrades::new(),
        });

    // 3. Add trade to footprint
    dp.add_trade(trade, step);
}
  ↓
Calculate POC for each datapoint:
for dp in datapoints.values_mut() {
    dp.calculate_poc();
}
```

**File**: `data/src/chart/kline.rs` → `KlineDataPoint::add_trade()`

**Trade Binning Flow**:
```rust
KlineDataPoint::add_trade(&trade, step)
  ↓
self.footprint.add_trade_to_nearest_bin(trade, step)
  ↓
KlineTrades::add_trade_to_nearest_bin(&trade, step)
  ↓
// Round price to nearest step
let price = trade.price.round_to_step(step);  // e.g., 50123.5 → 50124.0
  ↓
// Get or create GroupedTrades for this price
self.trades.entry(price)
    .and_modify(|group| group.add_trade(trade))
    .or_insert_with(|| GroupedTrades::new(trade));
```

**File**: `data/src/chart/kline.rs` → `GroupedTrades::add_trade()`

**Accumulation Flow**:
```rust
GroupedTrades::add_trade(&trade)
  ↓
if trade.is_sell {
    self.sell_qty += trade.qty;      // e.g., 0.5 BTC
    self.sell_count += 1;            // 1 trade
} else {
    self.buy_qty += trade.qty;       // e.g., 1.2 BTC
    self.buy_count += 1;             // 1 trade
}
self.last_time = trade.time;
```

**Result Structure**:
```rust
// After all trades processed:
KlineDataPoint {
    kline: Kline { open: 50000, high: 50200, low: 49900, close: 50100, ... },
    footprint: KlineTrades {
        trades: FxHashMap {
            Price(50000) → GroupedTrades { buy_qty: 2.5, sell_qty: 1.2, ... },
            Price(50010) → GroupedTrades { buy_qty: 0.8, sell_qty: 3.1, ... },
            Price(50020) → GroupedTrades { buy_qty: 5.2, sell_qty: 0.3, ... },
            // ... one entry per price level
        },
        poc: None  // Will be calculated next
    }
}
```

---

### 5. Point of Control (POC) Calculation

**File**: `data/src/chart/kline.rs` → `KlineTrades::calculate_poc()`

**Flow**:
```rust
calculate_poc()
  ↓
if self.trades.is_empty() { return; }
  ↓
let mut max_volume = 0.0;
let mut poc_price = Price::from_f32(0.0);
  ↓
// Find price with highest total volume
for (price, group) in &self.trades {
    let total_volume = group.total_qty();  // buy_qty + sell_qty
    if total_volume > max_volume {
        max_volume = total_volume;
        poc_price = *price;
    }
}
  ↓
self.poc = Some(PointOfControl {
    price: poc_price,      // e.g., Price(50020)
    volume: max_volume,    // e.g., 5.5 BTC
    status: NPoc::default(), // NPoc::None initially
});
```

**Updated Structure**:
```rust
KlineDataPoint {
    kline: Kline { ... },
    footprint: KlineTrades {
        trades: FxHashMap { ... },
        poc: Some(PointOfControl {
            price: Price(50020),
            volume: 5.5,
            status: NPoc::None,
        })
    }
}
```

---

### 6. Real-Time Trade Updates

**File**: `src/main.rs` → Subscription system

**WebSocket Flow**:
```
Exchange WebSocket
  ↓
exchange::Event::DepthReceived(stream_kind, timestamp, depth, trades)
  ↓
main.rs subscription() maps to Message::MarketWsEvent(event)
  ↓
FlowSurface::update(Message::MarketWsEvent(event))
  ↓
Dashboard::update_depth_and_trades(event)
```

**File**: `src/screen/dashboard.rs` → `Dashboard::update_depth_and_trades()`

**Routing Flow**:
```rust
update_depth_and_trades(stream_kind, trades)
  ↓
// Route to all matching panes
for (_, pane_state) in self.panes.iter_mut() {
    if let PaneContent::KlineChart { state: ChartState::Ready(chart), .. } = &mut pane_state.content {
        if chart.matches_stream(&stream_kind) {
            chart.insert_trades_buffer(&trades);
        }
    }
}
```

**File**: `src/chart/kline.rs` → `KlineChart::insert_trades_buffer()`

**Live Update Flow**:
```rust
insert_trades_buffer(trades_buffer: &[Trade])
  ↓
// Add to raw trades cache
self.raw_trades.extend_from_slice(trades_buffer);
  ↓
match self.data_source {
    PlotData::TimeBased(ref mut timeseries) => {
        timeseries.insert_trades(trades_buffer);
        // This calls the same logic as initialization:
        // - Bins trades by price
        // - Updates GroupedTrades
        // - Recalculates POC if needed
    }
}
  ↓
self.invalidate(None);  // Trigger re-render
```

---

### 7. Rendering Pipeline (Canvas Drawing)

**File**: `src/chart/kline.rs` → `canvas::Program::draw()`

**Render Entry Point**:
```rust
KlineChart::draw(&self, interaction, renderer, theme, bounds, cursor) -> Vec<Geometry>
  ↓
match &self.kind {
    KlineChartKind::Footprint { clusters, scaling, studies } => {
        // Footprint rendering path
    }
}
```

**Main Canvas Setup**:
```rust
let klines = chart.cache.main.draw(renderer, bounds_size, |frame| {
    // 1. Set up coordinate system
    frame.translate(center);          // Center at viewport
    frame.scale(chart.scaling);       // Apply zoom
    frame.translate(chart.translation); // Apply pan

    // 2. Calculate visible region
    let region = chart.visible_region(frame.size());
    let (earliest, latest) = chart.interval_range(&region);

    // 3. Render footprint
    // ...
});
```

**Data Passed to Renderer**:
```rust
// Coordinate transform functions
let price_to_y = |price: Price| -> f32 {
    (chart.base_price_y - price) / chart.tick_size * chart.cell_height
};

let interval_to_x = |interval: u64| -> f32 {
    -(interval_diff / chart.cell_width)
};
```

---

### 8. Cluster Scaling Calculation

**File**: `src/chart/kline.rs` → `calc_qty_scales()`

**Flow**:
```rust
calc_qty_scales(earliest, latest, highest, lowest, step, cluster_kind) -> f32
  ↓
// Round price range
let rounded_highest = highest.round_to_side_step(false, step).add_steps(1, step);
let rounded_lowest = lowest.round_to_side_step(true, step).add_steps(-1, step);
  ↓
match &self.data_source {
    PlotData::TimeBased(timeseries) => {
        timeseries.max_qty_ts_range(
            cluster_kind,
            earliest,       // e.g., 1700000000000 (timestamp)
            latest,         // e.g., 1700003600000
            rounded_highest, // e.g., Price(50200)
            rounded_lowest,  // e.g., Price(49900)
        )
    }
}
```

**File**: `data/src/aggr/time.rs` → `TimeSeries::max_qty_ts_range()`

**Max Quantity Search**:
```rust
max_qty_ts_range(cluster_kind, earliest, latest, highest, lowest) -> f32
  ↓
let mut max_qty = 0.0;
  ↓
// Iterate visible candles
for (_, dp) in self.datapoints.range(earliest..=latest) {
    let candle_max = dp.max_cluster_qty(cluster_kind, highest, lowest);
    max_qty = max_qty.max(candle_max);
}
  ↓
return max_qty;  // e.g., 15.8 BTC (max cluster quantity in visible range)
```

**File**: `data/src/chart/kline.rs` → `KlineDataPoint::max_cluster_qty()`

**Per-Candle Max Calculation**:
```rust
max_cluster_qty(cluster_kind, highest, lowest) -> f32
  ↓
match cluster_kind {
    ClusterKind::BidAsk => {
        self.footprint.max_qty_by(highest, lowest, f32::max)
        // Returns max(buy_qty, sell_qty) for each price
    }
    ClusterKind::DeltaProfile => {
        self.footprint.max_qty_by(highest, lowest, |buy, sell| (buy - sell).abs())
        // Returns abs(delta) for each price
    }
    ClusterKind::VolumeProfile => {
        self.footprint.max_qty_by(highest, lowest, |buy, sell| buy + sell)
        // Returns total volume for each price
    }
}
```

**File**: `data/src/chart/kline.rs` → `KlineTrades::max_qty_by()`

**Generic Max Finder**:
```rust
max_qty_by<F>(highest, lowest, f: F) -> f32
where F: Fn(f32, f32) -> f32
  ↓
let mut max_qty = 0.0;
  ↓
for (price, group) in &self.trades {
    if *price >= lowest && *price <= highest {
        max_qty = max_qty.max(f(group.buy_qty, group.sell_qty));
    }
}
  ↓
return max_qty;
```

**Result**: `max_cluster_qty` = 15.8 BTC (used for scaling bars)

---

### 9. NPoC (Naked Point of Control) Rendering

**File**: `src/chart/kline.rs` → `draw_all_npocs()`

**Flow**:
```rust
draw_all_npocs(data_source, frame, price_to_y, interval_to_x, studies, ...)
  ↓
// Extract lookback from studies
let Some(lookback) = studies.iter().find_map(|study| {
    if let FootprintStudy::NPoC { lookback } = study {
        Some(*lookback)  // e.g., 80 candles
    }
});
  ↓
// Iterate last N candles
match data_source {
    PlotData::TimeBased(timeseries) => {
        timeseries.datapoints
            .iter()
            .rev()
            .take(lookback)
            .filter_map(|(timestamp, dp)| {
                dp.footprint.poc.as_ref().map(|poc| (*timestamp, poc))
            })
            .for_each(|(interval, poc)| draw_the_line(interval, poc));
    }
}
```

**Line Drawing Logic**:
```rust
draw_the_line(interval, poc)
  ↓
let start_x = start_x_for(interval_to_x(interval));
  ↓
match poc.status {
    NPoc::Naked => {
        // Draw from POC origin to current time (rightmost)
        let end_x = end_x_for(rightmost_cell_center_x);
        let line_width = end_x - start_x;
        frame.fill_rectangle(
            Point::new(start_x, price_to_y(poc.price)),
            Size::new(line_width, line_height),
            naked_color  // Yellow/orange
        );
    }
    NPoc::Filled { at } => {
        // Draw from POC origin to fill time
        let end_x = end_x_for(interval_to_x(at));
        let line_width = end_x - start_x;
        frame.fill_rectangle(..., filled_color);  // Gray
    }
    NPoc::None => return,
}
```

**Data Structure Used**:
```rust
PointOfControl {
    price: Price(50020),
    volume: 5.5,
    status: NPoc::Naked,  // or NPoc::Filled { at: 1700003600000 }
}
```

---

### 10. Cluster Rendering (Main Logic)

**File**: `src/chart/kline.rs` → `render_data_source()`

**Iteration Setup**:
```rust
render_data_source(data_source, frame, earliest, latest, interval_to_x, draw_fn)
  ↓
match data_source {
    PlotData::TimeBased(timeseries) => {
        timeseries.datapoints
            .range(earliest..=latest)
            .for_each(|(timestamp, dp)| {
                let x_position = interval_to_x(*timestamp);
                draw_fn(frame, x_position, &dp.kline, &dp.footprint);
            });
    }
}
```

**For Each Candle**:
```rust
// draw_fn is a closure defined in canvas::Program::draw()
|frame, x_position, kline, footprint| {
    // 1. Calculate effective scaling
    let cluster_scaling = effective_cluster_qty(
        scaling,          // VisibleRange | Hybrid | Datapoint
        max_cluster_qty,  // 15.8 BTC (from step 8)
        footprint,        // KlineTrades for this candle
        cluster_kind      // BidAsk | VolumeProfile | DeltaProfile
    );

    // 2. Draw clusters
    draw_clusters(frame, price_to_y, x_position, ..., footprint, ...);
}
```

**File**: `src/chart/kline.rs` → `effective_cluster_qty()`

**Scaling Strategy**:
```rust
effective_cluster_qty(scaling, visible_max, footprint, cluster_kind) -> f32
  ↓
// 1. Calculate individual candle max
let individual_max = match cluster_kind {
    ClusterKind::BidAsk => {
        footprint.trades.values()
            .map(|group| group.buy_qty.max(group.sell_qty))
            .fold(0.0, f32::max)  // e.g., 3.2 BTC
    }
    ClusterKind::VolumeProfile => {
        footprint.trades.values()
            .map(|group| group.buy_qty + group.sell_qty)
            .fold(0.0, f32::max)  // e.g., 5.5 BTC
    }
    // ...
};
  ↓
// 2. Apply scaling strategy
match scaling {
    ClusterScaling::VisibleRange => visible_max,      // 15.8 BTC
    ClusterScaling::Datapoint => individual_max,      // 5.5 BTC
    ClusterScaling::Hybrid { weight } => {
        visible_max * weight + individual_max * (1.0 - weight)
        // e.g., 15.8 * 0.2 + 5.5 * 0.8 = 7.56 BTC
    }
}
```

**Result**: Returns scaling factor used to normalize bar widths

---

### 11. Drawing Individual Clusters

**File**: `src/chart/kline.rs` → `draw_clusters()`

**Layout Calculation**:
```rust
draw_clusters(frame, price_to_y, x_position, cell_width, max_cluster_qty, footprint, cluster_kind, ...)
  ↓
// 1. Calculate layout areas
let bar_width_factor = 0.9;
let inset = (cell_width * (1.0 - bar_width_factor)) / 2.0;
let content_left = x_position - (cell_width / 2.0) + inset;
let content_right = x_position + (cell_width / 2.0) - inset;
  ↓
match cluster_kind {
    ClusterKind::BidAsk => {
        let area = BidAskArea::new(x_position, content_left, content_right, candle_width, spacing);
        // area contains:
        //   bid_area_left, bid_area_right (right side of candle)
        //   ask_area_left, ask_area_right (left side of candle)
        //   candle_center_x
    }
    ClusterKind::VolumeProfile | ClusterKind::DeltaProfile => {
        let area = ProfileArea::new(content_left, content_right, candle_width, spacing, has_imbalance);
        // area contains:
        //   bars_left, bars_width (single bar area)
        //   candle_center_x
        //   imb_marker_left, imb_marker_width
    }
}
```

**BidAsk Cluster Rendering**:
```rust
ClusterKind::BidAsk => {
    for (price, group) in &footprint.trades {
        let y = price_to_y(*price);  // Convert Price(50020) → y: -50.0 pixels

        // Draw buy side (right)
        if group.buy_qty > 0.0 {
            let bar_width = (group.buy_qty / max_cluster_qty) * right_area_width;
            // e.g., (2.5 / 15.8) * 80.0 = 12.7 pixels

            if show_text {
                draw_cluster_text(
                    frame,
                    &abbr_large_numbers(group.buy_qty),  // "2.5" or "2.5K"
                    Point::new(area.bid_area_left, y),
                    text_size,
                    text_color,
                    Alignment::Start,
                    Alignment::Center
                );
            }

            frame.fill_rectangle(
                Point::new(area.bid_area_left, y - (cell_height / 2.0)),
                Size::new(bar_width, cell_height),
                palette.success.base.color  // Green
            );
        }

        // Draw sell side (left)
        if group.sell_qty > 0.0 {
            let bar_width = (group.sell_qty / max_cluster_qty) * left_area_width;
            // e.g., (1.2 / 15.8) * 80.0 = 6.1 pixels

            if show_text {
                draw_cluster_text(...);
            }

            frame.fill_rectangle(
                Point::new(area.ask_area_right, y - (cell_height / 2.0)),
                Size::new(-bar_width, cell_height),  // Negative width = left
                palette.danger.base.color  // Red
            );
        }
    }
}
```

**VolumeProfile Cluster Rendering**:
```rust
ClusterKind::VolumeProfile => {
    for (price, group) in &footprint.trades {
        let y = price_to_y(*price);

        // Draw stacked buy/sell bar
        super::draw_volume_bar(
            frame,
            area.bars_left,
            y,
            group.buy_qty,      // 2.5 BTC (green portion)
            group.sell_qty,     // 1.2 BTC (red portion)
            max_cluster_qty,    // 15.8 BTC (for scaling)
            area.bars_width,    // 80.0 pixels
            cell_height,
            palette.success.base.color,  // Green
            palette.danger.base.color,   // Red
            bar_alpha,
            true  // bidask_split
        );

        if show_text {
            let total = group.total_qty();  // 3.7 BTC
            draw_cluster_text(
                frame,
                &abbr_large_numbers(total),  // "3.7"
                Point::new(area.bars_left, y),
                text_size,
                text_color,
                Alignment::Start,
                Alignment::Center
            );
        }
    }
}
```

**DeltaProfile Cluster Rendering**:
```rust
ClusterKind::DeltaProfile => {
    for (price, group) in &footprint.trades {
        let y = price_to_y(*price);
        let delta = group.delta_qty();  // buy_qty - sell_qty = 2.5 - 1.2 = 1.3

        if show_text {
            draw_cluster_text(
                frame,
                &abbr_large_numbers(delta),  // "+1.3" or "-0.5"
                Point::new(area.bars_left, y),
                text_size,
                text_color,
                Alignment::Start,
                Alignment::Center
            );
        }

        let bar_width = (delta.abs() / max_cluster_qty) * area.bars_width;
        // e.g., (1.3 / 15.8) * 80.0 = 6.6 pixels

        let color = if delta >= 0.0 {
            palette.success.base.color  // Green for positive delta
        } else {
            palette.danger.base.color   // Red for negative delta
        };

        frame.fill_rectangle(
            Point::new(area.bars_left, y - (cell_height / 2.0)),
            Size::new(bar_width, cell_height),
            color
        );
    }
}
```

**Visual Result Per Price Level**:
```
Price 50020: [====BUY:2.5====] [CANDLE] [==SELL:1.2==]  (BidAsk mode)
Price 50010: [===BUY:0.8===]   [CANDLE] [======SELL:3.1======]
Price 50000: [========BUY:5.2========] [CANDLE] [SELL:0.3]

OR

Price 50020: [IMBALANCE] [CANDLE] [====TOTAL:3.7====]  (VolumeProfile mode)
Price 50010: [IMBALANCE] [CANDLE] [=====TOTAL:3.9=====]
Price 50000: [         ] [CANDLE] [=======TOTAL:5.5=======]

OR

Price 50020: [IMBALANCE] [CANDLE] [===DELTA:+1.3===]  (DeltaProfile mode)
Price 50010: [IMBALANCE] [CANDLE] [====DELTA:-2.3====]
Price 50000: [IMBALANCE] [CANDLE] [======DELTA:+4.9======]
```

---

### 12. Imbalance Detection & Rendering

**File**: `src/chart/kline.rs` → `draw_imbalance_markers()`

**Called From**: Inside `draw_clusters()` loop

**Flow**:
```rust
// For each price level in footprint
for (price, group) in &footprint.trades {
    // ... draw bars ...

    if let Some((threshold, color_scale, ignore_zeros)) = imbalance {
        let step = PriceStep::from_f32(tick_size);
        let higher_price = Price::from_f32(price.to_f32() + tick_size).round_to_step(step);
        // e.g., Price(50020) + 10 = Price(50030)

        draw_imbalance_markers(
            frame,
            &price_to_y,
            footprint,        // Access to all price levels
            *price,           // Current price: 50020
            group.sell_qty,   // Sell at current: 1.2 BTC
            higher_price,     // Next price: 50030
            threshold,        // e.g., 200 (= 200% = 3x)
            color_scale,
            ignore_zeros,
            cell_height,
            palette,
            buyside_x,
            sellside_x,
            rect_width
        );
    }
}
```

**Imbalance Detection Logic**:
```rust
draw_imbalance_markers(price, sell_qty, higher_price, threshold, ...)
  ↓
// Ignore if current price has zero sell
if ignore_zeros && sell_qty <= 0.0 { return; }
  ↓
// Look up diagonal buy quantity at higher price
if let Some(group) = footprint.trades.get(&higher_price) {
    let diagonal_buy_qty = group.buy_qty;  // e.g., 3.8 BTC at Price(50030)

    if ignore_zeros && diagonal_buy_qty <= 0.0 { return; }

    // Compare diagonal buy vs current sell
    if diagonal_buy_qty >= sell_qty {
        let required_qty = sell_qty * (100 + threshold) as f32 / 100.0;
        // e.g., 1.2 * (100 + 200) / 100 = 3.6 BTC

        if diagonal_buy_qty > required_qty {
            // BUY IMBALANCE at higher_price
            // 3.8 > 3.6 ✓

            let ratio = diagonal_buy_qty / required_qty;  // 3.8 / 3.6 = 1.056
            let alpha = alpha_from_ratio(ratio);  // 0.2 + 0.8 * (0.056 / 3) = ~0.22

            let y = price_to_y(higher_price);  // Y position at 50030
            frame.fill_rectangle(
                Point::new(buyside_x, y - (rect_height / 2.0)),
                Size::new(rect_width, rect_height),
                palette.success.weak.color.scale_alpha(alpha)  // Green with alpha
            );
        }
    } else {
        // current sell > diagonal buy
        let required_qty = diagonal_buy_qty * (100 + threshold) as f32 / 100.0;

        if sell_qty > required_qty {
            // SELL IMBALANCE at current price

            let ratio = sell_qty / required_qty;
            let alpha = alpha_from_ratio(ratio);

            let y = price_to_y(price);  // Y position at current (50020)
            frame.fill_rectangle(
                Point::new(sellside_x, y - (rect_height / 2.0)),
                Size::new(rect_width, rect_height),
                palette.danger.weak.color.scale_alpha(alpha)  // Red with alpha
            );
        }
    }
}
```

**Diagonal Comparison Visual**:
```
Price 50030: BUY: 3.8  [█ BUY IMBALANCE]  ← diagonal_buy_qty
Price 50020: SELL: 1.2                    ← current sell_qty

Calculation:
  required_qty = 1.2 * 3.0 = 3.6
  3.8 > 3.6 ✓ → Draw green marker at 50030

Price 50010: BUY: 0.5
Price 50000: SELL: 2.8  [█ SELL IMBALANCE] ← current sell_qty > diagonal_buy

Calculation:
  required_qty = 0.5 * 3.0 = 1.5
  2.8 > 1.5 ✓ → Draw red marker at 50000
```

---

### 13. Candle Drawing (Footprint Mode)

**File**: `src/chart/kline.rs` → `draw_footprint_kline()`

**Called After** cluster/imbalance rendering

**Flow**:
```rust
draw_footprint_kline(frame, price_to_y, area.candle_center_x, candle_width, kline, palette)
  ↓
let y_open = price_to_y(kline.open);    // e.g., -10.0
let y_high = price_to_y(kline.high);    // e.g., -30.0
let y_low = price_to_y(kline.low);      // e.g., +10.0
let y_close = price_to_y(kline.close);  // e.g., -5.0
  ↓
// Draw candle body (thin)
let body_color = if kline.close >= kline.open {
    palette.success.weak.color  // Green
} else {
    palette.danger.weak.color   // Red
};

frame.fill_rectangle(
    Point::new(x_position - (candle_width / 8.0), y_open.min(y_close)),
    Size::new(candle_width / 4.0, (y_open - y_close).abs()),
    body_color
);
  ↓
// Draw wick (very thin)
let wick_color = body_color.scale_alpha(0.6);
frame.stroke(
    &Path::line(
        Point::new(x_position, y_high),
        Point::new(x_position, y_low)
    ),
    Stroke::with_color(Stroke { width: 1.0, .. }, wick_color)
);
```

**Visual Layering**:
```
Left side:     [====SELL CLUSTERS====]
Center:        [|] ← Thin candle wick + body
Right side:    [====BUY CLUSTERS====]
Imbalance:     [█] ← Small markers between candle and clusters
```

---

### 14. Crosshair & Tooltip

**File**: `src/chart/kline.rs` → `draw_crosshair_tooltip()`

**Triggered**: When cursor hovers over chart

**Flow**:
```rust
if let Some(cursor_position) = cursor.position_in(bounds) {
    let (_, rounded_aggregation) = chart.draw_crosshair(frame, theme, bounds_size, cursor_position, interaction);
    // rounded_aggregation is the timestamp/interval at cursor position

    draw_crosshair_tooltip(data_source, ticker_info, frame, palette, rounded_aggregation);
}
```

**Tooltip Data Extraction**:
```rust
draw_crosshair_tooltip(data, ticker_info, frame, palette, at_interval)
  ↓
// Find kline at cursor position
let kline_opt = match data {
    PlotData::TimeBased(timeseries) => {
        timeseries.datapoints
            .iter()
            .find(|(time, _)| **time == at_interval)
            .map(|(_, dp)| &dp.kline)
    }
};
  ↓
if let Some(kline) = kline_opt {
    let change_pct = ((kline.close - kline.open).to_f32() / kline.open.to_f32()) * 100.0;
    // e.g., (50100 - 50000) / 50000 * 100 = +0.20%

    let segments = [
        ("O", base_color, false),
        (&kline.open.to_string(precision), change_color, true),   // "50000.0"
        ("H", base_color, false),
        (&kline.high.to_string(precision), change_color, true),   // "50200.0"
        ("L", base_color, false),
        (&kline.low.to_string(precision), change_color, true),    // "49900.0"
        ("C", base_color, false),
        (&kline.close.to_string(precision), change_color, true),  // "50100.0"
        (&format!("{change_pct:+.2}%"), change_color, true),      // "+0.20%"
    ];

    // Draw tooltip background
    frame.fill_rectangle(tooltip_rect, palette.background.weakest.color.scale_alpha(0.9));

    // Draw text segments
    let mut x = position.x;
    for (text, seg_color, is_value) in segments {
        frame.fill_text(...);
        x += text.len() as f32 * 8.0 + spacing;
    }
}
```

**Visual Result**:
```
┌──────────────────────────────────────────────────┐
│ O 50000.0  H 50200.0  L 49900.0  C 50100.0  +0.20%│
└──────────────────────────────────────────────────┘
```

---

### 15. Final Geometry Output

**File**: `src/chart/kline.rs` → `canvas::Program::draw()` return

**Cache Strategy**:
```rust
let klines = chart.cache.main.draw(renderer, bounds_size, |frame| {
    // All rendering from steps 9-14
    // - NPoC lines
    // - Clusters (bars + text)
    // - Imbalance markers
    // - Candles
    // - Last price line
});

let crosshair = chart.cache.crosshair.draw(renderer, bounds_size, |frame| {
    // Crosshair lines + tooltip
});

vec![klines, crosshair]  // Return two geometry layers
```

**Invalidation**:
- `main` cache: Cleared on data change, scroll, zoom, resize
- `crosshair` cache: Cleared on cursor move

---

### 16. Configuration Updates (User Interaction)

**File**: `src/modal/pane/settings.rs`

**User Changes Cluster Type**:
```
User clicks "Delta Profile" button
  ↓
Message::PaneSettings(PaneSettingsMessage::SetClusterKind(ClusterKind::DeltaProfile))
  ↓
pane::State::update(message)
  ↓
if let PaneContent::KlineChart { state: ChartState::Ready(chart), kind, .. } = &mut self.content {
    chart.set_cluster_kind(ClusterKind::DeltaProfile);
    *kind = chart.kind().clone();  // Update pane state
}
  ↓
KlineChart::set_cluster_kind(new_kind)
  ↓
if let KlineChartKind::Footprint { ref mut clusters, .. } = self.kind {
    *clusters = new_kind;  // ClusterKind::DeltaProfile
}
self.invalidate(None);  // Trigger re-render
```

**User Adds Imbalance Study**:
```
User toggles "Imbalance" checkbox
  ↓
Message::PaneSettings(PaneSettingsMessage::StudyConfigurator(study::Message::ToggleStudy(FootprintStudy::Imbalance { ... })))
  ↓
KlineChart::update_study_configurator(message)
  ↓
match self.study_configurator.update(message) {
    Some(study::Action::ToggleStudy(study, is_selected)) => {
        if is_selected {
            if let KlineChartKind::Footprint { ref mut studies, .. } = self.kind {
                studies.push(FootprintStudy::Imbalance {
                    threshold: 200,
                    color_scale: Some(400),
                    ignore_zeros: true,
                });
            }
        } else {
            studies.retain(|s| !s.is_same_type(&study));
        }
    }
}
self.invalidate(None);  // Re-render with imbalance markers
```

**User Adjusts Cluster Scaling**:
```
User selects "Hybrid (0.2)" scaling
  ↓
Message::PaneSettings(PaneSettingsMessage::SetClusterScaling(ClusterScaling::Hybrid { weight: 0.2 }))
  ↓
KlineChart::set_cluster_scaling(new_scaling)
  ↓
if let KlineChartKind::Footprint { ref mut scaling, .. } = self.kind {
    *scaling = ClusterScaling::Hybrid { weight: 0.2 };
}
self.invalidate(None);
  ↓
// Next render uses new scaling in effective_cluster_qty()
```

---

### 17. Complete Data Flow Summary

```
┌─────────────────────────────────────────────────────────────────┐
│ APPLICATION ENTRY (main.rs)                                     │
├─────────────────────────────────────────────────────────────────┤
│ FlowSurface::new(State)                                         │
│   → Dashboard::new()                                            │
│     → pane_grid::State<pane::State>                             │
└─────────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│ PANE CREATION (dashboard/pane.rs)                               │
├─────────────────────────────────────────────────────────────────┤
│ User creates footprint pane                                     │
│ PaneContent::KlineChart {                                       │
│   state: ChartState::Loading,                                   │
│   kind: KlineChartKind::Footprint {                             │
│     clusters: BidAsk,                                           │
│     scaling: VisibleRange,                                      │
│     studies: vec![NPoC { lookback: 80 }]                        │
│   }                                                             │
│ }                                                               │
└─────────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│ DATA FETCHING (exchange/fetcher.rs)                             │
├─────────────────────────────────────────────────────────────────┤
│ Task::perform(fetch_klines) → Vec<Kline>                        │
│ Task::perform(fetch_trades) → Vec<Trade>                        │
│                                                                 │
│ Kline { time, open, high, low, close, volume, ... }            │
│ Trade { price, qty, time, is_sell }                            │
└─────────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│ CHART INITIALIZATION (chart/kline.rs)                           │
├─────────────────────────────────────────────────────────────────┤
│ KlineChart::new(...)                                            │
│   → TimeSeries::new(interval, step, trades, klines)             │
│     → BTreeMap<u64, KlineDataPoint>                             │
└─────────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│ TRADE AGGREGATION (data/aggr/time.rs)                           │
├─────────────────────────────────────────────────────────────────┤
│ TimeSeries::insert_trades(trades)                               │
│   for trade in trades:                                          │
│     interval = (trade.time / interval_ms) * interval_ms         │
│     dp = datapoints.entry(interval).or_insert(...)              │
│     dp.add_trade(trade, step)                                   │
│       → KlineTrades::add_trade_to_nearest_bin(trade, step)      │
│         price = trade.price.round_to_step(step)                 │
│         trades.entry(price)                                     │
│           .and_modify(|g| g.add_trade(trade))                   │
│           .or_insert(GroupedTrades::new(trade))                 │
│                                                                 │
│ Result per candle:                                              │
│ KlineDataPoint {                                                │
│   kline: Kline { ... },                                         │
│   footprint: KlineTrades {                                      │
│     trades: FxHashMap {                                         │
│       Price(50000) → GroupedTrades { buy_qty: 2.5, sell: 1.2 } │
│       Price(50010) → GroupedTrades { buy_qty: 0.8, sell: 3.1 } │
│       Price(50020) → GroupedTrades { buy_qty: 5.2, sell: 0.3 } │
│     }                                                           │
│   }                                                             │
│ }                                                               │
└─────────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│ POC CALCULATION (data/chart/kline.rs)                           │
├─────────────────────────────────────────────────────────────────┤
│ KlineTrades::calculate_poc()                                    │
│   max_volume = 0.0                                              │
│   for (price, group) in trades:                                 │
│     total = group.buy_qty + group.sell_qty                      │
│     if total > max_volume:                                      │
│       max_volume = total                                        │
│       poc_price = price                                         │
│                                                                 │
│   self.poc = Some(PointOfControl {                              │
│     price: Price(50020),                                        │
│     volume: 5.5,                                                │
│     status: NPoc::None                                          │
│   })                                                            │
└─────────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│ LIVE UPDATES (main.rs → dashboard.rs)                           │
├─────────────────────────────────────────────────────────────────┤
│ WebSocket: Event::DepthReceived(stream, depth, trades)          │
│   → Message::MarketWsEvent(event)                               │
│     → Dashboard::update_depth_and_trades(event)                 │
│       → KlineChart::insert_trades_buffer(trades)                │
│         → TimeSeries::insert_trades(trades)                     │
│         → [Same aggregation logic as initialization]            │
│         → invalidate() → trigger re-render                      │
└─────────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│ RENDERING SETUP (chart/kline.rs)                                │
├─────────────────────────────────────────────────────────────────┤
│ canvas::Program::draw()                                         │
│   cache.main.draw(|frame| {                                     │
│     frame.translate(center)                                     │
│     frame.scale(scaling)                                        │
│     frame.translate(translation)                                │
│                                                                 │
│     region = visible_region(bounds)                             │
│     (earliest, latest) = interval_range(region)                 │
│                                                                 │
│     price_to_y = |p| (base_price - p) / tick_size * cell_height│
│     interval_to_x = |i| -(i_diff / cell_width)                  │
└─────────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│ CLUSTER SCALING (chart/kline.rs)                                │
├─────────────────────────────────────────────────────────────────┤
│ max_cluster_qty = calc_qty_scales(earliest, latest, ...)        │
│   → TimeSeries::max_qty_ts_range(cluster_kind, ...)            │
│     for dp in datapoints.range(earliest..=latest):              │
│       candle_max = dp.max_cluster_qty(cluster_kind, ...)        │
│         → KlineTrades::max_qty_by(|buy, sell| ...)              │
│       max_qty = max(max_qty, candle_max)                        │
│     return max_qty  // e.g., 15.8 BTC                           │
└─────────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│ NPOC RENDERING (chart/kline.rs)                                 │
├─────────────────────────────────────────────────────────────────┤
│ draw_all_npocs(data_source, studies, ...)                       │
│   timeseries.datapoints.iter().rev().take(lookback)             │
│     .filter_map(|(t, dp)| dp.footprint.poc.map(|poc| (t, poc)))│
│     .for_each(|(interval, poc)| {                               │
│       match poc.status {                                        │
│         NPoc::Naked => draw_line(start → current, yellow)       │
│         NPoc::Filled{at} → draw_line(start → at, gray)          │
│       }                                                         │
│     })                                                          │
└─────────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│ CLUSTER ITERATION (chart/kline.rs)                              │
├─────────────────────────────────────────────────────────────────┤
│ render_data_source(data_source, earliest, latest, |frame, x, k, fp| {│
│   timeseries.datapoints.range(earliest..=latest)                │
│     .for_each(|(timestamp, dp)| {                               │
│       x_position = interval_to_x(timestamp)                     │
│                                                                 │
│       cluster_scaling = effective_cluster_qty(                  │
│         scaling,        // VisibleRange | Hybrid | Datapoint    │
│         max_cluster_qty, // 15.8                                │
│         dp.footprint,                                           │
│         cluster_kind                                            │
│       )                                                         │
│       // Returns: 15.8 (VisibleRange) or 5.5 (Datapoint)        │
│       // or 7.56 (Hybrid 0.2)                                   │
│                                                                 │
│       draw_clusters(frame, x_position, cluster_scaling, ...)    │
│     })                                                          │
│ })                                                              │
└─────────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│ CLUSTER DRAWING (chart/kline.rs)                                │
├─────────────────────────────────────────────────────────────────┤
│ draw_clusters(footprint, cluster_kind, ...)                     │
│                                                                 │
│ match cluster_kind {                                            │
│   BidAsk => {                                                   │
│     area = BidAskArea::new(...)                                 │
│     for (price, group) in footprint.trades {                    │
│       y = price_to_y(price)                                     │
│                                                                 │
│       // Right side (buy)                                       │
│       bar_width = (group.buy_qty / max_cluster_qty) * area_width│
│       frame.fill_rectangle(..., green)                          │
│       if show_text: draw_text("2.5")                            │
│                                                                 │
│       // Left side (sell)                                       │
│       bar_width = (group.sell_qty / max_cluster_qty) * area_width│
│       frame.fill_rectangle(..., red)                            │
│       if show_text: draw_text("1.2")                            │
│                                                                 │
│       // Imbalance markers                                      │
│       if has_imbalance_study:                                   │
│         draw_imbalance_markers(...)                             │
│     }                                                           │
│     draw_footprint_kline(frame, area.candle_center_x, ...)      │
│   }                                                             │
│                                                                 │
│   VolumeProfile => {                                            │
│     area = ProfileArea::new(...)                                │
│     for (price, group) in footprint.trades {                    │
│       y = price_to_y(price)                                     │
│       draw_volume_bar(buy_qty, sell_qty, ...)  // Stacked       │
│       if show_text: draw_text("3.7")  // Total                  │
│       if has_imbalance: draw_imbalance_markers(...)             │
│     }                                                           │
│     draw_footprint_kline(...)                                   │
│   }                                                             │
│                                                                 │
│   DeltaProfile => {                                             │
│     area = ProfileArea::new(...)                                │
│     for (price, group) in footprint.trades {                    │
│       delta = group.buy_qty - group.sell_qty                    │
│       bar_width = (delta.abs() / max_cluster_qty) * area_width  │
│       color = if delta >= 0 { green } else { red }             │
│       frame.fill_rectangle(..., color)                          │
│       if show_text: draw_text("+1.3" or "-0.5")                 │
│       if has_imbalance: draw_imbalance_markers(...)             │
│     }                                                           │
│     draw_footprint_kline(...)                                   │
│   }                                                             │
│ }                                                               │
└─────────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│ IMBALANCE DETECTION (chart/kline.rs)                            │
├─────────────────────────────────────────────────────────────────┤
│ draw_imbalance_markers(price, sell_qty, higher_price, threshold)│
│                                                                 │
│ diagonal_buy = footprint.trades.get(higher_price).buy_qty       │
│                                                                 │
│ if diagonal_buy > sell_qty * (1 + threshold/100):               │
│   // BUY IMBALANCE                                              │
│   // Current: 50020 SELL: 1.2                                   │
│   // Higher:  50030 BUY:  3.8                                   │
│   // 3.8 > 1.2 * 3.0 = 3.6 ✓                                    │
│   alpha = calculate_alpha_from_ratio(...)                       │
│   frame.fill_rectangle(buyside_x, y_higher, green.alpha)        │
│                                                                 │
│ else if sell_qty > diagonal_buy * (1 + threshold/100):          │
│   // SELL IMBALANCE                                             │
│   frame.fill_rectangle(sellside_x, y_current, red.alpha)        │
└─────────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│ CANDLE DRAWING (chart/kline.rs)                                 │
├─────────────────────────────────────────────────────────────────┤
│ draw_footprint_kline(kline, candle_center_x, ...)               │
│   y_open = price_to_y(kline.open)                               │
│   y_close = price_to_y(kline.close)                             │
│   y_high = price_to_y(kline.high)                               │
│   y_low = price_to_y(kline.low)                                 │
│                                                                 │
│   // Thin body (1/4 of candle_width)                            │
│   frame.fill_rectangle(body, color)                             │
│                                                                 │
│   // Very thin wick (1px stroke)                                │
│   frame.stroke(Path::line(high, low), color.alpha(0.6))         │
└─────────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│ CROSSHAIR & TOOLTIP (chart/kline.rs)                            │
├─────────────────────────────────────────────────────────────────┤
│ cache.crosshair.draw(|frame| {                                  │
│   if cursor_in_bounds:                                          │
│     (_, rounded_interval) = draw_crosshair(cursor_pos)          │
│     draw_crosshair_tooltip(data, rounded_interval)              │
│       kline = data.find(rounded_interval)                       │
│       frame.fill_rectangle(tooltip_bg)                          │
│       frame.fill_text("O 50000  H 50200  L 49900  C 50100 +0.2%")│
│ })                                                              │
└─────────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│ FINAL OUTPUT (chart/kline.rs)                                   │
├─────────────────────────────────────────────────────────────────┤
│ vec![klines_geometry, crosshair_geometry]                       │
│   ↓                                                             │
│ iced renders to GPU via wgpu                                    │
│   ↓                                                             │
│ User sees footprint chart on screen!                            │
└─────────────────────────────────────────────────────────────────┘
```

---

### 18. Key Data Structures Flow

**At Each Stage**:

1. **WebSocket** → `Vec<Trade>`
2. **Fetcher** → `Vec<Kline>`, `Vec<Trade>`
3. **TimeSeries** → `BTreeMap<u64, KlineDataPoint>`
4. **KlineDataPoint** → `{ Kline, KlineTrades }`
5. **KlineTrades** → `FxHashMap<Price, GroupedTrades>`, `Option<PointOfControl>`
6. **GroupedTrades** → `{ buy_qty, sell_qty, buy_count, sell_count, first_time, last_time }`
7. **Rendering** → Canvas geometries (rectangles, text, lines)

**Configuration Flow**:

1. **User Input** → `Message::PaneSettings`
2. **Settings Modal** → `KlineChart::set_cluster_kind()` / `set_cluster_scaling()` / `update_study_configurator()`
3. **Chart State** → `KlineChartKind::Footprint { clusters, scaling, studies }`
4. **Invalidation** → Clears caches
5. **Re-render** → Uses new configuration in `draw_clusters()`, `draw_imbalance_markers()`, `draw_all_npocs()`

---

## Conclusion

The footprint chart implementation follows a clean data flow:

1. **Data ingestion** (WebSocket/HTTP → Trade/Kline structs)
2. **Aggregation** (TimeSeries bins trades by price using nearest-step rounding)
3. **Analysis** (POC calculation, imbalance detection)
4. **Rendering** (Canvas geometry generation with cluster scaling)
5. **Interaction** (User configuration updates trigger re-render)

The architecture cleanly separates:
- **Data layer** (`data/` crate): Pure business logic, no rendering
- **UI layer** (`src/` crate): Rendering, interaction, state management
- **Exchange layer** (`exchange/` crate): Market data abstractions

This enables the footprint chart to handle real-time updates efficiently while maintaining visual accuracy and responsiveness.
