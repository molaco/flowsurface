# API Reference Documentation

## Overview

Flowsurface provides various internal APIs and data structures for extending and integrating with the application. This reference covers the main public interfaces, data structures, and integration points.

## Core Data Structures

### Market Data Types

#### Trade
```rust
pub struct Trade {
    pub price: OrderedFloat<f64>,
    pub qty: OrderedFloat<f64>,
    pub time: i64,
    pub is_buyer_maker: bool,
}
```

**Fields**:
- `price`: Trade execution price (ordered float for sorting)
- `qty`: Trade quantity/volume
- `time`: Unix timestamp in milliseconds
- `is_buyer_maker`: True if buyer was maker (sell-side liquidity)

**Usage**:
- Real-time trade processing in chart systems
- Historical trade data aggregation
- Volume analysis and footprint calculations

#### Kline (OHLCV Data)
```rust
pub struct Kline {
    pub open_time: i64,
    pub close_time: i64,
    pub open: OrderedFloat<f64>,
    pub high: OrderedFloat<f64>,
    pub low: OrderedFloat<f64>,
    pub close: OrderedFloat<f64>,
    pub volume: OrderedFloat<f64>,
    pub quote_volume: OrderedFloat<f64>,
    pub count: u32,
}
```

**Fields**:
- `open_time`/`close_time`: Candlestick time boundaries (Unix timestamps)
- `open`/`high`/`low`/`close`: OHLC price data
- `volume`: Base asset volume
- `quote_volume`: Quote asset volume (e.g., USDT volume for BTC/USDT)
- `count`: Number of trades in the period

**Usage**:
- Candlestick chart rendering
- Technical indicator calculations
- Time-series analysis

#### Depth (Order Book)
```rust
pub struct Depth {
    pub last_update_id: u64,
    pub bids: Vec<[OrderedFloat<f64>; 2]>,
    pub asks: Vec<[OrderedFloat<f64>; 2]>,
}
```

**Fields**:
- `last_update_id`: Exchange-specific update identifier
- `bids`: Array of [price, quantity] bid levels
- `asks`: Array of [price, quantity] ask levels

**Usage**:
- Order book visualization
- Market depth analysis
- Liquidity calculations

### Configuration Types

#### Ticker
```rust
pub struct Ticker {
    pub symbol: SmolStr,
    pub exchange: Exchange,
    pub base: SmolStr,
    pub quote: SmolStr,
}
```

**Fields**:
- `symbol`: Trading pair symbol (e.g., "BTCUSDT")
- `exchange`: Exchange enumeration
- `base`: Base asset symbol (e.g., "BTC")
- `quote`: Quote asset symbol (e.g., "USDT")

#### Theme
```rust
pub struct Theme {
    pub palette: Palette,
    pub background: Background,
    pub text: Text,
    pub primary: Primary,
    pub success: Success,
    pub danger: Danger,
}
```

**Usage**:
- Runtime theme customization
- Color scheme management
- UI styling coordination

## Exchange Integration API

### Exchange Trait
```rust
pub trait Exchange {
    async fn connect(&self) -> Result<Connection, ExchangeError>;
    async fn subscribe_trades(&self, ticker: &Ticker) -> Result<(), ExchangeError>;
    async fn subscribe_depth(&self, ticker: &Ticker) -> Result<(), ExchangeError>;
    async fn fetch_klines(&self, ticker: &Ticker, interval: Interval, limit: u16) -> Result<Vec<Kline>, ExchangeError>;
    fn rate_limiter(&self) -> &RateLimiter;
}
```

**Methods**:
- `connect()`: Establish WebSocket connection
- `subscribe_trades()`: Subscribe to real-time trade feed
- `subscribe_depth()`: Subscribe to order book updates
- `fetch_klines()`: Fetch historical candlestick data
- `rate_limiter()`: Access rate limiting controls

### Connection Management
```rust
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Error(String),
}

pub struct Connection {
    pub status: ConnectionStatus,
    pub last_ping: Option<Instant>,
    pub reconnect_attempts: u32,
}
```

