# Adding a Custom Indicator to Flowsurface

This guide provides step-by-step instructions for adding a new custom indicator to the Flowsurface charting system.

## Overview

Indicators in Flowsurface follow a consistent architecture:
- **Enum definition** in the data layer for type safety
- **Trait implementations** for market filtering and display
- **Implementation module** with data processing and rendering logic
- **Plot configuration** for visualization (bar charts, line charts, etc.)
- **UI integration** for toggling and reordering

## Architecture Reference

### Existing Indicators

**Kline Indicators** (data/src/chart/indicator.rs:14-44):
- `Volume` - Available for Spot + Perps
- `OpenInterest` - Available for Perps only

**Heatmap Indicators** (data/src/chart/indicator.rs:46-75):
- `Volume` - Available for Spot + Perps

---

## Step-by-Step Implementation

### Step 1: Add Enum Variant

**File:** `data/src/chart/indicator.rs`

Choose the appropriate indicator type (Kline or Heatmap) and add your variant:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize, Eq, Enum)]
pub enum KlineIndicator {
    Volume,
    OpenInterest,
    YourNewIndicator,  // ← Add your variant here
}
```

**Key derives required:**
- `Debug, Clone, Copy, PartialEq` - Basic traits
- `Deserialize, Serialize` - For persistence
- `Eq` - For equality comparisons
- `Enum` - From enum_map crate for efficient storage

### Step 2: Update Market Availability

In the same file, add your indicator to the appropriate market constant arrays:

```rust
impl KlineIndicator {
    // Indicators that can be used with spot market tickers
    const FOR_SPOT: [KlineIndicator; 2] = [
        KlineIndicator::Volume,
        KlineIndicator::YourNewIndicator,  // ← If available for spot
    ];

    // Indicators that can be used with perpetual swap market tickers
    const FOR_PERPS: [KlineIndicator; 3] = [
        KlineIndicator::Volume,
        KlineIndicator::OpenInterest,
        KlineIndicator::YourNewIndicator,  // ← If available for perps
    ];
}
```

**Important:** Every variant must appear in at least one market array, or it won't be accessible in the UI.

### Step 3: Implement Display Trait

Add display name for your indicator:

```rust
impl Display for KlineIndicator {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            KlineIndicator::Volume => write!(f, "Volume"),
            KlineIndicator::OpenInterest => write!(f, "Open Interest"),
            KlineIndicator::YourNewIndicator => write!(f, "Your Display Name"),
        }
    }
}
```

### Step 4: Create Implementation Module

**File:** `src/chart/indicator/kline/your_indicator.rs`

Create a new module implementing the `KlineIndicatorImpl` trait:

```rust
use crate::chart::{
    Caches, Message, ViewState,
    indicator::{
        indicator_row,
        kline::KlineIndicatorImpl,
        plot::{PlotTooltip, line::LinePlot},  // or bar::BarPlot
    },
};
use data::chart::{PlotData, kline::KlineDataPoint};
use data::util::format_with_commas;
use exchange::Kline;
use std::collections::BTreeMap;

pub struct YourIndicator {
    cache: Caches,
    pub data: BTreeMap<u64, f32>,  // timestamp -> value mapping
}

impl YourIndicator {
    pub fn new() -> Self {
        Self {
            cache: Caches::default(),
            data: BTreeMap::new(),
        }
    }

    fn indicator_elem<'a>(
        &'a self,
        main_chart: &'a ViewState,
        visible_range: std::ops::RangeInclusive<u64>,
    ) -> iced::Element<'a, Message> {
        // Define tooltip function
        let tooltip = |value: &f32, _next: Option<&f32>| {
            PlotTooltip::new(format!("Your Value: {}", format_with_commas(*value)))
        };

        // Define value extraction function
        let value_fn = |v: &f32| *v;

        // Configure plot (line or bar)
        let plot = LinePlot::new(value_fn)
            .stroke_width(1.0)
            .show_points(true)
            .point_radius_factor(0.2)
            .padding(0.08)
            .with_tooltip(tooltip);

        indicator_row(main_chart, &self.cache, plot, &self.data, visible_range)
    }
}

impl KlineIndicatorImpl for YourIndicator {
    fn clear_all_caches(&mut self) {
        self.cache.clear_all();
    }

    fn clear_crosshair_caches(&mut self) {
        self.cache.clear_crosshair();
    }

