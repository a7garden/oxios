//! Capability templates — preset CSpace configurations for common agent roles.
//!
//! Templates provide a declarative way to define an agent's initial
//! capability set. They encode the "principle of least privilege" by
//! starting from minimal access and layering on rights as the role
//! demands.
//!
//! # Hierarchy
//!
//! ```text
//! worker()     → Exec + Browser
//!   standard() → worker + Memory(READ)
//!   operator() → standard + Space + Agent + A2a + Program + MCP + Memory(WRITE)
//!   supervisor() → operator + Security + Budget + Resource + Cron
//! ```
//!
//! # Example
//!
//! ```
//! use oxios_kernel::capability::template::CapabilityTemplate;
//! use oxios_kernel::types::AgentId;
//!
//! let cspace = CapabilityTemplate::standard().build_for(AgentId::new_v4());
//! assert!(cspace.len() > 0);
//! ```

use uuid::Uuid;
use crate::types::AgentId;

use super::types::{CSpace, Capability, CapabilityId, Issuer, ResourceRef, Rights};

/// Builder for constructing preset capability spaces.
///
/// Use the associated constructors (`worker`, `standard`, `operator`,
/// `supervisor`, `with_skills`) to start from a template, then call
/// [`build`] or [`build_for`] to produce a [`CSpace`].
#[derive(Debug, Clone)]
pub struct CapabilityTemplate {
    caps: Vec<(ResourceRef, Rights)>,
}

impl CapabilityTemplate {
    // ── Preset constructors ─────────────────────────────────────────

    /// **Worker** — minimal execution capability.
    ///
    /// Rights: shell exec + headless browser.
    pub fn worker() -> Self {
        let mut t = Self { caps: Vec::new() };
        t.caps.push((
            ResourceRef::Exec {
                mode: "shell".into(),
            },
            Rights::EXECUTE | Rights::READ,
        ));
        t.caps
            .push((ResourceRef::Browser, Rights::READ | Rights::EXECUTE));
        t
    }

    /// **Standard** — worker + memory read access.
    ///
    /// Suitable for most agents that need to recall but not modify
    /// persistent state.
    pub fn standard() -> Self {
        let mut t = Self::worker();
        t.caps.push((
            ResourceRef::KernelDomain {
                domain: "memory".into(),
            },
            Rights::READ,
        ));
        t
    }

    /// **Operator** — standard + space, agent, A2A, persona, program,
    /// MCP, and memory write.
    ///
    /// Intended for agents that coordinate work across multiple
    /// subsystems (e.g., a project lead agent).
    pub fn operator() -> Self {
        let mut t = Self::standard();
        let extra = vec![
            (
                ResourceRef::Space { id: Uuid::nil() },
                Rights::READ | Rights::WRITE,
            ),
            (
                ResourceRef::Agent { id: AgentId::nil() },
                Rights::READ | Rights::WRITE,
            ),
            (
                ResourceRef::A2a,
                Rights::READ | Rights::WRITE | Rights::EXECUTE,
            ),
            (
                ResourceRef::KernelDomain {
                    domain: "persona".into(),
                },
                Rights::READ | Rights::WRITE,
            ),
            (
                ResourceRef::KernelDomain {
                    domain: "program".into(),
                },
                Rights::READ | Rights::WRITE | Rights::EXECUTE,
            ),
            (
                ResourceRef::Mcp { server: "*".into() },
                Rights::READ | Rights::EXECUTE,
            ),
            // Upgrade memory to RW
            (
                ResourceRef::KernelDomain {
                    domain: "memory".into(),
                },
                Rights::READ | Rights::WRITE,
            ),
        ];
        t.caps.extend(extra);
        t
    }

    /// **Supervisor** — operator + security, budget, resource, and cron
    /// kernel domains.
    ///
    /// The most privileged built-in template. Use sparingly.
    pub fn supervisor() -> Self {
        let mut t = Self::operator();
        let admin = vec![
            (
                ResourceRef::KernelDomain {
                    domain: "security".into(),
                },
                Rights::ALL,
            ),
            (
                ResourceRef::KernelDomain {
                    domain: "budget".into(),
                },
                Rights::READ | Rights::WRITE,
            ),
            (
                ResourceRef::KernelDomain {
                    domain: "resource".into(),
                },
                Rights::READ | Rights::WRITE,
            ),
            (
                ResourceRef::KernelDomain {
                    domain: "cron".into(),
                },
                Rights::READ | Rights::WRITE | Rights::EXECUTE,
            ),
        ];
        t.caps.extend(admin);
        t
    }

