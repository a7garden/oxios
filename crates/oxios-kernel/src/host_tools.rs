//! Host tool validation.
//!
//! Implements the "minimal container, host dependency" philosophy.
//! The container ships only essential tools; additional capabilities
//! must be provided by the host.
//!
//! # Philosophy
//!
//! Unix philosophy says: "Do one thing well."
//! In the context of Oxios, the container does the minimal work
//! (hosting the LLM, managing agent state), while the HOST provides
//! the rich environment (git, shell tools, system integrations).
//!
//! # Tool Categories
//!
//! - **Required tools**: Must be on the host (checked at startup)
//! - **Optional tools**: Checked when programs need them
//! - **Container tools**: Pre-installed in the minimal container

use std::collections::HashMap;
use std::process::Command;

/// Validates that required host tools are available.
///
/// Implements the "minimal container, host dependency" philosophy.
/// The container ships only essential tools; additional capabilities
/// must be provided by the host system.
pub struct HostToolValidator {
    /// Required tools that MUST be on the host
    required: Vec<String>,
    /// Optional tools that MAY be on the host
    optional: Vec<String>,
}

impl HostToolValidator {
    /// Create a new validator with the specified tool requirements
    pub fn new(required: Vec<String>, optional: Vec<String>) -> Self {
        Self { required, optional }
    }

    /// Check if all required tools are available on the host
    ///
    /// Returns a list of missing required tools. Empty list means all good.
    pub fn validate_required(&self) -> Vec<String> {
        self.required
            .iter()
            .filter(|tool| !Self::is_tool_available(tool))
            .cloned()
            .collect()
    }

    /// Check which optional tools are available
    ///
    /// Returns a map of tool name → availability status.
    pub fn check_optional(&self) -> HashMap<String, bool> {
        self.optional
            .iter()
            .map(|tool| (tool.clone(), Self::is_tool_available(tool)))
            .collect()
    }

    /// Check all required and optional tools at once
    ///
    /// Returns a comprehensive status report.
    pub fn full_check(&self) -> HostToolStatus {
        let missing_required = self.validate_required();
        let optional_available = self.check_optional();

        HostToolStatus {
            all_required_present: missing_required.is_empty(),
            missing_required,
            optional_available,
        }
    }

    /// Check if a specific tool is available on the host
    pub fn is_tool_available(tool: &str) -> bool {
        Self::check_command(tool, &["--version"])
            || Self::check_command(tool, &["-v"])
            || Self::check_command(tool, &["version"])
    }

    /// Check if a command exists and returns successfully
    fn check_command(cmd: &str, args: &[&str]) -> bool {
        Command::new(cmd)
            .args(args)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    /// Get the list of required tools
    pub fn required_tools(&self) -> &[String] {
        &self.required
    }

    /// Get the list of optional tools
    pub fn optional_tools(&self) -> &[String] {
        &self.optional
    }
}

/// Result of a full host tool status check
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HostToolStatus {
    /// Whether all required tools are present
    pub all_required_present: bool,
    /// List of missing required tools
    pub missing_required: Vec<String>,
    /// Map of optional tool → availability
    pub optional_available: HashMap<String, bool>,
}

/// Common host tools that Oxios uses
pub mod common {
    /// Required tools that should be on every host
    pub const REQUIRED: &[&str] = &["git"];

    /// Optional tools that enhance functionality
    pub const OPTIONAL: &[&str] = &[
        "gh",           // GitHub CLI
        "remindctl",    // Reminders CLI
        "shortcuts",    // macOS Shortcuts
        "osascript",    // AppleScript execution
        "open",         // Open files/URLs
        "jq",           // JSON processing
        "curl",         // HTTP client
        "ripgrep",      // Better grep
        "sqlite3",      // SQLite CLI
    ];