    fn element<'a>(
        &'a self,
        chart: &'a ViewState,
        visible_range: std::ops::RangeInclusive<u64>,
    ) -> iced::Element<'a, Message> {
        self.indicator_elem(chart, visible_range)
    }

    fn rebuild_from_source(&mut self, source: &PlotData<KlineDataPoint>) {
        // Rebuild data from scratch
        self.data.clear();
        match source {
            PlotData::TimeBased(timeseries) => {
                for (time, dp) in &timeseries.datapoints {
                    // Calculate your indicator value from kline data
                    let value = dp.kline.close; // example
                    self.data.insert(*time, value);
                }
            }
            PlotData::TickBased(tickseries) => {
                for (idx, dp) in tickseries.datapoints.iter().enumerate() {
                    let value = dp.kline.close; // example
                    self.data.insert(idx as u64, value);
                }
            }
        }
        self.clear_all_caches();
    }

    fn on_insert_klines(&mut self, klines: &[Kline]) {
        // Handle new kline data incrementally
        for kline in klines {
            let value = kline.close; // example calculation
            self.data.insert(kline.time, value);
        }
        self.clear_all_caches();
    }

    // Optional: Implement if you need external data fetching
    fn fetch_range(&mut self, _ctx: &FetchCtx) -> Option<FetchRange> {
        None
    }

    // Optional: Handle trade updates for tick charts
    fn on_insert_trades(&mut self, _trades: &[Trade], _old_dp_len: usize, _source: &PlotData<KlineDataPoint>) {}

    // Optional: Handle tick size changes
    fn on_ticksize_change(&mut self, source: &PlotData<KlineDataPoint>) {
        self.rebuild_from_source(source);
    }

    // Optional: Handle basis (timeframe/tick) changes
    fn on_basis_change(&mut self, source: &PlotData<KlineDataPoint>) {
        self.rebuild_from_source(source);
    }
}
```

### Step 5: Register in Module Tree

**File:** `src/chart/indicator/kline.rs`

Add your module declaration and update the factory function:

```rust
pub mod volume;
pub mod open_interest;
pub mod your_indicator;  // ← Add module declaration

pub fn make_empty(which: KlineIndicator) -> Box<dyn KlineIndicatorImpl> {
    match which {
        KlineIndicator::Volume => Box::new(super::kline::volume::VolumeIndicator::new()),
        KlineIndicator::OpenInterest => {
            Box::new(super::kline::open_interest::OpenInterestIndicator::new())
        }
        KlineIndicator::YourNewIndicator => {
            Box::new(super::kline::your_indicator::YourIndicator::new())
        }
    }
}
```

**Location:** src/chart/indicator/kline.rs:60-67

---

## Plot Types

### Bar Plot (Volume-style)

Best for: Volume, delta indicators, discrete quantities

```rust
use crate::chart::indicator::plot::bar::{BarClass, BarPlot};

let bar_kind = |&value: &f32| BarClass::Single;
let value_fn = |&value: &f32| value;

let plot = BarPlot::new(value_fn, bar_kind)
    .bar_width_factor(0.9)
    .with_tooltip(tooltip);
```

**Bar Types:**
- `BarClass::Single` - Single colored bar (uses secondary.strong color)
- `BarClass::Overlay { overlay: f32 }` - Dual-layer bar with signed overlay (success/danger colors)

**Configuration options:**
- `.bar_width_factor(0.9)` - Bar width as % of cell width (0.0-1.0)
- `.padding(0.1)` - Extra vertical space as % of range
- `.baseline(Baseline::Zero)` - How bars are anchored (Zero, Min, Fixed)

### Line Plot (Open Interest-style)

Best for: Continuous data, price-like indicators, trends

```rust
use crate::chart::indicator::plot::line::LinePlot;

let value_fn = |v: &f32| *v;

let plot = LinePlot::new(value_fn)
    .stroke_width(1.0)
    .show_points(true)
    .point_radius_factor(0.2)
    .padding(0.08)
    .with_tooltip(tooltip);
```

**Configuration options:**
- `.stroke_width(1.0)` - Line thickness in pixels
- `.show_points(true)` - Draw circles at datapoints
- `.point_radius_factor(0.2)` - Circle size as % of cell width
- `.padding(0.08)` - Vertical padding as % of range

---

## Data Structures

### Data Storage

Use `BTreeMap<u64, T>` for indicator data:

```rust
pub struct YourIndicator {
    cache: Caches,
    pub data: BTreeMap<u64, YourDataType>,
}
```

**Key types:**
- `u64` - Timestamp (milliseconds) for time-based charts
- `u64` - Index for tick-based charts

**Value types:**
- `f32` - Single value indicators (volume, price)
- `(f32, f32)` - Dual value indicators (buy/sell split)
- Custom structs - Complex multi-part data

### Cache Management

Every indicator must maintain render caches:

```rust
use crate::chart::Caches;

