# AGENT_CONTEXT.md

> **Purpose**: This document provides high-level system context and workflow guidelines for agents.  
> **Related Documentation**:
> - `CLAUDE.md` - Project overview and human-readable documentation
> - `.claude/agents/` - Detailed agent-optimized documentation files

## System Overview
Flowsurface is a Rust-based desktop charting application for cryptocurrency trading analysis. It provides real-time market data visualization with support for multiple chart types, exchanges (Binance, Bybit, Hyperliquid), and advanced trading features using the Iced GUI framework.

### Architecture Overview
- **Desktop Application**: Rust application using Iced GUI framework with daemon architecture and window management
- **Workspace Structure**: Root crate (main GUI), data/ (management and persistence), exchange/ (WebSocket adapters)
- **Data Flow**: Exchange WebSocket Adapters → Chart Data Aggregation → Real-time GUI Updates via Iced's Message System
- **Multi-Exchange Support**: Binance, Bybit, Hyperliquid adapters with unified trait implementations
- **Modern Architecture**: Message-driven architecture with async task handling, multi-window support with popout panels

### Core Capabilities
- **Multi-Chart Support**: Heatmap, candlestick, footprint chart implementations with real-time updates
- **Exchange Integration**: WebSocket adapters for Binance, Bybit, Hyperliquid with rate limiting and auto-reconnection
- **Advanced GUI**: Iced-based desktop interface with custom widgets (color picker, multi-split, toast notifications)
- **Theme System**: Built-in theme editor with runtime customization and persistent state
- **Layout Management**: Dynamic pane splitting, multi-layout support, popout window support for multi-monitor setups
- **Data Persistence**: JSON-based configuration and state saving with automatic cleanup
- **Audio System**: Real-time sound effects for trades using rodio
- **Performance Optimization**: High-performance JSON parsing with sonic-rs, fastwebsockets for exchange connections
- **Modular Design**: Clean separation between GUI, data management, and exchange layers

### Core Limitations
- **Desktop Only**: Single desktop application, no web interface
- **Exchange Dependencies**: Requires stable WebSocket connections to exchange APIs
- **Memory Usage**: In-memory chart data and real-time processing
- **Single Instance**: Designed for single-user desktop usage
- **Platform Specific**: Different build processes for Windows, macOS, and Linux
- **GUI Framework**: Tied to Iced framework development cycle (using dev version)

## File Ownership Map

> **Note**: For comprehensive file mapping and component responsibilities, see `.claude/agents/file_map.yaml`

### Key Integration Points
- **Main Application**: src/main.rs (Iced daemon setup), src/window.rs (window management)
- **GUI Core**: src/screen/ (dashboard and UI layout), src/modal/ (dialog system), src/widget/ (custom components)
- **Charts**: src/chart/ (heatmap, kline, indicators), src/chart/scale/ (timeseries, linear scaling)
- **Data Layer**: data/ workspace (configuration, persistence, audio, chart data management)
- **Exchange Layer**: exchange/ workspace (WebSocket adapters, rate limiting, connection management)
- **Style System**: src/style.rs (theming), src/modal/theme_editor.rs (runtime customization)

## Common Task Patterns

> **Note**: For detailed task patterns with complete implementation steps, see `.claude/agents/task_patterns.yaml`

Quick reference for common tasks:
- **Adding GUI Components**: src/widget/ → src/screen/ integration → Iced Element/Message pattern
- **Chart Features**: src/chart/ modifications → real-time data pipeline updates
- **Exchange Integration**: exchange/ workspace → trait implementation → WebSocket handling
- **Theme/Style Changes**: src/style.rs → src/modal/theme_editor.rs → persistent state updates
- **Layout Management**: src/layout.rs → pane splitting → multi-window support

## Critical Constraints

### Desktop Application
- **Iced Framework**: Using development version with specific git revision
- **Message Architecture**: All GUI updates through Iced's Element/Message pattern
- **Async Runtime**: Tokio-based async task handling for exchange connections
- **Window Management**: Multi-window support with daemon architecture

### Data Management
- **JSON Persistence**: Configuration saved to saved-state.json
- **Local Caching**: Market data cached locally with automatic cleanup
- **Data Path**: Use data_path() function for all file system operations
- **Theme Persistence**: Runtime theme changes saved across sessions

### Exchange Integration
- **WebSocket Reliability**: Auto-reconnection with exponential backoff
- **Rate Limiting**: Built-in request throttling per exchange
- **Error Handling**: Graceful degradation with comprehensive error propagation
- **Multi-Exchange**: Unified trait implementations across Binance, Bybit, Hyperliquid

