# Technical Architecture Documentation

## Overview

Flowsurface is a high-performance desktop cryptocurrency charting application built with Rust, leveraging the Iced GUI framework for cross-platform compatibility. The architecture follows a modular workspace design with clear separation between GUI, data management, and exchange connectivity layers.

## System Architecture

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Desktop Application                       │
│                   (Iced GUI Framework)                      │
├─────────────────────────────────────────────────────────────┤
│  Main Crate (src/)                                          │
│  ├── GUI Components (screen/, modal/, widget/)              │
│  ├── Chart System (chart/, indicator/)                      │
│  ├── Layout Management (layout.rs)                          │
│  ├── Theme System (style.rs)                                │
│  └── Window Management (window.rs, main.rs)                 │
├─────────────────────────────────────────────────────────────┤
│  Data Workspace (data/)                                     │
│  ├── Configuration Management (config/)                     │
│  ├── Chart Data Aggregation (chart/, aggr/)                 │
│  ├── Layout Persistence (layout/)                           │
│  ├── Audio System (audio.rs)                                │
│  └── Utility Functions (util.rs)                            │
├─────────────────────────────────────────────────────────────┤
│  Exchange Workspace (exchange/)                             │
│  ├── Exchange Adapters (adapter/)                           │
│  ├── WebSocket Management (connect.rs)                      │
│  ├── Data Fetching (fetcher.rs)                             │
│  ├── Rate Limiting (limiter.rs)                             │
│  └── Market Data Types (depth.rs)                           │
└─────────────────────────────────────────────────────────────┘
```

### Core Components

#### 1. Main Application Layer (`src/`)

**Primary Framework**: Iced GUI with Daemon Architecture
- **Entry Point**: `main.rs` - Initializes Iced daemon with window management
- **Window Management**: `window.rs` - Multi-window support with popout panels
- **Application State**: Centralized state management through Iced's Message pattern

**Key Design Patterns**:
- **Message-Driven Architecture**: All state updates flow through Iced's `Message` enum
- **Async Task Integration**: Seamless integration with Tokio runtime for exchange connections
- **Element/View Pattern**: Functional UI composition using Iced's Element system

#### 2. GUI System

**Screen Management** (`src/screen/`)
- `dashboard.rs` - Main dashboard layout and coordination
- `dashboard/pane.rs` - Individual pane management and content
- `dashboard/sidebar.rs` - Ticker selection and application controls
- `dashboard/tickers_table.rs` - Sortable ticker table with filtering

**Modal System** (`src/modal/`)
- `theme_editor.rs` - Runtime theme customization with live preview
- `layout_manager.rs` - Layout creation, modification, and persistence
- `pane/` - Pane-specific settings and configuration modals
- `audio.rs` - Audio system configuration and testing

**Custom Widgets** (`src/widget/`)
- `color_picker.rs` - HSV color picker with palette support
- `multi_split.rs` - Dynamic pane splitting and resizing
- `toast.rs` - Non-blocking notification system
- `column_drag.rs` - Draggable table column reordering
- `decorate.rs` - Visual decoration utilities

#### 3. Chart System

**Core Chart Types** (`src/chart/`)
- `heatmap.rs` - Historical DOM visualization with volume profile
- `kline.rs` - Traditional candlestick charts with custom intervals
- `indicator/` - Technical indicator system with extensible plotters

**Scaling and Rendering** (`src/chart/scale/`)
- `timeseries.rs` - Time-based coordinate transformation
- `linear.rs` - Price-based coordinate transformation
- Real-time data synchronization across multiple chart types

**Advanced Features**:
- **Footprint Charts**: Price-grouped trade visualization with imbalance detection
- **Volume Profile**: Fixed and visible range volume analysis
- **Time & Sales**: Real-time trade stream display
- **Multi-Timeframe Support**: Synchronized chart updates across different intervals

#### 4. Data Management Layer (`data/`)

**Configuration System** (`data/config/`)
- `theme.rs` - Theme persistence with custom palette support
- `sidebar.rs` - Sidebar state and ticker preferences
- `state.rs` - Application state serialization/deserialization
- `timezone.rs` - Timezone handling for market data

**Chart Data Processing** (`data/chart/`)
- `heatmap.rs` - DOM data aggregation and heat map generation
- `kline.rs` - OHLCV data processing and candlestick formation
- `indicator.rs` - Technical indicator calculation and caching
- `timeandsales.rs` - Trade stream processing and filtering

**Data Aggregation** (`data/aggr/`)
- `time.rs` - Time-based data aggregation (1m, 5m, 1h, etc.)
- `ticks.rs` - Tick-based data aggregation for volume-based intervals

**Persistence Layer**:
- JSON-based configuration storage (`saved-state.json`)
- Automatic cleanup of old market data
- Cross-platform data directory management using `data_path()`

#### 5. Exchange Integration Layer (`exchange/`)

**Exchange Adapters** (`exchange/adapter/`)
- `binance.rs` - Binance futures and spot market integration
- `bybit.rs` - Bybit derivatives market integration  
- `hyperliquid.rs` - Hyperliquid perpetuals integration

**Connection Management** (`exchange/`)
- `connect.rs` - WebSocket connection management with auto-reconnection
- `limiter.rs` - Exchange-specific rate limiting and throttling
- `fetcher.rs` - Historical data fetching and backfill operations
- `depth.rs` - Order book depth data structures

**Key Features**:
- **Unified Trait System**: Common interface across all exchange implementations
- **Auto-Reconnection**: Exponential backoff with connection health monitoring
- **Rate Limiting**: Exchange-specific request throttling to prevent API bans
- **Error Recovery**: Graceful degradation with comprehensive error handling

## Data Flow Architecture

### Real-Time Data Pipeline

```
Exchange WebSocket → Connection Manager → Data Aggregation → Chart Updates → GUI Rendering
                         ↓                      ↓                ↓
                   Auto-Reconnection     Time/Tick Grouping   Iced Messages
                         ↓                      ↓                ↓
                   Rate Limiting         Volume Profiles    Theme Application
