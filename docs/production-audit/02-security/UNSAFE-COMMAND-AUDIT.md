# Unsafe & Command Audit — 2026-05-31

Audited all `unsafe` blocks and `Command::new` / `std::process::Command` call sites in production code.

---

## Unsafe Blocks

### 1. `crates/oxios-kernel/src/memory/database.rs:245` — `sqlite3_auto_extension`

```rust
unsafe {
    // SAFETY: sqlite3_vec_init matches the sqlite3_auto_extension prototype.
    rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
        sqlite_vec::sqlite3_vec_init as *const (),
    )));
}
```

**Verdict:** ✅ **Sound. Already documented.**
- `sqlite3_vec_init` is the correct entry point for the sqlite-vec extension.
- The `transmute` is necessary because the C function pointer type doesn't match Rust's expected `Option<unsafe extern "C" fn(...)>` exactly.
- Protected by `AtomicBool` to ensure single registration.
- `#[allow(clippy::missing_transmute_annotations)]` is acceptable here — the target type is an FFI convention.
- **Recommendation:** Consider adding `#[allow(clippy::missing_transmute_annotations)]` with a comment explaining why it's safe, which is already done.

### 2. `crates/oxios-kernel/src/daemon.rs:107` — `libc::kill(SIGTERM)`

```rust
let ret = unsafe { libc::kill(pid as i32, libc::SIGTERM) };
```

**Verdict:** ✅ **Sound. Safety invariant: PID is read from PID file owned by Oxios.**
- The PID comes from `self.read_pid()` which reads a file that only Oxios writes to (`~/.oxios/oxios.pid`).
- Signal 0 (check) and SIGTERM are safe signals — no memory manipulation.
- `pid as i32` is safe: PID values fit in i32.
- **Recommendation:** Add `// SAFETY:` comment (see below).

### 3. `crates/oxios-kernel/src/daemon.rs:321` — `libc::kill(0)` (process check)

```rust
unsafe { libc::kill(pid as i32, 0) == 0 }
```

**Verdict:** ✅ **Sound. Signal 0 is a no-op check.**
- Same PID source as above. Signal 0 doesn't send any signal — just checks existence.
- **Recommendation:** Add `// SAFETY:` comment (see below).

### 4. `src/main.rs:856` — `libc::kill(SIGTERM)` in reset command

```rust
unsafe {
    libc::kill(pid as i32, libc::SIGTERM);
}
```

**Verdict:** ✅ **Sound. Same pattern as daemon.rs.**
- PID read from PID file during `oxios reset` command. Only called interactively by the user.
- **Recommendation:** Add `// SAFETY:` comment.

---

## SAFETY Comment Recommendations

Three blocks lack `// SAFETY:` comments. Recommend adding:

**`daemon.rs:107`:**
```rust
// SAFETY: PID is read from Oxios-owned PID file. libc::kill with SIGTERM
// is a standard POSIX signal delivery — no memory safety implications.
let ret = unsafe { libc::kill(pid as i32, libc::SIGTERM) };
```

**`daemon.rs:321`:**
```rust
// SAFETY: Signal 0 is a no-op existence check per POSIX. PID source is
// the Oxios-owned PID file. No signals are delivered to the target process.
unsafe { libc::kill(pid as i32, 0) == 0 }
```

**`main.rs:856`:**
```rust
// SAFETY: PID is read from the daemon PID file during interactive reset.
// SIGTERM is a standard termination signal with no memory safety implications.
unsafe {
    libc::kill(pid as i32, libc::SIGTERM);
}
```

---

## Command::new Audit

### 1. `crates/oxios-kernel/src/tools/exec_tool.rs:230` — Shell mode

```rust
let mut child = tokio::process::Command::new("bash")
    .arg("-c")
    .arg(command)
```

**Input trace:** `command` ← `params["command"]` ← agent tool call ← LLM-generated  
**Sanitization:**
- ✅ Guarded by `allow_shell_mode` config (default: `false`)
- ✅ Access control: `access.can_use_tool(agent_name, "bash")`
- ✅ Empty command check
- ✅ Environment stripped to safe subset (HOME, USER, PATH, LANG, TERM)
- ✅ Timeout enforced
- ✅ Shutdown signal handler kills child
- ⚠️ Command string is passed verbatim to `bash -c` — by design (shell mode = arbitrary shell)

