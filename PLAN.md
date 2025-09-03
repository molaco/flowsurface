# Database Persistent Storage with DuckDB Implementation Plan

## Task Analysis
**Primary Goal**: Implement database persistent storage using DuckDB to replace the current JSON-based persistence system in Flowsurface

**Scope**: 
- Replace JSON-based configuration and market data persistence with DuckDB
- Maintain backward compatibility during transition
- Optimize storage for financial time-series data (trades, klines, depth updates)
- Improve query performance for historical data analysis
- Support data export/import functionality

**Success Criteria**: 
- [ ] DuckDB successfully stores configuration and market data
- [ ] Application startup/shutdown maintains state persistence
- [ ] Performance improvements in data querying and aggregation
- [ ] Seamless migration from existing JSON persistence
- [ ] Reduced memory footprint for large datasets

**Complexity**: complex

**Risk Factors**: 
- Breaking backward compatibility with existing JSON state files
- Performance regression during initial implementation
- Complex migration path from existing persistence system
- Potential data corruption during migration
- Integration complexity with existing real-time data flows

---

## Core Application Layer

### app_architect
**When to Use**: Application lifecycle changes for database initialization/shutdown
**Sub-Prompt**: Modify main.rs and application lifecycle to initialize DuckDB connection pool on startup and ensure proper database cleanup on shutdown. Consider database connection management throughout application lifecycle.
**Deliverables**: Database initialization in main application, connection lifecycle management, graceful shutdown procedures
**Files**: src/main.rs, src/window.rs, src/logger.rs

### layout_specialist  
**When to Use**: Not directly required for database implementation
**Sub-Prompt**: N/A - Layout system will use database through data layer abstraction
**Deliverables**: N/A
**Files**: N/A

---

## GUI Layer

### widget_developer
**When to Use**: Not directly required for initial database implementation
**Sub-Prompt**: N/A - GUI components will access data through existing interfaces
**Deliverables**: N/A
**Files**: N/A

### modal_specialist
**When to Use**: Database settings and migration status dialogs
**Sub-Prompt**: Create modal interfaces for database migration progress, database settings configuration, and potential data export/import functionality.
**Deliverables**: Migration progress modal, database settings interface, export/import dialogs
**Files**: src/modal/, src/modal.rs

### theme_designer
**When to Use**: Not directly required for database implementation
**Sub-Prompt**: N/A - Theme persistence will be handled by database layer
**Deliverables**: N/A
**Files**: N/A

### sidebar_specialist
**When to Use**: Not directly required for initial implementation
**Sub-Prompt**: N/A - Sidebar will continue to use existing data layer interfaces
**Deliverables**: N/A
**Files**: N/A

---

## Chart System

### chart_architect
**When to Use**: Chart data persistence and query optimization
**Sub-Prompt**: Design chart data storage schema in DuckDB and optimize data retrieval for chart rendering. Consider time-series data compression and indexing strategies.
**Deliverables**: Chart data schema design, query optimization strategies, real-time data integration
**Files**: src/chart.rs, data/src/chart.rs

### chart_renderer
**When to Use**: Not directly required for database implementation
**Sub-Prompt**: N/A - Chart rendering will use data through existing interfaces
**Deliverables**: N/A
**Files**: N/A

### scaling_specialist
**When to Use**: Not directly required for database implementation
**Sub-Prompt**: N/A - Scaling logic remains independent of storage layer
**Deliverables**: N/A
**Files**: N/A

### indicator_developer
**When to Use**: Indicator data persistence and calculation optimization
**Sub-Prompt**: Design storage schema for technical indicator calculations and cached results. Consider pre-computed indicator storage for performance optimization.
**Deliverables**: Indicator data schema, calculation caching strategy, historical indicator data storage
**Files**: src/chart/indicator/, data/src/chart/indicator.rs

---

## Data Layer

