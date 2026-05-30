//! Task definition types and TOML parsing.
//!
//! Tasks are defined in TOML files under `suites/` and loaded at runtime.
//! No recompilation needed to add or modify tasks.

use crate::{AssertionResult, Phase, RunOutput, Tier};
use serde::Deserialize;
use std::path::PathBuf;

/// A loaded, ready-to-run benchmark task.
#[derive(Debug, Clone)]
pub struct Task {
    /// Unique task identifier (e.g. "ouroboros_simple").
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Execution tier.
    pub tier: Tier,
    /// Suite grouping (e.g. "ouroboros", "agent").
    pub suite: String,
    /// Tags for filtering (e.g. "smoke", "core").
    pub tags: Vec<String>,
    /// Timeout in seconds.
    pub timeout_secs: u64,
    /// Single-turn prompt (mutually exclusive with `turns`).
    pub prompt: Option<String>,
    /// Multi-turn conversation.
    pub turns: Vec<Turn>,
    /// Files to create before running.
    pub fixtures: Vec<Fixture>,
    /// Context file to pass via --context-file.
    pub context_file: Option<PathBuf>,
    /// Assertions to evaluate against the output.
    pub assertions: Vec<Assertion>,
}

impl Task {
    /// Whether this is a multi-turn task.
    pub fn is_multi_turn(&self) -> bool {
        !self.turns.is_empty()
    }

    /// Get all messages as a single list (for single-turn, wraps in one element).
    pub fn messages(&self) -> Vec<&str> {
        if let Some(ref p) = self.prompt {
            vec![p.as_str()]
        } else {
            self.turns.iter().map(|t| t.message.as_str()).collect()
        }
    }
}

/// A single turn in a multi-turn conversation.
#[derive(Debug, Clone)]
pub struct Turn {
    /// The user message.
    pub message: String,
    /// Per-turn assertions (in addition to task-level assertions).
    pub assertions: Vec<Assertion>,
}

/// A fixture file to create before running a task.
#[derive(Debug, Clone)]
pub struct Fixture {
    /// Relative path within the workspace.
    pub path: PathBuf,
    /// File content.
    pub content: String,
}

/// An assertion to evaluate against task output.
#[derive(Debug, Clone)]
pub enum Assertion {
    // ── Structural (from oxios run --json) ──
    /// Phase must be at least this value.
    PhaseReached { min: Phase },
    /// Ouroboros evaluation result must match.
    EvaluationPassed { expected: bool },
    /// A seed must have been created.
    RequireSeedId,
    /// An agent must have executed.
    RequireAgentId,
    /// A session must have been established.
    RequireSessionId,
    /// Duration must be under this threshold in ms.
    MaxDuration { ms: u64 },

    // ── Content (response text) ──
    /// Response must contain this text.
    Contains {
        text: String,
        case_sensitive: bool,
    },
    /// Response must NOT contain this text.
    NotContains { text: String },
    /// Response must match this regex.
    Regex { pattern: String },

    // ── Exit code ──
    /// Process exit code must match.
    ExitCode { expected: i32 },
}

impl Assertion {
    /// Weight for scoring (higher = more important).
    pub fn weight(&self) -> f64 {
        match self {
            // Structural assertions are most important
            Assertion::PhaseReached { .. } => 2.0,
            Assertion::EvaluationPassed { .. } => 2.0,
            Assertion::RequireSeedId => 1.5,
            Assertion::RequireAgentId => 1.5,
            Assertion::RequireSessionId => 1.0,
            Assertion::MaxDuration { .. } => 1.0,
            // Content assertions
            Assertion::Contains { .. } => 1.0,
            Assertion::NotContains { .. } => 1.0,
            Assertion::Regex { .. } => 1.0,
            // Exit code
            Assertion::ExitCode { .. } => 2.0,
        }
    }

