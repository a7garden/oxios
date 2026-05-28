//! Lateral thinking engine for stuck-state recovery.
//!
//! When the Ouroboros evolve loop detects stagnation (consecutive generations
//! with no meaningful change), this module selects an appropriate lateral
//! thinking persona to break the deadlock.
//!
//! Inspired by Ouroboros (Q00)'s affinity-matrix persona selection.

/// Stagnation patterns that indicate different types of stuckness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StagnationPattern {
    /// Agent is repeating the same action without progress.
    Spinning,
    /// Score is not changing at all (flatline).
    NoDrift,
    /// Each iteration makes smaller and smaller improvements.
    DiminishingReturns,
    /// Agent oscillates between two failing approaches.
    Oscillation,
}

/// Lateral thinking persona with philosophy, approach, and affinity.
#[derive(Debug, Clone)]
pub struct LateralPersona {
    /// Persona name.
    pub name: &'static str,
    /// Core philosophy (one-liner).
    pub philosophy: &'static str,
    /// Approach steps for breaking the deadlock.
    pub approach: &'static [&'static str],
    /// Questions to consider.
    pub questions: &'static [&'static str],
    /// Stagnation patterns this persona has affinity for.
    pub affinities: &'static [StagnationPattern],
}

/// Built-in lateral thinking personas.
pub static PERSONAS: &[LateralPersona] = &[
    LateralPersona {
        name: "Contrarian",
        philosophy: "What everyone assumes is true, you examine.",
        approach: &[
            "List the assumptions being made",
            "Consider the opposite of each assumption",
            "Challenge the problem statement itself",
            "What if the goal is wrong?",
        ],
        questions: &[
            "What is everyone assuming that might be false?",
            "If you inverted the goal, what would that look like?",
            "What would a critic say about this approach?",
        ],
        affinities: &[StagnationPattern::Spinning, StagnationPattern::Oscillation, StagnationPattern::DiminishingReturns, StagnationPattern::NoDrift],
    },
    LateralPersona {
        name: "Hacker",
        philosophy: "Rules are obstacles to route around.",
        approach: &[
            "Identify the constraints blocking progress",
            "Question whether each constraint is real or assumed",
            "Find the edge case that bypasses the constraint",
            "Can you achieve the goal without following the normal path?",
        ],
        questions: &[
            "What constraint is causing the most pain?",
            "Is this constraint real or self-imposed?",
            "Can you achieve a similar result with a completely different approach?",
        ],
        affinities: &[StagnationPattern::Spinning],
    },
    LateralPersona {
        name: "Simplifier",
        philosophy: "Complexity is the enemy. Cut until it works.",
        approach: &[
            "List every component in the current approach",
            "Challenge whether each component is necessary",
            "Find the minimum that could possibly work",
            "Implement the simplest thing first",
        ],
        questions: &[
            "What can you remove without breaking the goal?",
            "Is there a simpler way to express the same thing?",
            "What would YAGNI (You Ain't Gonna Need It) cut?",
        ],
        affinities: &[StagnationPattern::DiminishingReturns, StagnationPattern::Oscillation],
    },
    LateralPersona {
        name: "Researcher",
        philosophy: "Most problems exist because we're missing information.",
        approach: &[
            "Define what you don't know",
            "Gather evidence from the codebase or docs",
            "Look for patterns in the failure history",
            "Form a new hypothesis based on evidence",
        ],
        questions: &[
            "What information are you missing?",
            "What does the error/failure pattern tell you?",
            "Has anyone solved a similar problem before?",
        ],
        affinities: &[StagnationPattern::NoDrift, StagnationPattern::DiminishingReturns],
    },
    LateralPersona {
        name: "Architect",
        philosophy: "If you're fighting the architecture, the architecture is wrong.",
        approach: &[
            "Identify structural symptoms (repeated patterns, workarounds)",
            "Map the current structure",
            "Find the misalignment between structure and goal",
            "Propose a structural change that makes the goal natural",
        ],
        questions: &[
            "Is the current structure fighting the goal?",
            "What structural change would make this trivial?",
            "Are you working against the grain of the system?",
        ],
        affinities: &[StagnationPattern::Oscillation, StagnationPattern::NoDrift],
    },
];

/// Select the best persona for a given stagnation pattern.
///
/// Excludes already-tried personas to ensure diversity across retries.
pub fn select_persona(
    pattern: StagnationPattern,
    tried: &[&str],
) -> Option<&'static LateralPersona> {
    PERSONAS
        .iter()
        .filter(|p| p.affinities.contains(&pattern))
        .find(|p| !tried.contains(&p.name))
}

/// Build a lateral thinking prompt from a persona and problem context.
pub fn build_lateral_prompt(
    persona: &LateralPersona,
    goal: &str,
    current_approach: &str,
    failed_attempts: &[String],
) -> String {
    let mut parts = vec![
        format!("## Persona: {}", persona.name),
        format!("_\"{}\"_", persona.philosophy),
        String::new(),
        "## Problem Context".to_string(),
        goal.to_string(),
        String::new(),
        "## Current Approach (Not Working)".to_string(),
        current_approach.to_string(),
    ];

    if !failed_attempts.is_empty() {
        parts.push(String::new());
        parts.push("## Previous Failed Attempts".to_string());
        for attempt in failed_attempts {
            parts.push(format!("- {}", attempt));
        }
    }

    parts.push(String::new());
    parts.push("## Lateral Thinking Instructions".to_string());
    for step in persona.approach {
        parts.push(format!("- {}", step));
    }

    parts.push(String::new());
    parts.push("## Questions to Consider".to_string());
    for q in persona.questions {
        parts.push(format!("- {}", q));
    }

    parts.push(String::new());
    parts.push("## Your Alternative Approach".to_string());
    parts.push("Propose a fundamentally different approach that addresses the root cause.".to_string());

    parts.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_persona_by_affinity() {
        let p = select_persona(StagnationPattern::Spinning, &[]);
        assert!(p.is_some());
        assert_eq!(p.unwrap().name, "Contrarian");
    }

    #[test]
    fn test_select_persona_excludes_tried() {
        let p = select_persona(StagnationPattern::Spinning, &["Contrarian"]);
        assert!(p.is_some());
        assert_eq!(p.unwrap().name, "Hacker");
    }

    #[test]
    fn test_select_persona_returns_none_when_all_exhausted() {
        let tried: &[&str] = &["Contrarian", "Hacker"];
        let p = select_persona(StagnationPattern::Spinning, tried);
        assert!(p.is_none());
    }

    #[test]
    fn test_build_lateral_prompt_contains_persona() {
        let persona = &PERSONAS[0]; // Contrarian
        let prompt = build_lateral_prompt(
            persona,
            "Fix the auth bug",
            "Tried adding null checks",
            &[],
        );
        assert!(prompt.contains("Contrarian"));
        assert!(prompt.contains("auth bug"));
        assert!(prompt.contains("null checks"));
    }

    #[test]
    fn test_build_lateral_prompt_includes_failed_attempts() {
        let persona = &PERSONAS[0];
        let prompt = build_lateral_prompt(
            persona,
            "Goal",
            "Approach",
            &["Attempt 1".to_string(), "Attempt 2".to_string()],
        );
        assert!(prompt.contains("Attempt 1"));
        assert!(prompt.contains("Attempt 2"));
    }
}
