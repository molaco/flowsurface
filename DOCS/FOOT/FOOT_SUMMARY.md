# Footprint Chart: Concise Step-by-Step

## Overview

This document provides a condensed explanation of how the footprint chart works in FlowSurface, from data ingestion to final rendering.

---

## Step-by-Step Flow

### 1. **Initialization**
```
User creates footprint pane
  ↓
Fetch historical klines + trades from exchange
  ↓
KlineChart::new(klines, trades, kind: Footprint)
```

### 2. **Data Aggregation**
```
TimeSeries::new()
  ↓
For each trade:
  - Find time bucket: (trade.time / interval) * interval
  - Round price to tick: trade.price.round_to_step(tick_size)
  - Accumulate: trades[price].buy_qty += qty (or sell_qty)
  ↓
Result: BTreeMap<timestamp, KlineDataPoint { kline, footprint }>
  where footprint = HashMap<Price, GroupedTrades { buy_qty, sell_qty }>
```

### 3. **POC Calculation**
```
For each candle:
  Find price with max (buy_qty + sell_qty)
  Store as PointOfControl { price, volume, status: Naked }
```

### 4. **Live Updates**
```
WebSocket → trades
  ↓
insert_trades_buffer(trades)
  ↓
Same aggregation logic (step 2)
  ↓
invalidate() → triggers re-render
```

### 5. **Rendering**
```
canvas::draw()
  ↓
A. Calculate visible range & max quantity
   - max_qty = scan visible candles, find max cluster qty
  ↓
B. For each visible candle:
   - x_position = interval_to_x(timestamp)
   - For each price level in footprint:
     * y = price_to_y(price)
     * bar_width = (qty / max_qty) * area_width
     * draw_rectangle(x, y, bar_width, color)
  ↓
C. Draw extras:
   - NPoC lines (yellow = naked, gray = filled)
   - Imbalance markers (compare diagonal cells)
   - Thin candle wick + body
```

---

## Key Formula

```
Trade → Price bin (round_to_step)
      → Time bin (timestamp / interval)
      → GroupedTrades { buy_qty, sell_qty }
      → bar_width = (qty / max_qty) * area_width
      → Rectangle on canvas
```

---

## Data Flow Pipeline

```
Trade
  ↓
GroupedTrades (accumulated by price)
  ↓
KlineDataPoint (one per time interval)
  ↓
Canvas Rectangle (rendered on screen)
```

---

## Three Display Modes

1. **BidAsk**: Split bars (buy on right, sell on left)
2. **VolumeProfile**: Stacked bars (total volume)
3. **DeltaProfile**: Net delta bars (buy - sell)

---

## Core Data Structures

```rust
Trade {
    price: Price,
    qty: f32,
    time: u64,
    is_sell: bool,
}
  ↓
GroupedTrades {
    buy_qty: f32,
    sell_qty: f32,
    buy_count: usize,
    sell_count: usize,
    first_time: u64,
    last_time: u64,
}
  ↓
KlineDataPoint {
    kline: Kline,  // OHLC data
    footprint: KlineTrades {
        trades: FxHashMap<Price, GroupedTrades>,
        poc: Option<PointOfControl>,
    }
}
```

---

## Key Files

- **Data Layer**: `data/src/chart/kline.rs`, `data/src/aggr/time.rs`
- **Rendering**: `src/chart/kline.rs`
- **Settings**: `src/modal/pane/settings.rs`
- **Integration**: `src/screen/dashboard.rs`

---

## Performance Optimizations

1. **Price binning**: `round_to_step()` reduces price levels to manageable count
2. **Visible range culling**: Only render candles in viewport
3. **BTreeMap range queries**: O(log n) lookup for time ranges
4. **FxHashMap**: Fast hashing for price → trades lookup
5. **Canvas caching**: Separate caches for main chart vs crosshair

---

## Studies

- **NPoC (Naked Point of Control)**: Tracks POC price levels that haven't been revisited
- **Imbalance**: Detects buy/sell volume imbalances between diagonal price levels (configurable threshold)

---

For complete implementation details, see [FOOT.md](./FOOT.md)