    /// Human-readable description.
    pub fn describe(&self) -> String {
        match self {
            Assertion::PhaseReached { min } => format!("phase_reached >= {}", min),
            Assertion::EvaluationPassed { expected } => format!("evaluation_passed = {}", expected),
            Assertion::RequireSeedId => "seed_id exists".to_string(),
            Assertion::RequireAgentId => "agent_id exists".to_string(),
            Assertion::RequireSessionId => "session_id exists".to_string(),
            Assertion::MaxDuration { ms } => format!("duration < {}ms", ms),
            Assertion::Contains { text, .. } => format!("response contains {:?}", text),
            Assertion::NotContains { text } => format!("response not contains {:?}", text),
            Assertion::Regex { pattern } => format!("response matches /{}/", pattern),
            Assertion::ExitCode { expected } => format!("exit_code = {}", expected),
        }
    }

    /// Evaluate this assertion against a run output.
    pub fn evaluate(&self, output: &RunOutput) -> AssertionResult {
        match self {
            Assertion::PhaseReached { min } => {
                let actual_phase = output.phase();
                let passed = actual_phase.is_some_and(|p| p >= *min);
                AssertionResult {
                    assertion: self.describe(),
                    passed,
                    actual: output.phase_reached.clone(),
                    expected: min.to_string(),
                }
            }
            Assertion::EvaluationPassed { expected } => AssertionResult {
                assertion: self.describe(),
                passed: output.evaluation_passed == *expected,
                actual: output.evaluation_passed.to_string(),
                expected: expected.to_string(),
            },
            Assertion::RequireSeedId => AssertionResult {
                assertion: self.describe(),
                passed: output.seed_id.is_some(),
                actual: output
                    .seed_id
                    .clone()
                    .unwrap_or_else(|| "None".to_string()),
                expected: "Some(...)".to_string(),
            },
            Assertion::RequireAgentId => AssertionResult {
                assertion: self.describe(),
                passed: output.agent_id.is_some(),
                actual: output
                    .agent_id
                    .clone()
                    .unwrap_or_else(|| "None".to_string()),
                expected: "Some(...)".to_string(),
            },
            Assertion::RequireSessionId => AssertionResult {
                assertion: self.describe(),
                passed: output.session_id.is_some(),
                actual: output
                    .session_id
                    .clone()
                    .unwrap_or_else(|| "None".to_string()),
                expected: "Some(...)".to_string(),
            },
            Assertion::MaxDuration { ms } => AssertionResult {
                assertion: self.describe(),
                passed: output.duration_ms <= *ms,
                actual: format!("{}ms", output.duration_ms),
                expected: format!("<={}ms", ms),
            },
            Assertion::Contains {
                text,
                case_sensitive,
            } => {
                let passed = if *case_sensitive {
                    output.response.contains(text.as_str())
                } else {
                    output.response.to_lowercase().contains(&text.to_lowercase())
                };
                AssertionResult {
                    assertion: self.describe(),
                    passed,
                    actual: if passed {
                        "found".to_string()
                    } else {
                        "not found".to_string()
                    },
                    expected: format!("contains {:?}", text),
                }
            }
            Assertion::NotContains { text } => {
                let response_lower = output.response.to_lowercase();
                let text_lower = text.to_lowercase();
                let found = response_lower.contains(&text_lower);
                AssertionResult {
                    assertion: self.describe(),
                    passed: !found,
                    actual: if found {
                        "found (unexpected)".to_string()
                    } else {
                        "not found (ok)".to_string()
                    },
                    expected: format!("not contains {:?}", text),
                }
            }
            Assertion::Regex { pattern } => {
                let re = regex::Regex::new(pattern);
                match re {
                    Ok(re) => AssertionResult {
                        assertion: self.describe(),
                        passed: re.is_match(&output.response),
                        actual: if re.is_match(&output.response) {
                            "matched".to_string()
                        } else {
                            "no match".to_string()
                        },
                        expected: format!("matches /{}/", pattern),
                    },
                    Err(e) => AssertionResult {
                        assertion: self.describe(),
                        passed: false,
                        actual: format!("invalid regex: {}", e),
                        expected: format!("matches /{}/", pattern),
                    },
                }
            }
            Assertion::ExitCode { expected } => AssertionResult {
                assertion: self.describe(),
                passed: output.exit_code == *expected,
                actual: output.exit_code.to_string(),
                expected: expected.to_string(),
            },
        }
    }
}

// ── TOML deserialization types ──────────────────────────────────────────