### Rate Limiting
```rust
pub struct RateLimiter {
    requests_per_minute: u32,
    requests_per_second: u32,
    current_requests: AtomicU32,
    last_reset: AtomicU64,
}

impl RateLimiter {
    pub async fn acquire(&self) -> Result<(), RateLimitError>;
    pub fn remaining_requests(&self) -> u32;
    pub fn reset_time(&self) -> Duration;
}
```

## Chart System API

### Chart Trait
```rust
pub trait Chart {
    type Data;
    type Settings;
    
    fn new(settings: Self::Settings) -> Self;
    fn update(&mut self, data: Self::Data);
    fn view(&self) -> Element<Message>;
    fn settings_view(&self) -> Element<Message>;
}
```

### Scaling System
```rust
pub struct TimeScale {
    pub start: i64,
    pub end: i64,
    pub width: f32,
}

impl TimeScale {
    pub fn time_to_x(&self, time: i64) -> f32;
    pub fn x_to_time(&self, x: f32) -> i64;
    pub fn zoom(&mut self, factor: f32, center: f32);
    pub fn pan(&mut self, delta: f32);
}

pub struct PriceScale {
    pub min: f64,
    pub max: f64,
    pub height: f32,
}

impl PriceScale {
    pub fn price_to_y(&self, price: f64) -> f32;
    pub fn y_to_price(&self, y: f32) -> f64;
    pub fn auto_scale(&mut self, data: &[f64]);
}
```

### Indicator System
```rust
pub trait Indicator {
    type Input;
    type Output;
    type Settings;
    
    fn new(settings: Self::Settings) -> Self;
    fn calculate(&mut self, input: Self::Input) -> Self::Output;
    fn reset(&mut self);
}

// Example: Simple Moving Average
pub struct SMA {
    period: usize,
    values: VecDeque<f64>,
}

impl Indicator for SMA {
    type Input = f64;
    type Output = Option<f64>;
    type Settings = usize; // period
    
    fn calculate(&mut self, price: f64) -> Option<f64> {
        self.values.push_back(price);
        if self.values.len() > self.period {
            self.values.pop_front();
        }
        
        if self.values.len() == self.period {
            Some(self.values.iter().sum::<f64>() / self.period as f64)
        } else {
            None
        }
    }
}
```

## Data Aggregation API

### Time-based Aggregation
```rust
pub struct TimeAggregator {
    interval: Duration,
    current_bucket: Option<TimeBucket>,
}

pub struct TimeBucket {
    pub start_time: i64,
    pub end_time: i64,
    pub trades: Vec<Trade>,
    pub volume: f64,
    pub high: f64,
    pub low: f64,
}

impl TimeAggregator {
    pub fn new(interval: Duration) -> Self;
    pub fn add_trade(&mut self, trade: Trade) -> Option<TimeBucket>;
    pub fn current_bucket(&self) -> Option<&TimeBucket>;
    pub fn close_bucket(&mut self) -> Option<TimeBucket>;
}
```

### Volume-based Aggregation
```rust
pub struct VolumeAggregator {
    target_volume: f64,
    current_bucket: Option<VolumeBucket>,
}

pub struct VolumeBucket {
    pub trades: Vec<Trade>,
    pub total_volume: f64,
    pub start_time: i64,
    pub end_time: i64,
}
```

## Widget System API

### Custom Widget Development
```rust
pub trait Widget<Message> {
    fn update(&mut self, message: Message);
    fn view(&self) -> Element<Message>;
    fn subscription(&self) -> Subscription<Message>;
}

// Example: Color Picker Widget
pub struct ColorPicker {
    color: Color,
    is_open: bool,
    hue: f32,
    saturation: f32,
    value: f32,
}

impl Widget<ColorPickerMessage> for ColorPicker {
    fn update(&mut self, message: ColorPickerMessage) {
        match message {
            ColorPickerMessage::HueChanged(hue) => {
                self.hue = hue;
                self.update_color();
            }
            ColorPickerMessage::SaturationChanged(sat) => {
                self.saturation = sat;
                self.update_color();
            }
            // ... other message handling
        }
    }
    
    fn view(&self) -> Element<ColorPickerMessage> {
        // Widget rendering implementation
    }
}
```

