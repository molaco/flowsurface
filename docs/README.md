# Flowsurface Documentation

## Documentation Overview

This directory contains comprehensive documentation for the Flowsurface cryptocurrency charting application. The documentation is organized for both human developers and AI agents.

## Documentation Structure

### 📖 Human-Readable Documentation

#### Core Documentation
- **[User Guide](user-guide.md)** - Complete guide for end users
- **[Technical Architecture](technical-architecture.md)** - Detailed system architecture and design
- **[API Reference](api-reference.md)** - Programming interfaces and data structures

#### Quick References
- **[Project Setup](../CLAUDE.md)** - Development environment and commands
- **[Architecture Context](../AGENT_CONTEXT.md)** - High-level system overview
- **[Development Rules](../RULES.md)** - Workflow and development guidelines
- **[Feature Flags](../FLAGS.md)** - Agent operation modes and flags

### 🤖 Agent-Optimized Documentation

#### Core Agent Resources  
- **[File Map](agents/file_map.yaml)** - Complete file-to-purpose mapping
- **[Task Patterns](agents/task_patterns.yaml)** - Step-by-step implementation patterns
- **[Validation Rules](agents/validation_rules.yaml)** - Success criteria and testing procedures
- **[Quick Reference](agents/quick_reference.yaml)** - Essential commands and patterns

#### Specialized Agent Documentation
Located in `.claude/agents/` directory:
- **Frontend Developer** - UI and widget development
- **Backend Developer** - Data processing and exchange integration
- **Database Architect** - Data management and persistence
- **Architect** - System design and architectural decisions
- **Tester** - Testing strategies and quality assurance
- **Reviewer** - Code review guidelines and standards
- **Documentation** - Documentation maintenance and generation

## Getting Started

### For Users
Start with the **[User Guide](user-guide.md)** which covers:
- Installation and setup
- Interface overview and navigation
- Chart types and features
- Customization and themes
- Troubleshooting common issues

### For Developers
Begin with these key resources:
1. **[CLAUDE.md](../CLAUDE.md)** - Project overview and development setup
2. **[Technical Architecture](technical-architecture.md)** - System design and components
3. **[API Reference](api-reference.md)** - Programming interfaces
4. **[Agent File Map](agents/file_map.yaml)** - Codebase structure and file purposes

### For AI Agents
Use the agent-optimized documentation:
1. **[AGENT_CONTEXT.md](../AGENT_CONTEXT.md)** - System capabilities and constraints
2. **[File Map](agents/file_map.yaml)** - Complete file-to-purpose mapping
3. **[Task Patterns](agents/task_patterns.yaml)** - Implementation patterns for common tasks
4. **[Quick Reference](agents/quick_reference.yaml)** - Essential commands and shortcuts

## Documentation Categories

### User Documentation
**Target Audience**: End users, traders, market analysts  
**Format**: Markdown with screenshots and examples  
**Content**: Features, usage instructions, troubleshooting

Key Files:
- [User Guide](user-guide.md) - Comprehensive user documentation
- Installation guides and system requirements
- Feature explanations with visual examples
- Configuration and customization options

### Technical Documentation  
**Target Audience**: Developers, contributors, system integrators  
**Format**: Markdown with code examples and diagrams  
**Content**: Architecture, APIs, development processes

Key Files:
- [Technical Architecture](technical-architecture.md) - System design and component overview
- [API Reference](api-reference.md) - Programming interfaces and data structures
- Development setup and build processes
- Integration patterns and extension points

### Agent Documentation
**Target Audience**: AI agents and autonomous development tools  
**Format**: YAML and structured markdown for machine parsing  
**Content**: File mappings, task patterns, validation rules

Key Files:
- [File Map](agents/file_map.yaml) - Complete file-to-purpose mapping
- [Task Patterns](agents/task_patterns.yaml) - Step-by-step implementation guides
- [Validation Rules](agents/validation_rules.yaml) - Success criteria and testing
- [Quick Reference](agents/quick_reference.yaml) - Essential information for rapid development

## Project Architecture Overview

### High-Level Structure
```
Flowsurface (Rust Desktop Application)
├── Main Application (src/)           - Iced GUI framework
│   ├── Charts (chart/)              - Heatmap, candlestick, indicators
│   ├── GUI Components (screen/modal/widget/) - User interface
│   └── Layout Management (layout.rs) - Window and pane management
├── Data Management (data/)           - Configuration and processing
│   ├── Configuration (config/)      - Settings and persistence
│   ├── Chart Data (chart/)          - Data aggregation and processing
│   └── Layout Persistence (layout/) - Layout state management
└── Exchange Integration (exchange/)  - Market data connectivity
    ├── Adapters (adapter/)          - Exchange-specific implementations
    ├── WebSocket Management (connect.rs) - Connection handling
    └── Rate Limiting (limiter.rs)   - API compliance
```