pub struct YourIndicator {
    cache: Caches,  // Handles main chart + crosshair caching
    // ... other fields
}

impl KlineIndicatorImpl for YourIndicator {
    fn clear_all_caches(&mut self) {
        self.cache.clear_all();
    }

    fn clear_crosshair_caches(&mut self) {
        self.cache.clear_crosshair();
    }
}
```

**When to clear:**
- `clear_all_caches()` - Data changed (new klines, trades, or calculations)
- `clear_crosshair_caches()` - Cursor moved (tooltip/label updates)

---

## Data Update Lifecycle

### Required Methods

#### `rebuild_from_source`
Called when chart needs complete rebuild (basis change, timeframe switch):

```rust
fn rebuild_from_source(&mut self, source: &PlotData<KlineDataPoint>) {
    self.data.clear();
    match source {
        PlotData::TimeBased(timeseries) => {
            // Process time-based data
            for (time, dp) in &timeseries.datapoints {
                let value = self.calculate_value(dp);
                self.data.insert(*time, value);
            }
        }
        PlotData::TickBased(tickseries) => {
            // Process tick-based data
            for (idx, dp) in tickseries.datapoints.iter().enumerate() {
                let value = self.calculate_value(dp);
                self.data.insert(idx as u64, value);
            }
        }
    }
    self.clear_all_caches();
}
```

#### `on_insert_klines`
Called when new klines arrive from the exchange:

```rust
fn on_insert_klines(&mut self, klines: &[Kline]) {
    for kline in klines {
        let value = self.calculate_value_from_kline(kline);
        self.data.insert(kline.time, value);
    }
    self.clear_all_caches();
}
```

### Optional Methods

#### `fetch_range`
Implement if indicator needs external data (like Open Interest):

```rust
fn fetch_range(&mut self, ctx: &FetchCtx) -> Option<FetchRange> {
    // Check if data is missing in visible range
    let (earliest, latest) = self.data_timerange(ctx.kline_latest);

    if ctx.visible_earliest < earliest {
        return Some(FetchRange::YourDataType(ctx.prefetch_earliest, earliest));
    }

    None
}
```

#### `on_insert_trades`
For indicators that update from trade data (tick charts):

```rust
fn on_insert_trades(&mut self, trades: &[Trade], old_dp_len: usize, source: &PlotData<KlineDataPoint>) {
    match source {
        PlotData::TickBased(tickseries) => {
            let start_idx = old_dp_len.saturating_sub(1);
            for (idx, dp) in tickseries.datapoints.iter().enumerate().skip(start_idx) {
                let value = self.calculate_value(dp);
                self.data.insert(idx as u64, value);
            }
        }
        _ => {}
    }
    self.clear_all_caches();
}
```

#### `on_ticksize_change` / `on_basis_change`
Usually delegates to `rebuild_from_source`:

```rust
fn on_ticksize_change(&mut self, source: &PlotData<KlineDataPoint>) {
    self.rebuild_from_source(source);
}

