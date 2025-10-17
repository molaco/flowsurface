# Plan: Implement Moving Average Indicator

## Overview

Implement a Simple Moving Average (SMA) indicator for FlowSurface that calculates the average closing price over a configurable period (default: 20). The indicator will be available for both spot and perpetual markets, rendered as a line plot overlay.

## Implementation Steps

### Step 1: Add Enum Variant
**File:** `data/src/chart/indicator.rs`

**Tasks:**
- [ ] Add `MovingAverage` variant to `KlineIndicator` enum (line 14)
- [ ] Ensure all required derives are present: `Debug, Clone, Copy, PartialEq, Deserialize, Serialize, Eq, Enum`

**Code:**
```rust
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize, Eq, Enum)]
pub enum KlineIndicator {
    Volume,
    OpenInterest,
    MovingAverage,  // ← Add this
}
```

### Step 2: Update Market Availability Arrays
**File:** `data/src/chart/indicator.rs`

**Tasks:**
- [ ] Add `MovingAverage` to `FOR_SPOT` array (line 32)
- [ ] Add `MovingAverage` to `FOR_PERPS` array (line 34)

**Code:**
```rust
impl KlineIndicator {
    const FOR_SPOT: [KlineIndicator; 2] = [
        KlineIndicator::Volume,
        KlineIndicator::MovingAverage,
    ];

    const FOR_PERPS: [KlineIndicator; 3] = [
        KlineIndicator::Volume,
        KlineIndicator::OpenInterest,
        KlineIndicator::MovingAverage,
    ];
}
```

### Step 3: Implement Display Trait
**File:** `data/src/chart/indicator.rs`

**Tasks:**
- [ ] Add display case for `MovingAverage` (line 38-44)
- [ ] Use user-friendly label: "SMA (20)"

**Code:**
```rust
impl Display for KlineIndicator {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            KlineIndicator::Volume => write!(f, "Volume"),
            KlineIndicator::OpenInterest => write!(f, "Open Interest"),
            KlineIndicator::MovingAverage => write!(f, "SMA (20)"),
        }
    }
}
```

### Step 4: Create Implementation Module
**File:** `src/chart/indicator/kline/moving_average.rs` (new file)

**Tasks:**
- [ ] Create module file structure
- [ ] Add required import statements
- [ ] Define `MovingAverageIndicator` struct
- [ ] Implement `new()` constructor with configurable period
- [ ] Implement private `calculate_sma()` helper function (optional, can inline)
- [ ] Implement private `indicator_elem()` for UI rendering
- [ ] Implement `KlineIndicatorImpl` trait with all required methods

**Required Imports:**
```rust
use crate::chart::{
    Caches, Message, ViewState,
    indicator::{
        indicator_row,
        kline::KlineIndicatorImpl,
        plot::{PlotTooltip, line::LinePlot},
    },
};
use data::chart::{PlotData, kline::KlineDataPoint};
use data::util::format_with_commas;
use exchange::{Kline, Trade};
use std::{collections::BTreeMap, ops::RangeInclusive};
```

**Structure:**
```rust
pub struct MovingAverageIndicator {
    cache: Caches,
    data: BTreeMap<u64, f32>,     // timestamp/index -> SMA value (for rendering)
    closes: BTreeMap<u64, f32>,   // timestamp/index -> close price (source data)
    period: usize,                 // default: 20
}
```

**Critical:** We must store **both** the calculated MA values AND the source close prices. Without source prices, we cannot calculate new MA values when incremental updates arrive (`on_insert_klines`/`on_insert_trades`).

**Constructor:**
```rust
impl MovingAverageIndicator {
    pub fn new() -> Self {
        Self {
            cache: Caches::default(),
            data: BTreeMap::new(),
            closes: BTreeMap::new(),
            period: 20,
        }
    }
}
```

**Key Methods:**
- `calculate_sma(prices: &[f32]) -> f32` - Calculate average of last N prices
- `indicator_elem()` - Configure LinePlot with tooltip
- `rebuild_from_source()` - Calculate SMA for all datapoints
- `on_insert_klines()` - Update SMA for new klines
- `on_insert_trades()` - Update SMA for tick charts

### Step 5: Implement Core Logic

#### 5.1 SMA Calculation Logic
**Algorithm:**
```
For each price point at index i:
  if i < period:
    skip (insufficient data)
  else:
    sma = sum(prices[i-period+1..=i]) / period
```

**Implementation Strategy:**
Use Rust standard library's `.windows()` method for rolling window calculations (zero dependencies, simple and efficient).

