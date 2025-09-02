---
allowed-tools: Read, Write, Edit, MultiEdit, TodoWrite, Task
description: Main documentation orchestrator that manages documentation workflows following RULES.md
output-style: yaml-structured
---

# Documentation Command

Execute documentation workflows following the patterns defined in RULES.md. This command delegates tasks to specialized documentation workflows and agents.

## Usage

`/documentation "task description" [flags]`

## Workflow

The main documentation orchestrator must always use the documentation workflow.

1. Parse flags from prompt to detect workflow triggers (e.g `--docs`)
3. Load workflow file from `.claude/workflows/documentation.yaml`
4. Execute workflow steps sequentially with context injection

## Supported Flags

### Documentation Types
- `--api` - Generate API documentation from codebase analysis
- `--user` - Create user guides and tutorials 
- `--technical` - Generate technical specifications and architecture docs
- `--maintenance` - Create changelogs, migration guides, troubleshooting docs
- `--project` - Update project documentation (README, CLAUDE.md, etc.)
- `--agent-context` - Create agent-optimized documentation (file maps, task patterns)

### Operation Modes (Mutually Exclusive)
- `--generate` - Create new documentation from scratch
- `--update` - Update existing documentation 
- `--audit` - Audit documentation for completeness and accuracy
- `--sync` - Synchronize documentation with current codebase
- `--auto` - Auto-detect needed operation based on analysis

### Format Options
- `--format yaml` - YAML structured output for agents
- `--format markdown` - Markdown for human reading (default)
- `--format inline` - Inline code comments

### Additional Options
- `--validate` - Validate code examples and accuracy
- `--with-examples` - Include working code examples
- `--coverage` - Report documentation coverage
- `--diff` - Only update changed sections (with --update)

## Agent Selection Logic

- **Documentation Creation**: documentation agent
- **API Context**: backend_developer (if needed)
- **Schema Context**: database_architect (if needed)
- **Validation**: validator (if --validate flag)

## Quality Gates

- Flag validation must pass before proceeding
- Context gathering completed before documentation creation
- Validation phase determines success/retry (if requested)

## Integration Points

- **Codebase Analysis**: Uses serena MCP for code understanding
- **Documentation Patterns**: Uses context7 MCP for best practices
- **Multi-Agent Support**: Can coordinate with backend_developer, validator, database_architect

## Examples

```bash
# Generate agent-optimized documentation
/documentation "Create file map for all Rust files" --agent-context --generate --format yaml

# Generate API documentation with validation
/documentation "Generate API documentation for all endpoints" --api --generate --validate

# Update with diff mode
/documentation "Update API docs for new endpoints only" --api --update --diff

# Auto-detect what's needed
/documentation "Check and update project documentation" --project --auto

# Audit documentation coverage
/documentation "Check documentation coverage for API" --api --audit --coverage

# Create task patterns for agents
/documentation "Document common modification patterns" --agent-context --generate --format yaml
```

## Workflow Integration

When triggered, this command:
1. Loads `.claude/workflows/documentation.yaml`
2. Executes analyze → create → validate phases
3. Uses documentation agent as primary executor
4. Coordinates with other agents when needed
5. Validates output if requested

## Agent-Specific Documentation

When using `--agent-context`, creates:
- `AGENT_CONTEXT.md` - System overview for agents
- `docs/agents/file_map.yaml` - File-to-purpose mapping
- `docs/agents/task_patterns.yaml` - Reusable implementation patterns
- `docs/agents/validation_rules.yaml` - Success criteria
- `docs/agents/decision_tree.md` - If-this-then-that logic

# important-instruction-reminders
Do what has been asked; nothing more, nothing less.
NEVER create files unless they're absolutely necessary for achieving your goal.
ALWAYS prefer editing an existing file to creating a new one.
NEVER proactively create documentation files (*.md) or README files. Only create documentation files if explicitly requested by the User.