fn on_basis_change(&mut self, source: &PlotData<KlineDataPoint>) {
    self.rebuild_from_source(source);
}
```

---

## UI Integration

UI integration is **automatic** once you complete Steps 1-5. The indicator will:

1. **Appear in the Indicators modal** (src/modal/pane/indicators.rs:11-150)
   - Filtered by market type (Spot vs Perps)
   - Toggle on/off via button click
   - Reorderable via drag-and-drop

2. **Toggle flow** (src/screen/dashboard/pane.rs:1285-1323):
   ```
   Button Click → Message::ToggleIndicator(pane, indicator)
                → pane.content.toggle_indicator(indicator)
                → chart.toggle_indicator(indicator)
                → Creates/destroys Box<dyn KlineIndicatorImpl>
   ```

3. **Chart rendering** (src/chart/kline.rs:897-916):
   - Indicators stored in `EnumMap<KlineIndicator, Option<Box<dyn KlineIndicatorImpl>>>`
   - Toggling creates new instance via `make_empty()` factory
   - Panel splits automatically adjust for indicator count

---

## Advanced Features

### Custom Tooltips

Tooltips support multi-line text and next-value comparisons:

```rust
let tooltip = |value: &f32, next: Option<&f32>| {
    let current = format!("Value: {}", format_with_commas(*value));

    let change = if let Some(next_value) = next {
        let delta = next_value - *value;
        let sign = if delta >= 0.0 { "+" } else { "" };
        format!("Change: {}{}", sign, format_with_commas(delta))
    } else {
        "Change: N/A".to_string()
    };

    PlotTooltip::new(format!("{}\n{}", current, change))
};
```

### Conditional Rendering

Show different UI based on chart state:

```rust
fn indicator_elem<'a>(&'a self, main_chart: &'a ViewState, visible_range: RangeInclusive<u64>) -> iced::Element<'a, Message> {
    match main_chart.basis {
        Basis::Time(timeframe) => {
            if !self.is_supported_timeframe(timeframe) {
                return center(text("Not available for this timeframe")).into();
            }
            // ... normal rendering
        }
        Basis::Tick(_) => {
            return center(text("Not available for tick charts")).into();
        }
    }

    // ... render plot
}
```

See `src/chart/indicator/kline/open_interest.rs:31-61` for complete example.

### External Data Fetching

For indicators requiring external APIs (like Open Interest):

```rust
impl KlineIndicatorImpl for YourIndicator {
    fn fetch_range(&mut self, ctx: &FetchCtx) -> Option<FetchRange> {
        // 1. Check if indicator is supported
        if !self.is_supported(ctx.main_chart) {
            return None;
        }

        // 2. Find gaps in data
        let (data_earliest, data_latest) = self.data_timerange(ctx.kline_latest);

        // 3. Request missing data
        if ctx.visible_earliest < data_earliest {
            return Some(FetchRange::YourDataType(ctx.prefetch_earliest, data_earliest));
        }

        if data_latest < ctx.kline_latest {
            return Some(FetchRange::YourDataType(data_latest, ctx.kline_latest));
        }

        None
    }

    fn on_your_data_received(&mut self, data: &[YourDataType]) {
        self.data.extend(data.iter().map(|d| (d.time, d.value)));
        self.clear_all_caches();
    }
}
```

---

## Testing Checklist

After implementing your indicator:

- [ ] **Enum variant** added with all required derives
- [ ] **Market arrays** updated (FOR_SPOT, FOR_PERPS, or both)
- [ ] **Display trait** implemented with user-friendly name
- [ ] **Implementation module** created with all lifecycle methods
- [ ] **Factory function** updated in make_empty()
- [ ] **Compiles** without errors
- [ ] **Appears in UI** Indicators modal
- [ ] **Toggles on/off** without crashes
- [ ] **Renders correctly** with appropriate plot type
- [ ] **Tooltips work** on hover
- [ ] **Reorderable** via drag-and-drop (if multiple indicators)
- [ ] **Survives basis changes** (time ↔ tick)
- [ ] **Survives timeframe changes** (M1, M5, H1, etc.)
- [ ] **Data updates** on new klines/trades
- [ ] **Caches clear** appropriately
- [ ] **Market filtering** works (shows only for Spot/Perps as configured)

---

## Quick Reference: File Locations

| Step | File | Lines | Purpose |
|------|------|-------|---------|
| 1 | data/src/chart/indicator.rs | 14-44 | Enum definition |
| 2 | data/src/chart/indicator.rs | 28-35 | Market availability |
| 3 | data/src/chart/indicator.rs | 37-44 | Display name |
| 4 | src/chart/indicator/kline/your_indicator.rs | New file | Implementation |
| 5 | src/chart/indicator/kline.rs | 60-67 | Factory registration |

**Plot types:**
- Bar: src/chart/indicator/plot/bar.rs
- Line: src/chart/indicator/plot/line.rs

**Plot trait:** src/chart/indicator/plot.rs:150-172

**UI rendering:** src/chart/indicator/mod.rs:24-68 (indicator_row function)

**Toggle logic:**
- Pane level: src/screen/dashboard/pane.rs:1285-1323
- Chart level: src/chart/kline.rs:897-916

---

## Common Issues

### Issue: Indicator doesn't appear in UI
**Solution:** Ensure variant is added to FOR_SPOT or FOR_PERPS arrays

### Issue: Crash on toggle
**Solution:** Check `make_empty()` function includes your variant

### Issue: Empty chart rendering
**Solution:** Verify `rebuild_from_source()` is populating data correctly

### Issue: Outdated data
**Solution:** Ensure `on_insert_klines()` is implemented and clearing caches

### Issue: Tooltip not showing
**Solution:** Check `.with_tooltip()` is called on plot configuration

### Issue: Wrong market type shown
**Solution:** Verify indicator is in correct market array (FOR_SPOT vs FOR_PERPS)

---

## Example: RSI Indicator (Hypothetical)

Here's how you'd implement a 14-period RSI:

```rust
// Step 1-3: In data/src/chart/indicator.rs
pub enum KlineIndicator {
    Volume,
    OpenInterest,
    RSI,  // ← Add
}

