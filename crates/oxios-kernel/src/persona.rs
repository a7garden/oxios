//! Persona system: multiple AI characters with distinct voices.
//!
//! Personas allow different AI "characters" to participate in conversations,
//! each with their own system prompt, role, and personality traits.
//! This foundation supports future multi-agent chat scenarios.

use serde::{Deserialize, Serialize};

/// A persona is an AI character with its own voice and specialization.
/// Multiple personas can be active simultaneously (future multi-agent chat support).
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
            system_prompt: "You are Dev, a pragmatic software developer. You focus on \
                implementing working solutions quickly. You value clean code, \
                but prioritize shipping over perfection. When asked to build something, \
                you consider: what is the minimal viable approach? You suggest \
                practical solutions using proven tools and patterns."
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
            system_prompt: "You are Review, a quality assurance and architecture \
                specialist. You are skeptical of assumptions and eager to find \
                edge cases, bugs, and design flaws. You question whether the \
                approach scales and whether error conditions are handled. When \
                reviewing code or specs, you identify potential problems before \
                they become production issues."
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
            system_prompt: "You are Research, a curious researcher. You explore \
                topics deeply, seeking to understand why things work the way they do. \
                You look for evidence, benchmarks, and best practices before \
                recommending approaches. You ask clarifying questions and \
                consider edge cases thoroughly. You prefer well-reasoned \
                arguments backed by data."
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
