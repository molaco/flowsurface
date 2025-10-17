# Bubble Indicator for Volume Footprint with Order Quantity Filtering
## Research Report

**Date**: 2025-10-17
**Objective**: Research projects, libraries, and packages that implement bubble indicators for volume footprint data with filtering capabilities for order quantity.

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Understanding Bubble Indicators](#understanding-bubble-indicators)
3. [Commercial Platforms](#commercial-platforms)
4. [Open Source Projects](#open-source-projects)
5. [FlowSurface Current Implementation](#flowsurface-current-implementation)
6. [Implementation Techniques](#implementation-techniques)
7. [Recommendations](#recommendations)

---

## Executive Summary

### What Are Bubble Indicators?

**Bubble indicators** visualize executed volume in financial markets where:
- **Bubble size** represents the volume of trades executed
- **Bubble color** indicates buyer/seller aggression (green = buy, red = sell)
- **Bubble position** shows price level and time of execution
- **Filtering** allows threshold-based hiding of small orders to reduce noise

### Key Findings

1. **Bookmap** is the industry leader in bubble visualization with sophisticated filtering
2. **FlowSurface already has robust footprint implementation** but uses bars, not bubbles
3. **Multiple open-source projects** exist in Python/TypeScript, minimal in Rust
4. **Bubble visualization is a variant of footprint rendering** - same data, different visual representation

### Current State

✅ **FlowSurface Has**:
- Complete footprint data aggregation (`GroupedTrades` by price level)
- Trade clustering with buy/sell separation
- Multiple display modes (BidAsk, VolumeProfile, DeltaProfile)
- Real-time trade updates via WebSocket
- Price-level binning with configurable tick size

❌ **FlowSurface Needs** (for bubble mode):
- Bubble size scaling algorithm
- Circular/spherical rendering (vs rectangular bars)
- Quantity filtering UI controls
- Bubble clustering by time/volume thresholds
- Opacity/alpha scaling based on volume ratios

---

## Understanding Bubble Indicators

### Core Concepts

#### 1. Volume Bubbles (Bookmap Definition)

**Definition**: Aggregate volume per pixel-time slice, with bubbles representing executed trades.

**Key Attributes**:
- **Size**: Proportional to traded volume
- **Color**: Green (aggressive buy at ask), Red (aggressive sell at bid)
- **Position**: Price level (Y-axis) × Time (X-axis)
- **Transparency**: Indicates volume intensity or ratio

**Example**:
```
Price Level 50,020: ● (small red bubble) = 0.5 BTC sell
Price Level 50,010: ⬤ (large green bubble) = 5.2 BTC buy
Price Level 50,000: ◉ (medium red bubble) = 2.1 BTC sell
```

#### 2. Filtering Mechanisms

**Two Primary Filter Types**:

a) **Minimal Trade Size Filter**
   - Hides individual trades below threshold
   - Example: Only show trades ≥ 1.0 BTC
   - Reduces visual noise from retail orders

b) **Minimal Displayed Volume Filter**
   - Aggregates rapid/multiple transactions
   - Example: Only show price levels with total volume ≥ 5.0 BTC
   - Reveals institutional order flow

**Bookmap Implementation**:
```
Filter Type: "Minimal Trade Size"
Threshold: 1.0 BTC
Result: Only trades with qty ≥ 1.0 BTC are displayed

Filter Type: "Minimal Displayed Volume"
Threshold: 5.0 BTC
Result: Only price levels with aggregate volume ≥ 5.0 BTC shown
```

#### 3. Clustering Strategies

**Bookmap's Clustering Modes**:

1. **Smart Clustering**
   - Algorithmic aggregation of adjacent/overlapping bubbles
   - Prevents visual clutter at high zoom levels

2. **By Time**
   - One bubble per configured time period (e.g., 1 second)
   - Aggregates all trades within time window

3. **By Volume**
   - Creates bubble when volume threshold reached
   - E.g., create bubble every 10 BTC traded

4. **By Price**
   - Aggregates identical price levels
   - Similar to FlowSurface's current footprint

5. **By Price and Aggressor**
   - Combines price + buy/sell side
   - Most granular visualization

---

## Commercial Platforms

### 1. Bookmap

**Website**: https://bookmap.com
**License**: Commercial (subscription-based)
**Language**: Java

#### Features

**Bubble Visualization**:
- Dynamic volume bubbles on time-price grid
- Real-time liquidity heatmap overlay
- Configurable bubble scaling and transparency

**Filtering Capabilities**:
- Minimal accountable dot volume (aggregate threshold)
- Dot size scaling adjustment
- Transparency controls
- 2D and 3D bubble modes

**Display Modes**:
- **Gradient**: Relative aggressor size via color blending
- **Solid**: Distinct color sections for buy/sell
- **Pie**: Volume proportions as pie chart segments

**Technical Implementation** (from documentation):
```
Bubble Size Calculation:
  - Aggregate volume per pixel-time slice
  - Dynamically scaled based on chart zoom level
  - Algorithmically scaled against instrument's average trade volume

Color Coding:
  - Green bubbles: Volume hitting ask (buying volume)
  - Red bubbles: Volume hitting bid (selling volume)
  - Gradient intensity: Stronger color = more aggressive action

Filtering Logic:
  if trade.qty < minimal_trade_size:
      skip
  if aggregate_volume_at_price < minimal_displayed_volume:
      skip
  else:
      render_bubble(size=f(volume), color=f(side), alpha=f(ratio))
```

**Data Structure** (inferred):
```java
class VolumeBubble {
    Price price;
    long timestamp;
    double buyVolume;
    double sellVolume;
    double radius;  // Calculated from total volume
    Color color;    // Based on buy/sell ratio
    float alpha;    // Transparency based on intensity
}
```

**Strengths**:
- Industry-leading visualization quality
- Sophisticated clustering algorithms
- Institutional-grade filtering

**Weaknesses**:
- Proprietary/closed-source
- Expensive subscription model
- No Rust implementation

---

### 2. Sierra Chart

**Website**: https://www.sierrachart.com
**License**: Commercial (one-time fee)
**Language**: C++

#### Numbers Bars (Footprint Equivalent)

**Features**:
- Up to 3 columns of numbers per bar
- Shows bid/ask volume, delta, trades per price level
- Requires 1-tick data for accuracy

**Technical Details**:
```cpp
// Numbers Bars data structure (inferred from docs)
struct NumbersBar {
    std::map<double, VolumeData> priceVolumes;

    struct VolumeData {
        double askVolume;
        double bidVolume;
        int askCount;
        int bidCount;
    };
};

// Display modes
enum DisplayMode {
    BID_ASK_VOLUME,
    DELTA,
    TOTAL_VOLUME,
    TRADE_COUNT
};
```

**Filtering**:
- No native bubble visualization
- Bar-based representation only
- Price-level filtering available

**Strengths**:
- Fast C++ performance
- Deep market microstructure analysis
- Customizable studies

**Weaknesses**:
- No bubble visualization
- Windows-only
- Complex API

---

### 3. ATAS Platform

**Website**: https://atas.net
**License**: Commercial (subscription)
**Language**: C#

#### Cluster Charts (Footprint)

**Features**:
- Multiple cluster/footprint modes
- Volume profile integration
- Imbalance detection

**Data Structure** (from documentation):
```csharp
public class ClusterRow {
    public decimal Price { get; set; }
    public decimal BuyVolume { get; set; }
    public decimal SellVolume { get; set; }
    public int BuyTrades { get; set; }
    public int SellTrades { get; set; }
}

public class FootprintBar {
    public DateTime Time { get; set; }
    public List<ClusterRow> Clusters { get; set; }
    public decimal Open, High, Low, Close;
}
```

**Filtering**:
- Cluster size thresholds
- Volume-based filtering
- Time aggregation controls

**Strengths**:
- Comprehensive footprint analysis
- Advanced filtering options
- Good documentation

**Weaknesses**:
- No bubble visualization
- C# only (no Rust interop)
- Expensive

---

## Open Source Projects

### 1. OrderflowChart (Python + Plotly)

**Repository**: https://github.com/murtazayusuf/OrderflowChart
**Language**: Python
**License**: Open source

#### Implementation

**Data Structure**:
```python
# Required CSV columns
orderflow_data = pd.DataFrame({
    'bid_size': [1.2, 0.5, 3.1, ...],
    'price': [50000, 50010, 50020, ...],
    'ask_size': [2.5, 0.8, 5.2, ...],
    'identifier': ['candle_1', 'candle_1', 'candle_2', ...]
})

ohlc_data = pd.DataFrame({
    'open': [50000, ...],
    'high': [50200, ...],
    'low': [49900, ...],
    'close': [50100, ...],
    'identifier': ['candle_1', ...]
})
```

**Usage**:
```python
from orderflowchart import OrderFlowChart

chart = OrderFlowChart(
    orderflow_data,
    ohlc_data,
    identifier_col='identifier'
)

chart.plot()  # Generates Plotly chart
```

**Visualization Approach**:
- Uses Plotly bar charts (not bubbles)
- Interactive zoom/pan
- Web-based rendering

**Strengths**:
- Simple API
- Plotly interactivity
- Easy to extend

**Weaknesses**:
- No bubble visualization (uses bars)
- No filtering controls
- Python-only (slow for real-time)

**Applicability to FlowSurface**:
- ❌ Different language/ecosystem
- ✅ Data structure design is similar
- ✅ Aggregation logic is transferable

---

### 2. stack-orderflow (Python GUI)

**Repository**: https://github.com/tysonwu/stack-orderflow
**Language**: Python (PyQt6 + Finplot)
**License**: Open source

#### Implementation

**Architecture**:
```python
# Technology stack
GUI: PyQt6
Plotting: finplot (modified)
Data: Pandas DataFrames
```

**Features**:
- Candlestick plots with orderflow overlay
- Market profile charts
- Static and (planned) real-time data

**Quote from Author**:
> "There were little to no open source tools about orderflow charting and visualization given how specific it is."

**Visualization Technique**:
- Bar-based footprint
- No bubble implementation
- Uses financial charting library (finplot)

**Strengths**:
- Open source reference implementation
- Multiple chart types
- Desktop GUI

**Weaknesses**:
- Python (not Rust)
- No bubble visualization
- No quantity filtering UI

**Applicability to FlowSurface**:
- ❌ Different language
- ✅ Architecture patterns are similar (GUI + canvas rendering)
- ⚠️ Could inspire UI design

---

### 3. Crypto Orderflow Service (TypeScript)

**Repository**: https://github.com/tiagosiebler/orderflow
**Language**: TypeScript
**License**: Open source

#### Implementation

**Purpose**: Build footprint candles from real-time trade data

**Architecture**:
```typescript
// Core service components
WebSocket Connectors:
  - Binance
  - Bybit
  - OKX
  - Bitget
  - Gate.io

Data Pipeline:
  Trade → Footprint Candle Builder → TimescaleDB

Output:
  Footprint Candles (stored in DB)
```

**Data Model**:
```typescript
interface FootprintCandle {
  timestamp: number;
  open: number;
  high: number;
  low: number;
  close: number;
  clusters: Map<number, {  // price → volume
    buyVolume: number;
    sellVolume: number;
  }>;
}
```

**Strengths**:
- Multi-exchange support
- Real-time data processing
- TimescaleDB for time-series storage

**Weaknesses**:
- No visualization (backend service only)
- TypeScript (not Rust)
- No filtering logic

**Applicability to FlowSurface**:
- ❌ Different language
- ✅ Multi-exchange pattern is similar to FlowSurface
- ✅ Data aggregation logic is transferable

---

### 4. Rust Implementations

**Search Results**: No dedicated Rust libraries found for:
- Volume footprint charts
- Bubble indicators
- Order flow visualization

**Related Rust Crates**:

1. **orderbook-rs**
   - High-performance limit order book
   - No visualization component

2. **rust_ti** (Technical Indicators)
   - 70+ indicators
   - No footprint/orderflow support

3. **trading-charts**
   - Lightweight Charts bindings for Leptos
   - No footprint charts

4. **Rust Orderbook Visualization** (Medium article)
   - DOM (Depth of Market) ladder visualization
   - Uses Databento + React + Rust
   - Real-time order book only (not footprint)

**Conclusion**: **No existing Rust bubble indicator libraries**. Implementation must be custom.

---

## FlowSurface Current Implementation

### Overview

FlowSurface has **extensive footprint implementation** but uses **rectangular bars** instead of bubbles.

**Current Capabilities**:
- ✅ Trade aggregation by price level
- ✅ Buy/sell volume separation
- ✅ Real-time WebSocket updates
- ✅ Multiple display modes (BidAsk, VolumeProfile, DeltaProfile)
- ✅ Imbalance detection
- ✅ Point of Control (POC) tracking
- ✅ Configurable cluster scaling

**Missing for Bubble Mode**:
- ❌ Circular/spherical rendering
- ❌ Quantity filtering UI
- ❌ Bubble clustering algorithms
- ❌ Size-based opacity scaling

### Data Structures

**Location**: `data/src/chart/kline.rs`

```rust
pub struct KlineDataPoint {
    pub kline: Kline,                              // OHLC data
    pub footprint: FxHashMap<Price, GroupedTrades>, // Price-level trades
}

pub struct GroupedTrades {
    pub buy_qty: f32,       // Aggregate buy volume
    pub sell_qty: f32,      // Aggregate sell volume
    pub buy_count: usize,   // Number of buy trades
    pub sell_count: usize,  // Number of sell trades
    pub first_time: u64,    // First trade timestamp
    pub last_time: u64,     // Last trade timestamp
}
```

**This is identical to data needed for bubble visualization!**

### Current Rendering

**Location**: `src/chart/kline.rs` → `draw_clusters()`

**Bar Rendering Logic** (simplified):
```rust
for (price, group) in &footprint.trades {
    let y = price_to_y(*price);

    // Buy side (right)
    let buy_bar_width = (group.buy_qty / max_cluster_qty) * area_width;
    frame.fill_rectangle(
        Point::new(buy_area_left, y - (cell_height / 2.0)),
        Size::new(buy_bar_width, cell_height),
        palette.success.base.color  // Green
    );

    // Sell side (left)
    let sell_bar_width = (group.sell_qty / max_cluster_qty) * area_width;
    frame.fill_rectangle(
        Point::new(sell_area_right, y - (cell_height / 2.0)),
        Size::new(-sell_bar_width, cell_height),
        palette.danger.base.color  // Red
    );
}
```

**Converting to Bubble Mode**:
```rust
// Bubble rendering (proposed)
for (price, group) in &footprint.trades {
    let y = price_to_y(*price);

    // Buy bubble (right)
    if group.buy_qty >= min_volume_filter {
        let radius = calculate_bubble_radius(
            group.buy_qty,
            max_cluster_qty,
            base_radius
        );
        let alpha = calculate_alpha(group.buy_qty, group.sell_qty);

        frame.fill(
            &Path::circle(Point::new(buy_center_x, y), radius),
            palette.success.base.color.scale_alpha(alpha)
        );
    }

    // Sell bubble (left)
    if group.sell_qty >= min_volume_filter {
        let radius = calculate_bubble_radius(
            group.sell_qty,
            max_cluster_qty,
            base_radius
        );
        let alpha = calculate_alpha(group.sell_qty, group.buy_qty);

        frame.fill(
            &Path::circle(Point::new(sell_center_x, y), radius),
            palette.danger.base.color.scale_alpha(alpha)
        );
    }
}
```

### Quantity Filtering

**Current Implementation**: None for individual trades

**Where to Add**:

1. **UI Controls** (in `src/modal/pane/settings.rs`):
```rust
pub struct BubbleSettings {
    pub min_trade_size: f32,        // e.g., 1.0 BTC
    pub min_displayed_volume: f32,  // e.g., 5.0 BTC total at price level
    pub bubble_scale_factor: f32,   // Size multiplier
    pub clustering_mode: ClusteringMode,
}

pub enum ClusteringMode {
    ByPrice,
    ByTime(Duration),
    ByVolume(f32),
    Smart,
}
```

2. **Filtering Logic** (in rendering):
```rust
// Filter 1: Minimum trade size (for buy bubbles)
if group.buy_qty < settings.min_trade_size {
    continue;  // Skip this bubble
}

// Filter 2: Minimum total volume at price level
let total_volume = group.buy_qty + group.sell_qty;
if total_volume < settings.min_displayed_volume {
    continue;  // Skip entire price level
}

// Render bubble
render_bubble(group, settings);
```

### Display Modes

**Current Modes** (bar-based):
1. **BidAsk**: Split bars (buy right, sell left)
2. **VolumeProfile**: Stacked bars (total volume)
3. **DeltaProfile**: Net delta bars (buy - sell)

**Proposed Bubble Modes**:
1. **BubbleBidAsk**: Separate bubbles for buy/sell
2. **BubbleVolume**: Single bubble sized by total volume
3. **BubbleDelta**: Bubble colored by delta, sized by total

---

## Implementation Techniques

### 1. Bubble Size Calculation

**Linear Scaling** (simple):
```rust
fn calculate_bubble_radius(volume: f32, max_volume: f32, base_radius: f32) -> f32 {
    let ratio = volume / max_volume;
    base_radius * ratio.sqrt()  // sqrt for area-based scaling
}
```

**Logarithmic Scaling** (for large volume ranges):
```rust
fn calculate_bubble_radius_log(volume: f32, max_volume: f32, base_radius: f32) -> f32 {
    let ratio = volume.ln() / max_volume.ln();
    base_radius * ratio
}
```

**Bookmap-Style** (adaptive):
```rust
fn calculate_bubble_radius_adaptive(
    volume: f32,
    avg_volume: f32,
    zoom_level: f32
) -> f32 {
    let normalized = volume / avg_volume;
    let base = 3.0 + (normalized * 10.0);  // 3-13 pixel range
    base * zoom_level
}
```

### 2. Color and Transparency

**Gradient Mode** (Bookmap-style):
```rust
fn calculate_bubble_color(
    buy_qty: f32,
    sell_qty: f32,
    palette: &Palette
) -> Color {
    let total = buy_qty + sell_qty;
    let buy_ratio = buy_qty / total;

    // Blend colors based on ratio
    let green = palette.success.base.color;
    let red = palette.danger.base.color;

    Color::from_rgb(
        green.r * buy_ratio + red.r * (1.0 - buy_ratio),
        green.g * buy_ratio + red.g * (1.0 - buy_ratio),
        green.b * buy_ratio + red.b * (1.0 - buy_ratio),
    )
}

fn calculate_bubble_alpha(
    volume: f32,
    threshold: f32,
    max_volume: f32
) -> f32 {
    if volume < threshold {
        return 0.0;  // Invisible (filtered)
    }

    // Alpha from 0.3 to 1.0 based on volume
    let ratio = volume / max_volume;
    0.3 + (ratio * 0.7)
}
```

**Solid Mode** (distinct buy/sell):
```rust
fn get_bubble_color_solid(is_buy: bool, palette: &Palette) -> Color {
    if is_buy {
        palette.success.base.color
    } else {
        palette.danger.base.color
    }
}
```

### 3. Clustering Algorithms

**By Time** (aggregate trades in time windows):
```rust
fn cluster_by_time(
    trades: &[Trade],
    window_ms: u64
) -> Vec<ClusteredTrade> {
    let mut clusters = Vec::new();
    let mut current_bucket = Vec::new();
    let mut bucket_start = trades[0].time;

    for trade in trades {
        if trade.time - bucket_start > window_ms {
            // Aggregate current bucket
            clusters.push(aggregate_trades(&current_bucket));
            current_bucket.clear();
            bucket_start = trade.time;
        }
        current_bucket.push(trade.clone());
    }

    clusters
}

fn aggregate_trades(trades: &[Trade]) -> ClusteredTrade {
    let mut buy_qty = 0.0;
    let mut sell_qty = 0.0;

    for trade in trades {
        if trade.is_sell {
            sell_qty += trade.qty;
        } else {
            buy_qty += trade.qty;
        }
    }

    ClusteredTrade {
        price: trades[0].price,  // Use first trade's price
        time: trades[0].time,
        buy_qty,
        sell_qty,
    }
}
```

**By Volume** (create bubble when threshold reached):
```rust
fn cluster_by_volume(
    trades: &[Trade],
    volume_threshold: f32
) -> Vec<ClusteredTrade> {
    let mut clusters = Vec::new();
    let mut accumulator = TradeAccumulator::new();

    for trade in trades {
        accumulator.add(trade);

        if accumulator.total_volume() >= volume_threshold {
            clusters.push(accumulator.finalize());
            accumulator.reset();
        }
    }

    clusters
}
```

**Smart Clustering** (adaptive based on zoom):
```rust
fn cluster_smart(
    trades: &[Trade],
    visible_width_px: f32,
    zoom_level: f32
) -> Vec<ClusteredTrade> {
    // More granular at higher zoom
    let time_window = calculate_adaptive_window(zoom_level);
    cluster_by_time(trades, time_window)
}

fn calculate_adaptive_window(zoom: f32) -> u64 {
    // At 1x zoom: 1 second
    // At 10x zoom: 100ms
    (1000.0 / zoom) as u64
}
```

### 4. Rendering Optimization

**Culling** (don't render off-screen bubbles):
```rust
fn should_render_bubble(
    bubble_y: f32,
    bubble_radius: f32,
    visible_top: f32,
    visible_bottom: f32
) -> bool {
    let bubble_top = bubble_y - bubble_radius;
    let bubble_bottom = bubble_y + bubble_radius;

    // Check if bubble intersects visible region
    bubble_bottom >= visible_top && bubble_top <= visible_bottom
}
```

**Level of Detail** (reduce quality when zoomed out):
```rust
fn render_bubble_lod(
    frame: &mut Frame,
    center: Point,
    radius: f32,
    color: Color,
    zoom_level: f32
) {
    if zoom_level < 0.5 {
        // Very zoomed out: draw as square (faster)
        frame.fill_rectangle(
            Point::new(center.x - radius, center.y - radius),
            Size::new(radius * 2.0, radius * 2.0),
            color
        );
    } else {
        // Normal: draw as circle
        frame.fill(&Path::circle(center, radius), color);
    }
}
```

### 5. Quantity Filtering Implementation

**Two-Stage Filter**:

```rust
pub struct BubbleFilter {
    pub min_trade_size: f32,
    pub min_displayed_volume: f32,
}

impl BubbleFilter {
    pub fn should_render_buy(&self, group: &GroupedTrades) -> bool {
        // Stage 1: Individual trade size filter
        if group.buy_qty < self.min_trade_size {
            return false;
        }

        // Stage 2: Total volume filter
        let total = group.buy_qty + group.sell_qty;
        total >= self.min_displayed_volume
    }

    pub fn should_render_sell(&self, group: &GroupedTrades) -> bool {
        if group.sell_qty < self.min_trade_size {
            return false;
        }

        let total = group.buy_qty + group.sell_qty;
        total >= self.min_displayed_volume
    }
}
```

**Usage in Rendering**:
```rust
let filter = BubbleFilter {
    min_trade_size: 1.0,      // 1 BTC minimum
    min_displayed_volume: 5.0, // 5 BTC total at price
};

for (price, group) in &footprint.trades {
    if filter.should_render_buy(group) {
        render_buy_bubble(group, ...);
    }

    if filter.should_render_sell(group) {
        render_sell_bubble(group, ...);
    }
}
```

---

## Recommendations

### For FlowSurface Development

#### Option 1: Add Bubble Mode to Existing Footprint

**Advantages**:
- ✅ Reuse existing data structures (`GroupedTrades`)
- ✅ Minimal architectural changes
- ✅ Quick implementation (~2-4 days)

**Implementation Steps**:

1. **Add BubbleMode to KlineChartKind**:
```rust
pub enum KlineChartKind {
    Footprint { ... },
    Bubble {  // New variant
        scaling: BubbleScaling,
        filter: BubbleFilter,
        clustering: ClusteringMode,
    },
    Candlestick,
}

pub struct BubbleFilter {
    pub min_trade_size: f32,
    pub min_displayed_volume: f32,
}

pub enum BubbleScaling {
    Linear,
    Logarithmic,
    Adaptive,
}
```

2. **Add Bubble Rendering Function**:
```rust
// In src/chart/kline.rs
fn draw_bubbles(
    frame: &mut Frame,
    price_to_y: &impl Fn(Price) -> f32,
    x_position: f32,
    footprint: &KlineTrades,
    max_cluster_qty: f32,
    palette: &Palette,
    filter: &BubbleFilter,
    scaling: &BubbleScaling,
) {
    for (price, group) in &footprint.trades {
        let y = price_to_y(*price);

        // Buy bubble
        if group.buy_qty >= filter.min_trade_size {
            let total = group.buy_qty + group.sell_qty;
            if total >= filter.min_displayed_volume {
                let radius = calculate_radius(
                    group.buy_qty,
                    max_cluster_qty,
                    scaling
                );
                let alpha = calculate_alpha(group.buy_qty, total);

                frame.fill(
                    &Path::circle(
                        Point::new(x_position + 20.0, y),
                        radius
                    ),
                    palette.success.base.color.scale_alpha(alpha)
                );
            }
        }

        // Sell bubble (similar)
        // ...
    }
}
```

3. **Add UI Controls** (in `src/modal/pane/settings.rs`):
```rust
// Add to settings modal
pub fn bubble_settings_view<'a>(...) -> Element<'a, Message> {
    column![
        text("Bubble Settings").size(16),

        row![
            text("Min Trade Size:"),
            slider(0.1..=10.0, min_trade_size, |v| {
                Message::SetMinTradeSize(v)
            })
        ],

        row![
            text("Min Displayed Volume:"),
            slider(1.0..=50.0, min_displayed_volume, |v| {
                Message::SetMinDisplayedVolume(v)
            })
        ],

        row![
            text("Bubble Scaling:"),
            pick_list(
                [BubbleScaling::Linear, BubbleScaling::Logarithmic],
                Some(current_scaling),
                Message::SetBubbleScaling
            )
        ]
    ]
}
```

4. **Helper Functions**:
```rust
fn calculate_radius(
    volume: f32,
    max_volume: f32,
    scaling: &BubbleScaling
) -> f32 {
    let base_radius = 10.0;  // Base size in pixels

    match scaling {
        BubbleScaling::Linear => {
            let ratio = volume / max_volume;
            base_radius * ratio.sqrt()  // sqrt for area-based
        }
        BubbleScaling::Logarithmic => {
            let ratio = volume.ln() / max_volume.ln();
            base_radius * ratio
        }
        BubbleScaling::Adaptive => {
            // Bookmap-style adaptive
            let normalized = volume / max_volume;
            base_radius * (1.0 + normalized * 2.0)
        }
    }
}

fn calculate_alpha(side_volume: f32, total_volume: f32) -> f32 {
    let ratio = side_volume / total_volume;
    0.4 + (ratio * 0.6)  // Alpha from 0.4 to 1.0
}
```

**Estimated Effort**: 2-4 days
- Day 1: Add bubble rendering logic
- Day 2: Implement filtering
- Day 3: Add UI controls
- Day 4: Testing and refinement

---

#### Option 2: Hybrid Bar+Bubble Mode

**Concept**: Show both bars and bubbles simultaneously

**Visual Layout**:
```
|====BUY BAR====| [CANDLE] ● ● ●  (buy bubbles)
|===SELL BAR===| [CANDLE] ● ● ●  (sell bubbles)
```

**Advantages**:
- ✅ Best of both worlds
- ✅ Bars show aggregate, bubbles show detail
- ✅ Users can see both visualizations

**Implementation**:
```rust
fn draw_hybrid_clusters(...) {
    // Draw bars first (background)
    draw_clusters(frame, footprint, ...);

    // Overlay bubbles (foreground)
    draw_bubbles(frame, footprint, filter, ...);
}
```

---

#### Option 3: Extract from Open Source Projects

**Python Projects** (OrderflowChart, stack-orderflow):
- ❌ Different language (Python vs Rust)
- ❌ Different rendering (Plotly/Finplot vs iced Canvas)
- ✅ Useful for algorithm reference
- ✅ Useful for UX inspiration

**TypeScript Projects** (tiagosiebler/orderflow):
- ❌ Backend service only (no visualization)
- ✅ Data aggregation patterns are transferable
- ✅ Multi-exchange support patterns

**Recommendation**: Use as **reference only**, not direct code extraction.

---

### Suggested Implementation Priority

**Phase 1: Minimum Viable Bubble Mode** (1 week)
1. Add `BubbleMode` enum variant
2. Implement basic bubble rendering (circles)
3. Add min trade size filter UI
4. Linear size scaling

**Phase 2: Advanced Features** (1 week)
5. Add clustering modes (by time, by volume)
6. Implement transparency/alpha scaling
7. Add min displayed volume filter
8. Logarithmic/adaptive scaling options

**Phase 3: Polish** (3-5 days)
9. Performance optimization (culling, LOD)
10. Smooth animations for real-time updates
11. Color gradient modes
12. User presets (retail filter, whale filter, etc.)

**Total Estimated Effort**: 2-3 weeks

---

## Summary

### Key Findings

1. **Bubble indicators are well-established** in commercial platforms (Bookmap, ATAS, Sierra Chart)
2. **FlowSurface already has all the data needed** - just needs rendering changes
3. **No Rust libraries exist** - must implement custom
4. **Open source projects provide algorithms** but different languages/ecosystems

### What FlowSurface Can Leverage

From **Bookmap**:
- ✅ Bubble size scaling algorithms
- ✅ Filtering strategies (min trade size, min displayed volume)
- ✅ Clustering modes (time, volume, price)
- ✅ Color gradient techniques

From **Existing FlowSurface Code**:
- ✅ Trade aggregation (`GroupedTrades`)
- ✅ Real-time WebSocket updates
- ✅ Canvas rendering infrastructure
- ✅ Price-level binning

From **Open Source Projects**:
- ✅ UX patterns (sliders, dropdowns for filters)
- ✅ Data structure designs
- ✅ Algorithm references

### Implementation Path

**Recommended Approach**: Add bubble mode as variant to existing footprint system

**Key Components to Build**:
1. Bubble size calculation (linear/log/adaptive)
2. Quantity filtering (trade size + displayed volume)
3. Clustering algorithms (time/volume/smart)
4. Color/transparency scaling
5. UI controls for filter configuration

**Timeline**: 2-3 weeks for full implementation

**Complexity**: Medium (reuses existing data structures, new rendering logic)

---

## Appendix: Code Examples

### Complete Bubble Rendering Example

```rust
pub fn draw_bubble_footprint(
    frame: &mut Frame,
    price_to_y: &impl Fn(Price) -> f32,
    x_position: f32,
    footprint: &KlineTrades,
    settings: &BubbleSettings,
    max_cluster_qty: f32,
    palette: &Palette,
) {
    let buy_x = x_position + 25.0;   // Right side
    let sell_x = x_position - 25.0;  // Left side

    for (price, group) in &footprint.trades {
        let y = price_to_y(*price);
        let total_volume = group.buy_qty + group.sell_qty;

        // Filter: Check minimum displayed volume
        if total_volume < settings.min_displayed_volume {
            continue;
        }

        // Buy bubble
        if group.buy_qty >= settings.min_trade_size {
            let radius = calculate_bubble_radius(
                group.buy_qty,
                max_cluster_qty,
                &settings.scaling,
                settings.base_radius
            );

            let alpha = calculate_bubble_alpha(
                group.buy_qty,
                total_volume,
                settings.min_alpha,
                settings.max_alpha
            );

            frame.fill(
                &Path::circle(Point::new(buy_x, y), radius),
                palette.success.base.color.scale_alpha(alpha)
            );

            // Optional: Draw volume text
            if settings.show_text && radius > 8.0 {
                draw_bubble_text(
                    frame,
                    &format!("{:.1}", group.buy_qty),
                    Point::new(buy_x, y),
                    palette.success.base.text
                );
            }
        }

        // Sell bubble (symmetric)
        if group.sell_qty >= settings.min_trade_size {
            let radius = calculate_bubble_radius(
                group.sell_qty,
                max_cluster_qty,
                &settings.scaling,
                settings.base_radius
            );

            let alpha = calculate_bubble_alpha(
                group.sell_qty,
                total_volume,
                settings.min_alpha,
                settings.max_alpha
            );

            frame.fill(
                &Path::circle(Point::new(sell_x, y), radius),
                palette.danger.base.color.scale_alpha(alpha)
            );

            if settings.show_text && radius > 8.0 {
                draw_bubble_text(
                    frame,
                    &format!("{:.1}", group.sell_qty),
                    Point::new(sell_x, y),
                    palette.danger.base.text
                );
            }
        }
    }
}

fn calculate_bubble_radius(
    volume: f32,
    max_volume: f32,
    scaling: &BubbleScaling,
    base_radius: f32,
) -> f32 {
    let ratio = (volume / max_volume).clamp(0.0, 1.0);

    match scaling {
        BubbleScaling::Linear => {
            base_radius * ratio.sqrt()  // Area-based scaling
        }
        BubbleScaling::Logarithmic => {
            if volume <= 0.0 { return 0.0; }
            let log_ratio = volume.ln() / max_volume.ln().max(1.0);
            base_radius * log_ratio.clamp(0.2, 1.0)
        }
        BubbleScaling::Adaptive => {
            // Bookmap-style: larger range, more visible
            base_radius * (0.5 + ratio * 1.5)
        }
    }
}

fn calculate_bubble_alpha(
    side_volume: f32,
    total_volume: f32,
    min_alpha: f32,
    max_alpha: f32,
) -> f32 {
    if total_volume <= 0.0 { return min_alpha; }

    let ratio = side_volume / total_volume;
    min_alpha + (ratio * (max_alpha - min_alpha))
}

fn draw_bubble_text(
    frame: &mut Frame,
    text: &str,
    center: Point,
    color: Color,
) {
    use iced::widget::canvas::Text;
    use iced::alignment::{Horizontal, Vertical};

    frame.fill_text(Text {
        content: text.to_string(),
        position: center,
        size: iced::Pixels(10.0),
        color,
        horizontal_alignment: Horizontal::Center,
        vertical_alignment: Vertical::Center,
        ..Text::default()
    });
}
```

### Settings Structure

```rust
#[derive(Debug, Clone)]
pub struct BubbleSettings {
    pub min_trade_size: f32,        // Minimum individual trade (e.g., 1.0 BTC)
    pub min_displayed_volume: f32,  // Minimum total at price level (e.g., 5.0 BTC)
    pub base_radius: f32,           // Base bubble size in pixels (e.g., 10.0)
    pub scaling: BubbleScaling,     // Size calculation method
    pub show_text: bool,            // Show volume numbers
    pub min_alpha: f32,             // Minimum transparency (e.g., 0.3)
    pub max_alpha: f32,             // Maximum transparency (e.g., 1.0)
}

impl Default for BubbleSettings {
    fn default() -> Self {
        Self {
            min_trade_size: 0.1,
            min_displayed_volume: 1.0,
            base_radius: 10.0,
            scaling: BubbleScaling::Linear,
            show_text: true,
            min_alpha: 0.4,
            max_alpha: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum BubbleScaling {
    Linear,      // sqrt(volume/max) - area-based
    Logarithmic, // ln(volume)/ln(max)
    Adaptive,    // Bookmap-style dynamic
}
```

---

**End of Research Report**