**Verdict:** ✅ **Secure by design.** Shell mode is off by default (`allow_shell_mode = false`). When enabled, it requires explicit RBAC permission for `bash`. The audit trail logs the command (first 200 chars). This is the intended behavior — shell mode is for trusted agents that need full shell access.

### 2. `crates/oxios-kernel/src/tools/exec_tool.rs:355` — Structured mode

```rust
let mut child = tokio::process::Command::new(binary)
    .args(&args)
```

**Input trace:** `binary` + `args` ← agent tool call params ← LLM-generated  
**Sanitization:**
- ✅ Binary must be bare name (no `/` or `..`)
- ✅ Binary must be in allowlist (`ExecConfig::is_binary_allowed`)
- ✅ Arguments checked for shell metacharacters (`;`, `|`, `$`, `` ` ``, `<`, `>`, `(`, `)`, `{`, `}`, newlines)
- ✅ Arguments checked for path traversal (`..`)
- ✅ Environment stripped to safe subset
- ✅ Access control via `can_use_tool`
- ✅ Timeout enforced

**Verdict:** ✅ **Secure.** Defense in depth — three independent checks (allowlist, metachar block, RBAC).

### 3. `crates/oxios-kernel/src/skill/requirements.rs:7` — Binary check

```rust
std::process::Command::new("which").arg(bin).output()
```

**Input trace:** `bin` ← `SkillMetadata.requires.bins` ← SKILL.md YAML frontmatter ← filesystem  
**Sanitization:**
- `bin` comes from parsed YAML frontmatter in skill definition files
- `which` is a read-only utility that doesn't execute the target
- Arguments are passed as a single arg (not shell-expanded)

**Verdict:** ✅ **Secure.** `which` is a query tool, not execution. No user-controlled command injection possible.

### 4. `crates/oxios-kernel/src/daemon.rs:83` — Daemon self-spawn

```rust
let child = std::process::Command::new(&exe)
    .arg("--foreground")
    .arg("--config")
    .arg(config_path)
```

**Input trace:** `exe` = `std::env::current_exe()` (own binary), `config_path` ← CLI args  
**Sanitization:**
- Both inputs are controlled by the Oxios binary itself
- No user-controlled input reaches this path

**Verdict:** ✅ **Secure.** Self-spawn with hardcoded flags.

### 5. `crates/oxios-kernel/src/daemon.rs:115` — Windows taskkill

```rust
let _ = std::process::Command::new("taskkill")
    .args(["/PID", &pid.to_string(), "/F"])
    .output();
```

**Input trace:** `pid` ← PID file ← Oxios-owned  
**Sanitization:**
- PID is numeric (from `pid.to_string()`)
- Only runs on non-Unix (Windows)

**Verdict:** ✅ **Secure.** Numeric PID, no injection surface.

### 6. `crates/oxios-mcp/src/client.rs:87` — MCP server spawn

```rust
let mut child = Command::new(&self.server.command)
    .args(&self.server.args)
    .envs(&self.server.env)
```

**Input trace:** `server.command` + `server.args` + `server.env` ← `config.toml` [mcp] section  
**Sanitization:**
- All inputs come from `config.toml` — admin-controlled
- No agent-controlled input reaches this path
- MCP server configs are defined at deploy time, not runtime

**Verdict:** ✅ **Secure.** Admin-controlled configuration. However, note that if an agent could modify the config file, it could achieve arbitrary code execution. The config file should be owner-read-only.

---

## Summary

| Category | Count | Issues Found |
|----------|-------|-------------|
| `unsafe` blocks | 4 | 0 soundness issues. 3 missing `// SAFETY:` comments. |
| `Command::new` sites | 6 | 0 injection vulnerabilities. All properly sanitized or admin-controlled. |

### Action Items

1. ✅ Add `// SAFETY:` comments to 3 `unsafe` blocks in `daemon.rs` and `main.rs` (low priority, code hygiene)
2. ℹ️ Consider making config.toml owner-read-only during `oxios onboard` (defense in depth for MCP server commands)
3. ℹ️ No code changes required for security — all Command sites are properly gated