### Core Capabilities
- **Multi-Exchange Support**: Binance, Bybit, Hyperliquid
- **Advanced Charts**: Heatmaps, candlesticks, footprint charts, indicators
- **Real-Time Data**: WebSocket streams with auto-reconnection
- **Customization**: Themes, layouts, multi-window support
- **Performance**: Optimized for large datasets and high-frequency updates

## Development Workflow

### Standard Development Process
1. **Setup**: Follow [CLAUDE.md](../CLAUDE.md) for environment setup
2. **Architecture**: Review [Technical Architecture](technical-architecture.md)
3. **Implementation**: Use [Task Patterns](agents/task_patterns.yaml) for guidance
4. **Testing**: Apply [Validation Rules](agents/validation_rules.yaml)
5. **Integration**: Follow patterns in [API Reference](api-reference.md)

### Agent-Assisted Development
1. **Context**: Load [AGENT_CONTEXT.md](../AGENT_CONTEXT.md) for system overview
2. **File Mapping**: Reference [File Map](agents/file_map.yaml) for target files
3. **Implementation**: Follow [Task Patterns](agents/task_patterns.yaml) step-by-step
4. **Validation**: Use [Validation Rules](agents/validation_rules.yaml) for testing
5. **Quick Help**: Consult [Quick Reference](agents/quick_reference.yaml) for commands

## Key Features Documented

### Chart System
- **Heatmap Charts**: Historical DOM visualization with volume profiles
- **Candlestick Charts**: Traditional OHLCV display with multiple timeframes
- **Footprint Charts**: Price-level volume analysis with imbalance detection
- **Technical Indicators**: Extensible indicator system with custom plotters
- **Real-Time Updates**: Smooth data streaming with performance optimization

### Exchange Integration
- **WebSocket Connections**: Auto-reconnecting streams with error recovery
- **Rate Limiting**: Exchange-specific throttling to prevent API violations
- **Data Processing**: High-performance parsing with sonic-rs JSON library
- **Historical Data**: Backfill support with multiple data sources
- **Multi-Exchange**: Unified interface across different exchange APIs

### User Interface
- **Iced GUI Framework**: Cross-platform native desktop application
- **Custom Widgets**: Color picker, multi-split panes, toast notifications
- **Theme System**: Runtime customization with live preview
- **Layout Management**: Save/load layouts, multi-window support
- **Responsive Design**: Adaptive UI for different screen sizes

### Configuration System
- **JSON Persistence**: Human-readable configuration files
- **Migration Support**: Automatic config format updates
- **Validation**: Data integrity checks with fallback to defaults
- **Hot Reloading**: Runtime configuration changes
- **Cross-Platform**: Consistent behavior across operating systems

## Contributing to Documentation

### Documentation Standards
- **Accuracy**: Keep documentation synchronized with code
- **Completeness**: Cover all features and use cases
- **Clarity**: Write for the target audience skill level
- **Examples**: Include practical code examples and screenshots
- **Structure**: Follow established documentation patterns

### Updating Documentation
1. **User Docs**: Update after user-facing feature changes
2. **Technical Docs**: Update after API or architecture changes  
3. **Agent Docs**: Update after file structure or workflow changes
4. **Validation**: Verify documentation accuracy with testing

### Documentation Maintenance
- Review documentation quarterly for accuracy
- Update screenshots and examples with UI changes
- Validate code examples with current API
- Check links and references for validity
- Gather feedback from users and developers

## Support and Resources

### Getting Help
- **Issues**: Report bugs and request features on GitHub
- **Discussions**: Ask questions in GitHub Discussions
- **Documentation**: Check this documentation for answers
- **Code Examples**: Review examples in API reference

### External Resources
- **Iced Framework**: [iced.rs](https://iced.rs) - GUI framework documentation
- **Rust Language**: [doc.rust-lang.org](https://doc.rust-lang.org) - Rust documentation
- **Exchange APIs**: Official documentation for Binance, Bybit, Hyperliquid
- **Technical Analysis**: Reference materials for indicators and chart patterns

This documentation provides comprehensive coverage of the Flowsurface application for users, developers, and AI agents. Choose the appropriate documentation section based on your needs and expertise level.