/// Root TOML structure for a task definition file.
#[derive(Debug, Clone, Deserialize)]
pub struct TaskToml {
    pub task: TaskMetaToml,
    #[serde(default)]
    pub turns: Vec<TurnToml>,
    #[serde(default)]
    pub expect: ExpectToml,
    #[serde(default)]
    pub setup: Option<SetupToml>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TaskMetaToml {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub tier: String,
    #[serde(default)]
    pub suite: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    /// The prompt message (single-turn).
    #[serde(default)]
    pub prompt: Option<String>,
    /// Context file to pass via --context-file.
    #[serde(default)]
    pub context_file: Option<String>,
}

fn default_timeout() -> u64 {
    120
}
#[derive(Debug, Clone, Deserialize)]
pub struct TurnToml {
    pub message: String,
    #[serde(default)]
    pub expect: Option<ExpectToml>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ExpectToml {
    #[serde(default)]
    pub phase_reached: Option<String>,
    #[serde(default)]
    pub evaluation_passed: Option<bool>,
    #[serde(default)]
    pub require_seed_id: Option<bool>,
    #[serde(default)]
    pub require_agent_id: Option<bool>,
    #[serde(default)]
    pub require_session_id: Option<bool>,
    #[serde(default)]
    pub response_contains: Vec<String>,
    #[serde(default)]
    pub response_not_contains: Vec<String>,
    #[serde(default)]
    pub response_regex: Vec<String>,
    #[serde(default)]
    pub exit_code: Option<i32>,
    #[serde(default)]
    pub max_duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SetupToml {
    #[serde(default)]
    pub files: Vec<FileFixtureToml>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileFixtureToml {
    pub path: String,
    pub content: String,
}

impl TaskToml {
    /// Parse a TOML string into a task definition.
    pub fn parse(toml_str: &str) -> anyhow::Result<Self> {
        let task: TaskToml = toml::from_str(toml_str)?;
        Ok(task)
    }

    /// Convert into a runtime Task, loading assertions from the expect section.
    pub fn into_task(self, suites_dir: Option<PathBuf>) -> anyhow::Result<Task> {
        let tier = Tier::from_str_opt(&self.task.tier).unwrap_or(Tier::E2e);

        // Build assertions from expect section
        let assertions = self.expect.into_assertions();

        // Resolve context file relative to suites dir
        let context_file = self.task.context_file.and_then(|cf| {
            suites_dir
                .as_ref()
                .map(|dir| dir.join(&cf))
                .or_else(|| Some(PathBuf::from(&cf)))
        });

        // Build turns if present
        let turns = self
            .turns
            .into_iter()
            .map(|t| Turn {
                message: t.message,
                assertions: t.expect.map(|e| e.into_assertions()).unwrap_or_default(),
            })
            .collect();

        // Build fixtures
        let fixtures = self
            .setup.map(|s| s.files)
            .unwrap_or_default()
            .into_iter()
            .map(|f| Fixture {
                path: PathBuf::from(&f.path),
                content: f.content,
            })
            .collect();

        Ok(Task {
            id: self.task.id,
            name: self.task.name,
            tier,
            suite: self.task.suite,
            tags: self.task.tags,
            timeout_secs: self.task.timeout_secs,
            prompt: self.task.prompt,
            turns,
            fixtures,
            context_file,
            assertions,
        })
    }
}

impl ExpectToml {
    /// Convert TOML expect fields into typed Assertions.
    pub fn into_assertions(self) -> Vec<Assertion> {
        let mut assertions = Vec::new();

        if let Some(ref phase_str) = self.phase_reached {
            if let Some(phase) = Phase::from_str_opt(phase_str) {
                assertions.push(Assertion::PhaseReached { min: phase });
            }
        }
        if let Some(ep) = self.evaluation_passed {
            assertions.push(Assertion::EvaluationPassed { expected: ep });
        }
        if self.require_seed_id.unwrap_or(false) {
            assertions.push(Assertion::RequireSeedId);
        }
        if self.require_agent_id.unwrap_or(false) {
            assertions.push(Assertion::RequireAgentId);
        }
        if self.require_session_id.unwrap_or(false) {
            assertions.push(Assertion::RequireSessionId);
        }
        if let Some(ms) = self.max_duration_ms {
            assertions.push(Assertion::MaxDuration { ms });
        }
        for text in self.response_contains {
            assertions.push(Assertion::Contains {
                text,
                case_sensitive: false,
            });
        }
        for text in self.response_not_contains {
            assertions.push(Assertion::NotContains { text });
        }
        for pattern in self.response_regex {
            assertions.push(Assertion::Regex { pattern });
        }
        if let Some(ec) = self.exit_code {
            assertions.push(Assertion::ExitCode { expected: ec });
        }

        assertions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_task() {
        let toml_str = r#"
[task]
id = "test_simple"
name = "Simple test"
tier = "integration"
suite = "test"
tags = ["smoke"]
timeout_secs = 30

prompt = "hello world"

[expect]
phase_reached = "Execute"
evaluation_passed = true
"#;
        let parsed = TaskToml::parse(toml_str).unwrap();
        assert!(parsed.task.prompt.is_some(), "prompt field should parse as Some");
        let task = parsed.into_task(None).unwrap();
        assert_eq!(task.id, "test_simple");
        assert_eq!(task.tier, Tier::Integration);
        // prompt gets mapped into task.prompt
        assert_eq!(task.prompt, Some("hello world".to_string()));
        assert_eq!(task.assertions.len(), 2);
    }

    #[test]
    fn test_parse_multi_turn() {
        let toml = r#"
[task]
id = "multi_turn_test"
name = "Multi-turn test"
tier = "integration"
suite = "test"

[[turns]]
message = "remember X=42"

[[turns]]
message = "what is X?"
[turns.expect]
response_contains = ["42"]
"#;
        let parsed = TaskToml::parse(toml).unwrap();
        let task = parsed.into_task(None).unwrap();
        assert!(task.is_multi_turn());
        assert_eq!(task.turns.len(), 2);
        assert_eq!(task.turns[1].assertions.len(), 1);
    }

    #[test]
    fn test_assertion_phase_reached() {
        let assertion = Assertion::PhaseReached {
            min: Phase::Execute,
        };
        let output = RunOutput {
            response: String::new(),
            session_id: None,
            project_id: None,
            project_tag: None,
            seed_id: None,
            agent_id: None,
            phase_reached: "Execute".to_string(),
            evaluation_passed: true,
            exit_code: 0,
            duration_ms: 100,
            workspace: PathBuf::new(),
        };
        let result = assertion.evaluate(&output);
        assert!(result.passed);
    }

    #[test]
    fn test_assertion_phase_not_reached() {
        let assertion = Assertion::PhaseReached {
            min: Phase::Evaluate,
        };
        let output = RunOutput {
            response: String::new(),
            session_id: None,
            project_id: None,
            project_tag: None,
            seed_id: None,
            agent_id: None,
            phase_reached: "Execute".to_string(),
            evaluation_passed: false,
            exit_code: 1,
            duration_ms: 100,
            workspace: PathBuf::new(),
        };
        let result = assertion.evaluate(&output);
        assert!(!result.passed);
    }

    #[test]
    fn test_assertion_contains() {
        let assertion = Assertion::Contains {
            text: "hello".to_string(),
            case_sensitive: false,
        };
        let output = RunOutput {
            response: "Hello, World!".to_string(),
            session_id: None,
            project_id: None,
            project_tag: None,
            seed_id: None,
            agent_id: None,
            phase_reached: "Execute".to_string(),
            evaluation_passed: true,
            exit_code: 0,
            duration_ms: 100,
            workspace: PathBuf::new(),
        };
        let result = assertion.evaluate(&output);
        assert!(result.passed);
    }

    #[test]
    fn test_assertion_not_contains() {
        let assertion = Assertion::NotContains {
            text: "error".to_string(),
        };
        let output = RunOutput {
            response: "success!".to_string(),
            session_id: None,
            project_id: None,
            project_tag: None,
            seed_id: None,
            agent_id: None,
            phase_reached: "Execute".to_string(),
            evaluation_passed: true,
            exit_code: 0,
            duration_ms: 100,
            workspace: PathBuf::new(),
        };
        let result = assertion.evaluate(&output);
        assert!(result.passed);
    }
}
