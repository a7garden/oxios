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
| [**Skills Guide**](programs-and-skills.md) | Unified skill system — SKILL.md frontmatter, requirements, install specs, built-in skills (formerly Programs & Skills) |
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
| [RFC-003: CLI UX Improvements](rfc-003-cli-ux-improvements.md) | CLI user experience improvements |
| [RFC-003: Web Dashboard Audit](rfc-003-web-dashboard-audit.md) | Web dashboard audit findings |
| [RFC-003: Knowledge Separation](rfc-003-knowledge-separation.md) | Knowledge vs memory architecture separation |
| [RFC-004: Knowledge System](rfc-004-knowledge-system.md) | Knowledge system design |
| [RFC-005: Knowledge Integration](rfc-005-knowledge-integration.md) | Knowledge system integration with AI engine |
| [RFC-006: JS/Space Integration](rfc-006-js-space-integration.md) | JavaScript/Space integration design |
| [RFC-007: Remaining Port](rfc-007-remaining-port.md) | Remaining feature porting |
| [RFC-008: Memory Consolidation](rfc-008-memory-consolidation.md) | Tiered memory with Dream-time compaction |
| [RFC-009: Skill Unification](rfc-009-skill-unification.md) | Unified Skill model (Programs + Skills merged) |
| [RFC-010: Clawhub Marketplace](rfc-010-clawhub-marketplace.md) | Marketplace for sharing skills and agents |
| [Refactoring Design](refactoring-design.md) | Large-scale refactoring plan |
| [Remaining Items](remaining-items-design.md) | Outstanding design items |

## Archive

Historical design documents are in the [`archive/`](archive/) directory. This includes:
- Old RFCs (RFC-001, RFC-002) — superseded by current architecture
- Analysis results (clippy reports, security audits, kernel analysis)
- Previous architecture iterations
- Work-in-progress designs in `designs/`

For recursive-improvement-loop artifacts, see `archive/designs/loop*.md`.

---

## Quick Links

**New to Oxios?** Start with the [Getting Started Guide](getting-started.md).

**Building an integration?** Read the [Channel Plugin Guide](channel-plugin-guide.md) and [REST API Reference](api-reference.md).

**Contributing code?** Read the [Architecture](architecture.md) doc and the project's [AGENTS.md](../AGENTS.md).

**Security audit?** Read the [Security Guide](security.md).

---

*All documentation is in English. User-facing messages in the application are in Korean.*
