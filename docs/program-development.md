# Program Development Guide

> **Deprecated (2026-05-25):** Programs have been unified into Skills per RFC-009. See `docs/rfc-009-skill-unification.md` for the current model. This document is retained for historical reference.

> Read this when creating or modifying Oxios programs.

## What is a Program?

Programs are the OS-level installable applications for Oxios. They provide structured capabilities that agents can leverage. Think of them as "apps" for the Agent OS.

## Program Structure

```
my-program/
├── program.toml     # Metadata (required)
├── SKILL.md        # Instruction file (required)
├── bin/            # Optional: executable scripts
├── config/         # Optional: configuration files
└── README.md       # Optional: documentation
```

## program.toml Format

```toml
[program]
name = "my-program"
version = "1.0.0"
description = "What this program does"
author = "oxios"

[tools]
my_tool = { description = "What the tool does" }

[host_requirements]
required = ["git", "curl"]
optional = ["gh", "osascript"]
```

## SKILL.md Format

```markdown
# My Program

## Purpose
Brief description.

## Usage
How agents should use the program.

## Tools
- `my_tool`: Description

## Examples
\`\`\`bash
example command
\`\`\`
```

## Host Dependencies

### Common Host Tools

| Tool | Purpose | Required for |
|------|---------|--------------|
| `git` | Version control | Any git operations |
| `gh` | GitHub CLI | GitHub integration |
| `osascript` | AppleScript | macOS automation |
| `open` | Open files/URLs | Browser integration |
| `sqlite3` | Database CLI | Database operations |

### Rules

1. **Document** in `program.toml` under `[host_requirements]`
2. **Validate** using `HostToolValidator` before use
3. **Gracefully degrade** if optional tools are missing
4. **Fail fast** if required tools are missing
5. **Log** access decisions to audit log via AccessManager

## Best Practices

- **One program, one responsibility** — Unix philosophy
- **Make SKILL.md comprehensive** — Agents use this to understand capability
- **Version semantically** — Follow SemVer
