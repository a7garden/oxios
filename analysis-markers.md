# Codebase Analysis: Markers, Risks & Stubs

> Generated: 2026-05-28  
> Scope: All `.rs` files in `/Volumes/MERCURY/PROJECTS/oxios`

---

## 1. TODO Markers

### Production Code

| File | Line | TODO |
|------|------|------|
| `src/kernel.rs` | 287 | `// TODO: wire bundled defaults dir` — `init_default_skills()` discards the `defaults_dir` param entirely |
| `channels/oxios-cli/src/interactive.rs` | 151 | `// TODO: wire to kernel model switching` |
| `channels/oxios-cli/src/interactive.rs` | 160 | `// TODO: wire to kernel persona switching` |
| `crates/oxios-kernel/src/agent_runtime.rs` | 410 | `// TODO: replace with TrailAuditSink when wired` — Uses `TracingAuditSink` (logs only) instead of real audit trail |
| `crates/oxios-ouroboros/src/degraded.rs` | 46 | `/// TODO: Connect to generate_seed() fallback when full integration is done.` |
| `crates/oxios-markdown/src/schedule.rs` | 88 | `// TODO: disallow split for read/watch` |

### Test Code (Ignored Tests)

| File | Line | TODO |
|------|------|------|
| `crates/oxios-kernel/src/space/detection.rs` | 313 | `#[ignore] // TODO: regex pattern in full context` |
| `crates/oxios-kernel/src/space/detection.rs` | 321 | `#[ignore] // TODO: keyword matching needs verification` |

### `todo!()` / `unimplemented!()` Macros

**None found.** The codebase has zero `todo!()` or `unimplemented!()` macro calls.

### FIXME / HACK / XXX Markers

**None found.** The codebase has zero `FIXME`, `HACK`, or `XXX` comments.

---

## 2. `panic!()` Calls in Production Paths

| File | Line | Context |
|------|------|---------|
| `channels/oxios-web/src/error.rs` | 94 | `_ => panic!("Expected Internal variant")` — inside a test-only match arm |
| `crates/oxios-kernel/src/audit_trail.rs` | 677 | `_ => panic!("expected ChainBroken error")` — test-only |
| `crates/oxios-kernel/src/embedding/gguf/mod.rs` | 309 | `_ => panic!("Expected DenseF32")` — test-only |

**All `panic!()` calls are inside test code.** No production panics found.

---

## 3. `.unwrap()` Calls in Non-Test Production Code

### High Severity — Can Crash at Runtime

These `.unwrap()` calls are in production code paths and will panic on failure:

