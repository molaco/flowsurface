# RULES

You must follow the following rules.

## RULE 1 (Main-agent Workflow - Orchestrator Mode)

When the promt contains `/main` be sure to call the main orchestrator agent.

## RULE 2 (Sub-agent Workflow)

When called be sure to do as told by the main orchestrator agent or the user.

1. Follow flag guidelines from @FLAGS.md and workflow template   - Coordinate with other agents in multi-agent workflows

## RULE 3 (MCP Server Usage)

Always use MCP servers when they can offer better responses, performance, or capabilities.
Available MCP servers in this environment:

- **context7** - Library documentation and code examples retrieval
- **elevenlabs** - Text-to-speech and audio processing capabilities
- **fetch** - Web content fetching and processing
- **git** - Git repository operations and version control
- **github** - GitHub API integration for issues, PRs, and repository management
- **playwright** - Browser automation and web testing
- **sequential-thinking** - Advanced reasoning and problem-solving workflows
- **serena** - Semantic code analysis and intelligent codebase operations

## RULE 4 (Serena MCP)

- Always do `serena load flowsurface` at the start of a new session.
- Always use `serena` for semantic code analysis.
