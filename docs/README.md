# Oxios Documentation

> Complete documentation for the Oxios Agent Operating System.

---

## Getting Started

| Document | Description |
|----------|-------------|
| [**Getting Started Guide**](getting-started.md) | Installation, configuration, CLI reference, daemon management — everything you need to start using Oxios |
| [**README**](../README.md) | Project overview, architecture at a glance, quick start |

## Reference

| Document | Description |
|----------|-------------|
| [**Architecture**](architecture.md) | Internal architecture — layers, subsystems, data flow, KernelHandle, dependency rules |
| [**REST API Reference**](api-reference.md) | Complete API documentation for all 76 endpoints with curl examples |
| [**Programs & Skills**](programs-and-skills.md) | Program system (installable agent capabilities), skill templates, built-in programs |
| [**Security**](security.md) | Security model, RBAC, audit trail, circuit breaker, execution security, best practices |

## Integration

| Document | Description |
|----------|-------------|
| [**Channel Plugin Guide**](channel-plugin-guide.md) | How to create custom channels, REST API integration, SSE events, Telegram webhooks |
| [**Channel Registry**](channel-registry.md) | Channel plugin system architecture and feature flags |

## Design Documents (Internal)

These documents describe internal design decisions and are primarily for contributors:

| Document | Description |
|----------|-------------|
| [RFC-001: Kernel Facade](rfc-001-kernel-facade.md) | KernelHandle facade pattern design |
| [RFC-002: Kernel Module Organization](rfc-002-kernel-module-organization.md) | Module reorganization plan |
| [RFC-003: CLI UX Improvements](rfc-003-cli-ux-improvements.md) | CLI user experience improvements |
| [RFC-003: Web Dashboard Audit](rfc-003-web-dashboard-audit.md) | Web dashboard audit findings |
| [Refactoring Design](refactoring-design.md) | Large-scale refactoring plan |
| [Remaining Items](remaining-items-design.md) | Outstanding design items |

## Archive

Historical design documents are in the [`archive/`](archive/) and [`design/`](design/) directories.

---

## Quick Links

**New to Oxios?** Start with the [Getting Started Guide](getting-started.md).

**Building an integration?** Read the [Channel Plugin Guide](channel-plugin-guide.md) and [REST API Reference](api-reference.md).

**Contributing code?** Read the [Architecture](architecture.md) doc and the project's [AGENTS.md](../AGENTS.md).

**Security audit?** Read the [Security Guide](security.md).

---

*All documentation is in English. User-facing messages in the application are in Korean.*