## Integration Points

### Adding New Exchange Adapters
**Primary**: exchange/ workspace → implement common traits (connection, data fetching, rate limiting)
**Secondary**: data/ workspace → update ticker/market data structures as needed
**GUI Integration**: src/screen/dashboard/sidebar.rs → ticker selection updates
**Validation**: WebSocket connection stability, data format consistency, error handling, auto-reconnection

### GUI Extensions
**Primary**: src/widget/ → custom components, src/screen/ → UI layout integration
**Secondary**: src/modal/ → dialog system extensions, src/style.rs → theming updates
**Charts**: src/chart/ → chart type implementations, indicator system
**Validation**: Iced Element/Message pattern compliance, theme compatibility, multi-window support
**Features**: Responsive layout, toast notifications, color picker, multi-split panes

### Data Layer Extensions
**Primary**: data/ workspace → configuration management, persistence layer
**Secondary**: src/main.rs → application state integration
**Validation**: JSON serialization compatibility, data path consistency, cleanup procedures

### Chart System Extensions
**Primary**: src/chart/ → chart implementations (heatmap, kline, indicators)
**Secondary**: src/chart/scale/ → scaling logic, src/chart/indicator/ → technical indicators
**Validation**: Real-time data pipeline compatibility, performance optimization, rendering accuracy

## Agent Workflow Guidelines

### Pre-Implementation Analysis
1. **Git Status Check**: Always run `git status` to understand current repository state
2. **Workspace Analysis**: Understand root crate vs data/ vs exchange/ workspace structure
3. **Integration Points**: Identify GUI components, data flows, and exchange dependencies
4. **Risk Assessment**: Evaluate impact on Iced message flow and real-time data streams
5. **Validation Strategy**: Define testing approach for desktop GUI and data accuracy

### Implementation Best Practices
1. **Code Patterns**: Follow Iced Element/Message pattern, Rust 2024 edition, serde serialization
2. **GUI Development**: Use existing widget system, follow modal framework, maintain theme compatibility
3. **Exchange Integration**: Implement common traits, preserve auto-reconnection, handle rate limits
4. **Data Management**: Use data_path() function, maintain JSON compatibility, ensure cleanup
5. **Performance**: Consider memory usage, chart rendering performance, WebSocket efficiency
6. **Testing**: Build with `cargo build`, test GUI interactions, verify real-time data

### Post-Implementation Validation
1. **Build Verification**: Run `cargo build` and `cargo clippy` for compilation and linting
2. **GUI Testing**: Test desktop application launch, window management, theme system
3. **Exchange Testing**: Verify WebSocket connections, data accuracy, reconnection logic
4. **Data Flow**: Confirm chart updates, persistence, configuration saving
5. **Regression Testing**: Ensure existing charts, exchanges, and features work
6. **Performance Check**: Monitor memory usage, GUI responsiveness, data throughput
7. **Cross-Platform**: Consider impact on Windows, macOS, Linux builds if applicable

### Common Pitfalls to Avoid
- Breaking Iced's Element/Message pattern when adding GUI components
- Disrupting WebSocket connection stability and auto-reconnection logic
- Modifying data structures without considering JSON serialization compatibility
- Not following workspace dependency patterns (use workspace = true)
- Missing error handling in async exchange operations
- Not preserving theme system compatibility when adding UI components
- Breaking chart synchronization and real-time data updates
- Not testing with multiple exchange WebSocket connections simultaneously
- Forgetting to update both data layer and GUI when adding features
- Not using data_path() function for file system operations
- Breaking multi-window support and popout functionality
- Not following established widget patterns for consistency
- Attempting to use incompatible Iced framework versions
- Not preserving layout management and pane splitting functionality
- Breaking audio system integration for trade notifications
- Not maintaining cross-platform build compatibility

## Quick Reference

> **Note**: For complete development commands, build processes, and feature documentation, see:
> - `.claude/agents/quick_reference.yaml` - Comprehensive quick reference
> - `CLAUDE.md` - Human-readable project overview and setup

### Essential Commands
- `cargo run --release` - Run release build
- `cargo run --features debug` - Run with hot reloading
- `cargo fmt` - Format code (max_width=100)
- `cargo clippy` - Run linter
- `cargo test` - Run tests

### Key Environment Variables
- `FLOWSURFACE_DATA_PATH` - Override default data directory location