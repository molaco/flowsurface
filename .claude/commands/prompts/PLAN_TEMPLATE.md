## Task Analysis
**Primary Goal**: [Describe what needs to be accomplished]
**Scope**: [What's included/excluded]  
**Success Criteria**: [How to measure completion]
**Complexity**: [simple|medium|complex]
**Risk Factors**: [Potential complications]

---

## Core Application Layer

### app_architect
**When to Use**: Application lifecycle, window management, main.rs changes
**Sub-Prompt**: Analyze the task for application-level architecture impacts. Focus on src/main.rs, src/window.rs, and Iced daemon coordination. Consider window management and application state.
**Deliverables**: Architecture decisions, main application changes, window coordination
**Files**: src/main.rs, src/window.rs, src/logger.rs

### layout_specialist  
**When to Use**: UI layout changes, pane management, screen organization
**Sub-Prompt**: Design layout modifications for the task. Consider pane splitting, layout persistence, and screen coordination. Focus on src/layout.rs and src/screen/ integration.
**Deliverables**: Layout designs, pane management updates, screen organization
**Files**: src/layout.rs, src/screen/, data/src/layout/

---

## GUI Layer

### widget_developer
**When to Use**: Custom UI components, new interactions, visual elements
**Sub-Prompt**: Create custom widgets needed for the task. Follow Iced patterns and integrate with existing widget system. Consider reusability and theming compatibility.
**Deliverables**: Custom widgets, UI components, interaction handlers
**Files**: src/widget/, src/widget.rs

### modal_specialist
**When to Use**: Settings dialogs, configuration UIs, modal windows
**Sub-Prompt**: Design modal interfaces for the task. Follow existing modal patterns and integrate with application state management.
**Deliverables**: Modal dialogs, settings interfaces, configuration UIs
**Files**: src/modal/, src/modal.rs

### theme_designer
**When to Use**: Visual styling, color schemes, theming changes
**Sub-Prompt**: Create theme modifications for the task. Ensure compatibility with existing theme system and runtime customization.
**Deliverables**: Theme updates, styling changes, color palette modifications
**Files**: src/style.rs, src/modal/theme_editor.rs, data/src/config/theme.rs

### sidebar_specialist
**When to Use**: Ticker selection, sidebar controls, application navigation
**Sub-Prompt**: Modify sidebar functionality for the task. Consider ticker management, filtering, sorting, and sidebar state persistence.
**Deliverables**: Sidebar updates, ticker management changes, navigation improvements
**Files**: src/screen/dashboard/sidebar.rs, data/src/config/sidebar.rs

---

## Chart System

### chart_architect
**When to Use**: Chart system coordination, new chart types, rendering pipeline
**Sub-Prompt**: Design chart system changes for the task. Coordinate between chart types and ensure data binding compatibility.
**Deliverables**: Chart architecture, type coordination, rendering pipeline design
**Files**: src/chart.rs, data/src/chart.rs

### chart_renderer
**When to Use**: Chart visualization, canvas rendering, real-time updates
**Sub-Prompt**: Implement chart rendering for the task. Focus on performance, real-time updates, and visual accuracy.
**Deliverables**: Chart implementations, rendering optimizations, visual updates
**Files**: src/chart/heatmap.rs, src/chart/kline.rs, data/src/chart/

### scaling_specialist
**When to Use**: Chart scaling, zoom/pan, axis management
**Sub-Prompt**: Handle chart scaling requirements for the task. Consider time series and linear scaling coordination.
**Deliverables**: Scaling logic, zoom/pan functionality, axis management
**Files**: src/chart/scale/, src/chart/scale/linear.rs, src/chart/scale/timeseries.rs

### indicator_developer
**When to Use**: Technical indicators, overlays, analysis tools
**Sub-Prompt**: Create indicator functionality for the task. Follow existing indicator patterns and ensure chart integration.
**Deliverables**: Technical indicators, analysis overlays, calculation engines
**Files**: src/chart/indicator/, data/src/chart/indicator.rs

---

## Data Layer

### data_architect
**When to Use**: Data structure changes, serialization, workspace coordination
**Sub-Prompt**: Design data layer modifications for the task. Consider serialization compatibility and workspace structure.
**Deliverables**: Data structures, serialization schemas, workspace coordination
**Files**: data/src/lib.rs, data/src/util.rs, data/Cargo.toml

### config_manager
**When to Use**: Configuration changes, persistence, state management
**Sub-Prompt**: Handle configuration requirements for the task. Ensure JSON compatibility and state persistence.
**Deliverables**: Configuration schemas, persistence logic, state management
**Files**: data/src/config.rs, data/src/config/

### aggregator_specialist
**When to Use**: Data aggregation, time series processing, real-time pipelines
**Sub-Prompt**: Design aggregation logic for the task. Focus on time-based processing and real-time data flows.
**Deliverables**: Aggregation logic, time series processing, data pipelines
**Files**: data/src/aggr.rs, data/src/aggr/

### audio_specialist
**When to Use**: Sound system, trade notifications, audio configuration
**Sub-Prompt**: Implement audio features for the task. Consider audio playback and notification systems.
**Deliverables**: Audio implementations, notification sounds, configuration UIs
**Files**: data/src/audio.rs, src/modal/audio.rs

---

## Exchange Layer

### exchange_architect
**When to Use**: Exchange integration, common interfaces, adapter coordination
**Sub-Prompt**: Design exchange layer changes for the task. Focus on abstraction and adapter coordination.
**Deliverables**: Exchange abstractions, interface definitions, adapter coordination
**Files**: exchange/src/lib.rs, exchange/src/adapter.rs

### websocket_specialist
**When to Use**: WebSocket connections, real-time streams, connection management
**Sub-Prompt**: Handle WebSocket requirements for the task. Consider connection pooling, rate limiting, and reliability.
**Deliverables**: WebSocket management, connection logic, rate limiting
**Files**: exchange/src/connect.rs, exchange/src/limiter.rs

### exchange_adapters
**When to Use**: Exchange-specific implementations, API protocols, data parsing
**Sub-Prompt**: Implement exchange-specific functionality for the task. Follow existing adapter patterns.
**Deliverables**: Exchange implementations, API integrations, data parsers
**Files**: exchange/src/adapter/binance.rs, exchange/src/adapter/bybit.rs, exchange/src/adapter/hyperliquid.rs

### market_data_specialist
**When to Use**: Market data processing, order books, historical data
**Sub-Prompt**: Handle market data requirements for the task. Focus on order book processing and data structures.
**Deliverables**: Market data processing, order book handling, historical data management
**Files**: exchange/src/fetcher.rs, exchange/src/depth.rs

---

## Cross-Cutting Concerns

### performance_optimizer
**When to Use**: Performance requirements, optimization needs, bottleneck analysis
**Sub-Prompt**: Analyze performance implications of the task. Identify optimization opportunities and bottlenecks.
**Deliverables**: Performance analysis, optimization recommendations, benchmarks
**Focus Areas**: Rendering performance, real-time data processing, memory usage

### security_auditor
**When to Use**: Security considerations, data validation, secure connections
**Sub-Prompt**: Review security implications of the task. Focus on trading system security and data validation.
**Deliverables**: Security analysis, validation logic, security recommendations
**Focus Areas**: Exchange connections, data handling, configuration security

### integration_tester
**When to Use**: Cross-system testing, end-to-end validation, integration points
**Sub-Prompt**: Design testing strategy for the task. Focus on integration points and real-time system validation.
**Deliverables**: Test strategies, integration tests, validation procedures
**Focus Areas**: GUI-Data integration, Data-Exchange integration, real-time flows

---

## Infrastructure

### build_specialist
**When to Use**: Build system changes, cross-platform compilation, deployment
**Sub-Prompt**: Handle build requirements for the task. Consider cross-platform compatibility and deployment.
**Deliverables**: Build configurations, compilation scripts, deployment procedures
**Files**: scripts/, Cargo.toml configurations

### documentation_specialist
**When to Use**: Documentation updates, API docs, developer guides
**Sub-Prompt**: Create documentation for the task. Focus on developer experience and technical accuracy.
**Deliverables**: Documentation updates, API documentation, developer guides
**Files**: docs/, README.md, CLAUDE.md

---

## Coordination Matrix

**High Interaction Pairs**:
- chart_architect ↔ chart_renderer
- layout_specialist ↔ widget_developer  
- exchange_architect ↔ websocket_specialist
- data_architect ↔ config_manager

**Execution Phases**:
1. **Architecture Phase**: app_architect, data_architect, exchange_architect define interfaces
2. **Implementation Phase**: Specialized agents build features
3. **Integration Phase**: Cross-cutting agents validate and optimize
4. **Validation Phase**: integration_tester, build_specialist verify completion

**Dependencies**: [Agent A] must complete before [Agent B] can proceed

---

## Success Validation

**Build Verification**:
- [ ] `cargo build` succeeds
- [ ] `cargo clippy` passes
- [ ] `cargo test` passes

**Functional Testing**:
- [ ] GUI functionality works
- [ ] Real-time data flows correctly
- [ ] Exchange connections stable
- [ ] Configuration persists

**Integration Points**:
- [ ] Chart updates reflect data changes
- [ ] UI responds to user interactions
- [ ] WebSocket streams process correctly
- [ ] Theme system remains functional