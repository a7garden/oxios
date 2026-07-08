//! Persona system: multiple AI characters with distinct voices.
//!
//! Personas allow different AI "characters" to participate in conversations,
//! each with their own system prompt, role, and personality traits.
//! This foundation supports future multi-agent chat scenarios.

pub mod manager;
pub mod store;

pub use manager::PersonaManager;
pub use store::PersonaStore;

use serde::{Deserialize, Serialize};

/// A persona is an AI character with its own voice and specialization.
/// Exactly one persona is active at a time (single slot). RFC-039.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Persona {
    /// Unique identifier.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Role or archetype (developer, qa, architect, researcher...).
    pub role: String,
    /// Brief description of this persona.
    pub description: String,
    /// The persona's character definition (system prompt).
    pub system_prompt: String,
    /// Whether this persona is enabled for use.
    pub enabled: bool,
    /// Optional model override for this persona.
    pub model: Option<String>,
    /// Personality traits (curious, skeptical, creative...).
    pub personality_traits: Vec<String>,
}

impl Default for Persona {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Default".to_string(),
            role: "assistant".to_string(),
            description: "Default AI assistant persona".to_string(),
            system_prompt: "You are a helpful AI assistant.".to_string(),
            enabled: true,
            model: None,
            personality_traits: vec![],
        }
    }
}

impl Persona {
    /// Creates a new persona with the given parameters.
    pub fn new(name: &str, role: &str, description: &str, system_prompt: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.to_string(),
            role: role.to_string(),
            description: description.to_string(),
            system_prompt: system_prompt.to_string(),
            enabled: true,
            model: None,
            personality_traits: vec![],
        }
    }

    /// Creates a persona with the given ID (used when loading from storage).
    pub fn with_id(
        id: &str,
        name: &str,
        role: &str,
        description: &str,
        system_prompt: &str,
    ) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            role: role.to_string(),
            description: description.to_string(),
            system_prompt: system_prompt.to_string(),
            enabled: true,
            model: None,
            personality_traits: vec![],
        }
    }
}

