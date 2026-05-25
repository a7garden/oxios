//! Prompt formatting.

use std::path::Path;
use super::types::SkillEntry;

pub fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}

pub fn compact_path(path: &Path) -> String {
    if let Some(home) = dirs::home_dir() {
        let home_str = home.to_string_lossy();
        let path_str = path.to_string_lossy();
        if let Some(rest) = path_str.strip_prefix(home_str.as_ref()) { return format!("~{}", rest); }
    }
    path.to_string_lossy().into_owned()
}

pub fn format_skills_for_prompt(skills: &[&SkillEntry]) -> String {
    if skills.is_empty() { return String::new(); }
    let mut lines = vec![
        "\n\nThe following skills provide specialized instructions for specific tasks.".into(),
        "Use the read tool to load a skill's file when the task matches its description.".into(),
        "When a skill file references a relative path, resolve it against the skill directory (parent of SKILL.md / dirname of the path) and use that absolute path in tool commands.".into(),
        String::new(),
        "<available_skills>".into(),
    ];
    for skill in skills {
        lines.push("  <skill>".into());
        lines.push(format!("    <name>{}</name>", escape_xml(&skill.skill.name)));
        lines.push(format!("    <description>{}</description>", escape_xml(&skill.skill.description)));
        lines.push(format!("    <location>{}</location>", escape_xml(&compact_path(&skill.skill.file_path))));
        lines.push("  </skill>".into());
    }
    lines.push("</available_skills>".into());
    lines.join("\n")
}
