# FlowSurface Indicator Implementation Guide

## Table of Contents
1. [Indicator Trait Architecture](#1-indicator-trait-architecture)
2. [Plot System and Rendering](#2-plot-system-and-rendering)
3. [KlineIndicator Lifecycle Management](#3-klineindicator-lifecycle-management)
4. [Volume Indicator Implementation](#4-volume-indicator-implementation)
5. [UI Integration and Selection](#5-ui-integration-and-selection)
6. [Chart Canvas and Crosshair](#6-chart-canvas-and-crosshair)
7. [Adding Custom Indicators Guide](#7-adding-custom-indicators-guide)

---

## 1. Indicator Trait Architecture

### Overview
FlowSurface uses a trait-based architecture to define and manage indicators, with clear separation between different chart types and market contexts.

### Core Trait: `Indicator`
**Location:** `data/src/chart/indicator.rs:7-11`

```rust
pub trait Indicator: PartialEq + Display + 'static {
    fn for_market(market: MarketKind) -> &'static [Self]
    where
        Self: Sized;
}
```

**Purpose:** Defines which indicators are available for different market types (Spot, LinearPerps, InversePerps).

### Indicator Types

#### 1. KlineIndicator
**Location:** `data/src/chart/indicator.rs:13-44`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize, Eq, Enum)]
pub enum KlineIndicator {
    Volume,
    OpenInterest,
}
```

**Market-Specific Availability:**
- **FOR_SPOT:** `[Volume]` - Only volume available for spot markets
- **FOR_PERPS:** `[Volume, OpenInterest]` - Both volume and open interest for perpetual markets

**Key Points:**
- Used for time-series and tick-based charts
- Each variant must be present in either `FOR_SPOT`, `FOR_PERPS`, or both
- Display implementation provides user-facing names

#### 2. HeatmapIndicator
**Location:** `data/src/chart/indicator.rs:46-75`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize, Eq, Enum)]
pub enum HeatmapIndicator {
    Volume,
}
```

**Market-Specific Availability:**
- **FOR_SPOT:** `[Volume]`
- **FOR_PERPS:** `[Volume]`

**Key Points:**
- Used exclusively for heatmap/volume profile charts
- Currently only supports Volume indicator
- Separate from KlineIndicator to allow different implementations

#### 3. UiIndicator Wrapper
**Location:** `data/src/chart/indicator.rs:77-95`

```rust
pub enum UiIndicator {
    Heatmap(HeatmapIndicator),
    Kline(KlineIndicator),
}
```

**Purpose:**
- Temporary workaround for UI polymorphism
- Allows unified handling of indicators in UI components
- Implements `From<KlineIndicator>` and `From<HeatmapIndicator>`

### Type Hierarchy

```
Indicator (trait)
├── KlineIndicator (enum)
│   ├── Volume
│   └── OpenInterest
└── HeatmapIndicator (enum)
    └── Volume

UiIndicator (wrapper enum)
├── Kline(KlineIndicator)
└── Heatmap(HeatmapIndicator)
```

---

## 2. Plot System and Rendering

### Series Abstraction Layer

#### Series Trait
**Location:** `src/chart/indicator/plot.rs:14-24`

```rust
pub trait Series {
    type Y;

    fn for_each_in<F: FnMut(u64, &Self::Y)>(&self, range: RangeInclusive<u64>, f: F);
    fn at(&self, x: u64) -> Option<&Self::Y>;
    fn next_after<'a>(&'a self, x: u64) -> Option<(u64, &'a Self::Y)>;
}
```

**Purpose:** Generic abstraction over time-series data that supports:
- Iteration over ranges (`for_each_in`)
- Point queries (`at`)
- Sequential access (`next_after`) for tooltip rendering

#### AnySeries Implementation
**Location:** `src/chart/indicator/plot.rs:88-132`

```rust
pub enum AnySeries<'a, Y> {
    Forward(&'a BTreeMap<u64, Y>),
    Reversed(ReversedBTreeSeries<'a, Y>),
}

impl<'a, Y> AnySeries<'a, Y> {
    pub fn for_basis(basis: Basis, data: &'a BTreeMap<u64, Y>) -> Self {
        match basis {
            Basis::Tick(_) => Self::Reversed(ReversedBTreeSeries::new(data)),
            Basis::Time(_) => Self::Forward(data),
        }
    }
}
```

**Key Features:**
- **Forward:** Time-based charts where newer data has higher timestamps
- **Reversed:** Tick-based charts where data is indexed from most recent (tick 0)
- Automatically selected based on chart basis type

### Plot Trait
**Location:** `src/chart/indicator/plot.rs:150-172`

```rust
pub trait Plot<S: Series> {
    fn y_extents(&self, s: &S, range: RangeInclusive<u64>) -> Option<(f32, f32)>;

    fn adjust_extents(&self, min: f32, max: f32) -> (f32, f32) {
        (min, max)
    }

    fn draw<'a>(
        &'a self,
        frame: &'a mut canvas::Frame,
        ctx: &'a ViewState,
        theme: &Theme,
        s: &S,
        range: RangeInclusive<u64>,
        scale: &YScale,
    );

    fn tooltip_fn(&self) -> Option<&TooltipFn<S::Y>>;

    fn tooltip(&self, y: &S::Y, next: Option<&S::Y>, _theme: &Theme) -> Option<PlotTooltip>;
}
```

**Methods:**
- **`y_extents`:** Calculate min/max values for visible range (determines Y-axis scaling)
- **`adjust_extents`:** Apply padding or adjustments to extents
- **`draw`:** Render the plot to the canvas
- **`tooltip_fn`/`tooltip`:** Generate tooltip content for crosshair

### YScale System
**Location:** `src/chart/indicator/plot.rs:134-148`

```rust
pub struct YScale {
    pub min: f32,
    pub max: f32,
    pub px_height: f32,
}

impl YScale {
    pub fn to_y(&self, v: f32) -> f32 {
        if self.max <= self.min {
            self.px_height
        } else {
            self.px_height - ((v - self.min) / (self.max - self.min)) * self.px_height
        }
    }
}
```

**Purpose:** Converts data values to pixel coordinates for rendering.

### Plot Implementations

#### BarPlot
**Location:** `src/chart/indicator/plot/bar.rs`

**Features:**
```rust
pub struct BarPlot<V, CL, T> {
    pub value: V,              // Maps datapoint to bar height
    pub bar_width_factor: f32, // Width relative to cell width (default 0.9)
    pub padding: f32,          // Top padding percentage
    pub classify: CL,          // Single vs Overlay bar type
    pub tooltip: Option<TooltipFn<T>>,
    pub baseline: Baseline,    // Zero, Min, or Fixed
}

pub enum Baseline {
    Zero,      // Classic volume baseline at 0
    Min,       // Use minimum value in range
    Fixed(f32),// Custom baseline value
}

pub enum BarClass {
    Single,                    // Single color bar
    Overlay { overlay: f32 },  // Two-layer bar with signed overlay
}
```

**Rendering Logic:**
- **Single:** Draws one bar using `secondary.strong.color`
- **Overlay:**
  - Base layer: `success/danger.base.color` at 30% alpha (full width)
  - Overlay layer: Full opacity overlay based on sign (buy=success, sell=danger)
  - Used for volume delta visualization

**Example Usage (Volume):**
```rust
let bar_kind = |&(buy, sell): &(f32, f32)| {
    if buy == -1.0 {
        BarClass::Single
    } else {
        BarClass::Overlay { overlay: buy - sell }
    }
};
```

#### LinePlot
**Location:** `src/chart/indicator/plot/line.rs`

**Features:**
```rust
pub struct LinePlot<V, T> {
    pub value: V,                 // Maps datapoint to Y value
    pub tooltip: Option<TooltipFn<T>>,
    pub padding: f32,             // Percentage padding (default 0.08)
    pub stroke_width: f32,        // Line thickness (default 1.0)
    pub show_points: bool,        // Draw circles on points (default true)
    pub point_radius_factor: f32, // Circle size relative to cell width (default 0.2)
}
```

**Rendering Logic:**
1. Draws polyline connecting all datapoints
2. Optionally draws circles at each point (visible when zoomed in)
3. Circle radius capped at 5px maximum
4. Uses `secondary.strong.color`

**Example Usage (Open Interest):**
```rust
let plot = LinePlot::new(value_fn)
    .stroke_width(1.0)
    .show_points(true)
    .point_radius_factor(0.2)
    .padding(0.08)
    .with_tooltip(tooltip);
```

### Tooltip System
**Location:** `src/chart/indicator/plot.rs:356-434`

**PlotTooltip Structure:**
```rust
pub struct PlotTooltip {
    pub text: String,
}

impl PlotTooltip {
    const TOOLTIP_CHAR_W: f32 = 8.0;
    const TOOLTIP_LINE_H: f32 = 14.0;
    const TOOLTIP_PAD_X: f32 = 8.0;
    const TOOLTIP_PAD_Y: f32 = 6.0;
}
```

**Positioning Algorithm:**
1. Calculate tooltip dimensions from text (guesstimate based on character count)
2. Determine side placement:
   - Default to left side if cursor is in right half
   - Switch sides if tooltip would overflow bounds
   - Special handling for tall tooltips (> 3x height)
3. Align text left or right based on side
4. Use semi-transparent background (`background.weakest.color` at 90% alpha)

---

## 3. KlineIndicator Lifecycle Management

### KlineIndicatorImpl Trait
**Location:** `src/chart/indicator/kline.rs:12-50`

```rust
pub trait KlineIndicatorImpl {
    fn clear_all_caches(&mut self);
    fn clear_crosshair_caches(&mut self);

    fn element<'a>(
        &'a self,
        chart: &'a ViewState,
        visible_range: std::ops::RangeInclusive<u64>,
    ) -> iced::Element<'a, Message>;

    fn fetch_range(&mut self, _ctx: &FetchCtx) -> Option<FetchRange> { None }
    fn rebuild_from_source(&mut self, _source: &PlotData<KlineDataPoint>) {}
    fn on_insert_klines(&mut self, _klines: &[Kline]) {}
    fn on_insert_trades(&mut self, _trades: &[Trade], _old_dp_len: usize, _source: &PlotData<KlineDataPoint>) {}
    fn on_ticksize_change(&mut self, _source: &PlotData<KlineDataPoint>) {}
    fn on_basis_change(&mut self, _source: &PlotData<KlineDataPoint>) {}
    fn on_open_interest(&mut self, _pairs: &[exchange::OpenInterest]) {}
}
```

### Lifecycle Hooks

#### Cache Management

**`clear_all_caches()`**
- Called when: Complete redraw needed
- Invalidates: All canvas caches (main plot, crosshair, labels)
- Use case: Data structure changes, new data inserted

**`clear_crosshair_caches()`**
- Called when: Partial redraw for crosshair movement
- Invalidates: Only crosshair and label caches
- Use case: Mouse movement, tooltip updates

#### Data Updates

**`rebuild_from_source(source: &PlotData<KlineDataPoint>)`**
- **When:** Complete rebuild from kline data source
- **Triggers:**
  - Initial chart load
  - Timeframe change
  - Tick size change
  - Basis change (time ↔ tick)
- **Implementation pattern:**
  ```rust
  fn rebuild_from_source(&mut self, source: &PlotData<KlineDataPoint>) {
      match source {
          PlotData::TimeBased(timeseries) => {
              self.data = timeseries.volume_data();
          }
          PlotData::TickBased(tickseries) => {
              self.data = tickseries.volume_data();
          }
      }
      self.clear_all_caches();
  }
  ```

**`on_insert_klines(klines: &[Kline])`**
- **When:** New kline data arrives (WebSocket or fetch)
- **Pattern:** Incremental update
  ```rust
  fn on_insert_klines(&mut self, klines: &[Kline]) {
      for kline in klines {
          self.data.insert(kline.time, (kline.volume.0, kline.volume.1));
      }
      self.clear_all_caches();
  }
  ```

**`on_insert_trades(trades: &[Trade], old_dp_len: usize, source: &PlotData<KlineDataPoint>)`**
- **When:** New trade data arrives (tick charts only)
- **Pattern:** Update from old length to current
  ```rust
  fn on_insert_trades(&mut self, _trades: &[Trade], old_dp_len: usize, source: &PlotData<KlineDataPoint>) {
      match source {
          PlotData::TimeBased(_) => return,
          PlotData::TickBased(tickseries) => {
              let start_idx = old_dp_len.saturating_sub(1);
              for (idx, dp) in tickseries.datapoints.iter().enumerate().skip(start_idx) {
                  self.data.insert(idx as u64, (dp.kline.volume.0, dp.kline.volume.1));
              }
          }
      }
      self.clear_all_caches();
  }
  ```

#### Basis Changes

**`on_ticksize_change(source: &PlotData<KlineDataPoint>)`**
- **When:** User changes tick chart interval
- **Pattern:** Usually calls `rebuild_from_source()`

**`on_basis_change(source: &PlotData<KlineDataPoint>)`**
- **When:** Switch between Time and Tick basis
- **Pattern:** Usually calls `rebuild_from_source()`

#### Data Fetching

**`fetch_range(ctx: &FetchCtx) -> Option<FetchRange>`**
- **When:** Chart scrolls to area without data
- **Purpose:** Request additional data from exchange
- **Context:**
  ```rust
  pub struct FetchCtx<'a> {
      pub main_chart: &'a ViewState,
      pub timeframe: Timeframe,
      pub visible_earliest: u64,
      pub kline_latest: u64,
      pub prefetch_earliest: u64,
  }
  ```
- **Example (Open Interest):**
  ```rust
  fn fetch_range(&mut self, ctx: &FetchCtx) -> Option<FetchRange> {
      let (oi_earliest, oi_latest) = self.oi_timerange(ctx.kline_latest);

      if ctx.visible_earliest < oi_earliest {
          return Some(FetchRange::OpenInterest(ctx.prefetch_earliest, oi_earliest));
      }

      if oi_latest < ctx.kline_latest {
          return Some(FetchRange::OpenInterest(oi_latest.max(ctx.prefetch_earliest), ctx.kline_latest));
      }

      None
  }
  ```

**`on_open_interest(data: &[exchange::OpenInterest])`**
- **When:** Open interest data arrives from fetch
- **Pattern:**
  ```rust
  fn on_open_interest(&mut self, data: &[exchange::OpenInterest]) {
      self.data.extend(data.iter().map(|oi| (oi.time, oi.value)));
      self.clear_all_caches();
  }
  ```

### Data Flow Diagram

```
Market Data Arrives
       ↓
┌──────────────────┐
│ Kline WebSocket  │ → on_insert_klines()
│ Trade WebSocket  │ → on_insert_trades()
│ OI Fetch Response│ → on_open_interest()
└──────────────────┘
       ↓
Update BTreeMap data
       ↓
clear_all_caches()
       ↓
Next frame: element() called
       ↓
indicator_row() renders with updated data
```

### Factory Pattern
**Location:** `src/chart/indicator/kline.rs:60-67`

```rust
pub fn make_empty(which: KlineIndicator) -> Box<dyn KlineIndicatorImpl> {
    match which {
        KlineIndicator::Volume => Box::new(super::kline::volume::VolumeIndicator::new()),
        KlineIndicator::OpenInterest => Box::new(super::kline::open_interest::OpenInterestIndicator::new())
    }
}
```

**Usage:** Creates trait objects stored in `EnumMap<KlineIndicator, Option<Box<dyn KlineIndicatorImpl>>>`

---

## 4. Volume Indicator Implementation

### Structure
**Location:** `src/chart/indicator/kline/volume.rs`

```rust
pub struct VolumeIndicator {
    cache: Caches,
    data: BTreeMap<u64, (f32, f32)>,  // (buy_volume, sell_volume)
}
```

**Data Format:**
- **Key:** Timestamp (time-based) or index (tick-based)
- **Value:** `(buy_volume, sell_volume)` tuple
  - **Special case:** `(-1.0, total_volume)` for exchanges without buy/sell split (Bybit workaround)

### Plot Configuration

```rust
fn indicator_elem<'a>(&'a self, main_chart: &'a ViewState, visible_range: RangeInclusive<u64>) -> iced::Element<'a, Message> {
    // Value function: total volume
    let value_fn = |&(buy, sell): &(f32, f32)| {
        if buy == -1.0 { sell } else { buy + sell }
    };

    // Classification: single bar vs overlay
    let bar_kind = |&(buy, sell): &(f32, f32)| {
        if buy == -1.0 {
            BarClass::Single
        } else {
            BarClass::Overlay {
                overlay: buy - sell  // Positive = more buying, negative = more selling
            }
        }
    };

    // Tooltip formatting
    let tooltip = |&(buy, sell): &(f32, f32), _next: Option<&(f32, f32)>| {
        if buy == -1.0 {
            PlotTooltip::new(format!("Volume: {}", format_with_commas(sell)))
        } else {
            let buy_t = format!("Buy Volume: {}", format_with_commas(buy));
            let sell_t = format!("Sell Volume: {}", format_with_commas(sell));
            PlotTooltip::new(format!("{buy_t}\n{sell_t}"))
        }
    };

    let plot = BarPlot::new(value_fn, bar_kind)
        .bar_width_factor(0.9)
        .with_tooltip(tooltip);

    indicator_row(main_chart, &self.cache, plot, &self.data, visible_range)
}
```

### Visual Representation

**Overlay Mode (exchanges with buy/sell data):**
```
Bar Height = buy_volume + sell_volume
├─ Base Layer (30% alpha): Full height, color based on delta sign
│  └─ Green if buy > sell
│  └─ Red if sell > buy
└─ Overlay Layer (100% opacity): Height = |buy - sell|
   └─ Positioned from baseline
```

**Single Mode (Bybit workaround):**
```
Bar Height = total_volume
└─ Single solid bar (secondary.strong.color)
```

### Lifecycle Implementation

**Time-Based Charts:**
```rust
fn on_insert_klines(&mut self, klines: &[Kline]) {
    for kline in klines {
        self.data.insert(kline.time, (kline.volume.0, kline.volume.1));
    }
    self.clear_all_caches();
}
```

**Tick-Based Charts:**
```rust
fn on_insert_trades(&mut self, _trades: &[Trade], old_dp_len: usize, source: &PlotData<KlineDataPoint>) {
    match source {
        PlotData::TimeBased(_) => return,
        PlotData::TickBased(tickseries) => {
            let start_idx = old_dp_len.saturating_sub(1);
            for (idx, dp) in tickseries.datapoints.iter().enumerate().skip(start_idx) {
                self.data.insert(idx as u64, (dp.kline.volume.0, dp.kline.volume.1));
            }
        }
    }
    self.clear_all_caches();
}
```

**Basis/Ticksize Changes:**
```rust
fn on_ticksize_change(&mut self, source: &PlotData<KlineDataPoint>) {
    self.rebuild_from_source(source);
}

fn on_basis_change(&mut self, source: &PlotData<KlineDataPoint>) {
    self.rebuild_from_source(source);
}
```

---

## 5. UI Integration and Selection

### Indicator Pane View
**Location:** `src/modal/pane/indicators.rs:11-32`

```rust
pub fn view<'a, I>(
    pane: pane_grid::Pane,
    state: &'a pane::State,
    selected: &[I],
    market_type: Option<exchange::adapter::MarketKind>,
) -> Element<'a, Message>
where
    I: Indicator + Copy + Into<UiIndicator>,
```

**Structure:**
- Container with `chart_modal` style (max width 200px, padding 16px)
- Title: "Indicators" (size 14)
- Selected indicators list (with drag support if applicable)
- Available indicators list (not yet selected)

### Indicator Row Building

```rust
fn build_indicator_row<'a, I>(
    pane: pane_grid::Pane,
    indicator: &I,
    is_selected: bool,
) -> Element<'a, Message>
where
    I: Indicator + Copy + Into<UiIndicator>,
{
    let content = if is_selected {
        row![
            text(indicator.to_string()),
            space::horizontal(),
            container(icon_text(Icon::Checkmark, 12)),
        ]
        .width(Length::Fill)
    } else {
        row![text(indicator.to_string())].width(Length::Fill)
    };

    button(content)
        .on_press(Message::ToggleIndicator(pane, (*indicator).into()))
        .width(Length::Fill)
        .style(move |theme, status| style::button::modifier(theme, status, is_selected))
        .into()
}
```

**Visual States:**
- **Selected:** Shows checkmark icon, highlighted button style
- **Available:** No checkmark, default button style
- **Click action:** `Message::ToggleIndicator` to add/remove

### Drag-to-Reorder

```rust
fn selected_list<'a, I>(
    pane: pane_grid::Pane,
    selected: &[I],
    reorderable: bool,
) -> Element<'a, Message>
where
    I: Indicator + Copy + Into<UiIndicator>,
{
    let elements: Vec<Element<_>> = selected
        .iter()
        .map(|indicator| {
            let base = build_indicator_row(pane, indicator, true);
            dragger_row(base, reorderable)
        })
        .collect();

    if reorderable {
        let mut draggable_column = column_drag::Column::new()
            .on_drag(move |event| Message::ReorderIndicator(pane, event))
            .spacing(4);
        for element in elements {
            draggable_column = draggable_column.push(element);
        }
        draggable_column.into()
    } else {
        iced::widget::Column::with_children(elements).spacing(4).into()
    }
}
```

**Reorderable Conditions:**
- Only for Kline charts (`content_allows_dragging`)
- Only when 2+ indicators selected
- Uses `column_drag::Column` widget with drag events

### Market-Aware Filtering

```rust
fn content_row<'a, I>(
    pane: pane_grid::Pane,
    selected: &[I],
    market: exchange::adapter::MarketKind,
    allows_drag: bool,
) -> Element<'a, Message>
where
    I: Indicator + Copy + Into<UiIndicator>,
{
    let available: Vec<I> = I::for_market(market)
        .iter()
        .filter(|indicator| !selected.contains(indicator))
        .cloned()
        .collect();

    // ... render selected and available lists
}
```

**Logic:**
1. Get all indicators for market type via `I::for_market(market)`
2. Filter out already-selected indicators
3. Show remaining as available for selection

### Toggle Implementation
**Location:** `src/screen/dashboard/pane.rs`

```rust
pub fn toggle_indicator(&mut self, indicator: UiIndicator) {
    match (self, indicator) {
        (Content::Kline { chart, indicators, .. }, UiIndicator::Kline(ind)) => {
            let Some(chart) = chart else { return; };

            if let Some(pos) = indicators.iter().position(|i| *i == ind) {
                indicators.remove(pos);  // Remove if exists
            } else {
                indicators.push(ind);     // Add if doesn't exist
            }
            chart.toggle_indicator(ind);  // Update chart state
        }
        // ... similar for Heatmap
    }
}
```

**Flow:**
1. Button clicked → `Message::ToggleIndicator(pane, indicator)`
2. Pane state updates indicator list
3. Chart state toggles indicator (creates/destroys indicator impl)
4. UI re-renders with updated state

### Reorder Implementation

```rust
pub fn reorder_indicators(&mut self, event: &column_drag::DragEvent) {
    match self {
        Content::Kline { indicators, .. } => column_drag::reorder_vec(indicators, event),
        Content::Heatmap { indicators, .. } => column_drag::reorder_vec(indicators, event),
        // ...
    }
}
```

**Flow:**
1. User drags indicator → `Message::ReorderIndicator(pane, event)`
2. `column_drag::reorder_vec` updates order in indicator vector
3. Chart re-renders with new order

### Indicator Row Composition
**Location:** `src/chart/indicator.rs:23-68`

```rust
pub fn indicator_row<'a, P, Y>(
    main_chart: &'a ViewState,
    cache: &'a Caches,
    plot: P,
    datapoints: &'a BTreeMap<u64, Y>,
    visible_range: RangeInclusive<u64>,
) -> Element<'a, Message>
where
    P: Plot<AnySeries<'a, Y>> + 'a,
{
    let series = AnySeries::for_basis(main_chart.basis, datapoints);

    let (min, max) = plot
        .y_extents(&series, visible_range)
        .map(|(min, max)| plot.adjust_extents(min, max))
        .unwrap_or((0.0, 0.0));

    let canvas = Canvas::new(ChartCanvas { /* ... */ })
        .height(Length::Fill)
        .width(Length::Fill);

    let labels = Canvas::new(IndicatorLabel { /* ... */ })
        .height(Length::Fill)
        .width(main_chart.y_labels_width());

    row![
        canvas,
        rule::vertical(1).style(crate::style::split_ruler),
        container(labels),
    ]
    .into()
}
```

**Layout:**
```
┌─────────────────────────┬─┬────────┐
│                         │ │        │
│   ChartCanvas (plot)    │ │ Labels │
│                         │ │        │
└─────────────────────────┴─┴────────┘
        Fill width         1px  Fixed
```

---

## 6. Chart Canvas and Crosshair

### ChartCanvas Program
**Location:** `src/chart/indicator/plot.rs:174-354`

```rust
pub struct ChartCanvas<'a, P, S>
where
    P: Plot<S>,
    S: Series,
{
    pub indicator_cache: &'a Cache,
    pub crosshair_cache: &'a Cache,
    pub ctx: &'a ViewState,
    pub plot: P,
    pub series: S,
    pub max_for_labels: f32,
    pub min_for_labels: f32,
}

impl<P, S> canvas::Program<Message> for ChartCanvas<'_, P, S>
where
    P: Plot<S>,
    S: Series,
```

### Update Logic (Mouse Interaction)

```rust
fn update(
    &self,
    interaction: &mut Interaction,
    event: &canvas::Event,
    bounds: Rectangle,
    cursor: mouse::Cursor,
) -> Option<canvas::Action<Message>> {
    match event {
        canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
            let msg = matches!(*interaction, Interaction::None)
                .then(|| cursor.is_over(bounds))
                .and_then(|over| over.then_some(Message::CrosshairMoved));
            let action = msg.map_or(canvas::Action::request_redraw(), canvas::Action::publish);
            Some(match interaction {
                Interaction::None => action,
                _ => action.and_capture(),
            })
        }
        _ => None,
    }
}
```

**Behavior:**
- On cursor movement: Requests redraw to update crosshair
- Publishes `CrosshairMoved` message if not interacting
- Captures event during panning/zooming

### Draw Logic

#### 1. Indicator Layer (Cached)

```rust
let indicator = self.indicator_cache.draw(renderer, bounds.size(), |frame| {
    let center = Vector::new(bounds.width / 2.0, bounds.height / 2.0);

    frame.translate(center);
    frame.scale(ctx.scaling);
    frame.translate(Vector::new(
        ctx.translation.x,
        (-bounds.height / ctx.scaling) / 2.0,
    ));

    let width = frame.width() / ctx.scaling;
    let region = Rectangle {
        x: -ctx.translation.x - width / 2.0,
        y: 0.0,
        width,
        height: frame.height() / ctx.scaling,
    };
    let (earliest, latest) = ctx.interval_range(&region);

    let scale = YScale {
        min: self.min_for_labels,
        max: self.max_for_labels,
        px_height: frame.height() / ctx.scaling,
    };

    self.plot.draw(frame, ctx, theme, &self.series, earliest..=latest, &scale);
});
```

**Coordinate Transformations:**
1. Translate to center
2. Apply zoom scaling
3. Apply panning translation
4. Calculate visible region in world coordinates
5. Draw plot with transformed coordinates

#### 2. Crosshair Layer (Cached Separately)

**Vertical Crosshair (Snapped to Data):**
```rust
let crosshair = self.crosshair_cache.draw(renderer, bounds.size(), |frame| {
    let dashed = dashed_line(theme);
    if let Some(cursor_position) = cursor.position_in(ctx.bounds) {
        // Snap to basis (time or tick)
        let (rounded_x, snap_ratio) = match ctx.basis {
            Basis::Time(tf) => {
                let step = tf.to_milliseconds() as f64;
                let rx = ((earliest + crosshair_ratio * (latest - earliest)) / step).round() as u64 * step as u64;
                let sr = ((rx as f64 - earliest) / (latest - earliest)) as f32;
                (rx, sr)
            }
            Basis::Tick(_) => {
                let world_x = region.x + (cursor_position.x / bounds.width) * region.width;
                let snapped_world_x = (world_x / ctx.cell_width).round() * ctx.cell_width;
                let sr = (snapped_world_x - region.x) / region.width;
                let rx = ctx.x_to_interval(snapped_world_x);
                (rx, sr)
            }
        };

        frame.stroke(
            &Path::line(
                Point::new(snap_ratio * bounds.width, 0.0),
                Point::new(snap_ratio * bounds.width, bounds.height),
            ),
            dashed,
        );

        // Draw tooltip at snapped position
        if let Some(y) = self.series.at(rounded_x) {
            let next = self.series.next_after(rounded_x).map(|(_, v)| v);
            if let Some(tooltip) = self.plot.tooltip(y, next, theme) {
                tooltip.draw(frame, theme, bounds, cursor_position.x);
            }
        }
    }
});
```

**Snapping Logic:**
- **Time basis:** Round to nearest timeframe interval
- **Tick basis:** Round to nearest cell width
- Crosshair drawn at snapped position
- Tooltip shows data at snapped X coordinate

**Horizontal Crosshair (On Y-axis label area):**
```rust
else if let Some(cursor_position) = cursor.position_in(bounds) {
    let highest = self.max_for_labels;
    let lowest = self.min_for_labels;
    let tick = guesstimate_ticks(highest - lowest);

    let ratio = cursor_position.y / bounds.height;
    let value = highest + ratio * (lowest - highest);
    let rounded = round_to_tick(value, tick);
    let snap_ratio = (rounded - highest) / (lowest - highest);

    frame.stroke(
        &Path::line(
            Point::new(0.0, snap_ratio * bounds.height),
            Point::new(bounds.width, snap_ratio * bounds.height),
        ),
        dashed,
    );
}
```

**Snapping Logic:**
- Snaps to nearest tick based on value range
- Only shown when cursor over indicator (not main chart)

### Cache Strategy

**Three-Cache System:**
```rust
pub struct Caches {
    pub main: Cache,         // Indicator plot
    pub crosshair: Cache,    // Crosshair lines + tooltips
    pub y_labels: Cache,     // Y-axis labels
}

impl Caches {
    pub fn clear_all(&mut self) {
        self.main.clear();
        self.crosshair.clear();
        self.y_labels.clear();
    }

    pub fn clear_crosshair(&mut self) {
        self.crosshair.clear();
        self.y_labels.clear();
    }
}
```

**Invalidation Strategy:**
- **Data changes:** Clear all caches
- **Mouse movement:** Clear only crosshair caches
- **Pan/zoom:** Clear all caches (handled by main chart)

### Mouse Interaction States

```rust
fn mouse_interaction(
    &self,
    interaction: &Interaction,
    bounds: Rectangle,
    cursor: mouse::Cursor,
) -> mouse::Interaction {
    match interaction {
        Interaction::Panning { .. } => mouse::Interaction::Grabbing,
        Interaction::Zoomin { .. } => mouse::Interaction::ZoomIn,
        Interaction::None if cursor.is_over(bounds) => mouse::Interaction::Crosshair,
        _ => mouse::Interaction::default(),
    }
}
```

---

## 7. Adding Custom Indicators Guide

### Step-by-Step Implementation

#### Step 1: Add Enum Variant

**Choose indicator type:**
- **KlineIndicator** for time-series/tick charts
- **HeatmapIndicator** for volume profile charts

**Location:** `data/src/chart/indicator.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize, Eq, Enum)]
pub enum KlineIndicator {
    Volume,
    OpenInterest,
    MyCustomIndicator,  // Add your variant
}
```

#### Step 2: Update Market Availability

```rust
impl KlineIndicator {
    const FOR_SPOT: [KlineIndicator; 2] = [
        KlineIndicator::Volume,
        KlineIndicator::MyCustomIndicator,  // Add if available for spot
    ];

    const FOR_PERPS: [KlineIndicator; 3] = [
        KlineIndicator::Volume,
        KlineIndicator::OpenInterest,
        KlineIndicator::MyCustomIndicator,  // Add if available for perps
    ];
}
```

#### Step 3: Add Display Implementation

```rust
impl Display for KlineIndicator {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            KlineIndicator::Volume => write!(f, "Volume"),
            KlineIndicator::OpenInterest => write!(f, "Open Interest"),
            KlineIndicator::MyCustomIndicator => write!(f, "My Custom Indicator"),
        }
    }
}
```

#### Step 4: Create Indicator Struct

**Location:** `src/chart/indicator/kline/my_custom_indicator.rs`

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
use exchange::{Kline, Trade};
use std::{collections::BTreeMap, ops::RangeInclusive};

pub struct MyCustomIndicator {
    cache: Caches,
    data: BTreeMap<u64, f32>,  // Adjust data type as needed
}

impl MyCustomIndicator {
    pub fn new() -> Self {
        Self {
            cache: Caches::default(),
            data: BTreeMap::new(),
        }
    }
}
```

#### Step 5: Implement KlineIndicatorImpl

```rust
impl KlineIndicatorImpl for MyCustomIndicator {
    fn clear_all_caches(&mut self) {
        self.cache.clear_all();
    }

    fn clear_crosshair_caches(&mut self) {
        self.cache.clear_crosshair();
    }

    fn element<'a>(
        &'a self,
        chart: &'a ViewState,
        visible_range: RangeInclusive<u64>,
    ) -> iced::Element<'a, Message> {
        // Define tooltip
        let tooltip = |value: &f32, _next: Option<&f32>| {
            PlotTooltip::new(format!("Value: {}", format_with_commas(*value)))
        };

        // Define value extraction
        let value_fn = |v: &f32| *v;

        // Choose plot type: LinePlot or BarPlot
        let plot = LinePlot::new(value_fn)
            .stroke_width(1.0)
            .show_points(true)
            .padding(0.08)
            .with_tooltip(tooltip);

        indicator_row(chart, &self.cache, plot, &self.data, visible_range)
    }

    fn rebuild_from_source(&mut self, source: &PlotData<KlineDataPoint>) {
        match source {
            PlotData::TimeBased(timeseries) => {
                // Extract your data from timeseries
                self.data = timeseries.datapoints.iter().map(|dp| {
                    let custom_value = calculate_indicator_value(&dp.kline);
                    (dp.kline.time, custom_value)
                }).collect();
            }
            PlotData::TickBased(tickseries) => {
                // Extract your data from tickseries
                self.data = tickseries.datapoints.iter().enumerate().map(|(idx, dp)| {
                    let custom_value = calculate_indicator_value(&dp.kline);
                    (idx as u64, custom_value)
                }).collect();
            }
        }
        self.clear_all_caches();
    }

    fn on_insert_klines(&mut self, klines: &[Kline]) {
        for kline in klines {
            let custom_value = calculate_indicator_value(kline);
            self.data.insert(kline.time, custom_value);
        }
        self.clear_all_caches();
    }

    fn on_insert_trades(
        &mut self,
        _trades: &[Trade],
        old_dp_len: usize,
        source: &PlotData<KlineDataPoint>,
    ) {
        match source {
            PlotData::TimeBased(_) => return,
            PlotData::TickBased(tickseries) => {
                let start_idx = old_dp_len.saturating_sub(1);
                for (idx, dp) in tickseries.datapoints.iter().enumerate().skip(start_idx) {
                    let custom_value = calculate_indicator_value(&dp.kline);
                    self.data.insert(idx as u64, custom_value);
                }
            }
        }
        self.clear_all_caches();
    }

    fn on_ticksize_change(&mut self, source: &PlotData<KlineDataPoint>) {
        self.rebuild_from_source(source);
    }

    fn on_basis_change(&mut self, source: &PlotData<KlineDataPoint>) {
        self.rebuild_from_source(source);
    }
}

// Helper function to calculate your indicator
fn calculate_indicator_value(kline: &Kline) -> f32 {
    // Your custom calculation logic
    (kline.high + kline.low) / 2.0  // Example: midpoint
}
```

#### Step 6: Register in Factory

**Location:** `src/chart/indicator/kline.rs`

```rust
pub mod my_custom_indicator;  // Add module declaration

pub fn make_empty(which: KlineIndicator) -> Box<dyn KlineIndicatorImpl> {
    match which {
        KlineIndicator::Volume => Box::new(super::kline::volume::VolumeIndicator::new()),
        KlineIndicator::OpenInterest => Box::new(super::kline::open_interest::OpenInterestIndicator::new()),
        KlineIndicator::MyCustomIndicator => Box::new(super::kline::my_custom_indicator::MyCustomIndicator::new()),
    }
}
```

#### Step 7: Add Module Declaration

**Location:** `src/chart/indicator/kline.rs`

Add at the top of the file:
```rust
pub mod my_custom_indicator;
```

### Advanced Features

#### Custom Data Fetching

If your indicator needs external data (like Open Interest):

```rust
fn fetch_range(&mut self, ctx: &FetchCtx) -> Option<FetchRange> {
    // Check if data is needed
    let (earliest_data, latest_data) = self.data_range();

    if ctx.visible_earliest < earliest_data {
        return Some(FetchRange::Custom {
            from: ctx.prefetch_earliest,
            to: earliest_data,
        });
    }

    None
}

fn on_custom_data_arrival(&mut self, data: &[CustomData]) {
    self.data.extend(data.iter().map(|d| (d.time, d.value)));
    self.clear_all_caches();
}
```

#### Complex Data Structures

For indicators needing multiple values per point:

```rust
pub struct ComplexIndicator {
    cache: Caches,
    data: BTreeMap<u64, IndicatorData>,
}

struct IndicatorData {
    value: f32,
    upper_band: f32,
    lower_band: f32,
}
```

Use multiple plots or custom rendering:
```rust
fn element<'a>(&'a self, chart: &'a ViewState, visible_range: RangeInclusive<u64>) -> iced::Element<'a, Message> {
    // Transform data for main line
    let main_data: BTreeMap<u64, f32> = self.data.iter()
        .map(|(k, v)| (*k, v.value))
        .collect();

    // Create main plot
    let main_plot = LinePlot::new(|v: &f32| *v)
        .with_tooltip(/* ... */);

    indicator_row(chart, &self.cache, main_plot, &main_data, visible_range)
}
```

#### Exchange/Timeframe Restrictions

Like Open Interest, restrict availability:

```rust
fn element<'a>(&'a self, chart: &'a ViewState, visible_range: RangeInclusive<u64>) -> iced::Element<'a, Message> {
    let exchange = chart.ticker_info.exchange();

    if !Self::is_supported_exchange(exchange) {
        return center(text(format!("Not available for {exchange}"))).into();
    }

    // ... normal rendering
}

impl MyCustomIndicator {
    pub fn is_supported_exchange(exchange: Exchange) -> bool {
        matches!(exchange, Exchange::Binance | Exchange::Bybit)
    }
}
```

### Testing Checklist

- [ ] Indicator appears in UI menu for correct market types
- [ ] Toggle on/off works correctly
- [ ] Data loads on initial chart load
- [ ] Data updates on new klines/trades
- [ ] Tooltips display correct values
- [ ] Crosshair snaps correctly
- [ ] Cache invalidation works (smooth rendering)
- [ ] Timeframe changes rebuild data correctly
- [ ] Tick chart mode works (if applicable)
- [ ] Ticksize changes rebuild data correctly
- [ ] Basis changes (time ↔ tick) work correctly
- [ ] Pan/zoom interactions work smoothly
- [ ] Drag-to-reorder works (for kline indicators)
- [ ] No panics with empty data
- [ ] Performance acceptable with large datasets

### Common Patterns

**Pattern 1: Simple Value Indicator (like midpoint price)**
- Data: `BTreeMap<u64, f32>`
- Plot: `LinePlot`
- Calculation: Direct from kline OHLC

**Pattern 2: Dual-Value Indicator (like volume with buy/sell)**
- Data: `BTreeMap<u64, (f32, f32)>`
- Plot: `BarPlot` with `BarClass::Overlay`
- Calculation: Aggregate from kline volume tuple

**Pattern 3: External Data Indicator (like open interest)**
- Data: `BTreeMap<u64, f32>`
- Plot: `LinePlot`
- Fetching: Implement `fetch_range()` and custom data handler
- Source: Network fetch, not derived from klines

**Pattern 4: Multi-Band Indicator (like Bollinger Bands)**
- Data: `BTreeMap<u64, BandData>` with multiple fields
- Plot: Multiple `LinePlot` instances or custom rendering
- Calculation: Statistical computation over rolling window

---

## Summary

FlowSurface's indicator system is built on:

1. **Trait-based polymorphism** for extensibility
2. **Series abstraction** for time/tick chart compatibility
3. **Plot trait** for flexible rendering (bars, lines, custom)
4. **Lifecycle hooks** for efficient data synchronization
5. **Cache strategy** for performance optimization
6. **Market-aware configuration** for context-specific availability

Adding a new indicator requires:
- Enum variant + Display implementation
- Struct implementing `KlineIndicatorImpl` or `HeatmapIndicatorImpl`
- Plot configuration (BarPlot or LinePlot)
- Factory registration
- Lifecycle hook implementations

The architecture separates concerns cleanly: data models (in `data/`), rendering logic (Plot trait), UI integration (indicators pane), and state management (lifecycle hooks).