    /// Tools pre-installed in the minimal container
    pub const CONTAINER_MINIMAL: &[&str] = &[
        "bash",
        "python3",
        "git",
        "curl",
        "jq",
        "ripgrep",
        "sqlite3",
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- HostToolValidator tests ---

    #[test]
    fn test_validate_required_all_present() {
        // Use tools that should exist on most systems.
        let validator = HostToolValidator::new(vec!["echo".to_string()], Vec::new());

        let missing = validator.validate_required();
        assert!(missing.is_empty());
    }

    #[test]
    fn test_validate_required_missing() {
        let validator = HostToolValidator::new(
            vec!["definitely-not-a-real-tool-12345".to_string()],
            Vec::new(),
        );

        let missing = validator.validate_required();
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0], "definitely-not-a-real-tool-12345");
    }

    #[test]
    fn test_validate_required_multiple_missing() {
        let validator = HostToolValidator::new(
            vec![
                "not-real-1".to_string(),
                "not-real-2".to_string(),
            ],
            Vec::new(),
        );

        let missing = validator.validate_required();
        assert_eq!(missing.len(), 2);
    }

    #[test]
    fn test_check_optional() {
        let validator = HostToolValidator::new(
            Vec::new(),
            vec!["echo".to_string(), "definitely-not-real".to_string()],
        );

        let results = validator.check_optional();
        assert_eq!(results.len(), 2);
        assert!(results["echo"]);
        assert!(!results["definitely-not-real"]);
    }

    #[test]
    fn test_is_tool_available() {
        // These should exist on most Unix-like systems.
        assert!(HostToolValidator::is_tool_available("echo"));
        assert!(HostToolValidator::is_tool_available("ls"));
        assert!(HostToolValidator::is_tool_available("cat"));
    }

    #[test]
    fn test_is_tool_available_not_found() {
        assert!(!HostToolValidator::is_tool_available(
            "this-tool-definitely-does-not-exist-abc123"
        ));
    }

    #[test]
    fn test_full_check() {
        let validator = HostToolValidator::new(
            vec!["echo".to_string()],
            vec!["cat".to_string()],
        );

        let status = validator.full_check();
        assert!(status.all_required_present);
        assert!(status.missing_required.is_empty());
        assert!(status.optional_available["cat"]);
    }

    #[test]
    fn test_full_check_missing_required() {
        let validator = HostToolValidator::new(
            vec![
                "echo".to_string(),
                "not-real-xyz".to_string(),
            ],
            Vec::new(),
        );

        let status = validator.full_check();
        assert!(!status.all_required_present);
        assert_eq!(status.missing_required.len(), 1);
    }

    #[test]
    fn test_required_tools_accessors() {
        let validator = HostToolValidator::new(
            vec!["git".to_string(), "gh".to_string()],
            vec!["jq".to_string()],
        );

        assert_eq!(validator.required_tools(), &["git", "gh"]);
        assert_eq!(validator.optional_tools(), &["jq"]);
    }

    // --- common module constants ---

    #[test]
    fn test_common_tools_constants() {
        assert!(!common::REQUIRED.is_empty());
        assert!(common::REQUIRED.contains(&"git"));

        assert!(!common::OPTIONAL.is_empty());
        assert!(common::OPTIONAL.contains(&"gh"));
        assert!(common::OPTIONAL.contains(&"jq"));
        assert!(common::OPTIONAL.contains(&"curl"));

        assert!(!common::CONTAINER_MINIMAL.is_empty());
        assert!(common::CONTAINER_MINIMAL.contains(&"bash"));
        assert!(common::CONTAINER_MINIMAL.contains(&"git"));
    }

    // --- HostToolStatus ---

    #[test]
    fn test_host_tool_status_serialization() {
        let status = HostToolStatus {
            all_required_present: true,
            missing_required: vec!["git".to_string()],
            optional_available: HashMap::from([
                ("jq".to_string(), true),
                ("curl".to_string(), false),
            ]),
        };

        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("all_required_present"));
        assert!(json.contains("missing_required"));
        assert!(json.contains("optional_available"));

        // Deserialize back.
        let loaded: HostToolStatus = serde_json::from_str(&json).unwrap();
        assert!(loaded.all_required_present);
        assert_eq!(loaded.missing_required.len(), 1);
    }
}