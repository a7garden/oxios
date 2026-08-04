//! CodeApi — facade for coding workspace: session management, file operations, PTY.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use dashmap::DashMap;
use serde::{Deserialize, Serialize};

use crate::code::{CodeSession, CodeSessionState};
use crate::pty::PtyManager;

pub struct CodeApi {
    sessions: DashMap<String, Arc<CodeSessionState>>,
    pty_manager: Arc<PtyManager>,
}

impl CodeApi {
    pub fn new() -> Self {
        Self {
            sessions: DashMap::new(),
            pty_manager: Arc::new(PtyManager::new()),
        }
    }

    // ── Session management ──────────────────────────────────────────

    pub async fn create_session(
        &self,
        project_path: PathBuf,
        model: Option<String>,
    ) -> anyhow::Result<CodeSession> {
        if !project_path.exists() {
            anyhow::bail!("project path does not exist: {}", project_path.display());
        }

        // Ensure the project is a git repo — change detection relies on
        // `git status` / `git show HEAD` to compute diffs and originals.
        // Auto-init silently if .git is absent; this is non-destructive.
        if !project_path.join(".git").exists() {
            match std::process::Command::new("git")
                .arg("init")
                .current_dir(&project_path)
                .output()
            {
                Ok(o) if o.status.success() => {
                    tracing::info!("Auto-initialized git repo in {}", project_path.display());
                }
                Ok(o) => {
                    tracing::warn!(
                        "git init failed (non-fatal): {}",
                        String::from_utf8_lossy(&o.stderr)
                    );
                }
                Err(e) => {
                    tracing::warn!("git init failed (non-fatal): {e}");
                }
            }
        }

        let title = project_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Untitled".to_string());

        let session = CodeSession {
            id: uuid::Uuid::new_v4().to_string(),
            project_path: project_path.clone(),
            model,
            created_at: chrono::Utc::now(),
            title,
        };

        let state = Arc::new(CodeSessionState::new(session.clone()));
        self.sessions.insert(session.id.clone(), state);
        tracing::info!(
            "Created code session {} for {}",
            session.id,
            project_path.display()
        );
        Ok(session)
    }

    pub fn list_sessions(&self) -> Vec<CodeSession> {
        self.sessions.iter().map(|r| r.session.clone()).collect()
    }

    pub fn get_session(&self, id: &str) -> Option<Arc<CodeSessionState>> {
        self.sessions.get(id).map(|r| Arc::clone(r.value()))
    }

    pub fn delete_session(&self, id: &str) -> bool {
        self.sessions.remove(id).is_some()
    }

    // ── File operations ─────────────────────────────────────────────

    pub fn browse_dir(&self, path: &Path) -> anyhow::Result<Vec<DirEntry>> {
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            let name = entry.file_name().to_string_lossy().to_string();
            entries.push(DirEntry {
                name: name.clone(),
                path: entry.path(),
                is_dir: metadata.is_dir(),
                is_file: metadata.is_file(),
                size: if metadata.is_file() {
                    Some(metadata.len())
                } else {
                    None
                },
                modified: metadata
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs()),
            });
        }
        // Sort: directories first, then alphabetical (case-insensitive).
        entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        });
        Ok(entries)
    }

    pub fn read_file(&self, path: &Path) -> anyhow::Result<FileContent> {
        let content = std::fs::read_to_string(path)?;
        Ok(FileContent {
            content,
            language: detect_language(path),
            path: path.to_path_buf(),
        })
    }

    pub fn write_file(&self, path: &Path, content: &str) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn create_file_or_dir(&self, path: &Path, is_dir: bool) -> anyhow::Result<()> {
        if is_dir {
            std::fs::create_dir_all(path)?;
        } else {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, "")?;
        }
        Ok(())
    }

    pub fn delete_path(&self, path: &Path) -> anyhow::Result<()> {
        if path.is_dir() {
            std::fs::remove_dir_all(path)?;
        } else {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }

    pub fn move_path(&self, from: &Path, to: &Path) -> anyhow::Result<()> {
        std::fs::rename(from, to)?;
        Ok(())
    }

    // ── PTY ─────────────────────────────────────────────────────────

    pub fn pty_manager(&self) -> &Arc<PtyManager> {
        &self.pty_manager
    }
}

impl Default for CodeApi {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_file: bool,
    pub size: Option<u64>,
    pub modified: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileContent {
    pub content: String,
    pub language: String,
    pub path: PathBuf,
}

fn detect_language(path: &Path) -> String {
    match path.extension().and_then(|e| e.to_str()) {
        Some("rs") => "rust",
        Some("ts") | Some("tsx") => "typescript",
        Some("js") | Some("jsx") => "javascript",
        Some("py") => "python",
        Some("go") => "go",
        Some("java") => "java",
        Some("c") | Some("h") => "c",
        Some("cpp") | Some("cc") | Some("cxx") | Some("hpp") => "cpp",
        Some("css") => "css",
        Some("html") => "html",
        Some("json") => "json",
        Some("yaml") | Some("yml") => "yaml",
        Some("toml") => "toml",
        Some("md") => "markdown",
        Some("sh") | Some("bash") => "shell",
        Some("sql") => "sql",
        Some("vue") => "vue",
        Some("svelte") => "svelte",
        Some("swift") => "swift",
        Some("kt") => "kotlin",
        Some("rb") => "ruby",
        Some("php") => "php",
        Some("dart") => "dart",
        Some("lua") => "lua",
        Some("dockerfile") | Some(_)
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.eq_ignore_ascii_case("dockerfile"))
                .unwrap_or(false) =>
        {
            "dockerfile"
        }
        _ => "plaintext",
    }
    .to_string()
}