| File | Line | Code | Risk |
|------|------|------|------|
| `src/kernel.rs` | 230 | `ClawHubClient::new(Some("https://clawhub.ai".to_string())).unwrap()` | Falls through error branch; hardcoded URL parse should succeed, but masks future breakage |
| `src/kernel.rs` | 363 | `.and_hms_opt(3, 0, 0).unwrap()` | **Time calculation** — always valid (3am), but defensive coding preferred |
| `src/kernel.rs` | 365 | `.and_local_timezone(Local).unwrap()` | **Timezone conversion** — can theoretically fail at DST boundaries |
| `channels/oxios-web/src/plugin.rs` | 144 | `user_web_version_file().unwrap()` | **File path resolution** — will panic if home dir is unavailable |
| `channels/oxios-web/src/plugin.rs` | 243 | `.body(Body::from(data)).unwrap()` | HTTP response builder — should always succeed but chained on file reads |
| `channels/oxios-web/src/plugin.rs` | 252 | `.body(Body::from(data)).unwrap()` | Same pattern, assets/ prefix path |
| `channels/oxios-web/src/plugin.rs` | 274 | `.body(Body::from(content.data.to_vec())).unwrap()` | Embedded asset response builder |
| `channels/oxios-web/src/plugin.rs` | 276 | `Response::builder().status(404).body(Body::empty()).unwrap()` | 404 fallback — should always succeed |
| `channels/oxios-web/src/plugin.rs` | 303 | `.body(Body::from(data)).unwrap()` | index.html filesystem serve |
| `channels/oxios-web/src/plugin.rs` | 313 | `.body(Body::from(content.data.to_vec())).unwrap()` | Embedded index.html |
| `channels/oxios-web/src/plugin.rs` | 319 | `.body(Body::from("not found")).unwrap()` | 404 for index.html missing |
| `channels/oxios-cli/src/channel.rs` | 180 | `self.session.lock().unwrap()` | **Mutex poisoning** — will panic if another thread panicked while holding the lock |
| `crates/oxios-kernel/src/orchestrator.rs` | 789 | `best_eval.unwrap()` | **Orchestrator** — panics if evaluation loop invariant is violated |
| `crates/oxios-kernel/src/orchestrator.rs` | 794 | `best_eval.unwrap()` | Same pattern, max_iterations == 0 branch |
| `crates/oxios-kernel/src/orchestrator.rs` | 828 | `best_eval.unwrap()` | Same pattern, evolve returned None branch |
| `crates/oxios-kernel/src/onboarding.rs` | 363 | `.find(\|\|&p\|\| == selected.id).unwrap()` | **Onboarding** — panics if user selects a provider not in the list (UI race?) |
| `crates/oxios-kernel/src/onboarding.rs` | 520 | `.template("...").unwrap()` | Spinner template construction — should always succeed |
| `crates/oxios-kernel/src/clawhub/installer.rs` | 117 | `origin_path.parent().unwrap()` | Path construction — could fail if path is root |
| `crates/oxios-kernel/src/clawhub/installer.rs` | 187 | `origin_path.parent().unwrap()` | Same pattern |
| `crates/oxios-kernel/src/clawhub/installer.rs` | 348 | `zip.by_index(i).unwrap()` | **Zip iteration** — panics on corrupt archive |
| `crates/oxios-kernel/src/state_store.rs` | 566 | `self.last_prune.lock().unwrap()` | **Mutex poisoning** in prune path |
| `crates/oxios-kernel/src/skill/manager.rs` | 86 | `skill.path.parent().unwrap()` | **Skill loading** — panics if skill path has no parent directory |
| `crates/oxios-kernel/src/space/manager.rs` | 81 | `Uuid::parse_str("00000000-...").unwrap()` | Hardcoded UUID — always succeeds, but defensive coding preferred |
| `crates/oxios-kernel/src/space.rs` | 189 | `Uuid::parse_str("00000000-...").unwrap()` | Same pattern |
| `crates/oxios-ouroboros/src/regression.rs` | 71 | `self.generations.last().unwrap()` | **Regression tracking** — panics if called with empty generations |

### Medium Severity — `Regex::new().unwrap()` (Compile-Time Regex)

These panic only if the regex pattern is invalid (a code bug, not runtime condition):

| File | Lines |
|------|-------|
| `crates/oxios-markdown/src/chat.rs` | 13, 19, 27, 28, 60, 75, 76, 99, 163 |
| `crates/oxios-markdown/src/worker.rs` | 126, 128, 285, 300 |
| `crates/oxios-markdown/src/html.rs` | 27, 38, 310, 330, 339, 345 |
| `crates/oxios-markdown/src/merge.rs` | 86, 87 |
| `crates/oxios-markdown/src/parser.rs` | 236, 242, 251, 269 |
| `crates/oxios-markdown/src/tgtxt.rs` | 35, 36, 37 |
| `crates/oxios-markdown/src/journal.rs` | 43, 73 |
| `crates/oxios-markdown/src/habits.rs` | 276 |

### Low Severity — Date/Time `unwrap()` (Always-Valid Constants)

| File | Lines |
|------|-------|
| `crates/oxios-markdown/src/habits.rs` | 77, 218, 233, 234 |
| `crates/oxios-markdown/src/stats.rs` | 74 |
| `crates/oxios-markdown/src/schedule.rs` | 179, 198, 201, 211 |
| `crates/oxios-markdown/src/worker.rs` | 273, 279, 352, 365, 368 |

---

## 4. `.expect()` Calls in Production Code

### High Severity — Startup/Init Paths

| File | Line | Code |
|------|------|------|
| `src/main.rs` | 1750 | `gateway.run().await.expect("gateway run error")` — **Gateway fatal error** kills the spawned task |
| `src/kernel.rs` | 82 | `KnowledgeBase::new(...).expect("KnowledgeBase init failed")` — **Kernel startup** |
| `src/kernel.rs` | 89 | `KnowledgeLens::new(...).expect("KnowledgeLens init failed")` — **Kernel startup** |
| `src/kernel.rs` | 776 | `KnowledgeBase::new(...).expect(...)` — Kernel builder alt path |
| `src/kernel.rs` | 785 | `KnowledgeBase::new(...).expect(...)` — Kernel builder alt path |
| `src/kernel.rs` | 789 | `KnowledgeLens::new(...).expect(...)` — Kernel builder alt path |
| `crates/oxios-kernel/src/kernel_handle/mod.rs` | 177 | `KnowledgeBase::new(...).expect("Failed to create KnowledgeBase")` |
| `crates/oxios-kernel/src/kernel_handle/mod.rs` | 181 | `KnowledgeLens::new(...).expect("Failed to create KnowledgeLens")` |
| `crates/oxios-kernel/src/kernel_handle/mod.rs` | 227 | `ClawHubClient::new(...).expect("valid ClawHub client")` |
| `crates/oxios-kernel/src/clawhub/installer.rs` | 68 | `ClawHubClient::new(base_url).expect("valid ClawHub base URL")` |
| `crates/oxios-markdown/src/i18n.rs` | 13 | `.expect("Failed to parse embedded emojis.json")` — Embedded asset load |