/// Creates the three default personas for Oxios.
pub fn default_personas() -> Vec<Persona> {
    vec![
        Persona {
            id: "dev".to_string(),
            name: "Dev".to_string(),
            role: "developer".to_string(),
            description: "Pragmatic developer focused on implementation".to_string(),
            system_prompt: "You are Dev, a pragmatic software developer. You ship.\n\
                \n## Philosophy\n\
                \"Perfect is the enemy of shipped.\" You value working code over elegant theory.\n\
                When faced with ambiguity, you choose the path that produces running output fastest.\n\
                You can always iterate — but you can't iterate on nothing.\n\
                \n## Approach\n\
                1. Identify the minimum viable change\n\
                2. Implement it with proven tools and patterns\n\
                3. Verify it works before refining\n\
                4. Ship, then measure — don't speculate\n\
                \n## What You Do NOT Do\n\
                - Architect systems when a function would do\n\
                - Debate frameworks when the user asked for a feature\n\
                - Write tests for code that doesn't exist yet\n\
                - Refactor code that works without being asked\n\
                \n## Voice\n\
                Direct, practical, code-first. You show code, you don't describe it.\n\
                When you're uncertain, you say so — you don't hedge."
                .to_string(),
            enabled: true,
            model: None,
            personality_traits: vec![
                "pragmatic".to_string(),
                "action-oriented".to_string(),
                "practical".to_string(),
            ],
        },
        Persona {
            id: "review".to_string(),
            name: "Review".to_string(),
            role: "qa".to_string(),
            description: "Quality-focused reviewer with skepticism for assumptions".to_string(),
            system_prompt: "You are Review, a quality assurance specialist. You find what others miss.\n\
                \n## Philosophy\n\
                \"Assumptions are bugs waiting to happen.\" You are not cynical — you are thorough.\n\
                Every edge case is someone's 3 AM incident. Your job is to make sure it's not yours.\n\
                \n## Approach\n\
                1. Read the code like an adversary — what inputs break it?\n\
                2. Trace every error path — are errors handled or swallowed?\n\
                3. Check boundaries — off-by-one, null, empty, overflow, race\n\
                4. Verify intent — does it do what the author THINKS it does?\n\
                \n## What You Do NOT Do\n\
                - Rubber-stamp code without reading it\n\
                - Suggest rewrites when a targeted fix would do\n\
                - Comment on style when security issues exist\n\
                - Say \"looks good to me\" without evidence\n\
                \n## Voice\n\
                Precise, evidence-based. Every finding has a file:line reference.\n\
                Severity is honest — critical means critical, not \"I want attention.\""
                .to_string(),
            enabled: true,
            model: None,
            personality_traits: vec![
                "skeptical".to_string(),
                "thorough".to_string(),
                "quality-focused".to_string(),
            ],
        },
        Persona {
            id: "research".to_string(),
            name: "Research".to_string(),
            role: "researcher".to_string(),
            description: "Curious researcher focused on understanding and evidence".to_string(),
            system_prompt: "You are Research, an investigative analyst. You go deeper.\n\
                \n## Philosophy\n\
                \"The first answer is rarely the best answer.\" You don't accept surface-level\n\
                explanations. You dig for root causes, benchmarks, and evidence before concluding.\n\
                \n## Approach\n\
                1. Clarify the question — what are we actually trying to learn?\n\
                2. Search broadly — the answer might be in an unexpected place\n\
                3. Compare approaches with evidence, not opinion\n\
                4. Present findings with confidence levels — \"proven\" vs \"likely\" vs \"speculative\"\n\
                \n## What You Do NOT Do\n\
                - Recommend without evidence\n\
                - Confuse popular with correct\n\
                - Skip \"why does this work?\" and jump to \"use this\"\n\
                - Ignore contradictory evidence\n\
                \n## Voice\n\
                Analytical, measured, evidence-first. You cite your sources.\n\
                You distinguish \"I know\" from \"I believe\" from \"I suspect.\""
                .to_string(),
            enabled: true,
            model: None,
            personality_traits: vec![
                "curious".to_string(),
                "analytical".to_string(),
                "evidence-focused".to_string(),
            ],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_persona_default() {
        let p = Persona::default();
        assert!(!p.id.is_empty());
        assert_eq!(p.name, "Default");
        assert_eq!(p.role, "assistant");
        assert!(p.enabled);
        assert!(p.model.is_none());
        assert!(p.personality_traits.is_empty());
    }

    #[test]
    fn test_persona_new() {
        let p = Persona::new("Dev", "developer", "A dev", "You are a dev");
        assert!(!p.id.is_empty());
        assert_eq!(p.name, "Dev");
        assert_eq!(p.role, "developer");
        assert!(p.enabled);
    }

    #[test]
    fn test_persona_with_id() {
        let p = Persona::with_id("dev", "Dev", "developer", "A dev", "You are a dev");
        assert_eq!(p.id, "dev");
        assert_eq!(p.name, "Dev");
    }

    #[test]
    fn test_persona_serialization_roundtrip() {
        let mut p = Persona::new("Test", "tester", "Test persona", "Test prompt");
        p.model = Some("anthropic/claude-sonnet-4".to_string());
        p.personality_traits = vec!["curious".to_string(), "thorough".to_string()];

        let json = serde_json::to_string(&p).unwrap();
        let restored: Persona = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, p.id);
        assert_eq!(restored.name, "Test");
        assert_eq!(restored.model.as_deref(), Some("anthropic/claude-sonnet-4"));
        assert_eq!(restored.personality_traits.len(), 2);
    }

    #[test]
    fn test_default_personas_contains_three() {
        let personas = default_personas();
        assert_eq!(personas.len(), 3);

        let ids: Vec<&str> = personas.iter().map(|p| p.id.as_str()).collect();
        assert!(ids.contains(&"dev"));
        assert!(ids.contains(&"review"));
        assert!(ids.contains(&"research"));

        // All should be enabled
        for p in &personas {
            assert!(p.enabled);
            assert!(!p.system_prompt.is_empty());
            assert!(!p.personality_traits.is_empty());
        }
    }

    #[test]
    fn test_default_personas_have_unique_roles() {
        let personas = default_personas();
        let roles: std::collections::HashSet<&str> =
            personas.iter().map(|p| p.role.as_str()).collect();
        assert_eq!(roles.len(), 3);
    }

    #[test]
    fn test_persona_with_disabled() {
        let mut p = Persona::new("Off", "unused", "Disabled persona", "N/A");
        p.enabled = false;
        assert!(!p.enabled);

        let json = serde_json::to_string(&p).unwrap();
        let restored: Persona = serde_json::from_str(&json).unwrap();
        assert!(!restored.enabled);
    }
}
