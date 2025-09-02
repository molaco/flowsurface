# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Important files

@RULES.md
@FLAGS.md
@AGENT_CONTEXT.md
@docs/*
@.claude/commands/*
@CHANGES.md

## Project Overview

Flowsurface is a Rust-based desktop charting application for cryptocurrency trading analysis. It provides real-time market data visualization with support for multiple chart types, exchanges (Binance, Bybit, Hyperliquid), and advanced trading features.

## Common Development Commands

### Building and Running
- `cargo build --release` - Build the release version
- `cargo run --release` - Build and run the release version
- `cargo build` - Build debug version
- `cargo run` - Run debug version

### Development Tools
- `cargo fmt` - Format code (max_width=100, edition="2024")
- `cargo clippy` - Run linter (configured in clippy.toml)
- `cargo test` - Run tests
- `cargo check` - Quick compilation check

### Build Scripts
- `scripts/build-windows.sh` - Windows cross-compilation
- `scripts/build-macos.sh` - macOS universal binary build
- `scripts/package-linux.sh` - Linux packaging

### Debug Features
- Use `--features debug` to enable hot reloading: `cargo run --features debug`

## Architecture Overview

### Workspace Structure
- **Root crate**: Main GUI application using Iced framework
- **data/**: Data management, configuration, persistence, audio
- **exchange/**: WebSocket adapters for cryptocurrency exchanges

### Key Components

#### Main Application (`src/main.rs`)
- Uses Iced's daemon architecture with window management
- Core `Flowsurface` struct manages application state
- Message-driven architecture with async task handling
- Multi-window support with popout panels

#### Core Modules
- **Screen/Dashboard**: Main UI layout with pane management
- **Charts**: Heatmap, candlestick, footprint chart implementations
- **Modal System**: Settings, layout manager, theme editor dialogs
- **Sidebar**: Ticker selection and application controls
- **Widget System**: Custom UI components (color picker, multi-split, toast notifications)

#### Data Layer (`data/`)
- **Config Management**: Theme, sidebar state, timezone settings
- **Chart Data**: Aggregation, time series handling, indicators
- **Persistence**: JSON-based state saving/loading
- **Audio**: Real-time sound effects for trades

#### Exchange Layer (`exchange/`)
- **Adapters**: Binance, Bybit, Hyperliquid WebSocket implementations
- **Data Fetching**: Historical trades and market data
- **Rate Limiting**: Built-in request throttling
- **Connection Management**: Auto-reconnection and error handling

### Key Data Structures
- `Ticker`: Optimized fixed-size ticker representation with exchange info
- `TickerInfo`: Market metadata (tick size, minimum quantity)
- `Trade`/`Kline`/`Depth`: Market data structures
- `SerTicker`: Serializable ticker for persistence

### Theme System
- Uses Iced's theming with custom palette support
- Built-in theme editor for runtime customization
- Persistent theme state across sessions

### Layout Management
- Dynamic pane splitting and management
- Multi-layout support with persistence
- Popout window support for multi-monitor setups

## Development Guidelines

### Code Style
- Use rustfmt with 100 character line width
- Follow Rust 2024 edition conventions
- Clippy configuration allows up to 16 function arguments and 5 enum variant names

### Exchange Integration
- All exchange adapters implement common traits in `exchange/adapter/`
- New exchanges should follow the pattern of existing adapters
- WebSocket connections are managed centrally with auto-reconnection

### UI Development
- Follow Iced's Element/Message pattern
- Use the existing widget system for consistency
- Modal dialogs should use the established modal framework

### Data Persistence
- Configuration is saved as JSON to `saved-state.json`
- Market data is cached locally with automatic cleanup
- Use `data_path()` function for file system operations

### Testing Market Data
- Enable trade fetching for Binance tickers in settings (experimental)
- Historical data backfill available for supported timeframes
- Real-time WebSocket streams for live data

## Environment Variables
- `FLOWSURFACE_DATA_PATH`: Override default data directory location

## Dependencies Notes
- Uses development version of Iced framework (pinned to specific git rev)
- sonic-rs for high-performance JSON parsing
- fastwebsockets for exchange connections
- rodio for audio playback
- Extensive use of workspace dependencies for version consistency