### Layout Widgets
```rust
pub struct MultiSplit<Message> {
    panes: PaneGrid<PaneContent>,
    splits: HashMap<pane_grid::Split, f32>,
}

impl<Message> MultiSplit<Message> {
    pub fn new() -> Self;
    pub fn add_pane(&mut self, content: PaneContent) -> pane_grid::Pane;
    pub fn split(&mut self, pane: pane_grid::Pane, axis: Axis) -> Result<pane_grid::Split, SplitError>;
    pub fn close(&mut self, pane: pane_grid::Pane);
    pub fn resize(&mut self, split: pane_grid::Split, ratio: f32);
}
```

## Configuration API

### Settings Management
```rust
pub struct Settings {
    pub theme: Theme,
    pub sidebar: SidebarConfig,
    pub layout: LayoutConfig,
    pub exchanges: ExchangeConfig,
}

impl Settings {
    pub fn load() -> Result<Self, ConfigError>;
    pub fn save(&self) -> Result<(), ConfigError>;
    pub fn reset_to_defaults(&mut self);
    pub fn migrate_from_version(&mut self, version: &str) -> Result<(), MigrationError>;
}

pub trait Configurable {
    type Config;
    fn apply_config(&mut self, config: &Self::Config);
    fn get_config(&self) -> Self::Config;
}
```

### State Persistence
```rust
pub struct AppState {
    pub settings: Settings,
    pub layout: DashboardLayout,
    pub sidebar_state: SidebarState,
    pub window_states: Vec<WindowState>,
}

impl AppState {
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), StateError>;
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, StateError>;
    pub fn auto_save(&self);
}
```

## Event System

### Message Types
```rust
pub enum Message {
    // Exchange messages
    TradeReceived(Exchange, Trade),
    DepthUpdated(Exchange, Depth),
    KlineUpdated(Exchange, Kline),
    ConnectionStatusChanged(Exchange, ConnectionStatus),
    
    // UI messages
    TickerSelected(Ticker),
    ChartTypeChanged(ChartType),
    TimeframeChanged(Timeframe),
    ThemeChanged(Theme),
    
    // Layout messages
    PaneAdded(PaneConfig),
    PaneRemoved(PaneId),
    PaneSplit(PaneId, Axis),
    LayoutSaved(String),
    LayoutLoaded(String),
    
    // Settings messages
    SettingsOpened,
    SettingsClosed,
    SettingsChanged(SettingCategory),
}
```

### Event Handling
```rust
pub trait EventHandler<T> {
    fn handle_event(&mut self, event: T) -> Vec<Command<Message>>;
}

pub struct EventBus {
    handlers: Vec<Box<dyn EventHandler<Message>>>,
}

impl EventBus {
    pub fn register<H: EventHandler<Message> + 'static>(&mut self, handler: H);
    pub fn publish(&mut self, message: Message) -> Vec<Command<Message>>;
    pub fn subscribe<F>(&mut self, filter: F) where F: Fn(&Message) -> bool + 'static;
}
```

## Error Handling

### Error Types
```rust
#[derive(Debug, thiserror::Error)]
pub enum FlowsurfaceError {
    #[error("Exchange error: {0}")]
    Exchange(#[from] ExchangeError),
    
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),
    
    #[error("Chart error: {0}")]
    Chart(#[from] ChartError),
    
    #[error("Network error: {0}")]
    Network(#[from] NetworkError),
    
    #[error("Data processing error: {0}")]
    DataProcessing(String),
}

#[derive(Debug, thiserror::Error)]
pub enum ExchangeError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
    
    #[error("WebSocket error: {0}")]
    WebSocket(String),
}
```

### Error Recovery
```rust
pub trait Recoverable {
    fn can_recover(&self) -> bool;
    fn recover(&mut self) -> Result<(), Self::Error>;
}

pub struct ErrorRecovery {
    max_attempts: u32,
    backoff_strategy: BackoffStrategy,
    recovery_handlers: HashMap<ErrorType, Box<dyn Fn(&Error) -> RecoveryAction>>,
}
```