### data_architect
**When to Use**: Core database integration and data structure redesign
**Sub-Prompt**: Design the database schema for all application data including configuration, market data, and user preferences. Create database connection management and migration system from JSON to DuckDB.
**Deliverables**: Database schema design, connection management, data migration system, abstraction layer
**Files**: data/src/lib.rs, data/src/database.rs (new), data/src/migration.rs (new), data/Cargo.toml

### config_manager
**When to Use**: Configuration persistence migration to database
**Sub-Prompt**: Migrate configuration management from JSON files to DuckDB tables. Maintain compatibility with existing configuration structures while optimizing for database storage.
**Deliverables**: Configuration database schema, migration utilities, backward compatibility layer
**Files**: data/src/config.rs, data/src/config/database.rs (new)

### aggregator_specialist
**When to Use**: Time-series data aggregation and storage optimization
**Sub-Prompt**: Design efficient storage and aggregation of tick data, klines, and depth updates in DuckDB. Implement real-time data ingestion pipeline and historical data compression strategies.
**Deliverables**: Time-series schema design, real-time ingestion pipeline, data compression strategies, aggregation queries
**Files**: data/src/aggr.rs, data/src/aggr/database.rs (new), data/src/database/timeseries.rs (new)

### audio_specialist
**When to Use**: Audio configuration persistence
**Sub-Prompt**: Migrate audio configuration from JSON to database storage. Design schema for audio settings and notification preferences.
**Deliverables**: Audio configuration database schema, migration from JSON settings
**Files**: data/src/audio.rs, data/src/config/audio.rs (new)

---

## Exchange Layer

### exchange_architect
**When to Use**: Not directly required for database implementation
**Sub-Prompt**: N/A - Exchange interfaces remain unchanged, data flows through data layer
**Deliverables**: N/A
**Files**: N/A

### websocket_specialist
**When to Use**: Not directly required for database implementation
**Sub-Prompt**: N/A - WebSocket data continues to flow through existing data layer interfaces
**Deliverables**: N/A
**Files**: N/A

### exchange_adapters
**When to Use**: Not directly required for database implementation
**Sub-Prompt**: N/A - Exchange adapters continue to provide data through existing interfaces
**Deliverables**: N/A
**Files**: N/A

### market_data_specialist
**When to Use**: Market data storage schema and real-time ingestion
**Sub-Prompt**: Design optimal database schema for market data storage (trades, klines, depth updates) and implement efficient real-time data ingestion from exchange streams.
**Deliverables**: Market data schema, real-time ingestion pipeline, data retention policies, query optimization
**Files**: exchange/src/fetcher.rs, exchange/src/depth.rs, data/src/database/market_data.rs (new)

---

## Cross-Cutting Concerns

### performance_optimizer
**When to Use**: Database query optimization and indexing strategy
**Sub-Prompt**: Analyze performance implications of DuckDB integration. Design indexing strategies for time-series data and optimize query performance for real-time chart updates. Benchmark against existing JSON persistence.
**Deliverables**: Performance benchmarks, indexing strategy, query optimization, memory usage analysis
**Focus Areas**: Database query performance, real-time data ingestion, memory usage optimization

### security_auditor
**When to Use**: Database security and data validation
**Sub-Prompt**: Review security implications of database integration including connection security, data validation, and potential SQL injection prevention. Design secure migration procedures.
**Deliverables**: Security analysis, data validation schemas, secure connection management, migration safety procedures
**Focus Areas**: Database connection security, data validation, migration security

### integration_tester
**When to Use**: Database integration testing and validation
**Sub-Prompt**: Design comprehensive testing strategy for database integration including unit tests, integration tests, and migration validation. Test real-time data flows and backward compatibility.
**Deliverables**: Test suite for database operations, migration testing, integration test scenarios, performance tests
**Focus Areas**: Database operations testing, migration validation, real-time data flow testing

---

## Infrastructure

### build_specialist
**When to Use**: Build system integration for DuckDB dependency
**Sub-Prompt**: Integrate DuckDB dependency into the build system with proper feature flags and cross-platform compilation support. Consider bundled vs system library options.
**Deliverables**: Cargo.toml updates, build configuration, cross-platform compatibility, feature flags
**Files**: data/Cargo.toml, Cargo.toml, scripts/

