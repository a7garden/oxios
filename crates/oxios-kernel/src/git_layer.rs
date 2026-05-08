//! Git-based version control layer using gix.
//! Provides in-process commits, logs, tags, and restore.

use std::path::{Path, PathBuf};
use anyhow::{bail, Result};
use gix::hash::ObjectId;
use gix::objs::tree::EntryKind;
use gix::refs::transaction::PreviousValue;
use gix::bstr::BStr;
use parking_lot::Mutex;
use std::sync::Arc;

const GITIGNORE: &str = r#"# Oxios
*.tmp
*.lock
.env
api-keys.json
container_volumes/
"#;

/// Commit information returned after a successful commit.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CommitInfo {
    /// Full commit hash (hex).
    pub hash: String,
    /// Short hash (7 chars).
    pub short_hash: String,
    /// Commit message.
    pub message: String,
    /// ISO-8601 timestamp.
    pub timestamp: String,
    /// Author name.
    pub author: String,
}

/// A single commit log entry.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LogEntry {
    /// Full commit hash (hex).
    pub hash: String,
    /// Short hash (7 chars).
    pub short_hash: String,
    /// Commit message.
    pub message: String,
    /// Timestamp string.
    pub timestamp: String,
    /// Author name.
    pub author: String,
}

/// Git-based version control layer.
///
/// Uses `gix` for in-process git operations — no subprocess spawning,
/// no performance overhead of forking `git` CLI commands.
pub struct GitLayer {
    repo: Arc<Mutex<gix::Repository>>,
    root: PathBuf,
    committer_name: String,
    committer_email: String,
    enabled: bool,
}

impl GitLayer {
    /// Create a new GitLayer, initializing a repo if needed.
    pub fn new(root: PathBuf, enabled: bool) -> Result<Self> {
        let repo = if root.join(".git").exists() {
            gix::open(&root)?
        } else {
            std::fs::create_dir_all(&root)?;
            gix::init(&root)?
        };

        // Write .gitignore
        let gitignore = root.join(".gitignore");
        if !gitignore.exists() {
            std::fs::write(&gitignore, GITIGNORE)?;
        }

        let repo_ref = Arc::new(Mutex::new(repo));

        // Create initial commit if repo is empty
        if Self::head_id_detached(&repo_ref).is_none() {
            Self::create_initial_commit(&repo_ref, &root)?;
        }

        Ok(Self {
            repo: repo_ref,
            root,
            committer_name: "oxios".into(),
            committer_email: "oxios@oxios".into(),
            enabled,
        })
    }

    /// Get head commit as ObjectId (detached, no repo reference).
    fn head_id_detached(repo_arc: &Arc<Mutex<gix::Repository>>) -> Option<ObjectId> {
        let repo = repo_arc.lock();
        repo.head_id().ok().map(|id| id.detach())
    }

    fn create_initial_commit(repo: &Arc<Mutex<gix::Repository>>, root: &PathBuf) -> Result<()> {
        let repo_lock = repo.lock();
        let gitignore = root.join(".gitignore");
        let content = std::fs::read(&gitignore)?;
        let blob_id = repo_lock.write_blob(&content)?;
        let empty_tree = ObjectId::empty_tree(repo_lock.object_hash());
        let mut editor = repo_lock.edit_tree(empty_tree)?;
        editor.upsert(".gitignore", EntryKind::Blob, blob_id)?;
        let tree_id = editor.write()?;
        let sig = self_signature_ref();
        repo_lock.commit_as(
            self_signature_ref(),
            self_signature_ref(),
            "refs/heads/main",
            "Initial commit",
            tree_id.detach(),
            Vec::<ObjectId>::new(),
        )?;
        Ok(())
    }

    /// Commit a single file with a message.
    pub fn commit_file(&self, rel_path: &str, message: &str) -> Result<CommitInfo> {
        if !self.enabled {
            return self.noop_commit(message);
        }
        let repo = self.repo.lock();
        let abs = self.root.join(rel_path);
        if !abs.exists() {
            bail!("File not found: {}", rel_path);
        }

        let content = std::fs::read(&abs)?;
        let blob_id = repo.write_blob(&content)?;
        let head_tree = Self::head_tree_oid(&repo)?;
        let mut editor = repo.edit_tree(head_tree)?;
        editor.upsert(rel_path, EntryKind::Blob, blob_id)?;
        let tree_id = editor.write()?;

        let parent = Self::head_id_detached(&self.repo);
        let sig = self_signature_ref();
        let commit_id = repo.commit_as(
            self_signature_ref(),
            self_signature_ref(),
            "refs/heads/main",
            message,
            tree_id.detach(),
            parent.into_iter().collect::<Vec<_>>(),
        )?;

        Ok(self.make_info(&commit_id, message))
    }

