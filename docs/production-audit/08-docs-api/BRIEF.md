# Brief 08: Documentation & Public API Surface

**Area:** Public API documentation, doc comments, type-level docs  
**Severity:** 🟢 Normal  
**Estimated scope:** `#![warn(missing_docs)]` enabled, 1 warning, no `#[doc]` attributes  

---

## Context

Oxios has excellent **architectural documentation** (ARCHITECTURE.md, RFCs,
AGENTS.md, CHANGELOG.md). The **code-level documentation** is a different story:

- `#![warn(missing_docs)]` is enabled on public crates ✅
- Only **1 missing_docs warning** currently — suggests most public items
  are documented
- But `#[doc]` attribute count is **0** — no structured doc attributes
- Doc-tests: 5 pass, **11 ignored** in oxios-kernel
- API reference exists (`docs/api-reference.md`) but is likely stale
- No generated documentation (no `cargo doc` deployment)

**What exists:**
- `docs/api-reference.md` — 61KB, likely comprehensive but may be outdated
- `docs/getting-started.md` — 32KB
- `docs/USER-GUIDE.md` — exists
- Per-module inline comments — decent, tracing-guided

**What's missing:**
- Doc examples that compile and run (the 11 ignored doc-tests)
- `cargo doc` output that's browsable
- Public API stability documentation

---

## Objective

1. **Fix** the 11 ignored doc-tests in oxios-kernel
2. **Audit** `docs/api-reference.md` for staleness
3. **Assess** `cargo doc` output quality
4. **Document** API stability guarantees

This does NOT mean:
- ❌ Adding `#[doc]` attributes to every public item
- ❌ Generating a documentation website
- ❌ Writing tutorials or guides
- ❌ Creating API versioning schemes

It DOES mean:
- ✅ Making existing doc examples compile and pass
- ✅ Identifying stale documentation
- ✅ Ensuring `cargo doc --workspace` produces useful output

---

## Approach

### Phase 1: Doc-test Fix

For each of the 11 ignored doc-tests in oxios-kernel:

1. Read the doc comment containing the ignored example
2. Determine why it's ignored:
   - Missing imports → add `use` statements
   - References non-existent API → update to current API
   - Requires runtime state → add setup or change to `no_run`
   - Outdated example → rewrite for current API
3. Fix and remove `ignore` annotation
4. Verify: `cargo test --doc -p oxios-kernel`

Write results to `docs/production-audit/08-docs-api/DOCTEST-FIXES.md`

### Phase 2: API Reference Audit

1. Read `docs/api-reference.md`
2. Spot-check against current code:
   - Are the tool names still correct?
   - Are the parameter schemas up to date?
   - Are the route paths still valid?
3. Note sections that need updates — do NOT rewrite the entire document
4. Write a changelog to `docs/production-audit/08-docs-api/API-REFERENCE-DELTA.md`

### Phase 3: cargo doc Assessment

1. Run `cargo doc --workspace --no-deps`
2. Check for warnings (broken links, missing docs)
3. Open the generated HTML — is it navigable?
4. Identify the top 5 most important public types that lack docs
5. Write assessment to `docs/production-audit/08-docs-api/CARGO-DOC-ASSESSMENT.md`

### Phase 4: API Stability Note

Add a short section to the project README or `docs/`:

```markdown
## API Stability

Oxios is pre-1.0. Public APIs may change between minor versions.
The following are considered **stable** within 0.x:
- CLI interface (`oxios run`, `oxios status`, `oxios doctor`)
- Configuration file format (`config.toml`)
- Skill definition format (`SKILL.md` frontmatter)

The following are **unstable** and may change:
- Rust library API (all `pub` items in crates)
- Web API routes and response shapes
- Internal tool protocols
```

---

## Constraints

- **Do not** rewrite documentation from scratch
- **Do not** add new documentation files (only update existing)
- **Do not** change public API signatures
- **Do not** create documentation infrastructure (no mdbook, docusaurus, etc.)
- **Preserve** the existing documentation style and formatting

## Verification

1. `cargo test --doc -p oxios-kernel` — more tests passing, fewer ignored
2. `cargo doc --workspace --no-deps` — fewer warnings
3. Doc changes are minimal and targeted
