//! Project detection: find a Project matching user input.
//!
//! Simplified from Space's 3-layer detection. Phase 1 uses:
//! 1. Direct name match
//! 2. Path extraction + match
//! 3. Tag/keyword match
//!
//! AI-based classification is deferred to Phase 2.

use std::path::PathBuf;

#[cfg(test)]
use super::ProjectSource;
use super::{Project, ProjectId};

/// Result of a project lookup attempt.
#[derive(Debug)]
pub enum DetectionResult {
    /// Found a matching project.
    Found(ProjectId),
    /// No project matched. Optionally, a path was detected.
    NoMatch { detected_path: Option<PathBuf> },
}

/// Try to detect a project from a user message.
///
/// Detection layers:
/// 1. Direct name match ("oxios" → project with name "oxios")
/// 2. Path extraction ("/Volumes/MERCURY/PROJECTS/oxios" → project with matching path)
/// 3. Tag match (keywords → project tags)
pub fn detect_project(message: &str, projects: &[Project]) -> DetectionResult {
    // Layer 1: Direct name match (case-insensitive)
    let lower = message.to_lowercase();
    for project in projects {
        if lower.contains(&project.name.to_lowercase()) {
            return DetectionResult::Found(project.id);
        }
    }

    // Layer 2: Path extraction
    if let Some(path) = extract_path(message) {
        for project in projects {
            if project
                .paths
                .iter()
                .any(|p| path.starts_with(p) || p.starts_with(&path))
            {
                return DetectionResult::Found(project.id);
            }
        }
        return DetectionResult::NoMatch {
            detected_path: Some(path),
        };
    }

    // Layer 3: Tag match
    for project in projects {
        for tag in &project.tags {
            if lower.contains(&tag.to_lowercase()) {
                return DetectionResult::Found(project.id);
            }
        }
    }

    DetectionResult::NoMatch {
        detected_path: None,
    }
}

/// Extract a filesystem path from a message string.
///
/// Looks for patterns like `/path/to/something`.
pub fn extract_path(message: &str) -> Option<PathBuf> {
    // Find substrings that look like absolute paths
    for word in message.split_whitespace() {
        let cleaned = word.trim_matches(|c: char| {
            !c.is_alphanumeric() && c != '/' && c != '.' && c != '-' && c != '_'
        });
        if cleaned.starts_with('/') && cleaned.len() > 2 {
            let path = PathBuf::from(cleaned);
            // Check it looks like a real path (has at least one directory component)
            if path.parent().is_some() {
                return Some(path);
            }
        }
    }

    // Check for ~-prefixed paths
    for word in message.split_whitespace() {
        let cleaned = word.trim_matches(|c: char| {
            !c.is_alphanumeric() && c != '/' && c != '.' && c != '-' && c != '_' && c != '~'
        });
        if cleaned.starts_with("~/")
            && cleaned.len() > 2
            && let Some(home) = std::env::var_os("HOME")
        {
            let expanded = cleaned.replacen("~", &home.to_string_lossy(), 1);
            return Some(PathBuf::from(expanded));
        }
    }

    None
}

/// Find a project by exact ID.
pub fn find_by_id(projects: &[Project], id: ProjectId) -> Option<&Project> {
    projects.iter().find(|p| p.id == id)
}

/// Find a project by name (case-insensitive).
pub fn find_by_name<'a>(projects: &'a [Project], name: &str) -> Option<&'a Project> {
    let lower = name.to_lowercase();
    projects.iter().find(|p| p.name.to_lowercase() == lower)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_projects() -> Vec<Project> {
        let mut oxios = Project::new("oxios", ProjectSource::Manual);
        oxios
            .paths
            .push(PathBuf::from("/Volumes/MERCURY/PROJECTS/oxios"));
        oxios.add_tag("agent-os");

        let mut oxi = Project::new("oxi", ProjectSource::Manual);
        oxi.paths
            .push(PathBuf::from("/Volumes/MERCURY/PROJECTS/oxi"));
        oxi.add_tag("sdk");

        let mut blog = Project::new("my-blog", ProjectSource::Manual);
        blog.add_tag("writing");
        blog.add_tag("content");

        vec![oxios, oxi, blog]
    }

    #[test]
    fn test_detect_by_name() {
        let projects = make_projects();
        let result = detect_project("oxios 코드리뷰해줘", &projects);
        assert!(matches!(result, DetectionResult::Found(id) if id == projects[0].id));
    }

    #[test]
    fn test_detect_by_path() {
        let projects = make_projects();
        let result = detect_project("/Volumes/MERCURY/PROJECTS/oxios에서 작업", &projects);
        assert!(matches!(result, DetectionResult::Found(id) if id == projects[0].id));
    }

    #[test]
    fn test_detect_by_tag() {
        let projects = make_projects();
        let result = detect_project("writing 관련 도움이 필요해", &projects);
        assert!(matches!(result, DetectionResult::Found(id) if id == projects[2].id));
    }

    #[test]
    fn test_detect_no_match_with_path() {
        let projects = make_projects();
        let result = detect_project("/Volumes/MERCURY/PROJECTS/unknown 에서 작업", &projects);
        assert!(matches!(
            result,
            DetectionResult::NoMatch {
                detected_path: Some(_)
            }
        ));
    }

    #[test]
    fn test_detect_no_match() {
        let projects = make_projects();
        let result = detect_project("오늘 점심 뭐 먹지?", &projects);
        assert!(matches!(
            result,
            DetectionResult::NoMatch {
                detected_path: None
            }
        ));
    }

    #[test]
    fn test_extract_path() {
        assert_eq!(
            extract_path("/Volumes/MERCURY/PROJECTS/oxios"),
            Some(PathBuf::from("/Volumes/MERCURY/PROJECTS/oxios"))
        );
        assert_eq!(extract_path("no path here"), None);
    }

    #[test]
    fn test_find_by_name() {
        let projects = make_projects();
        assert!(find_by_name(&projects, "oxios").is_some());
        assert!(find_by_name(&projects, "Oxios").is_some()); // case-insensitive
        assert!(find_by_name(&projects, "nonexistent").is_none());
    }
}
