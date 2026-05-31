# Production Audit — Area Briefs

> **Purpose:** One markdown brief per area. Each brief is a self-contained task description
> that an agent can read and execute independently in a separate session.
>
> **How to use:** Open a new agent session, point it at the brief file, and let it run.
>
> **Generated:** 2026-05-31
> **Oxios version:** 0.6.0

## Areas

| # | Area | Brief | Severity |
|---|------|-------|----------|
| 01 | Code Quality — unwrap() & error handling | [01-code-quality/BRIEF.md](01-code-quality/BRIEF.md) | 🔴 Critical |
| 02 | Security — dependency vulnerabilities | [02-security/BRIEF.md](02-security/BRIEF.md) | 🔴 Critical |
| 03 | Testing — E2E & coverage gaps | [03-testing/BRIEF.md](03-testing/BRIEF.md) | 🟡 High |
| 04 | Frontend — TypeScript & bundle | [04-frontend/BRIEF.md](04-frontend/BRIEF.md) | 🟡 High |
| 05 | Resilience — session persistence & retry | [05-resilience/BRIEF.md](05-resilience/BRIEF.md) | 🟡 Medium |
| 06 | Observability — OTel & metrics | [06-observability/BRIEF.md](06-observability/BRIEF.md) | 🟢 Normal |
| 07 | Infrastructure — release profile & deploy | [07-infra/BRIEF.md](07-infra/BRIEF.md) | 🟡 High |
| 08 | Docs & Public API surface | [08-docs-api/BRIEF.md](08-docs-api/BRIEF.md) | 🟢 Normal |

## Guiding Principles (apply to ALL briefs)

1. **No false positives** — Every finding must be verified against the actual code before acting. If a pattern is intentional and safe (e.g., `lock().unwrap()` on a lock that cannot fail in that context), leave it alone and document why.
2. **No over-engineering** — Do not split files or create new modules purely for "clean code" aesthetics. A 500-line file is fine if it has clear internal structure. Split only when there is a concrete maintainability or ownership problem.
3. **Respect legacy structure** — Existing module boundaries exist for a reason. Do not reorganize directory structures unless the brief explicitly calls for it. Work within the current architecture.
4. **Beautiful but practical** — Aim for readable, idiomatic Rust. Avoid clever abstractions that obscure intent. Prefer `?` over `map_err` when the error type is already correct.
5. **Backwards compatible** — All changes must pass `cargo test --workspace`. No API breaks unless explicitly scoped in the brief.