### documentation_specialist
**When to Use**: Documentation for database integration and migration procedures
**Sub-Prompt**: Create comprehensive documentation for database integration, migration procedures, and new database-related APIs. Update existing documentation to reflect persistence changes.
**Deliverables**: Database integration guide, migration documentation, API documentation updates
**Files**: docs/, README.md, CLAUDE.md

---

## Coordination Matrix

**High Interaction Pairs**:
- data_architect ↔ config_manager (schema design coordination)
- aggregator_specialist ↔ market_data_specialist (time-series data optimization)
- performance_optimizer ↔ data_architect (query optimization and indexing)
- app_architect ↔ data_architect (application lifecycle integration)

**Execution Phases**:
1. **Architecture Phase**: 
   - data_architect defines database schema and connection management
   - app_architect plans application lifecycle integration
   - build_specialist prepares dependency integration

2. **Implementation Phase**: 
   - config_manager implements configuration migration
   - aggregator_specialist implements time-series data storage
   - market_data_specialist implements real-time data ingestion
   - modal_specialist creates migration UI components

3. **Integration Phase**: 
   - performance_optimizer optimizes queries and indexing
   - security_auditor validates security measures
   - integration_tester validates all integration points

4. **Validation Phase**: 
   - integration_tester runs comprehensive test suite
   - build_specialist validates cross-platform compatibility
   - documentation_specialist provides complete documentation

**Dependencies**: 
- [data_architect] must complete schema design before [config_manager] and [aggregator_specialist] can proceed
- [build_specialist] must integrate DuckDB before implementation phase can begin
- [app_architect] must complete lifecycle integration before [config_manager] migration can be tested
- [modal_specialist] depends on [data_architect] migration system design

---

## Implementation Details

### Database Schema Design

**Configuration Tables**:
```sql
-- Application configuration
CREATE TABLE app_config (
    id INTEGER PRIMARY KEY,
    theme_name TEXT,
    custom_theme_data TEXT, -- JSON blob for custom theme
    timezone TEXT,
    scale_factor REAL,
    preferred_currency TEXT,
    trade_fetch_enabled BOOLEAN,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Window specifications
CREATE TABLE window_specs (
    id INTEGER PRIMARY KEY,
    window_type TEXT, -- 'main' or 'popout'
    window_id TEXT,
    position_x INTEGER,
    position_y INTEGER,
    width INTEGER,
    height INTEGER,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Layout manager
CREATE TABLE layouts (
    id INTEGER PRIMARY KEY,
    layout_id TEXT UNIQUE,
    name TEXT,
    is_active BOOLEAN DEFAULT FALSE,
    dashboard_config TEXT, -- JSON blob for dashboard configuration
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Sidebar configuration
CREATE TABLE sidebar_config (
    id INTEGER PRIMARY KEY,
    position TEXT, -- 'Left' or 'Right'
    tickers_table_settings TEXT, -- JSON blob for table settings
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Audio configuration
CREATE TABLE audio_config (
    id INTEGER PRIMARY KEY,
    volume REAL,
    enabled BOOLEAN,
    sound_settings TEXT, -- JSON blob for sound configuration
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
```

