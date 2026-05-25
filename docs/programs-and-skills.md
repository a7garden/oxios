# Programs & Skills Guide

> **Note (2026-05-25):** This document describes the legacy Programs system. As of RFC-009, Programs and Skills have been unified into a single **Skill** model. See `docs/rfc-009-skill-unification.md` for the new architecture. The skill format described in §4 (SKILL.md) is still valid, but `program.toml` is deprecated — all metadata now goes in SKILL.md YAML frontmatter.

> Programs are OS-level installable capabilities for Oxios agents. Skills are markdown instruction templates that teach agents how to perform specific tasks. Together, they form the application layer of the Agent Operating System.

---

## Table of Contents

1. [What Are Programs?](#1-what-are-programs)
2. [Program Structure](#2-program-structure)
3. [program.toml Reference](#3-programtoml-reference)
4. [SKILL.md Guide](#4-skillmd-guide)
5. [Host Dependencies](#5-host-dependencies)
6. [MCP Server Integration](#6-mcp-server-integration)
7. [Installing Programs](#7-installing-programs)
8. [Managing Programs](#8-managing-programs)
9. [Creating Custom Programs](#9-creating-custom-programs)
10. [Built-in Programs](#10-built-in-programs)
11. [Sharing Programs](#11-sharing-programs)
12. [Best Practices](#12-best-practices)
13. [Skills System](#13-skills-system)

---

## 1. What Are Programs?

Programs are the **application layer** of the Oxios Agent OS. They are self-contained, installable capabilities that extend what agents can do. Think of them as "apps" for your agent OS.

A program provides:

| Component | Purpose |
|-----------|---------|
| **Metadata** | Name, version, description, author (`program.toml`) |
| **Instructions** | How the agent should behave (`SKILL.md`) |
| **Tools** | Tool definitions the agent can use |
| **Host requirements** | System tools needed (git, gh, etc.) |
| **MCP servers** | External tool servers the program registers |

Programs compose kernel system calls into complete workflows. They do NOT bypass the kernel — every operation still goes through KernelHandle with full RBAC enforcement.

```
┌────────────────────────────────────────────────────┐
│                    Program                          │
│                                                    │
│  program.toml ── metadata, tools, requirements     │
│  SKILL.md     ── structured agent instructions     │
│  bin/         ── optional executable scripts        │
│  config/      ── optional configuration files       │
│                                                    │
│  Program uses ONLY Kernel System Calls             │
│  Kernel internals are invisible to programs        │
└────────────────────────────────────────────────────┘
```

---

## 2. Program Structure

Every program is a directory containing at minimum two files:

```
my-program/
├── program.toml     # Metadata (required)
├── SKILL.md         # Agent instructions (required)
├── bin/             # Optional: executable scripts
│   └── analyze.sh
├── config/          # Optional: configuration files
│   └── defaults.json
└── README.md        # Optional: human documentation
```

### program.toml

The manifest file that defines the program's identity, tools, and dependencies:

```toml
[program]
name = "my-program"
version = "1.0.0"
description = "What this program does"
author = "your-name"

# Tools the program provides to agents
[[tools]]
name = "analyze"
description = "Analyze code quality"
command = "analyze"

[[tools]]
name = "report"
description = "Generate quality report"
command = "report"

# MCP servers the program registers
[[mcp_servers]]
name = "my-mcp-server"
command = "node"
args = ["server.js"]
enabled = true

[mcp_servers.env]
API_KEY = "from-config"

# Host tool dependencies
[host_requirements]
required = ["git"]
optional = ["gh", "osascript"]
```

### SKILL.md

A markdown file containing structured instructions that the agent follows when this program is activated. This is the program's "source code" — it tells the agent what to do, step by step.

```markdown
# My Program

## Purpose
Analyze code quality and generate reports.

## Workflow

### Phase 1: Discovery
1. Use `find` to discover source files in the target directory
2. Use `read` to examine each file
3. Determine language and framework

### Phase 2: Analysis
1. Check for common issues (unused imports, dead code)
2. Run linting tools via `bash`
3. Measure complexity metrics

### Phase 3: Reporting
1. Compile findings into a structured report
2. Use `write` to save the report
3. Use `audit()` to log the analysis

## Output
Markdown report with severity-ranked findings.
```

---

## 3. program.toml Reference

### `[program]` Section

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | `string` | Yes | Unique program identifier (lowercase, hyphens) |
| `version` | `string` | Yes | Semantic version (e.g., `"1.0.0"`) |
| `description` | `string` | Yes | Human-readable description |
| `author` | `string` | No | Author name or organization |

### `[[tools]]` Array

Each tool entry defines a capability the program provides:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | `string` | Yes | Tool identifier |
| `description` | `string` | Yes | What the tool does |
| `command` | `string` | Yes | Command or action name |

Example:

```toml
[[tools]]
name = "scan"
description = "Scan for security vulnerabilities"
command = "security-scan"

[[tools]]
name = "fix"
description = "Auto-fix detected issues"
command = "auto-fix"
```

### `[[mcp_servers]]` Array

MCP servers that the program registers with the kernel:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | `string` | Yes | Server identifier |
| `command` | `string` | Yes | Command to start the server |
| `args` | `string[]` | No | Command-line arguments |
| `enabled` | `boolean` | No | Whether to auto-start (default: `true`) |
| `[mcp_servers.env]` | `map` | No | Environment variables |

Example:

```toml
[[mcp_servers]]
name = "filesystem-server"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/workspace"]
enabled = true

[mcp_servers.env]
LOG_LEVEL = "info"
```

### `[host_requirements]` Section

Declares system tools the program needs:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `required` | `string[]` | No | Tools that MUST be present |
| `optional` | `string[]` | No | Tools that enhance functionality |

```toml
[host_requirements]
required = ["git", "node"]
optional = ["gh", "docker", "cargo"]
```

---

## 4. SKILL.md Guide

SKILL.md is the instruction file that agents read when a program is activated. Writing effective SKILL.md files is the key to creating useful programs.

### Structure Template

```markdown
# Program Name

## Purpose
One-sentence description of what this program does.

## Usage
How and when agents should use this program. Be specific about
trigger conditions and expected inputs.

## Workflow

### Phase 1: Preparation
1. Step description — use `tool_name` to action
2. Step description — if condition: skip to Phase N
3. Step description — validate prerequisites

### Phase 2: Execution
1. Step description — main work
2. On failure: describe fallback behavior
3. Step description — continue

### Phase 3: Verification
1. Step description — validate results
2. Use `audit()` to record the outcome
3. Report findings to user

## Error Handling
- If step X fails: do Y
- If step Z fails: rollback and notify

## Output
Description of what the agent should report to the user.

## Constraints
- Rule 1
- Rule 2
```

### Writing Effective Instructions

**Do:**
- Use numbered steps within phases
- Reference specific tools by name (`bash`, `read`, `write`, etc.)
- Include conditional logic (`if X, skip to Phase N`)
- Define error handling for each phase
- Specify the expected output format

**Don't:**
- Write vague instructions ("make it better")
- Assume the agent knows project-specific context
- Skip error handling
- Use language-specific jargon without context

### Example: Code Review SKILL.md

```markdown
# Oxios Code Review

## Purpose
Deep code review with quality domain analysis across security,
performance, and correctness.

## Workflow

### Phase 1: Discovery
1. If `target` is a file, `read` it directly
2. If `target` is a directory, use `find` to discover source files
3. Determine the language/framework of the project

### Phase 2: Quality Analysis
Perform analysis across these domains:

**Correctness & Robustness**
- Trace logic paths for bugs (off-by-one, null dereference, race conditions)
- Check error handling — are errors caught or swallowed?
- Verify test coverage and test quality

**Security**
- Check for injection vulnerabilities (SQL, command, path)
- Verify authentication/authorization boundaries
- Look for exposed secrets or credentials

**Performance**
- Identify N+1 queries, redundant computations
- Check for memory leaks (unbounded collections, missing drop)
- Verify async safety patterns

### Phase 3: Reporting
For each finding, provide:
    [SEVERITY] Component: Description
      Location: file:line
      Evidence: concrete code excerpt
      Impact: why this matters
      Recommendation: specific fix

## Output Format
Return a markdown report with sections:
1. Summary (files reviewed, issues found, severity breakdown)
2. Critical Issues (must fix before merge)
3. Important Issues (should fix)
4. Minor Issues (nice to have)
5. Positive Findings (what's done well)

## Constraints
- Never modify files — review only
- Never execute code you don't understand
- Use actual file paths and line numbers
```

---

## 5. Host Dependencies

Programs can declare system tools they need via `[host_requirements]`. The `HostToolValidator` checks availability at install time and runtime.

### Common Host Tools

| Tool | Purpose | macOS | Linux |
|------|---------|-------|-------|
| `git` | Version control | ✓ | ✓ |
| `gh` | GitHub CLI | ✦ install | ✦ install |
| `node` / `npx` | JavaScript runtime | ✦ install | ✦ install |
| `cargo` | Rust build system | ✓ (if Rust installed) | ✓ |
| `osascript` | macOS automation | ✓ | ✗ |
| `open` | Open files/URLs | ✓ | `xdg-open` |
| `docker` | Container runtime | ✦ install | ✦ install |
| `sqlite3` | Database CLI | ✦ install | ✦ install |

✓ = typically available, ✦ = requires separate installation

### Rules

1. **Document** all dependencies in `[host_requirements]`
2. **Distinguish** required vs optional tools
3. **Validate** at program startup using HostToolValidator
4. **Degrade gracefully** if optional tools are missing
5. **Fail fast** with a clear message if required tools are missing

### Checking Host Tools

```bash
# CLI — check all host tools
$ oxios program my-program
# Shows required/optional tools with availability

# API
$ curl http://127.0.0.1:4200/api/programs/my-program/host-requirements
$ curl http://127.0.0.1:4200/api/host-tools
```

---

## 6. MCP Server Integration

Programs can register MCP (Model Context Protocol) servers that provide additional tools to agents. When a program is installed and its MCP servers are enabled, they're automatically registered with the kernel's `McpBridge`.

### How It Works

```
Program installs
       │
       ▼
ProgramManager reads program.toml
       │
       ▼
For each [[mcp_servers]] entry:
  ├── McpBridge.register_server(McpServer { name, command, args, env })
  └── Server starts on kernel boot
       │
       ▼
Agent discovers MCP tools via ToolRetriever
       │
       ▼
Agent calls MCP tool through McpBridge
```

### Configuration

```toml
# In program.toml
[[mcp_servers]]
name = "filesystem"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/workspace"]
enabled = true

[mcp_servers.env]
LOG_LEVEL = "info"
```

---

## 7. Installing Programs

### From a Local Directory

```bash
$ oxios pkg install ./my-program
  Installed 'my-program v1.0.0'
```

### From a Git Repository

```bash
$ oxios pkg install https://github.com/example/oxios-program.git
  Cloning... done
  Installed 'oxios-program v2.1.0'

# With a specific branch
$ oxios pkg install https://github.com/example/oxios-program.git --branch dev
```

### From a Tarball URL

```bash
$ oxios pkg install https://example.com/my-program.tar.gz
  Downloading... done
  Installed 'my-program v1.0.0'
```

### Via API

```bash
$ curl -X POST http://127.0.0.1:4200/api/programs \
  -H "Content-Type: application/json" \
  -d '{"path": "https://github.com/example/oxios-program.git"}'
```

### What Happens During Install

1. **Fetch** — Clone repo, download tarball, or read local directory
2. **Parse** — Read and validate `program.toml`
3. **Validate** — Check host requirements (warn, don't block)
4. **Store** — Copy program files to `~/.oxios/workspace/programs/<name>/`
5. **Register** — Index tools in ToolRetriever for semantic discovery
6. **MCP** — Register any MCP servers defined in the program

---

## 8. Managing Programs

### List Installed Programs

```bash
$ oxios pkg list

NAME                             VERSION    DESCRIPTION
code-review                      1.0.0      Deep code review with quality analysis
debug                            1.0.0      Systematic debugging with hypothesis-driven approach
deploy                           1.0.0      Safe deployment with pre-flight checks and rollback
guardian                         1.0.0      Background daemon for system integrity checks
refactor                         1.0.0      Safe refactoring with behavior preservation
program-creator                  1.0.0      Meta-program for creating new Oxios programs

# Detailed search view
$ oxios pkg search

code-review (1.0.0)
  Deep code review with quality analysis
  Tools: scan, report

debug (1.0.0)
  Systematic debugging with hypothesis-driven approach
  Tools: diagnose, trace, verify
...
```

### View Program Details

```bash
$ oxios program code-review

  code-review v1.0.0
  ──────────────────────────────────────────────────
  Deep code review with quality analysis

  SKILL.md:
  (full skill content displayed)

  Tools:
  • scan: Scan code for issues
  • report: Generate quality report

  Required host tools: git
  Optional host tools: gh
```

### Enable/Disable Programs

```bash
# Disable without removing
$ oxios pkg disable code-review

# Re-enable
$ oxios pkg enable code-review
```

### Uninstall Programs

```bash
$ oxios pkg uninstall my-program
  Uninstalled 'my-program'
```

---

## 9. Creating Custom Programs

### Step-by-Step Guide

#### Step 1: Create the Directory

```bash
mkdir my-security-scanner
cd my-security-scanner
```

#### Step 2: Write program.toml

```toml
[program]
name = "security-scanner"
version = "1.0.0"
description = "Automated security vulnerability scanner"
author = "security-team"

[[tools]]
name = "scan"
description = "Scan codebase for security vulnerabilities"
command = "security-scan"

[[tools]]
name = "audit-dependencies"
description = "Check dependencies for known CVEs"
command = "dep-audit"

[host_requirements]
required = ["git"]
optional = ["npm", "cargo-audit"]
```

#### Step 3: Write SKILL.md

```markdown
# Security Scanner

## Purpose
Automated security vulnerability scanning across multiple domains.

## Workflow

### Phase 1: Target Discovery
1. Use `find` to locate all source files in the target directory
2. Identify languages and frameworks present
3. Use `bash` to check for dependency manifests (package.json, Cargo.toml, etc.)

### Phase 2: Static Analysis
1. Scan for hardcoded secrets (API keys, tokens, passwords)
   - Use `grep` with patterns: `sk-`, `token`, `password`, `secret`
2. Check input validation in web-facing code
3. Verify authentication middleware coverage
4. Look for SQL injection points

### Phase 3: Dependency Audit
1. If `npm` is available: run `npm audit --json`
2. If `cargo-audit` is available: run `cargo audit`
3. Parse and summarize findings

### Phase 4: Reporting
1. Compile findings into a severity-ranked report
2. Use `write` to save report to workspace
3. Use `audit()` to log scan results

## Output Format
```markdown
## Security Scan Report
**Target:** [path]
**Date:** [timestamp]
**Files scanned:** [count]

### Critical (must fix)
- [finding]

### High (should fix)
- [finding]

### Medium
- [finding]

### Low / Info
- [finding]
```

## Constraints
- Read-only — never modify source files
- Do not exploit found vulnerabilities
- Report all findings, even if they seem minor
```

#### Step 4: Install and Test

```bash
# Install
$ oxios pkg install ./my-security-scanner
  Installed 'security-scanner v1.0.0'

# Verify installation
$ oxios program security-scanner

# Test via chat
$ oxios chat
You: Scan my project for security issues
Agent: (uses security-scanner program)
```

---

## 10. Built-in Programs

Oxios ships with six built-in programs, installed automatically on first run.

### code-review

**Purpose:** Deep code review with quality domain analysis across security, performance, and correctness.

**Workflow:**
1. Discovery — find and read source files
2. Quality Analysis — correctness, security, performance
3. Reporting — severity-ranked findings with evidence

**Focus areas:** quality, security, performance, all

**Usage:** "Review the code in src/main.rs" or "Review this project focusing on security"

---

### debug

**Purpose:** Systematic debugging with hypothesis-driven approach.

**Workflow:**
1. Problem Characterization — parse error, identify scope
2. Hypothesis Formation — generate ranked hypotheses
3. Investigation — test each hypothesis
4. Root Cause Analysis — "5 Whys" technique
5. Fix & Verify — minimal fix + regression test

**Usage:** "The auth module crashes on startup with error X"

---

### deploy

**Purpose:** Safe deployment with pre-flight checks, rollback planning, and post-deploy verification.

**Workflow:**
1. Pre-Flight Checks — resources, credentials, backups
2. Rollback Planning — checkpoint, trigger conditions, procedure
3. Deployment Execution — step-by-step with logging
4. Post-Deploy Verification — smoke tests, metrics
5. Monitoring Period — 10-minute watch

**Safety constraints:**
- Production deployments require explicit user confirmation
- Always have rollback ready
- Monitor for minimum 10 minutes

**Usage:** "Deploy to staging" or "Deploy v2.1.0 to production"

---

### guardian

**Purpose:** Background daemon that periodically verifies system integrity.

**Checks (every 5 minutes):**
- Audit chain integrity (blake3 hash-chain)
- Resource overload detection (CPU, memory, load)
- Git repository integrity
- Budget status monitoring
- Periodic state checkpoint (auto-commit)

**Implementation:** Runs as a `tokio::spawn` background task. Uses only Kernel System Calls, not agent tools. Started automatically when the kernel boots.

---

### refactor

**Purpose:** Safe, incremental refactoring that preserves behavior.

**Workflow:**
1. Understanding — read code, identify public API, trace call sites
2. Planning — define goal, break into minimal steps, identify risks
3. Safe Refactoring — Martin Fowler's categories (preparatory, composing, encapsulating)
4. Verification — run tests before and after, check behavior
5. Commit — descriptive commit message

**Focus areas:** readability, performance, maintainability

**Constraints:**
- One refactoring goal at a time
- Maximum 10 files per session
- Always run tests before committing

**Usage:** "Refactor src/auth.rs for readability"

---

### program-creator

**Purpose:** Meta-program for creating new Oxios programs through conversation.

**Capabilities:**
- Translates natural language descriptions into structured programs
- Generates `program.toml` with proper metadata and tool definitions
- Writes effective `SKILL.md` instructions
- Installs and tests new programs

**Available Kernel APIs it can reference:**
- State: save, load, delete, list, commit
- Git: log, tag, restore, verify
- Agents: list, kill
- Memory: remember, search, stats
- Audit: audit, verify, query
- Budget: check, set, is_overloaded
- Scheduling: schedule, unschedule, list
- Programs: list, install, uninstall
- Skills: create, list
- Events: subscribe, publish

**Usage:** "Create a program that automatically runs tests on a schedule"

---

## 11. Sharing Programs

### Via Git Repository

The most common distribution method:

```bash
# Create a repository for your program
cd my-program
git init
git add .
git commit -m "Initial release"
git remote add origin https://github.com/you/oxios-program-my-program.git
git push -u origin main
```

Others install with:

```bash
oxios pkg install https://github.com/you/oxios-program-my-program.git
```

### Via Tarball

Host a `.tar.gz` file on any HTTP server:

```bash
tar czf my-program.tar.gz my-program/
# Upload my-program.tar.gz to your server
```

Install with:

```bash
oxios pkg install https://example.com/my-program.tar.gz
```

### Via Local Directory

For private or development programs:

```bash
oxios pkg install ./local-path/to/my-program
```

---

## 12. Best Practices

### Program Design

| Practice | Why |
|----------|-----|
| One program, one responsibility | Unix philosophy — compose small pieces |
| Make SKILL.md comprehensive | Agents use this to understand capabilities |
| Version semantically (SemVer) | Clear upgrade expectations |
| Declare all host dependencies | Prevent runtime failures |
| Include error handling in SKILL.md | Agents need to know fallback behavior |

### SKILL.md Writing

| Practice | Why |
|----------|-----|
| Use numbered steps | Clear execution order |
| Reference tools by name | Agents know exactly what to call |
| Include conditional logic | Handle different scenarios |
| Define output format | Consistent, parseable results |
| List constraints explicitly | Prevent unwanted side effects |

### Security

| Practice | Why |
|----------|-----|
| Never require root/sudo | Least privilege |
| Audit all operations | Accountability |
| Validate inputs in SKILL.md | Prevent injection |
| Use structured exec mode | Safer than raw shell |
| Declare minimum required tools | Reduce attack surface |

---

## 13. Skills System

### What Are Skills?

Skills are **markdown instruction templates** that teach agents how to perform specific tasks. Unlike programs, skills don't have tool definitions or host requirements — they're pure instruction documents.

### Skill Format

A skill is a markdown file stored in `~/.oxios/workspace/skills/`:

```markdown
---
name: my-skill
description: What this skill does
triggers: ["keyword1", "keyword2"]
---

# My Skill

## Purpose
Description of when and how to use this skill.

## Instructions
1. Step one
2. Step two
3. Step three
```

### Managing Skills

#### Via CLI

```bash
# Skills are managed through the program system
$ oxios pkg list    # Lists programs which contain skills
```

#### Via API

```bash
# List all skills
$ curl http://127.0.0.1:4200/api/skills

# Get a specific skill
$ curl http://127.0.0.1:4200/api/skills/code-review

# Create a new skill
$ curl -X POST http://127.0.0.1:4200/api/skills \
  -H "Content-Type: application/json" \
  -d '{
    "name": "my-custom-skill",
    "description": "Custom skill for my workflow",
    "content": "# My Custom Skill\n\n## Steps\n1. Do X\n2. Do Y"
  }'

# Delete a skill
$ curl -X DELETE http://127.0.0.1:4200/api/skills/my-custom-skill
```

### Skills vs Programs

| Aspect | Skill | Program |
|--------|-------|---------|
| **Scope** | Instruction template only | Full capability package |
| **Files** | Single `.md` file | Directory with `program.toml` + `SKILL.md` |
| **Tools** | Uses whatever tools are available | Declares and provides tools |
| **Dependencies** | None declared | Declares host requirements |
| **MCP** | No | Can register MCP servers |
| **Installation** | API or file copy | `oxios pkg install` |
| **Distribution** | Simple file | Git repo or tarball |

### Default Skills

Oxios ships with default skills in the `share/default-skills/` directory. These are initialized on first run:

- **code-review** — Code review instructions
- **debug** — Debugging workflow
- **refactor** — Refactoring guidelines
- Other project-specific skills

---

## Quick Reference

### Program Commands

```bash
oxios pkg install <source>       # Install from dir, git, or tarball
oxios pkg uninstall <name>       # Remove a program
oxios pkg list                   # List installed programs
oxios pkg search                 # Detailed program listing
oxios program <name>             # View program details + SKILL.md
```

### Program File Locations

| Path | Purpose |
|------|---------|
| `~/.oxios/workspace/programs/` | Installed programs |
| `~/.oxios/workspace/skills/` | Skill definitions |
| `.programs/` | Built-in programs (source) |
| `share/default-skills/` | Default skills (source) |
| `share/default-programs/` | Default programs (source) |

### API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/programs` | List programs |
| `POST` | `/api/programs` | Install program |
| `GET` | `/api/programs/{name}` | Get program details |
| `DELETE` | `/api/programs/{name}` | Uninstall program |
| `POST` | `/api/programs/{name}/enable` | Enable program |
| `POST` | `/api/programs/{name}/disable` | Disable program |
| `GET` | `/api/programs/{name}/host-requirements` | Check requirements |
| `GET` | `/api/skills` | List skills |
| `POST` | `/api/skills` | Create skill |
| `GET` | `/api/skills/{name}` | Get skill details |
| `DELETE` | `/api/skills/{name}` | Delete skill |

---

*Programs and skills are the application layer of the Oxios Agent OS. They compose kernel system calls into complete workflows that agents can execute autonomously.*