    /// Commit multiple files in a single commit.
    pub fn commit_files(&self, rel_paths: &[&str], message: &str) -> Result<CommitInfo> {
        if !self.enabled {
            return self.noop_commit(message);
        }
        let repo = self.repo.lock();
        let head_tree = Self::head_tree_oid(&repo)?;
        let mut editor = repo.edit_tree(head_tree)?;

        for path in rel_paths {
            let abs = self.root.join(path);
            if abs.exists() {
                let content = std::fs::read(&abs)?;
                let blob_id = repo.write_blob(&content)?;
                editor.upsert(*path, EntryKind::Blob, blob_id)?;
            }
        }
        let tree_id = editor.write()?;

        let parent = Self::head_id_detached(&self.repo);
        let sig = self_signature_ref();
        let commit_id = repo.commit_as(
            self_signature_ref(),
            self_signature_ref(),
            "refs/heads/main",
            message,
            tree_id.detach(),
            parent.into_iter().collect::<Vec<_>>(),
        )?;

        Ok(self.make_info(&commit_id, message))
    }

    /// Remove a file from the repo and commit.
    pub fn remove_file(&self, rel_path: &str, message: &str) -> Result<CommitInfo> {
        if !self.enabled {
            return self.noop_commit(message);
        }
        let repo = self.repo.lock();
        let head_tree = Self::head_tree_oid(&repo)?;
        let mut editor = repo.edit_tree(head_tree)?;
        editor.remove(rel_path)?;
        let tree_id = editor.write()?;

        let parent = Self::head_id_detached(&self.repo);
        let sig = self_signature_ref();
        let commit_id = repo.commit_as(
            self_signature_ref(),
            self_signature_ref(),
            "refs/heads/main",
            message,
            tree_id.detach(),
            parent.into_iter().collect::<Vec<_>>(),
        )?;

        Ok(self.make_info(&commit_id, message))
    }

    /// Append an audit entry to a monthly audit log file and commit it.
    pub fn log_action(
        &self,
        agent: &str,
        action: &str,
        target: &str,
        allowed: bool,
        detail: Option<&str>,
    ) -> Result<()> {
        let now = chrono::Utc::now();
        let filename = format!("audit/{}.audit", now.format("%Y-%m"));
        let entry = format!(
            "{} | {} | {} | {} | {} | {}\n",
            now.to_rfc3339(),
            agent,
            action,
            target,
            if allowed { "ALLOW" } else { "DENY" },
            detail.unwrap_or("-")
        );
        let dir = self.root.join("audit");
        std::fs::create_dir_all(&dir)?;
        use std::io::Write;
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .write(true)
            .open(self.root.join(&filename))?
            .write_all(entry.as_bytes())?;
        self.commit_file(&filename, &format!("audit: {} {} {}", agent, action, target))?;
        Ok(())
    }