    /// **With skills** — worker + specific named skills.
    ///
    /// Creates a worker-level agent with EXECUTE rights on the listed
    /// skills only. This is the recommended template for agents that
    /// should have access to a known set of tools.
    pub fn with_skills(names: &[&str]) -> Self {
        let mut t = Self::worker();
        for name in names {
            t.caps.push((
                ResourceRef::Skill {
                    name: (*name).into(),
                },
                Rights::EXECUTE | Rights::READ,
            ));
        }
        t
    }

    // ── Builder methods ─────────────────────────────────────────────

    /// Add an additional capability to the template.
    pub fn with(mut self, resource: ResourceRef, rights: Rights) -> Self {
        self.caps.push((resource, rights));
        self
    }

    /// Build a CSpace with kernel-issued capabilities for a fresh agent ID.
    pub fn build(&self) -> CSpace {
        self.build_for(AgentId::new_v4())
    }

    /// Build a CSpace with kernel-issued capabilities for a specific agent.
    pub fn build_for(&self, agent_id: AgentId) -> CSpace {
        let mut cspace = CSpace::new(agent_id);
        for (resource, rights) in &self.caps {
            let cap = Capability {
                id: CapabilityId::new(),
                resource: resource.clone(),
                rights: *rights,
                issuer: Issuer::Kernel,
            };
            cspace.insert(cap);
        }
        cspace
    }

    /// Returns the number of capabilities in this template.
    pub fn len(&self) -> usize {
        self.caps.len()
    }

    /// Returns true if the template has no capabilities.
    pub fn is_empty(&self) -> bool {
        self.caps.is_empty()
    }
}

impl Default for CapabilityTemplate {
    fn default() -> Self {
        Self::worker()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_has_exec_and_browser() {
        let cs = CapabilityTemplate::worker().build();
        assert!(cs.can(
            &ResourceRef::Exec {
                mode: "shell".into()
            },
            Rights::EXECUTE
        ));
        assert!(cs.can(&ResourceRef::Browser, Rights::READ));
        assert_eq!(cs.len(), 2);
    }

    #[test]
    fn standard_adds_memory_read() {
        let cs = CapabilityTemplate::standard().build();
        assert!(cs.can(
            &ResourceRef::KernelDomain {
                domain: "memory".into()
            },
            Rights::READ
        ));
        assert!(!cs.can(
            &ResourceRef::KernelDomain {
                domain: "memory".into()
            },
            Rights::WRITE
        ));
    }

    #[test]
    fn operator_has_a2a_and_mcp() {
        let cs = CapabilityTemplate::operator().build();
        assert!(cs.can(&ResourceRef::A2a, Rights::EXECUTE));
        assert!(cs.can(&ResourceRef::Mcp { server: "*".into() }, Rights::EXECUTE));
    }

    #[test]
    fn supervisor_has_security_all() {
        let cs = CapabilityTemplate::supervisor().build();
        assert!(cs.can(
            &ResourceRef::KernelDomain {
                domain: "security".into()
            },
            Rights::ALL
        ));
    }

    #[test]
    fn with_skills_scoped() {
        let cs = CapabilityTemplate::with_skills(&["git", "gh"]).build();
        assert!(cs.can(
            &ResourceRef::Skill { name: "git".into() },
            Rights::EXECUTE
        ));
        assert!(cs.can(&ResourceRef::Skill { name: "gh".into() }, Rights::EXECUTE));
        assert!(!cs.can(
            &ResourceRef::Skill {
                name: "curl".into()
            },
            Rights::EXECUTE
        ));
    }

    #[test]
    fn builder_chaining() {
        let cs = CapabilityTemplate::worker()
            .with(
                ResourceRef::KernelDomain {
                    domain: "custom".into(),
                },
                Rights::READ,
            )
            .build();
        assert!(cs.can(
            &ResourceRef::KernelDomain {
                domain: "custom".into()
            },
            Rights::READ
        ));
    }

    #[test]
    fn build_for_specific_agent() {
        let id = AgentId::new_v4();
        let cs = CapabilityTemplate::worker().build_for(id);
        assert_eq!(cs.agent_id, id);
    }
}