**Market Data Tables**:
```sql
-- Trade data with time-series optimization
CREATE TABLE trades (
    id BIGINT PRIMARY KEY,
    timestamp TIMESTAMP WITH TIME ZONE,
    exchange TEXT,
    symbol TEXT,
    price DECIMAL(18,8),
    quantity DECIMAL(18,8),
    side TEXT, -- 'buy' or 'sell'
    trade_id TEXT,
    INDEX idx_trades_timestamp (timestamp),
    INDEX idx_trades_symbol_timestamp (symbol, timestamp),
    INDEX idx_trades_exchange_symbol (exchange, symbol)
);

-- Kline data
CREATE TABLE klines (
    id BIGINT PRIMARY KEY,
    timestamp TIMESTAMP WITH TIME ZONE,
    exchange TEXT,
    symbol TEXT,
    interval_type TEXT, -- '1m', '5m', '1h', etc.
    open_price DECIMAL(18,8),
    high_price DECIMAL(18,8),
    low_price DECIMAL(18,8),
    close_price DECIMAL(18,8),
    volume DECIMAL(18,8),
    INDEX idx_klines_symbol_interval_timestamp (symbol, interval_type, timestamp),
    INDEX idx_klines_exchange_symbol (exchange, symbol)
);

-- Depth/Order book data
CREATE TABLE depth_updates (
    id BIGINT PRIMARY KEY,
    timestamp TIMESTAMP WITH TIME ZONE,
    exchange TEXT,
    symbol TEXT,
    bids TEXT, -- JSON array of [price, quantity] pairs
    asks TEXT, -- JSON array of [price, quantity] pairs
    INDEX idx_depth_symbol_timestamp (symbol, timestamp),
    INDEX idx_depth_exchange_symbol (exchange, symbol)
);

-- Ticker information
CREATE TABLE tickers (
    id INTEGER PRIMARY KEY,
    exchange TEXT,
    symbol TEXT,
    tick_size DECIMAL(18,8),
    min_quantity DECIMAL(18,8),
    base_currency TEXT,
    quote_currency TEXT,
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(exchange, symbol)
);

-- Technical indicators cache
CREATE TABLE indicator_cache (
    id BIGINT PRIMARY KEY,
    symbol TEXT,
    indicator_type TEXT,
    period INTEGER,
    timestamp TIMESTAMP WITH TIME ZONE,
    value DECIMAL(18,8),
    parameters TEXT, -- JSON blob for indicator parameters
    INDEX idx_indicator_symbol_type_timestamp (symbol, indicator_type, timestamp),
    INDEX idx_indicator_symbol_type_period (symbol, indicator_type, period)
);
```

### Migration Strategy

**Phase 1: Parallel System**
- Implement database layer alongside existing JSON persistence
- Dual-write to both systems during transition period
- Validate data consistency between systems

**Phase 2: Migration Utility**
- Create migration tool to convert existing JSON state to database
- Provide rollback capability to JSON system if needed
- Batch migrate historical data if present

**Phase 3: Cutover**
- Switch reads to database system
- Maintain JSON backup during initial deployment
- Remove JSON system after stability confirmation

### Performance Optimizations

**Indexing Strategy**:
- Time-series data indexed by (symbol, timestamp) for efficient range queries
- Exchange-symbol composite indexes for filtering
- Partial indexes on active tickers only

**Query Optimization**:
- Use DuckDB's columnar storage for analytical queries
- Implement proper connection pooling
- Cache frequently accessed configuration data
- Use prepared statements for repeated queries

**Data Retention**:
- Implement automatic data cleanup for old market data
- Configurable retention periods per data type
- Archive old data to separate tables or files

---

## Success Validation

**Build Verification**:
- [ ] `cargo build` succeeds with DuckDB dependency
- [ ] `cargo clippy` passes with database code
- [ ] `cargo test` passes including database tests
- [ ] Cross-platform builds work (Windows, macOS, Linux)

**Functional Testing**:
- [ ] Application starts with empty database
- [ ] Configuration persistence works correctly
- [ ] Market data ingestion and retrieval functional
- [ ] Migration from JSON to database successful
- [ ] Real-time data flows continue working

**Performance Testing**:
- [ ] Database queries perform better than JSON loading
- [ ] Memory usage comparable or improved
- [ ] Real-time data ingestion maintains low latency
- [ ] Historical data queries complete within acceptable time

**Integration Points**:
- [ ] Chart updates reflect database data changes
- [ ] UI configuration persists across application restarts
- [ ] WebSocket streams write to database correctly
- [ ] Theme system works with database persistence
- [ ] Layout management persists properly

**Migration Validation**:
- [ ] Existing JSON state files migrate correctly
- [ ] Data integrity maintained during migration
- [ ] Rollback to JSON system works if needed
- [ ] No data loss during migration process