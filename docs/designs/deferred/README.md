# Deferred Designs

Designs that were explored but have **no implementation planned**. Kept for
historical context and to preserve the reasoning behind decisions made
elsewhere.

A file here is not abandoned — it is parked. To resume one:

1. Move the file (and its sub-specs) back to `docs/designs/`.
2. Drop the `DEFERRED` note from its status header.
3. Open a new spec → plan → implementation cycle for it.

## Current contents

| File | What | Why deferred |
|---|---|---|
| `2026-07-29-managed-relay-architecture.md` | Managed-relay remote access (level-0). Oxios-operated relay (`relay.oxios.com`) + outbound tunnel from the host binary + E2E Noise encryption. Decomposes into 5 sub-specs. | Direction is sound, but scope (5 subsystems, Cloudflare infra, relay ops cost) is large relative to current priorities. Revisit when remote access becomes a priority. |
| `2026-07-29-managed-relay-A-oauth-broker.md` | Sub-spec A — OAuth broker Worker. | Parent architecture is deferred. |
| `2026-07-29-remote-access-architecture-design.md.superseded` | Prior Tailscale-Serve-as-default design. | Superseded by the managed-relay doc above; kept for its §2–§3 analysis, which still informs any future remote-access work. |

## Related (not deferred)

The `tailscale_auth` config sketch in the superseded doc (§5.1) remains a
candidate for a small, local-only improvement if Tailscale users ask for it —
that would be a separate, much smaller design and does not require the
managed relay.
