# Progress

## Status
RFC-009 Phase 1-3: skill.rs rewrite Complete ✅

## Tasks — Phase 1-3 (skill.rs rewrite)
- [x] Types defined: Requirements, SkillInstallSpec, InstallKind, RequirementsCheck, ConfigCheck, SkillStatus, SkillSource, SkillInvocationPolicy, SkillMetadata, SkillEntry, SkillSnapshot, SkillRef, SkillConfig, SkillState, Skill, SkillMeta
- [x] Frontmatter parser: full OpenClaw format (requires, install, os, always, invocation policy, all scalar fields)
- [x] Requirements evaluation: bins (which), any_bins (OR), env, config (stub), os, always bypass
- [x] SkillManager: new, init, list_skills, get_skill, get_skill_content, set_enabled (state.json), build_snapshot, create_skill, delete_skill, load_skill, list_skills_meta
- [x] Prompt formatting: format_skills_for_prompt() with XML output, escape_xml, compact_path (~)
- [x] SkillStore backward-compat shim (wraps SkillManager via RwLock)
- [x] 30+ unit tests (frontmatter, requirements, XML, types)
- [x] Integration: extension_api.rs rewritten (2-arg, no ProgramManager), kernel_handle/mod.rs updated, agent_runtime.rs program tools disabled, supervisor.rs and kernel_bridge.rs test calls updated, lib.rs exports expanded

## Tasks — Prior Phase 3 (integration)
- [x] kernel_bridge.rs — Updated ExtensionApi::new
- [x] extension_api.rs — Rewritten to use SkillManager only (no ProgramManager)
- [x] capability/types.rs — ResourceRef::Program → ResourceRef::Skill
- [x] capability/template.rs — with_programs() → with_skills()
- [x] lib.rs — All new skill types re-exported
- [x] kernel_handle/mod.rs — from_subsystems updated
- [x] kernel.rs (binary) — Uses SkillManager
- [x] tools/registration.rs — ResourceRef::Program → ResourceRef::Skill
- [x] supervisor.rs — ExtensionApi::new 2-arg call
- [x] All ResourceRef::Program and with_programs references removed

## Files Changed
- crates/oxios-kernel/src/skill.rs — Complete rewrite with unified Skill system (OpenClaw model)
- crates/oxios-kernel/src/kernel_handle/extension_api.rs — Rewritten: 2-arg constructor, skill-centric API
- crates/oxios-kernel/src/kernel_handle/mod.rs — from_subsystems: removed program_manager arg
- crates/oxios-kernel/src/agent_runtime.rs — Disabled program tool registration (pending Phase 3 cleanup)
- crates/oxios-kernel/src/supervisor.rs — Updated test: 2-arg ExtensionApi::new
- crates/oxios-kernel/src/tools/kernel_bridge.rs — Updated test: 2-arg ExtensionApi::new
- crates/oxios-kernel/src/lib.rs — Expanded public exports for all new types

## Notes
- Pre-existing compile errors (22) from memory module: MemoryEntry fields, normalizer module, Phase4Result, auto_protect, dream module. None from skill changes.
- skill.rs: 0 errors, 0 warnings
- No new dependencies added (uses existing tokio, serde, anyhow, chrono, dirs)
- Program tool registration in agent_runtime.rs temporarily disabled — needs re-design for unified Skill model
- SkillConfig integration (per-skill config from config.toml) typed but not wired into check_requirements() — config checks are stubs
- File watching not implemented yet — can be added as follow-up