```

### Message Flow Pattern

```rust
// Simplified message flow structure
enum Message {
    // Exchange events
    TradeReceived(Trade),
    DepthUpdated(Depth),
    ConnectionStatus(Status),
    
    // GUI events
    TickerSelected(Ticker),
    ThemeChanged(Theme),
    LayoutModified(Layout),
    
    // Chart events
    ChartScaled(Scale),
    IndicatorToggled(Indicator),
    TimeframeChanged(Timeframe),
}
```

## Performance Characteristics

### Memory Management
- **Bounded Data Structures**: Chart data is bounded to prevent memory leaks
- **Efficient Serialization**: Using sonic-rs for high-performance JSON processing
- **Resource Cleanup**: Automatic cleanup of old market data and connections

### Real-Time Processing
- **Lock-Free Data Structures**: Using Rust's ownership model to avoid locks
- **Async WebSocket Handling**: Non-blocking I/O with Tokio runtime
- **Optimized Rendering**: Iced's retained mode GUI with selective updates

### Scalability
- **Multi-Exchange Support**: Concurrent connections to multiple exchanges
- **Multi-Timeframe Data**: Efficient aggregation across different time intervals
- **Multi-Window Support**: Independent chart windows with shared data

## Dependencies and Technology Stack

### Core Framework
- **Iced 0.14.0-dev** - Cross-platform GUI framework with hardware acceleration
- **Tokio** - Async runtime for WebSocket connections and background tasks
- **Serde** - Serialization framework for configuration and data persistence

### Exchange Connectivity
- **fastwebsockets** - High-performance WebSocket client implementation
- **reqwest** - HTTP client for REST API interactions
- **sonic-rs** - High-performance JSON parser for market data

### Data Processing
- **chrono** - Date and time handling with timezone support
- **rust_decimal** - High-precision decimal arithmetic for financial calculations
- **ordered-float** - Ordered floating-point numbers for price data

### Audio and Multimedia
- **rodio** - Cross-platform audio playback for trade notifications
- **palette** - Color space manipulation for theme system

### System Integration
- **dirs-next** - Cross-platform directory path resolution
- **zip** - Archive handling for historical data downloads
- **csv** - CSV parsing for market data import

## Build and Deployment

### Build Configuration
- **Rust Edition 2024** - Latest language features and optimizations
- **Release Optimization** - Aggressive optimization for production builds
- **Cross-Platform Targets** - Windows, macOS, and Linux support

### Development Features
- **Hot Reloading** - Enabled with `--features debug` flag
- **Logging System** - Comprehensive logging with configurable levels
- **Code Formatting** - rustfmt with 100-character line width
- **Linting** - Clippy with project-specific configuration

### Packaging Scripts
- `scripts/build-windows.sh` - Windows cross-compilation with MinGW
- `scripts/build-macos.sh` - Universal macOS binary creation
- `scripts/package-linux.sh` - Linux AppImage packaging

## Security Considerations

### Network Security
- **TLS Encryption** - All exchange connections use TLS 1.3
- **Certificate Validation** - WebPKI root certificate validation
- **Rate Limiting** - Built-in protection against API abuse

### Data Security
- **Local Storage Only** - No external data transmission beyond exchange APIs
- **Configuration Encryption** - Sensitive settings can be encrypted at rest
- **Memory Safety** - Rust's ownership model prevents common vulnerabilities

### Application Security
- **Sandboxed Environment** - Desktop application with minimal system access
- **Input Validation** - Comprehensive validation of user input and market data
- **Error Boundaries** - Graceful error handling prevents application crashes

## Testing and Quality Assurance

### Testing Strategy
- **Unit Tests** - Core logic and data processing functions
- **Integration Tests** - Exchange adapter and WebSocket connection testing
- **GUI Testing** - Iced component testing with mock data
- **Performance Testing** - Load testing with high-frequency market data

### Quality Metrics
- **Code Coverage** - Comprehensive test coverage across all modules
- **Memory Profiling** - Regular memory usage analysis and optimization
- **Performance Benchmarking** - Latency and throughput measurement
- **Cross-Platform Testing** - Validation across Windows, macOS, and Linux

This technical architecture provides the foundation for a robust, scalable, and performant cryptocurrency charting application with real-time capabilities and professional-grade features.