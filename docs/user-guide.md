# Flowsurface User Guide

## Table of Contents

1. [Getting Started](#getting-started)
2. [Interface Overview](#interface-overview)
3. [Chart Types](#chart-types)
4. [Exchange Setup](#exchange-setup)
5. [Customization](#customization)
6. [Layout Management](#layout-management)
7. [Settings and Configuration](#settings-and-configuration)
8. [Troubleshooting](#troubleshooting)

## Getting Started

### Installation

#### Using Prebuilt Binaries
1. Download the latest release for your operating system from the [Releases page](https://github.com/akenshaw/flowsurface/releases)
2. Extract the archive to your desired location
3. Run the executable:
   - **Windows**: Double-click `flowsurface.exe`
   - **macOS**: Double-click `Flowsurface.app`
   - **Linux**: Run `./flowsurface` in terminal

#### Building from Source
```bash
# Install Rust toolchain from https://rustup.rs/
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and build
git clone https://github.com/akenshaw/flowsurface
cd flowsurface
cargo build --release
cargo run --release
```

### System Requirements
- **RAM**: Minimum 4GB, recommended 8GB or more
- **CPU**: Modern multi-core processor recommended
- **Display**: 1920x1080 minimum resolution
- **Network**: Stable internet connection for real-time data
- **Operating System**:
  - Windows 10 or later
  - macOS 10.15 or later  
  - Linux with modern desktop environment

### First Launch
1. The application will create a configuration directory:
   - **Windows**: `%APPDATA%/flowsurface/`
   - **macOS**: `~/Library/Application Support/flowsurface/`
   - **Linux**: `~/.local/share/flowsurface/`

2. Default configuration will be loaded automatically
3. The main dashboard will appear with sample layout
4. Exchange connections will initialize in the background

## Interface Overview

### Main Dashboard

The Flowsurface interface consists of several key areas:

```
┌─────────────────────────────────────────────────────────────┐
│  Menu Bar                                                   │
├─────────────┬───────────────────────────────────────────────┤
│             │                                               │
│   Sidebar   │            Chart Panes                        │
│             │                                               │
│ - Exchanges │  ┌─────────────┬─────────────────────────────┐ │
│ - Tickers   │  │   Chart 1   │        Chart 2              │ │
│ - Favorites │  │             │                             │ │
│ - Search    │  │             │                             │ │
│             │  ├─────────────┼─────────────────────────────┤ │
│ - Settings  │  │   Chart 3   │        Chart 4              │ │
│ - Layout    │  │             │                             │ │
│ - Theme     │  │             │                             │ │
│             │  └─────────────┴─────────────────────────────┘ │
└─────────────┴───────────────────────────────────────────────┘
```

### Sidebar Components

#### Exchange Selection
- **Binance**: Futures and spot markets
- **Bybit**: Derivatives and inverse perpetuals
- **Hyperliquid**: Perpetuals trading

#### Ticker Table
- **Search**: Filter tickers by symbol or name
- **Sort**: Click column headers to sort by volume, price, change
- **Favorites**: Star tickers for quick access
- **Filter**: Show only favorites or specific exchanges

#### Quick Actions
- **Layout Manager**: Create, save, and load layouts
- **Theme Editor**: Customize colors and appearance  
- **Audio Settings**: Configure trade sound effects
- **Application Settings**: General preferences

## Chart Types

### Heatmap Chart (Historical DOM)

The heatmap visualizes order book activity over time, showing where trading volume has been concentrated.

**Features**:
- **Price Levels**: Horizontal bands showing price ranges
- **Volume Profile**: Right-side histogram showing volume at each price
- **Heat Intensity**: Color coding shows trading activity concentration
- **Time Navigation**: Scroll horizontally through historical data

**Configuration Options**:
- **Price Grouping**: Adjust price level granularity (0.01, 0.1, 1.0, etc.)
- **Time Interval**: Set data aggregation period (1s, 5s, 15s, 1m, etc.)
- **Volume Profile**: Toggle fixed vs. visible range profile
- **Color Scheme**: Adjust heat map color intensity

**How to Read**:
- **Red/Hot Areas**: High trading activity
- **Cool Areas**: Low trading activity
- **Volume Profile Peaks**: Price levels with most trading
- **White Line**: Current price level

### Candlestick Chart (Kline)

Traditional price action visualization showing open, high, low, close (OHLC) data.

**Features**:
- **Candlestick Bodies**: Show open/close price relationship
- **Wicks/Shadows**: Show high/low price extremes
- **Color Coding**: Green/red for up/down movements
- **Time Intervals**: Multiple timeframes supported

**Configuration Options**:
- **Timeframe**: 1m, 5m, 15m, 1h, 4h, 1d, 1w
- **Candlestick Style**: Traditional, hollow, line
- **Color Scheme**: Customize up/down colors
- **Indicators**: Add technical analysis overlays

**How to Read**:
- **Green Candles**: Closing price higher than opening
- **Red Candles**: Closing price lower than opening
- **Long Wicks**: High volatility during the period
- **Small Bodies**: Low volatility, indecision

### Footprint Chart

Advanced visualization combining price action with volume distribution at each price level.

**Features**:
- **Volume Clusters**: Numbers inside candlesticks show volume
- **Bid/Ask Breakdown**: Separate buy/sell volume display
- **Imbalance Detection**: Highlight volume imbalances
- **POC (Point of Control)**: Mark highest volume price levels

**Configuration Options**:
- **Clustering Method**: Volume, tick count, or dollar volume
- **Imbalance Ratio**: Set threshold for imbalance highlighting
- **Display Format**: Show raw numbers or scaled values
- **Color Coding**: Different colors for bid/ask volume

**How to Read**:
- **Large Numbers**: High volume at that price level
- **Red Numbers**: Sell volume (market sells)
- **Green Numbers**: Buy volume (market buys)
- **Yellow Highlights**: Volume imbalances

### Time & Sales

Real-time trade feed showing individual transactions as they occur.

**Features**:
- **Live Updates**: Real-time trade stream
- **Trade Details**: Price, size, timestamp for each trade
- **Color Coding**: Buy/sell indication
- **Filtering**: Size-based trade filtering

**Configuration Options**:
- **Size Filter**: Only show trades above certain size
- **Color Scheme**: Customize buy/sell colors
- **Display Format**: Price precision and size units
- **Update Speed**: Control refresh rate

**How to Read**:
- **Green Trades**: Trades at ask price (buyers)
- **Red Trades**: Trades at bid price (sellers)
- **Large Sizes**: Institutional or whale activity
- **Rapid Updates**: High activity periods

## Exchange Setup

### Connecting to Exchanges

Flowsurface connects to exchange public data feeds automatically. No API keys required for market data.

#### Binance Configuration
```
Status: ● Connected
Markets: Spot, Futures
Data: Real-time trades, order book, klines
Historical: Available via data.binance.vision and REST API
Rate Limits: Automatically managed
```

#### Bybit Configuration  
```
Status: ● Connected
Markets: Derivatives, Inverse Perpetuals
Data: Real-time trades, order book, klines
Historical: Limited (no suitable REST API)
Rate Limits: Automatically managed
```

#### Hyperliquid Configuration
```
Status: ● Connected
Markets: Perpetuals
Data: Real-time trades, order book, custom formats
Historical: Available via REST API
Rate Limits: Automatically managed
```

### Connection Management

#### Auto-Reconnection
- Connections automatically reconnect if network issues occur
- Exponential backoff prevents rapid reconnection attempts
- Connection status displayed in sidebar

#### Data Quality
- Invalid data is filtered out automatically
- Data validation prevents corrupted information
- Missing data is handled gracefully

#### Performance Optimization
- Connections use efficient WebSocket protocols
- Rate limiting prevents API violations
- Data compression reduces bandwidth usage

## Customization

### Theme System

#### Built-in Themes
- **Dark Theme**: Professional dark interface (default)
- **Light Theme**: Clean light interface
- **High Contrast**: Enhanced visibility theme
- **Custom Themes**: Create your own color schemes

#### Theme Editor
1. Click **Theme** button in sidebar
2. Adjust colors using color picker:
   - **Background Colors**: Main and surface backgrounds
   - **Text Colors**: Primary and secondary text
   - **Chart Colors**: Candlestick, volume, indicators
   - **UI Colors**: Buttons, borders, highlights
3. Changes apply immediately with live preview
4. Save custom themes for future use

#### Color Customization
- **HSV Color Picker**: Precise color selection
- **Palette Support**: Save frequently used colors
- **Export/Import**: Share themes with others
- **Reset Options**: Return to default values

### UI Customization

#### Sidebar Configuration
- **Width Adjustment**: Resize sidebar to fit content
- **Panel Visibility**: Show/hide different sections
- **Ticker Table**: Customize columns and sorting
- **Search Preferences**: Set default filters

#### Pane Management
- **Split Panes**: Create multiple chart areas
- **Resize**: Drag pane borders to adjust sizes
- **Close**: Remove unnecessary panes
- **Popout**: Move panes to separate windows

## Layout Management

### Creating Layouts

#### Layout Templates
1. **Single Chart**: One large chart pane
2. **Dual Charts**: Two charts side-by-side  
3. **Quad Layout**: Four charts in 2x2 grid
4. **Analysis Layout**: Main chart with Time & Sales
5. **Custom Layout**: Design your own arrangement

#### Saving Layouts
1. Arrange panes as desired
2. Click **Layout Manager** in sidebar
3. Click **Save Current Layout**
4. Enter name and description
5. Layout saved for future use

#### Loading Layouts
1. Click **Layout Manager** in sidebar
2. Select layout from list
3. Click **Load Layout**
4. Current layout replaced with saved version

### Multi-Monitor Support

#### Popout Windows
1. Right-click on pane header
2. Select **Popout to Window**
3. Drag window to second monitor
4. Window operates independently

#### Window Management
- Each popout window maintains own state
- Theme changes apply to all windows
- Layout saving includes popout configuration
- Windows can be closed and recreated

## Settings and Configuration

### Application Settings

#### General Settings
```
Data Directory: ~/.local/share/flowsurface/
Log Level: Info
Auto-save Interval: 30 seconds
Startup Behavior: Restore last layout
```

#### Performance Settings
```
Max Memory Usage: 500MB
Chart Update Rate: 30 FPS
Data Cleanup: Daily
Connection Timeout: 30 seconds
```

#### Audio Settings
```
Master Volume: 75%
Trade Sounds: Enabled
UI Sounds: Disabled
Sound Pack: Default
```

### Chart Settings

#### Default Chart Settings
- **Default Chart Type**: Candlestick
- **Default Timeframe**: 1m
- **Default Indicators**: None
- **Auto-scaling**: Enabled

#### Pane Settings
Each pane can be configured independently:
- **Chart Type**: Select primary chart display
- **Data Source**: Choose exchange and ticker
- **Indicators**: Add technical analysis tools
- **Styling**: Override theme colors for this pane

### Data Management

#### Historical Data
- **Auto-fetch**: Automatically fetch historical data when available
- **Cache Duration**: Keep historical data for 7 days
- **Storage Limit**: Maximum 1GB cached data
- **Cleanup Schedule**: Daily maintenance

#### Real-time Data
- **Buffer Size**: Keep 10,000 recent data points in memory
- **Update Frequency**: Process updates every 100ms
- **Compression**: Compress older data to save memory
- **Persistence**: Save important data across sessions

## Troubleshooting

### Common Issues

#### Application Won't Start
**Symptoms**: Application crashes on launch or doesn't appear
**Solutions**:
1. Check system requirements are met
2. Delete configuration file to reset settings:
   - Windows: `%APPDATA%/flowsurface/saved-state.json`
   - macOS: `~/Library/Application Support/flowsurface/saved-state.json`
   - Linux: `~/.local/share/flowsurface/saved-state.json`
3. Run from terminal to see error messages
4. Update graphics drivers
5. Try running with `--features debug` flag

#### No Market Data
**Symptoms**: Charts empty, no ticker updates
**Solutions**:
1. Check internet connection
2. Verify exchange status (exchanges may have downtime)
3. Check firewall settings (allow outbound HTTPS/WSS)
4. Try different exchange in sidebar
5. Restart application to reset connections

#### Poor Performance
**Symptoms**: Slow UI response, choppy charts
**Solutions**:
1. Close unnecessary panes to reduce resource usage
2. Reduce chart update rate in settings
3. Clear cached data in settings
4. Increase memory limit in performance settings
5. Check system resources (RAM, CPU usage)

#### Theme Not Applying
**Symptoms**: Theme changes don't appear or partially apply
**Solutions**:
1. Restart application after theme changes
2. Reset theme to default and try again
3. Check for theme file corruption
4. Recreate custom theme if using custom colors
5. Update to latest version

### Connection Issues

#### WebSocket Connection Problems
**Symptoms**: "Disconnected" status, no real-time updates
**Solutions**:
1. Check network connectivity
2. Verify exchange is operational
3. Check corporate firewall/proxy settings
4. Try different exchange to isolate issue
5. Wait for automatic reconnection (up to 30 seconds)

#### Rate Limiting
**Symptoms**: Slow data updates, connection errors
**Solutions**:
1. Rate limiting is automatic - no action needed
2. Reduce number of simultaneous connections
3. Wait for rate limits to reset (usually 1 minute)
4. Check for multiple Flowsurface instances running

### Data Issues

#### Incorrect Chart Data
**Symptoms**: Wrong prices, missing candlesticks, gaps in data
**Solutions**:
1. Refresh chart by switching timeframes
2. Clear cached data and reload
3. Verify ticker symbol is correct
4. Check exchange is providing data for that symbol
5. Compare with exchange website to verify accuracy

#### Historical Data Missing
**Symptoms**: Charts start from current time, no historical context
**Solutions**:
1. Enable historical data fetching in pane settings
2. Wait for data to download (may take several minutes)
3. Some exchanges don't provide historical data
4. Try different timeframe (1m data may be limited)
5. Check internet connection during data fetch

### Performance Optimization

#### Memory Usage
```bash
# Monitor memory usage
# Windows: Task Manager → Details → flowsurface.exe
# macOS: Activity Monitor → Memory tab
# Linux: htop or ps aux | grep flowsurface
```

**Optimization Tips**:
- Limit number of open panes (4 or fewer recommended)
- Use longer timeframes (1h instead of 1m) to reduce data points
- Enable data cleanup in settings
- Close popout windows when not needed
- Restart application daily during heavy usage

#### CPU Usage
**Optimization Tips**:
- Reduce chart update rate from 30 FPS to 15 FPS
- Disable unnecessary indicators
- Use fewer simultaneous exchange connections
- Close unused charts or switch to simpler chart types
- Consider hardware acceleration if available

### Log Files and Debugging

#### Log File Locations
- **Windows**: `%APPDATA%/flowsurface/logs/`
- **macOS**: `~/Library/Application Support/flowsurface/logs/`
- **Linux**: `~/.local/share/flowsurface/logs/`

#### Debug Mode
Run with debug logging:
```bash
# Enable debug features
cargo run --features debug

# Or set environment variable
RUST_LOG=debug ./flowsurface
```

#### Reporting Issues
When reporting bugs, include:
1. Operating system and version
2. Flowsurface version
3. Steps to reproduce the issue
4. Relevant log files
5. Screenshots if UI-related
6. System specifications (RAM, CPU)

### Getting Help

#### Community Support
- **GitHub Issues**: Report bugs and request features
- **Discussions**: Ask questions and share tips
- **Documentation**: Check latest documentation online

#### Self-Help Resources
1. Check this user guide for common solutions
2. Review log files for error messages
3. Try with default settings/theme
4. Test with minimal layout (single pane)
5. Compare behavior with fresh installation

Remember that Flowsurface is actively developed software. Check for updates regularly as new versions often include bug fixes and performance improvements.