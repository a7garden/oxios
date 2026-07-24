# Oxios Security Posture — 2026-07-24

**Audit scope:** dependency vulnerabilities, command execution, optional Wasm sandbox, and local web API.
**Review date:** 2026-07-24

## Vulnerability Summary

| Finding | Status | Action |
|---|---|---|
| Wasmtime 36.0.12 | Resolved | Keep `wasm-sandbox` opt-in and rerun its feature gate after upgrades. |
| `rsa` RUSTSEC-2023-0071 | Accepted exception | No upstream fix; monitor `oxi-sdk`/`rsa`. |
| Unmaintained/yanked transitive crates | Tracked | Review when upstream dependency updates become available. |

## Security Controls

- Shell execution is disabled by default and gated by AccessManager.
- Structured execution uses an allowlist and metacharacter checks.
- Workspace and knowledge paths use canonicalization/sandbox checks.
- API defaults to loopback binding and restricted CORS.
- Wasm sandbox memory, fuel, module-size, and wall-clock limits are enforced.

## Operational Caveat

Binding the API to a non-loopback interface requires explicit authentication and
TLS termination. Authenticated URL skill import and query-string WebSocket fallback
remain review items because they can expose internal network details or credentials.

## Verification

- Rust formatting, clippy, workspace build, nextest, and doctests pass.
- Frontend typecheck, tests, Biome lint, and production build pass.
- `cargo audit` reports no unignored vulnerabilities.
