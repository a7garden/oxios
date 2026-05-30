#![allow(missing_docs)]
//! Requirements evaluation.

use super::types::*;

fn has_bin(bin: &str) -> bool {
    std::process::Command::new("which").arg(bin).output().map(|o| o.status.success()).unwrap_or(false)
}

pub fn current_platform() -> &'static str {
    if cfg!(target_os = "macos") { "darwin" } else if cfg!(target_os = "windows") { "windows" } else { "linux" }
}

pub fn check_requirements(metadata: &SkillMetadata) -> RequirementsCheck {
    let platform = current_platform();
    let missing_bins: Vec<String> = metadata.requires.bins.iter().filter(|b| !has_bin(b)).cloned().collect();
    let missing_any_bins = if metadata.requires.any_bins.is_empty() || metadata.requires.any_bins.iter().any(|b| has_bin(b)) { Vec::new() } else { metadata.requires.any_bins.clone() };
    let missing_env: Vec<String> = metadata.requires.env.iter().filter(|e| std::env::var(e).is_err()).cloned().collect();
    let config_checks: Vec<ConfigCheck> = metadata.requires.config.iter().map(|path| ConfigCheck { path: path.clone(), satisfied: true }).collect();
    let missing_config: Vec<String> = config_checks.iter().filter(|c| !c.satisfied).map(|c| c.path.clone()).collect();
    let missing_os = if metadata.os.is_empty() || metadata.os.iter().any(|o| o == platform) { Vec::new() } else { metadata.os.clone() };
    let eligible = metadata.always || (missing_bins.is_empty() && missing_any_bins.is_empty() && missing_env.is_empty() && missing_config.is_empty() && missing_os.is_empty());
    RequirementsCheck { missing_bins, missing_any_bins, missing_env, missing_config, missing_os, eligible, config_checks }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn test_no_reqs() { assert!(check_requirements(&SkillMetadata::default()).eligible); }
    #[test] fn test_always() { let mut m = SkillMetadata::default(); m.always = true; m.requires.bins = vec!["nonexistent-xyz".into()]; assert!(check_requirements(&m).eligible); }
    #[test] fn test_existing_bin() { let mut m = SkillMetadata::default(); m.requires.bins = vec!["echo".into()]; assert!(check_requirements(&m).eligible); }
    #[test] fn test_missing_bin() { let mut m = SkillMetadata::default(); m.requires.bins = vec!["nonexistent-xyz".into()]; assert!(!check_requirements(&m).eligible); }
    #[test] fn test_missing_env() { let mut m = SkillMetadata::default(); m.requires.env = vec!["OXIOS_TEST_MISSING".into()]; assert!(!check_requirements(&m).eligible); }
}