### Medium Severity — I/O/Construction

| File | Line | Code |
|------|------|------|
| `crates/oxios-mcp/src/client.rs` | 99 | `.expect("stdin not captured — stdin was piped")` |
| `crates/oxios-mcp/src/client.rs` | 103 | `.expect("stdout not captured — stdout was piped")` |
| `crates/oxios-kernel/src/orchestrator.rs` | 961 | `.expect("execute_single_subtask is only called when subtasks is non-empty")` |
| `crates/oxios-kernel/src/auth.rs` | 185 | `getrandom::getrandom(&mut bytes).expect("failed to generate random bytes")` |
| `crates/oxios-kernel/src/memory/database.rs` | 310 | `chunk.try_into().expect("chunk must be 4 bytes")` |
| `crates/oxios-kernel/src/memory/graph.rs` | 44, 49 | `.expect("entry(or_default) guarantees existence")` |

---

## 5. Empty Function Bodies / Stub Implementations

| File | Line | Signature | Notes |
|------|------|-----------|-------|
| `crates/oxios-ouroboros/src/protocol.rs` | 75 | `fn set_persona_prompt(&self, _prompt: Option<String>) {}` | **No-op trait default impl** — persona prompt is silently ignored |
| `crates/oxios-kernel/src/access_manager/audit_sink.rs` | 233 | `fn record(&self, _event: AuditEvent) {}` | **No-op** — `#[cfg(test)]` only, acceptable |

---

## 6. Commented-Out Code

Only one instance of meaningful commented-out code found:

| File | Line | Code |
|------|------|------|
| `channels/oxios-web/src/routes/workspace.rs` | 1025 | `// if body.len() > MAX_FILE_SIZE { return PayloadTooLarge }` — **File size limit** was disabled |

---

## Summary Statistics

| Category | Count | Production vs Test |
|----------|-------|--------------------|
| `todo!()` / `unimplemented!()` macros | **0** | — |
| `FIXME` / `HACK` / `XXX` | **0** | — |
| `TODO` comments | **8** | 6 production, 2 test |
| `panic!()` calls | **3** | 0 production, 3 test |
| `.unwrap()` production (high severity) | **~25** | All production |
| `.unwrap()` production (regex/date — low risk) | **~40** | All production |
| `.expect()` production (startup/init) | **11** | All production |
| `.expect()` production (I/O/construction) | **6** | All production |
| Empty/stub function bodies | **1** | 1 production (`set_persona_prompt`) |
| Commented-out code blocks | **1** | 1 production |

### Top Priority Fixes

1. **`channels/oxios-cli/src/channel.rs:180`** — `Mutex::lock().unwrap()` should use `.lock().unwrap_or_else(|e| e.into_inner())` to survive poison
2. **`crates/oxios-kernel/src/state_store.rs:566`** — Same mutex poisoning risk
3. **`crates/oxios-kernel/src/orchestrator.rs:789-828`** — Three `.unwrap()` on `best_eval` in orchestrator loop; should return error or use `ok_or_else`
4. **`crates/oxios-kernel/src/clawhub/installer.rs:348`** — `zip.by_index(i).unwrap()` on untrusted archive data
5. **`crates/oxios-kernel/src/onboarding.rs:363`** — `.find().unwrap()` on user-driven selection
6. **`crates/oxios-kernel/src/agent_runtime.rs:410`** — TODO: audit trail not connected; agents run without audit
7. **`src/kernel.rs:287`** — TODO: default skills directory is discarded, never wired
8. **`channels/oxios-web/src/routes/workspace.rs:1025`** — File size limit commented out (potential DoS vector)
9. **`crates/oxios-ouroboros/src/protocol.rs:75`** — `set_persona_prompt` is a no-op; persona customization doesn't work
10. **`channels/oxios-web/src/plugin.rs:144-319`** — Many `.unwrap()` on HTTP response builders; should use `unwrap_or_default` or error handler