    /// Create an annotated tag at the current HEAD.
    pub fn tag(&self, name: &str, message: &str) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }
        let repo = self.repo.lock();
        let head_id = Self::head_id_detached(&self.repo)
            .ok_or_else(|| anyhow::anyhow!("No HEAD commit to tag"))?;
        let sig = self_signature_ref();
        repo.tag(
            name,
            head_id,
            gix::objs::Kind::Commit,
            Some(sig),
            message,
            PreviousValue::MustNotExist,
        )?;
        Ok(())
    }

    /// List all tags in the repository.
    pub fn list_tags(&self) -> Result<Vec<String>> {
        let repo = self.repo.lock();
        let mut tags = Vec::new();
        for reference in repo.references()?.all()? {
            let reference = reference.map_err(|e| anyhow::anyhow!("ref iter: {e:#}"))?;
            let name = reference.name().shorten().to_string();
            if name.starts_with("tags/") || (!name.contains('/') && !name.is_empty()) {
                let tag_name = name.strip_prefix("tags/").unwrap_or(&name);
                tags.push(tag_name.to_string());
            }
        }
        Ok(tags)
    }

    /// Return commit log entries, most recent first.
    pub fn log(&self, max_count: usize) -> Result<Vec<LogEntry>> {
        let repo = self.repo.lock();
        let head_id = repo.head_id()?.detach();
        let mut entries = Vec::new();
        let mut current_id: Option<ObjectId> = Some(head_id);

        while let Some(id) = current_id {
            if entries.len() >= max_count {
                break;
            }
            let commit = repo.find_commit(id)?;
            let decoded = commit.decode()?;
            let msg = decoded.message.to_string();
            let timestamp = format!("{:?}", decoded.committer.time);
            let author = decoded.author.name.to_string();
            let hex = id.to_hex().to_string();
            entries.push(LogEntry {
                hash: hex.clone(),
                short_hash: hex[..7].into(),
                message: msg,
                timestamp,
                author,
            });
            // First parent via iterator
            current_id = decoded.parents().next();
        }

        Ok(entries)
    }

    /// Restore a file to its state in a specific commit.
    pub fn restore_file(&self, rel_path: &str, hash: &str) -> Result<()> {
        let repo = self.repo.lock();
        let commit_id = ObjectId::from_hex(hash.as_bytes())?;
        let commit = repo.find_commit(commit_id)?;
        let decoded = commit.decode()?;
        let tree_id = ObjectId::from_hex(decoded.tree)?;
        let tree = repo.find_tree(tree_id)?;
        let decoded_tree = tree.decode()?;

        // Find entry by filename (as bytes)
        let rel_bytes = BStr::new(rel_path);
        let entry = decoded_tree
            .entries
            .iter()
            .find(|e| e.filename == rel_bytes)
            .ok_or_else(|| {
                anyhow::anyhow!("Path {} not found in commit {}", rel_path, hash)
            })?;

        let blob = repo.find_blob(entry.oid.to_owned())?;
        std::fs::write(self.root.join(rel_path), &blob.data)?;
        Ok(())
    }

    /// Verify repository integrity.
    pub fn verify(&self) -> Result<bool> {
        let repo = self.repo.lock();
        let refs = repo.references()?;
        for reference in refs.all()? {
            let _ = reference.map_err(|e| anyhow::anyhow!("ref verify: {e:#}"))?;
        }
        repo.head_id()?;
        Ok(true)
    }

    /// Whether auto-commit is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    /// Get the current HEAD tree's ObjectId.
    fn head_tree_oid(repo: &gix::Repository) -> Result<ObjectId> {
        match Self::head_id_detached_raw(repo) {
            Some(id) => {
                let commit = repo.find_commit(id)?;
                let decoded = commit.decode()?;
                let oid = ObjectId::from_hex(decoded.tree)?;
                Ok(oid)
            }
            None => Ok(ObjectId::empty_tree(repo.object_hash())),
        }
    }

    /// Get head commit as ObjectId (raw, borrowed repo).
    fn head_id_detached_raw(repo: &gix::Repository) -> Option<ObjectId> {
        repo.head_id().ok().map(|id| id.detach())
    }

    fn noop_commit(&self, message: &str) -> Result<CommitInfo> {
        Ok(CommitInfo {
            hash: "(disabled)".into(),
            short_hash: "(dis)".into(),
            message: message.into(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            author: "oxios".into(),
        })
    }

    fn make_info(&self, id: &gix::Id, message: &str) -> CommitInfo {
        let hex = id.to_hex().to_string();
        CommitInfo {
            short_hash: hex[..7].into(),
            hash: hex,
            message: message.into(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            author: self.committer_name.clone(),
        }
    }
}

/// Create a signature ref for committer/author identity.
fn self_signature_ref() -> gix::actor::SignatureRef<'static> {
    gix::actor::SignatureRef {
        name: "oxios".into(),
        email: "oxios@oxios".into(),
        time: gix::date::Time::now_local_or_utc(),
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, GitLayer) {
        let dir = tempfile::tempdir().unwrap();
        let layer = GitLayer::new(dir.path().to_path_buf(), true).unwrap();
        (dir, layer)
    }

    #[test]
    fn test_init_creates_repo() {
        let (dir, _) = setup();
        assert!(dir.path().join(".git").exists());
    }

    #[test]
    fn test_commit_file() {
        let (dir, layer) = setup();
        std::fs::write(dir.path().join("test.json"), b"{\"hello\":1}").unwrap();
        let info = layer.commit_file("test.json", "test commit").unwrap();
        assert!(!info.hash.is_empty());
        assert_eq!(info.short_hash.len(), 7);
        assert_eq!(info.message, "test commit");
        assert!(info.hash.starts_with(&info.short_hash));
    }

    #[test]
    fn test_log_query() {
        let (dir, layer) = setup();
        std::fs::write(dir.path().join("a.json"), b"1").unwrap();
        layer.commit_file("a.json", "first").unwrap();
        std::fs::write(dir.path().join("a.json"), b"2").unwrap();
        layer.commit_file("a.json", "second").unwrap();
        let log = layer.log(10).unwrap();
        assert!(log.len() >= 2);
        assert!(log[0].message.contains("second"));
    }

    #[test]
    fn test_tag_create_list() {
        let (dir, layer) = setup();
        std::fs::write(dir.path().join("x.json"), b"1").unwrap();
        layer.commit_file("x.json", "tag test").unwrap();
        layer.tag("v1", "first tag").unwrap();
        let tags = layer.list_tags().unwrap();
        assert!(tags.iter().any(|t| t.contains("v1")));
    }

    #[test]
    fn test_disabled_noop() {
        let dir = tempfile::tempdir().unwrap();
        let layer = GitLayer::new(dir.path().to_path_buf(), false).unwrap();
        std::fs::write(dir.path().join("test.json"), b"1").unwrap();
        let info = layer.commit_file("test.json", "noop").unwrap();
        assert_eq!(info.hash, "(disabled)");
        assert_eq!(info.short_hash, "(dis)");
    }

    #[test]
    fn test_log_action() {
        let (dir, layer) = setup();
        layer
            .log_action("agent-A", "read", "file.txt", true, None)
            .unwrap();
        let audit_file = dir
            .path()
            .join("audit")
            .join(format!("{}.audit", chrono::Utc::now().format("%Y-%m")));
        assert!(audit_file.exists());
        let content = std::fs::read_to_string(&audit_file).unwrap();
        assert!(content.contains("agent-A"));
        assert!(content.contains("ALLOW"));
    }

    #[test]
    fn test_verify() {
        let (_, layer) = setup();
        assert!(layer.verify().unwrap());
    }

    #[test]
    fn test_remove_file() {
        let (dir, layer) = setup();
        std::fs::write(dir.path().join("todelete.json"), b"1").unwrap();
        layer.commit_file("todelete.json", "add file").unwrap();
        std::fs::remove_file(dir.path().join("todelete.json")).unwrap();
        let info = layer.remove_file("todelete.json", "remove file").unwrap();
        assert!(!info.hash.is_empty());
        assert!(info.hash != "(disabled)");
    }

    #[test]
    fn test_commit_files_batch() {
        let (dir, layer) = setup();
        std::fs::write(dir.path().join("a.json"), b"1").unwrap();
        std::fs::write(dir.path().join("b.json"), b"2").unwrap();
        let info = layer
            .commit_files(&["a.json", "b.json"], "batch commit")
            .unwrap();
        assert!(!info.hash.is_empty());
        assert_eq!(info.message, "batch commit");
    }

    #[test]
    fn test_restore_file() {
        let (dir, layer) = setup();
        std::fs::write(dir.path().join("state.json"), b"v1").unwrap();
        let first = layer.commit_file("state.json", "v1").unwrap();
        std::fs::write(dir.path().join("state.json"), b"v2").unwrap();
        layer.commit_file("state.json", "v2").unwrap();
        layer
            .restore_file("state.json", &first.short_hash)
            .unwrap();
        let content = std::fs::read_to_string(dir.path().join("state.json")).unwrap();
        assert_eq!(content, "v1");
    }

    #[test]
    fn test_gitignore_created() {
        let (dir, _) = setup();
        assert!(dir.path().join(".gitignore").exists());
        let content = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert!(content.contains("Oxios"));
    }
}