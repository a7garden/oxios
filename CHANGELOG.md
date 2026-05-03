# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0-alpha] - 2026-05-03

### Added

- **Core kernel** (`oxios-kernel`) with supervisor, event bus, and state store
- **Ouroboros protocol** (`oxios-ouroboros`) — spec-first workflow:
  interview → seed → execute → evaluate → evolve
- **Gateway** (`oxios-gateway`) with channel-agnostic message routing
- **Web dashboard** (`oxios-web`) with chat, control, and browse panels
- **Apple Container integration** with Garden lifecycle management
- **Host Exec Bridge** for secure macOS command execution
- **Skill system** for markdown-based agent instruction templates
- **CLI** with `garden`, `run`, `status`, and `config` subcommands
- **38 tests** (25 unit + 13 integration)
- **7006 lines** of Rust code across 27 source files
- **1761 lines** of HTML for the web dashboard
