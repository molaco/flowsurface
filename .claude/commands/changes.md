---
allowed-tools: Read, Write, Edit, MultiEdit, mcp__git__git_log, mcp__git__git_diff, mcp__git__git_status, TodoWrite
description: Update CHANGES.md with concise git history analysis
output-style: concise
---

# Changes Command

Analyzes git history and updates CHANGES.md with concise, ordered list of changes.

## Usage

- `/changes` - Update with recent commits from current branch
- `/changes --branch <branch-name>` - Analyze specific branch  
- `/changes --since <date>` - Changes since specific date
- `/changes --last <n>` - Last n commits only

## Process

1. **Git Analysis**: Extract commit history using specified parameters
2. **Filter Commits**: Remove trivial commits (typos, minor refactors, dependency updates)
3. **Extract Features**: Identify user-facing features, fixes, and improvements
4. **Format Entries**: Create concise one-line descriptions (<80 chars each)
5. **Update File**: Add entries to CHANGES.md maintaining existing structure

## Change Criteria

Include:
- New features and functionality
- User interface improvements  
- Bug fixes that affect user experience
- Performance enhancements
- API changes
- Configuration updates

Exclude:
- Dependency updates
- Code refactoring (unless user-visible impact)
- Documentation changes
- Test additions
- Typo fixes
- Internal helper functions

## Output Format

Each change entry:
- Starts with action verb (Added, Fixed, Improved, etc.)
- Focuses on user/developer impact
- Avoids technical implementation details
- Maximum 80 characters per line