**Helper Function (optional):**
```rust
fn calculate_sma(&self, prices: &[f32]) -> Option<f32> {
    if prices.len() < self.period {
        return None;
    }

    let sum: f32 = prices.iter().take(self.period).sum();
    Some(sum / self.period as f32)
}
```

**Direct .windows() Approach (recommended):**
```rust
// For a Vec of prices, calculate all MAs at once
let mas: Vec<f32> = prices
    .windows(period)
    .map(|window| window.iter().sum::<f32>() / period as f32)
    .collect();
```

#### 5.2 Data Rebuild (Full Recalculation)
**Tasks:**
- [ ] Handle `PlotData::TimeBased` case
- [ ] Handle `PlotData::TickBased` case
- [ ] Use rolling window approach
- [ ] Populate both `closes` and `data` structures
- [ ] Clear caches after update

**Approach:**
1. Clear both `self.data` and `self.closes`
2. Extract all close prices from datapoints and store in `self.closes`
3. For each position >= period, calculate SMA from window
4. Insert calculated MA values into `self.data` BTreeMap
5. Clear all caches

**Implementation Pattern (Using stdlib .windows()):**
```rust
fn rebuild_from_source(&mut self, source: &PlotData<KlineDataPoint>) {
    self.data.clear();
    self.closes.clear();

    match source {
        PlotData::TimeBased(timeseries) => {
            // Collect all close prices with timestamps (ordered)
            let close_vec: Vec<(u64, f32)> = timeseries.datapoints
                .iter()
                .map(|(time, dp)| (*time, dp.kline.close))
                .collect();

            // Store all closes in BTreeMap for incremental updates
            for (time, close) in &close_vec {
                self.closes.insert(*time, *close);
            }

            // Use .windows() to calculate MAs efficiently
            if close_vec.len() >= self.period {
                for (i, window) in close_vec.windows(self.period).enumerate() {
                    let sum: f32 = window.iter().map(|(_, c)| c).sum();
                    let ma = sum / self.period as f32;
                    // Insert at the timestamp of the last point in the window
                    let timestamp = window[self.period - 1].0;
                    self.data.insert(timestamp, ma);
                }
            }
        }
        PlotData::TickBased(tickseries) => {
            // Collect all close prices (ordered by index)
            let closes: Vec<f32> = tickseries.datapoints
                .iter()
                .map(|dp| dp.kline.close)
                .collect();

            // Store all closes in BTreeMap
            for (idx, close) in closes.iter().enumerate() {
                self.closes.insert(idx as u64, *close);
            }

            // Use .windows() to calculate MAs efficiently
            if closes.len() >= self.period {
                for (i, window) in closes.windows(self.period).enumerate() {
                    let sum: f32 = window.iter().sum();
                    let ma = sum / self.period as f32;
                    // Index of the last point in the window
                    let idx = (i + self.period - 1) as u64;
                    self.data.insert(idx, ma);
                }
            }
        }
    }
    self.clear_all_caches();
}
```

**Key Improvements:**
- ✅ Uses stdlib `.windows()` - zero dependencies
- ✅ More efficient than manual indexing
- ✅ Clear and readable
- ✅ Works identically for both time-based and tick-based charts

