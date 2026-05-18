//! Extension API — programs, skills, host tools.

use crate::host_tools::HostToolStatus;
use crate::host_tools::HostToolValidator;
use crate::program::{HostRequirementsCheck, InstallSource, Program, ProgramManager};
use crate::skill::{Skill, SkillMeta, SkillStore};
use std::sync::Arc;

/// Extension system calls.
pub struct ExtensionApi {
    pub(crate) program_manager: Arc<ProgramManager>,
    pub(crate) skill_store: Arc<SkillStore>,
    pub(crate) host_tool_validator: Arc<HostToolValidator>,
}

impl ExtensionApi {
    /// Create a new ExtensionApi.
    pub fn new(
        program_manager: Arc<ProgramManager>,
        skill_store: Arc<SkillStore>,
        host_tool_validator: Arc<HostToolValidator>,
    ) -> Self {
        Self {
            program_manager,
            skill_store,
            host_tool_validator,
        }
    }
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

    /// List all skills.
    pub async fn list_skills(&self) -> anyhow::Result<Vec<SkillMeta>> {
        self.skill_store.list_skills().await
    }

    /// Load skill by name.
    pub async fn load_skill(&self, name: &str) -> anyhow::Result<Option<Skill>> {
        self.skill_store.load_skill(name).await
    }

    /// Create a new skill.
    pub async fn create_skill(
        &self,
        name: &str,
        description: &str,
        content: &str,
    ) -> anyhow::Result<()> {
        self.skill_store
            .create_skill(name, description, content)
            .await
    }

    /// Delete a skill.
    pub async fn delete_skill(&self, name: &str) -> anyhow::Result<()> {
        self.skill_store.delete_skill(name).await
    }

    /// Program manager reference.
    pub fn program_manager(&self) -> &Arc<ProgramManager> {
        &self.program_manager
    }

    /// Full host tool check.
    pub fn check_host_tools(&self) -> HostToolStatus {
        self.host_tool_validator.full_check()
    }
}