## Performance Monitoring

### Metrics Collection
```rust
pub struct Metrics {
    pub chart_render_time: HistogramMetric,
    pub data_processing_time: HistogramMetric,
    pub memory_usage: GaugeMetric,
    pub connection_count: GaugeMetric,
    pub message_throughput: CounterMetric,
}

pub trait MetricsCollector {
    fn record_duration(&self, name: &str, duration: Duration);
    fn increment_counter(&self, name: &str, value: u64);
    fn set_gauge(&self, name: &str, value: f64);
    fn record_histogram(&self, name: &str, value: f64);
}
```

### Performance Profiling
```rust
pub struct Profiler {
    active_spans: HashMap<SpanId, Span>,
    completed_spans: Vec<CompletedSpan>,
}

impl Profiler {
    pub fn start_span(&mut self, name: &str) -> SpanGuard;
    pub fn record_event(&mut self, name: &str, data: serde_json::Value);
    pub fn export_profile(&self) -> ProfileData;
}

#[macro_export]
macro_rules! profile_span {
    ($profiler:expr, $name:expr, $block:block) => {
        let _guard = $profiler.start_span($name);
        $block
    };
}
```

## Extension Points

### Plugin System (Future)
```rust
pub trait Plugin {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn initialize(&mut self, context: &PluginContext) -> Result<(), PluginError>;
    fn shutdown(&mut self);
    fn handle_message(&mut self, message: Message) -> Vec<Command<Message>>;
}

pub struct PluginManager {
    plugins: Vec<Box<dyn Plugin>>,
    plugin_configs: HashMap<String, serde_json::Value>,
}
```

### Custom Chart Types
```rust
pub trait CustomChart: Chart {
    fn chart_type_name(&self) -> &'static str;
    fn supported_data_types(&self) -> Vec<DataType>;
    fn default_settings(&self) -> Self::Settings;
    fn settings_schema(&self) -> serde_json::Value;
}

pub struct ChartRegistry {
    charts: HashMap<String, Box<dyn Fn() -> Box<dyn CustomChart>>>,
}
```

## Integration Examples

### Adding a Custom Exchange
```rust
use flowsurface::exchange::{Exchange, ExchangeError, Connection, RateLimiter};

pub struct MyExchange {
    rate_limiter: RateLimiter,
    base_url: String,
    ws_url: String,
}

#[async_trait]
impl Exchange for MyExchange {
    async fn connect(&self) -> Result<Connection, ExchangeError> {
        // WebSocket connection implementation
    }
    
    async fn subscribe_trades(&self, ticker: &Ticker) -> Result<(), ExchangeError> {
        // Trade subscription implementation
    }
    
    // ... other trait methods
}
```

### Creating a Custom Indicator
```rust
use flowsurface::chart::{Indicator, IndicatorSettings};

pub struct MACD {
    fast_ema: EMA,
    slow_ema: EMA,
    signal_ema: EMA,
    settings: MACDSettings,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct MACDSettings {
    fast_period: usize,
    slow_period: usize,
    signal_period: usize,
}

impl Indicator for MACD {
    type Input = f64;
    type Output = MACDValue;
    type Settings = MACDSettings;
    
    fn calculate(&mut self, price: f64) -> MACDValue {
        let fast = self.fast_ema.calculate(price);
        let slow = self.slow_ema.calculate(price);
        
        if let (Some(fast_val), Some(slow_val)) = (fast, slow) {
            let macd_line = fast_val - slow_val;
            let signal_line = self.signal_ema.calculate(macd_line);
            
            MACDValue {
                macd: macd_line,
                signal: signal_line,
                histogram: signal_line.map(|s| macd_line - s),
            }
        } else {
            MACDValue::default()
        }
    }
}
```

This API reference provides the foundation for extending Flowsurface with custom functionality, integrations, and features while maintaining compatibility with the existing architecture.