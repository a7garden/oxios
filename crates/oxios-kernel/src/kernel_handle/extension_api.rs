//! Extension API — skills, programs, and host tools (RFC-009 Phase 3).
//!
//! During the migration period, `ExtensionApi` holds both a `SkillManager`
//! (the new unified facade) and a `ProgramManager` (legacy, will be removed
//! once all program logic migrates into `SkillManager`).

use crate::host_tools::{HostToolStatus, HostToolValidator};
use crate::program::{HostRequirementsCheck, InstallSource, Program, ProgramManager};
use crate::skill::{RequirementsCheck, Skill, SkillEntry, SkillManager, SkillMeta};
use std::sync::Arc;

/// Extension system calls.
pub struct ExtensionApi {
    /// Unified skill manager (primary API going forward).
    pub(crate) skill_manager: Arc<SkillManager>,
    /// Legacy program manager (kept during migration for agent_runtime tool registration).
    pub(crate) program_manager: Arc<ProgramManager>,
    pub(crate) host_tool_validator: Arc<HostToolValidator>,
}

impl ExtensionApi {
    /// Create a new ExtensionApi.
    pub fn new(
        skill_manager: Arc<SkillManager>,
        program_manager: Arc<ProgramManager>,
        host_tool_validator: Arc<HostToolValidator>,
    ) -> Self {
        Self {
            skill_manager,
            program_manager,
            host_tool_validator,
        }
    }

    // ── Legacy program methods (delegate to ProgramManager) ────────

    /// List installed programs.
    pub async fn list_programs(&self) -> Vec<Program> {
        self.program_manager.list_programs().await
    }

    /// Get program details.
    pub async fn get_program(&self, name: &str) -> Option<Program> {
        self.program_manager.get_program(name).await
    }

    /// Install a program from source.
    pub async fn install_program(&self, source: InstallSource) -> anyhow::Result<Program> {
        self.program_manager.install_from(source).await
    }

    /// Uninstall a program.
    pub async fn uninstall_program(&self, name: &str) -> anyhow::Result<()> {
        self.program_manager.uninstall(name).await
    }

    /// Enable a program.
    pub async fn enable_program(&self, name: &str) -> anyhow::Result<()> {
        self.program_manager.set_enabled(name, true).await
    }

    /// Disable a program.
    pub async fn disable_program(&self, name: &str) -> anyhow::Result<()> {
        self.program_manager.set_enabled(name, false).await
    }

    /// Check host requirements for a program.
    pub async fn check_host_requirements(
        &self,
        name: &str,
    ) -> anyhow::Result<HostRequirementsCheck> {
        self.program_manager.check_host_requirements(name).await
    }

    /// Program manager reference (legacy, for migration).
    pub fn program_manager(&self) -> &Arc<ProgramManager> {
        &self.program_manager
    }

    // ── Skill methods (delegate to SkillManager) ───────────────────

    /// List installed skill entries (replaces list_programs for skills).
    pub async fn list_skills_entries(&self) -> Vec<SkillEntry> {
        self.skill_manager.list_skills().await
    }

    /// Get skill details (replaces get_program for skills).
    pub async fn get_skill_entry(&self, name: &str) -> Option<SkillEntry> {
        self.skill_manager.get_skill(name).await
    }

    /// Enable a skill (replaces enable_program for skills).
    pub async fn enable_skill(&self, name: &str) -> anyhow::Result<()> {
        self.skill_manager.set_enabled(name, true).await
    }

    /// Disable a skill (replaces disable_program for skills).
    pub async fn disable_skill(&self, name: &str) -> anyhow::Result<()> {
        self.skill_manager.set_enabled(name, false).await
    }

    /// Check requirements for a skill (replaces check_host_requirements for skills).
    pub async fn check_skill_requirements(&self, name: &str) -> Option<RequirementsCheck> {
        self.skill_manager
            .get_skill(name)
            .await
            .map(|e| e.eligibility)
    }

    /// List all skills (metadata only).
    pub async fn list_skills(&self) -> anyhow::Result<Vec<SkillMeta>> {
        Ok(self.skill_manager.list_skills_meta().await)
    }

    /// Load skill by name.
    pub async fn load_skill(&self, name: &str) -> anyhow::Result<Option<Skill>> {
        self.skill_manager.load_skill(name).await
    }

    /// Create a new skill.
    pub async fn create_skill(
        &self,
        name: &str,
        description: &str,
        content: &str,
    ) -> anyhow::Result<()> {
        self.skill_manager
            .create_skill(name, description, content)
            .await
    }

    /// Delete a skill.
    pub async fn delete_skill(&self, name: &str) -> anyhow::Result<()> {
        self.skill_manager.delete_skill(name).await
    }

    /// Skill manager reference.
    pub fn skill_manager(&self) -> &Arc<SkillManager> {
        &self.skill_manager
    }

    /// Full host tool check.
    pub fn check_host_tools(&self) -> HostToolStatus {
        self.host_tool_validator.full_check()
    }
}
