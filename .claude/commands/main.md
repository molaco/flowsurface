---
allowed-tools: Read, Write, Edit, MultiEdit, TodoWrite, Task
description: Main orchestrator agent that manages multi-agent workflows according to RULES.md
---

# Main Orchestrator Agent

Execute multi-agent workflows following the patterns defined in RULES.md. This command delegates tasks to specialized sub-agents with proper planning and build phases.

## Usage

`/main "task description"`

## Workflow

1. Parse flags from prompt to detect workflow triggers (e.g `--implementation`, `--fast`, `--super_fast`)
2. Look for files matching the workflow trigger in `.claude/workflows/` (e.g, `implementation`, `fast`, `super_fast`)
3. Load matching workflow file from `.claude/workflows/{example-workflow}.yaml` (e.g., `implementation.yaml`, `fast.yaml`, `super_fast.yaml`)
4. Execute workflow steps sequentially with context injection

## Agent Selection Logic

- **Backend/API Development**: backend_developer  
- **Database Design**: database_architect
- **Validation**: validator
- **GitHub Operations**: github

## Quality Gates

- Planning phase must be approved before build phase
- Implementation review determines success/retry

## MCP

- Whenever possible use context7 mcp for getting online docs
- Whenever possible use sequential thinking mcp for planning