const FOR_SPOT: [KlineIndicator; 2] = [KlineIndicator::Volume, KlineIndicator::RSI];
const FOR_PERPS: [KlineIndicator; 3] = [KlineIndicator::Volume, KlineIndicator::OpenInterest, KlineIndicator::RSI];

impl Display for KlineIndicator {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            KlineIndicator::RSI => write!(f, "RSI (14)"),
            // ...
        }
    }
}

// Step 4: src/chart/indicator/kline/rsi.rs
pub struct RSIIndicator {
    cache: Caches,
    data: BTreeMap<u64, f32>,
    period: usize,
}

impl RSIIndicator {
    pub fn new() -> Self {
        Self {
            cache: Caches::default(),
            data: BTreeMap::new(),
            period: 14,
        }
    }

    fn calculate_rsi(&self, prices: &[f32]) -> f32 {
        if prices.len() < self.period + 1 {
            return 50.0; // neutral
        }

        let mut gains = 0.0;
        let mut losses = 0.0;

        for i in 1..=self.period {
            let change = prices[i] - prices[i-1];
            if change > 0.0 {
                gains += change;
            } else {
                losses += change.abs();
            }
        }

        let avg_gain = gains / self.period as f32;
        let avg_loss = losses / self.period as f32;

        if avg_loss == 0.0 {
            return 100.0;
        }

        let rs = avg_gain / avg_loss;
        100.0 - (100.0 / (1.0 + rs))
    }

    fn indicator_elem<'a>(&'a self, main_chart: &'a ViewState, visible_range: std::ops::RangeInclusive<u64>) -> iced::Element<'a, Message> {
        let tooltip = |value: &f32, _: Option<&f32>| {
            let level = if *value > 70.0 {
                "Overbought"
            } else if *value < 30.0 {
                "Oversold"
            } else {
                "Neutral"
            };
            PlotTooltip::new(format!("RSI: {:.2} ({})", value, level))
        };

        let plot = LinePlot::new(|v: &f32| *v)
            .stroke_width(1.5)
            .show_points(false)
            .padding(0.05)
            .with_tooltip(tooltip);

        indicator_row(main_chart, &self.cache, plot, &self.data, visible_range)
    }
}

impl KlineIndicatorImpl for RSIIndicator {
    fn rebuild_from_source(&mut self, source: &PlotData<KlineDataPoint>) {
        self.data.clear();

        match source {
            PlotData::TimeBased(timeseries) => {
                let closes: Vec<f32> = timeseries.datapoints.values()
                    .map(|dp| dp.kline.close)
                    .collect();

                for (i, (time, _)) in timeseries.datapoints.iter().enumerate() {
                    if i >= self.period {
                        let rsi = self.calculate_rsi(&closes[i.saturating_sub(self.period)..=i]);
                        self.data.insert(*time, rsi);
                    }
                }
            }
            PlotData::TickBased(tickseries) => {
                let closes: Vec<f32> = tickseries.datapoints.iter()
                    .map(|dp| dp.kline.close)
                    .collect();

                for (i, _) in tickseries.datapoints.iter().enumerate() {
                    if i >= self.period {
                        let rsi = self.calculate_rsi(&closes[i.saturating_sub(self.period)..=i]);
                        self.data.insert(i as u64, rsi);
                    }
                }
            }
        }
        self.clear_all_caches();
    }

    fn on_insert_klines(&mut self, klines: &[Kline]) {
        // Simplified: full recalc recommended for rolling window indicators
        // In production, implement incremental update for efficiency
        for kline in klines {
            self.data.insert(kline.time, 50.0); // placeholder
        }
        self.clear_all_caches();
    }

    // ... standard trait implementations
}

// Step 5: src/chart/indicator/kline.rs
pub mod rsi;  // ← Add

pub fn make_empty(which: KlineIndicator) -> Box<dyn KlineIndicatorImpl> {
    match which {
        KlineIndicator::RSI => Box::new(super::kline::rsi::RSIIndicator::new()),
        // ...
    }
}
```

---

## Summary

Adding a custom indicator requires:
1. **Enum definition** with proper derives and market arrays
2. **Display implementation** for UI labels
3. **Module implementation** with KlineIndicatorImpl trait
4. **Plot configuration** (bar or line)
5. **Factory registration** in make_empty()

The system handles all UI integration, toggling, rendering, and lifecycle management automatically once these pieces are in place.
