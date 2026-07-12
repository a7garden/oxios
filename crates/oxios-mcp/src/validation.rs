//! MCP server spawn validation — the single chokepoint that prevents
//! arbitrary command execution via MCP server configuration.
//!
//! `validate_mcp_command` + `sanitize_env` are enforced inside
//! [`crate::client::McpClient::initialize`] (the spawn site), so EVERY
//! MCP server spawn — whether the `McpServer` came from the HTTP API,
//! the `[[mcp.servers]]` config, or an `OXIOS_MCP_*` env var — passes
//! through this gate. No caller can bypass it.
//!
//! The HTTP layer (`src/api/routes/infra.rs`) re-uses these functions
//! to give users a friendly 400 before registration; the boot path
//! (`init_mcp_bridge`) re-uses them to reject bad config at startup.
//! But the authoritative enforcement is here, at spawn.

use std::collections::HashMap;

/// Shell interpreters that must never be spawned directly as an MCP
/// server — they would allow `args = ["-c", "<arbitrary>"]` code
pub const BLOCKED_MCP_SHELLS: &[&str] = &[
    "sh", "bash", "dash", "zsh", "ksh", "csh", "tcsh", "fish", "ash", "busybox",
    // Windows interpreters — include the `.exe` variants because the
    // basename match is exact; `cmd` would not catch `cmd.exe`.
    "cmd", "cmd.exe", "powershell", "powershell.exe", "pwsh", "pwsh.exe",
    // Scripting interpreters that can eval arbitrary code from args.
    "python", "python2", "python3", "perl", "ruby", "node", "nodejs", "deno", "bun",
    "env",
];

/// Characters that must never appear in an MCP server command token.
/// MCP commands are a single token (e.g. `npx`, `uvx`); any of these
/// indicates an attempt to chain, inject, or shell-out.
const FORBIDDEN_CHARS: &[char] = &[
    ' ', '\t', ';', '|', '&', '>', '<', '`', '$', '(', ')', '{', '}', '\n', '\r', '*', '?',
    '\\', '"', '\'',
];

/// Validate an MCP server command before spawning it.
///
/// Rejects control bytes, shell metacharacters, path traversal (`..`),
/// and shell-interpreter basenames. Returns `Ok(())` if the command is
/// a safe single token that is not a known shell.
///
/// # Errors
/// Returns a human-readable reason string describing why the command
/// was rejected.
pub fn validate_mcp_command(command: &str) -> Result<(), String> {
    if command.is_empty() {
        return Err("command must not be empty".into());
    }
    // Reject control / NUL bytes outright.
    if command.chars().any(|c| c.is_control() || c == '\u{0}') {
        return Err("command contains control characters".into());
    }
    // Reject shell metacharacters and whitespace — MCP commands are a
    // single token. Any of these would indicate an attempt to chain or
    // inject.
    if command.contains(FORBIDDEN_CHARS) {
        return Err(format!(
            "command contains forbidden characters (shell metacharacters or whitespace): {command:?}"
        ));
    }
    // Reject path traversal in case the command is a path.
    if command.contains("..") {
        return Err("command must not contain path traversal (..)".into());
    }
    // Basename of the command for the shell blocklist check.
    let basename = command.rsplit('/').next().unwrap_or(command);
    let basename_lower = basename.to_ascii_lowercase();
    if BLOCKED_MCP_SHELLS.iter().any(|s| *s == basename_lower) {
        return Err(format!(
            "refusing to spawn shell interpreter '{basename}' as an MCP server \
             (would allow arbitrary command execution)"
        ));
    }
    Ok(())
}

/// Environment-variable prefixes that must never be inherited by an MCP
/// server child process — they allow a malicious server config to load
/// arbitrary shared libraries or modules into the child.
const BLOCKED_ENV_PREFIXES: &[&str] = &[
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
    "DYLD_",
    "PYTHONPATH",
    "PYTHONHOME",
    "SHLIB_PATH",
    "LIBPATH",
    "PERL5OPT",
    "PERLLIB",
    "NODE_OPTIONS",
    "ELECTRON_RUN_AS_NODE",
    "NODE_PATH",
];

/// Return a copy of `env` with security-sensitive variables removed.
///
/// Strips dynamic-loader paths (`LD_PRELOAD`, `LD_LIBRARY_PATH`,
/// `DYLD_*`), interpreter paths (`PYTHONPATH`, `PERLLIB`, `NODE_PATH`),
/// and a few interpreter option vectors (`NODE_OPTIONS`, `PERL5OPT`)
/// that would let a malicious MCP config inject code into the child.
///
/// Used at spawn so no caller can smuggle these in via the server's
/// `env` map.
pub fn sanitize_env(env: &HashMap<String, String>) -> HashMap<String, String> {
    env.iter()
        .filter(|(k, _)| {
            let key = k.as_str();
            !BLOCKED_ENV_PREFIXES.iter().any(|prefix| key.starts_with(prefix))
        })
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_safe_commands() {
        assert!(validate_mcp_command("npx").is_ok());
        assert!(validate_mcp_command("uvx").is_ok());
        assert!(validate_mcp_command("/usr/local/bin/mcp-server").is_ok());
    }

    #[test]
    fn rejects_shells() {
        for shell in ["sh", "bash", "zsh", "python", "python3", "node", "env"] {
            assert!(
                validate_mcp_command(shell).is_err(),
                "{shell} should be blocked"
            );
            // Path-qualified shell is also blocked (basename match).
            assert!(
                validate_mcp_command(&format!("/usr/bin/{shell}")).is_err(),
                "/usr/bin/{shell} should be blocked"
            );
        }
    }

    #[test]
    fn rejects_metacharacters_and_traversal() {
        assert!(validate_mcp_command("npx; rm -rf /").is_err());
        assert!(validate_mcp_command("npx && evil").is_err());
        assert!(validate_mcp_command("npx $(cat /etc/passwd)").is_err());
        assert!(validate_mcp_command("../escape").is_err());
        assert!(validate_mcp_command("").is_err());
    }

    #[test]
    fn sanitize_strips_loader_and_interp_paths() {
        let mut env = HashMap::new();
        env.insert("LD_PRELOAD".into(), "/tmp/x.so".into());
        env.insert("DYLD_INSERT_LIBRARIES".into(), "/tmp/y.dylib".into());
        env.insert("PYTHONPATH".into(), "/tmp".into());
        env.insert("NODE_OPTIONS".into(), "--require /tmp/z".into());
        env.insert("PATH".into(), "/usr/bin".into());
        env.insert("HOME".into(), "/root".into());

        let clean = sanitize_env(&env);
        assert!(!clean.contains_key("LD_PRELOAD"));
        assert!(!clean.contains_key("DYLD_INSERT_LIBRARIES"));
        assert!(!clean.contains_key("PYTHONPATH"));
        assert!(!clean.contains_key("NODE_OPTIONS"));
        // Benign vars survive.
        assert_eq!(clean.get("PATH").map(String::as_str), Some("/usr/bin"));
        assert_eq!(clean.get("HOME").map(String::as_str), Some("/root"));
    }
}