#### 5.3 Incremental Updates
**Tasks:**
- [ ] Implement `on_insert_klines()` for new kline data
- [ ] Implement `on_insert_trades()` for tick chart updates
- [ ] Ensure efficient updates (don't recalculate entire series)

**Strategy for `on_insert_klines()`:**
```rust
fn on_insert_klines(&mut self, klines: &[Kline]) {
    for kline in klines {
        // 1. Store the close price
        self.closes.insert(kline.time, kline.close);

        // 2. Get last N close prices for this timestamp
        let closes_before: Vec<f32> = self.closes
            .range(..=kline.time)
            .rev()
            .take(self.period)
            .map(|(_, &price)| price)
            .collect();

        // 3. Calculate MA if we have enough data
        if closes_before.len() >= self.period {
            let sum: f32 = closes_before.iter().sum();
            let ma = sum / self.period as f32;
            self.data.insert(kline.time, ma);
        }
    }
    self.clear_all_caches();
}
```

**Strategy for `on_insert_trades()` (tick charts):**
```rust
fn on_insert_trades(&mut self, _trades: &[Trade], old_dp_len: usize, source: &PlotData<KlineDataPoint>) {
    match source {
        PlotData::TimeBased(_) => return,
        PlotData::TickBased(tickseries) => {
            let start_idx = old_dp_len.saturating_sub(1);
            for (idx, dp) in tickseries.datapoints.iter().enumerate().skip(start_idx) {
                let idx_u64 = idx as u64;

                // Store close price
                self.closes.insert(idx_u64, dp.kline.close);

                // Calculate MA if we have enough data
                if idx >= self.period - 1 {
                    let closes_window: Vec<f32> = (idx.saturating_sub(self.period - 1)..=idx)
                        .filter_map(|i| self.closes.get(&(i as u64)).copied())
                        .collect();

                    if closes_window.len() == self.period {
                        let sum: f32 = closes_window.iter().sum();
                        let ma = sum / self.period as f32;
                        self.data.insert(idx_u64, ma);
                    }
                }
            }
        }
    }
    self.clear_all_caches();
}
```

### Step 6: Configure Plot Rendering

**Tasks:**
- [ ] Use `LinePlot` (not BarPlot) for continuous line
- [ ] Configure stroke width: 1.5px for visibility
- [ ] Disable point markers (smooth line only)
- [ ] Add 5% padding for better visibility
- [ ] Create informative tooltip showing SMA value

**Configuration:**
```rust
let plot = LinePlot::new(|v: &f32| *v)
    .stroke_width(1.5)
    .show_points(false)
    .padding(0.05)
    .with_tooltip(tooltip);
```

**Tooltip Format:**
```
SMA (20): 45,123.45
```

### Step 7: Register in Factory
**File:** `src/chart/indicator/kline.rs`

**Tasks:**
- [ ] Add module declaration at top of file: `pub mod moving_average;` (around line 9-10)
- [ ] Add match arm in `make_empty()` function (line 60-67)

**Code:**
```rust
pub mod moving_average;

pub fn make_empty(which: KlineIndicator) -> Box<dyn KlineIndicatorImpl> {
    match which {
        KlineIndicator::Volume => Box::new(super::kline::volume::VolumeIndicator::new()),
        KlineIndicator::OpenInterest => {
            Box::new(super::kline::open_interest::OpenInterestIndicator::new())
        }
        KlineIndicator::MovingAverage => {
            Box::new(super::kline::moving_average::MovingAverageIndicator::new())
        }
    }
}
```

### Step 8: Testing & Validation

**Compilation:**
- [ ] Run `cargo build` - verify no compilation errors
- [ ] Run `cargo clippy` - address any warnings
- [ ] Run `cargo fmt` - ensure code formatting

**Functional Testing:**
- [ ] Launch application
- [ ] Verify indicator appears in UI menu
- [ ] Test toggle on/off functionality
- [ ] Verify line renders correctly on chart
- [ ] Check tooltip displays SMA value on hover
- [ ] Test with different timeframes (M1, M5, H1, D1)
- [ ] Test basis changes (time-based ↔ tick-based)
- [ ] Test with both spot and perpetual markets
- [ ] Verify drag-to-reorder works with multiple indicators
- [ ] Test data updates with new klines
- [ ] Verify performance with large datasets

**Edge Cases:**
- [ ] Test with insufficient data (< 20 periods)
- [ ] Test with empty chart
- [ ] Test rapid timeframe switching
- [ ] Test during live market data updates

### Step 9: Documentation (Optional)

**Tasks:**
- [ ] Add inline code comments for complex logic
- [ ] Document the period parameter
- [ ] Add usage examples to developer docs

## Technical Considerations

### Data Structure Design
```rust
pub struct MovingAverageIndicator {
    cache: Caches,                    // Rendering caches (main + crosshair)
    data: BTreeMap<u64, f32>,         // Calculated SMA values (for rendering)
    closes: BTreeMap<u64, f32>,       // Source close prices (for calculations)
    period: usize,                    // Rolling window size (default: 20)
}
```

**Why two BTreeMaps?**
- `data`: Sparse map containing only MA values where we have sufficient history (>= period)
- `closes`: Complete map of all close prices needed for rolling window calculations
- Incremental updates need access to historical prices, not just previous MA values

### Performance Optimization
- **V1 (Simple):** Full recalculation on every update
- **V2 (Optimized):** Maintain rolling sum for O(1) updates
- **Trade-off:** V1 is simpler and sufficient for typical use cases

### Cache Invalidation Strategy
- **Full cache clear:** On data changes (new klines, trades, basis changes)
- **Crosshair only:** Not applicable for this indicator
- **Trigger points:** `rebuild_from_source()`, `on_insert_klines()`, `on_insert_trades()`

### Handling Insufficient Data
When there are fewer than `period` price points available:
- **Do NOT** insert MA values into `self.data` for those points
- **DO** still store close prices in `self.closes`
- **Result:** MA line will only appear once sufficient history exists
- **User Experience:** Chart will be empty initially, MA appears after N periods loaded
- **No error messages needed:** This is expected behavior for rolling window indicators

### Rolling Window Implementation
```
Datapoints: [p0, p1, p2, ..., p19, p20, p21, ...]
Window:                    [----20----]
SMA at p20: avg(p1..p20)
SMA at p21: avg(p2..p21)
```

## Implementation Order

1. ✅ **Data Layer** (Steps 1-3): Add enum variant and display
2. ✅ **Logic Layer** (Steps 4-5): Implement calculation and lifecycle
3. ✅ **Rendering Layer** (Step 6): Configure plot visualization
4. ✅ **Integration** (Step 7): Register in factory
5. ✅ **Validation** (Step 8): Test all functionality

## Success Criteria

- [ ] Indicator compiles without errors or warnings
- [ ] Appears in UI menu for both spot and perps markets
- [ ] Toggles on/off successfully
- [ ] Renders as smooth line on chart
- [ ] Tooltip shows correct SMA value
- [ ] Updates correctly with new market data
- [ ] Survives timeframe and basis changes
- [ ] No performance degradation
- [ ] No crashes or panics

## Future Enhancements (Post-MVP)

- [ ] Make period configurable via UI settings
- [ ] Add EMA (Exponential Moving Average) variant
- [ ] Add multiple MA periods (e.g., SMA 20, 50, 200)
- [ ] Color-code based on price position (above/below MA)
- [ ] Add crossover detection (price crosses MA)

## Library Research Results

### Decision: Use Rust Standard Library `.windows()` Method

After extensive research (see `LIBRARY_RESEARCH.md` for details), we decided **NOT** to add external dependencies.

**Libraries Evaluated:**
- ✅ **Rust stdlib `.windows()`** - SELECTED
- ⚠️ `yata` - Excellent performance but unnecessary for single indicator
- ⚠️ `ta` (ta-rs) - Good API but adds dependency
- ❌ `ta-lib-in-rust` - Requires Polars, architectural mismatch
- ❌ `average` crate - Doesn't support rolling windows

**Rationale for stdlib:**
1. **Zero dependencies** - No compile time or binary size impact
2. **Perfect fit** - Works seamlessly with our BTreeMap architecture
3. **Simple** - Easy to understand, test, and maintain
4. **Fast enough** - 1-10μs for typical chart data sizes
5. **Flexible** - Easy to extend to other MA types (EMA, WMA)

**Implementation:**
```rust
// Efficient batch calculation
let mas: Vec<f32> = prices
    .windows(period)
    .map(|w| w.iter().sum::<f32>() / period as f32)
    .collect();
```

**Future Consideration:**
If FlowSurface adds 5+ indicators (RSI, MACD, Bollinger Bands, etc.), reconsider `yata` crate for:
- Battle-tested algorithms
- Consistent API across indicators
- Optimal performance (3 ns/iter)

## Reference Files

- Example: `src/chart/indicator/kline/open_interest.rs` (LinePlot usage)
- Example: `src/chart/indicator/kline/volume.rs` (BarPlot usage)
- Plot API: `src/chart/indicator/plot/line.rs`
- Guide: `ADDING_CUSTOM_INDICATORS.md`
- Architecture: `INDICATOR_DOCS.md`
- Library Research: `LIBRARY_RESEARCH.md`

## Estimated Time

- **Data layer** (Steps 1-3): 15 minutes
- **Implementation** (Steps 4-6): 1-2 hours
- **Integration** (Step 7): 10 minutes
- **Testing** (Step 8): 30-45 minutes
- **Total:** ~2.5-3 hours

## Critical Implementation Notes

### ⚠️ Key Differences from Volume/OpenInterest Indicators

Unlike Volume or Open Interest indicators which can store their final values directly, **Moving Average is a rolling window indicator** that requires:

1. **Dual data storage:** Both source prices (`closes`) and calculated values (`data`)
2. **Historical price access:** Need previous N-1 prices to calculate each new MA value
3. **Incremental complexity:** Updates require querying historical window from `closes` BTreeMap
4. **Sparse results:** MA values only exist where sufficient history (>= period) is available

### Implementation Philosophy

- **Correctness first:** Start with clear, correct implementation
- **SMA is derived:** Only depends on price window, no external state needed
- **Period of 20:** Industry standard for daily moving averages
- **Line plot:** Continuous visualization appropriate for trend indicators
- **No external fetching:** All data derived from kline close prices

### Common Pitfalls to Avoid

❌ **Don't** store only MA values - you'll be unable to calculate new ones incrementally
❌ **Don't** try to calculate MA from previous MA values - mathematically incorrect
❌ **Don't** panic on insufficient data - just skip those points
✅ **Do** store both source closes and calculated MA values
✅ **Do** access historical window for each calculation
✅ **Do** handle time-based and tick-based charts differently
