# Loop 12: Git-as-Kernel — gix 기반 버전 관리 계층

> **핵심 철학:** Linux가 "모든 것은 파일이다"라면, Oxios는 "모든 상태는 버전 관리된다"
> **의존:** `gix = "0.70"` (순수 Rust, fork 없음, Cargo가 사용하는 같은 라이브러리)
> **목표:** Git을 커널 수준의 기반 계층으로 — gix로 in-process 커밋

---

## 1. 왜 gix인가

| | git CLI (subprocess) | gix (gitoxide) |
|---|---|---|
| 실행 방식 | `Command::new("git")` → fork+exec | 같은 프로세스 내 함수 호출 |
| 성능 | 매 커밋마다 프로세스 생성 (~50ms) | **함수 호출 (~0.1ms)** |
| 타입 안전 | 문자열 파싱 | Rust 타입 |
| Send 문제 | `tokio::process::Command` + lock 위험 | 동기 함수, lock 없이 가능 |
| 의존성 | 시스템 git 설치 필요 | Cargo crate만 |
| 성숙도 | 20년 | 프로덕션 (Cargo 자체가 사용) |
| 배치 커밋 | 복잡 | 간단 (index 조작 → 한 번 commit) |

**리뷰 C1/C2/C3가 전부 자연스럽게 해결:**
- C1 (성능): fork 오버헤드 없음 → 배치 커밋 불필요, save마다 커밋해도 OK
- C2 (Send): `parking_lot::Mutex` + `.await` 없음 → gix는 동기 API
- C3 (아키텍처): `GitLayer`가 `gix::Repository`를 필드로 보유 → StateStore 수정 최소화

---

## 2. 아키텍처

```
┌─────────────────────────────────────────────────────┐
│                    Oxios Kernel                       │
│                                                      │
│  ┌──────────────────────────────────────────────┐   │
│  │         GitLayer (gix 기반)                    │   │
│  │                                               │   │
│  │  repo: gix::Repository                       │   │
│  │  root: PathBuf (~/.oxios/)                    │   │
│  │                                               │   │
│  │  ├── state/     ← StateStore 파일             │   │
│  │  ├── audit/     ← 감사 로그 (.audit)          │   │
│  │  ├── memory/    ← 에이전트 메모리              │   │
│  │  ├── seeds/     ← 스펙 시드                   │   │
│  │  ├── config.toml ← 설정                       │   │
│  │  └── programs/  ← 설치된 프로그램              │   │
│  └──────────────────────────────────────────────┘   │
│                                                      │
│  StateStore.save_json()                              │
│      ↓ 파일 쓴 후                                    │
│  GitLayer.commit_file(path)    ← 동기, in-process   │
│                                                      │
│  AccessManager.log_access()                          │
│      ↓ audit/YYYY-MM.audit append 후                 │
│  GitLayer.commit_file(path)    ← 동기, in-process   │
│                                                      │
│  Orchestrator (seed 생성/평가 완료)                   │
│      ↓                                               │
│  GitLayer.tag(name, message)  ← gix::tag()          │
└─────────────────────────────────────────────────────┘
```

---

## 3. GitLayer

### 의존성

```toml
# crates/oxios-kernel/Cargo.toml
[dependencies]
gix = { version = "0.70", features = ["tree-editor"] }
```

`tree-editor` feature가 있어야 `edit_tree()` 사용 가능.

### 데이터 구조

```rust
use std::path::{Path, PathBuf};
use std::sync::Arc;
use gix::Repository;
use gix::actor::SignatureRef;
use gix::hash::ObjectId;
use gix::ref::transaction::PreviousValue;
use parking_lot::Mutex;
use anyhow::{bail, Result};

/// Git 기반 버전 관리 계층.
/// gix를 사용하여 in-process에서 커밋, 로그, 복원 수행.
pub struct GitLayer {
    /// gix 리포지토리 (ThreadSafeMode 필요 → into_sync() 또는 Mutex).
    repo: Arc<Mutex<Repository>>,
    /// 작업 공간 루트.
    root: PathBuf,
    /// 커밋 서명.
    committer_name: String,
    /// 자동 커밋 활성화.
    enabled: bool,
}

/// 커밋 정보.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CommitInfo {
    pub hash: String,
    pub short_hash: String,
    pub message: String,
    pub timestamp: String,
    pub author: String,
}

/// 로그 엔트리.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LogEntry {
    pub hash: String,
    pub short_hash: String,
    pub message: String,
    pub timestamp: String,
    pub author: String,
}
```

### 핵심 메서드

```rust
impl GitLayer {
    /// 새 GitLayer 생성. 필요 시 git init.
    pub fn new(root: PathBuf, enabled: bool) -> Result<Self> {
        let repo = if root.join(".git").exists() {
            Repository::open(&root)?
        } else {
            // git init
            Repository::init(&root)?
        };

        // .gitignore 작성
        let gitignore = root.join(".gitignore");
        if !gitignore.exists() {
            std::fs::write(&gitignore, GITIGNORE)?;
        }

        Ok(Self {
            repo: Arc::new(Mutex::new(repo)),
            root,
            committer_name: "oxios".into(),
            enabled,
        })
    }

    /// 파일 하나를 스테이징 + 커밋.
    /// gix로 index에 경로 추가 → tree 생성 → commit.
    pub fn commit_file(&self, rel_path: &str, message: &str) -> Result<CommitInfo> {
        if !self.enabled { return self.noop_commit(message); }

        let repo = self.repo.lock();
        let abs_path = self.root.join(rel_path);
        if !abs_path.exists() {
            bail!("File not found: {}", rel_path);
        }

        // 1. 파일을 blob으로 쓰기
        let content = std::fs::read(&abs_path)?;
        let blob_id = repo.write_blob(&content)?;

        // 2. 현재 HEAD의 tree를 가져와서 경로에 blob 삽입
        let head_tree = Self::head_tree_id(&repo)?;
        let mut editor = repo.edit_tree(head_tree)?;
        editor.upsert(rel_path, gix_object::tree::EntryKind::Blob, blob_id)?;
        let new_tree_id = editor.write()?;

        // 3. 커밋 생성
        let parent = Self::head_commit_id(&repo);
        let signature = Self::signature(&self.committer_name);
        let parents: Vec<ObjectId> = parent.into_iter().collect();

        let commit_id = repo.commit_as(
            &signature,
            &signature,
            "refs/heads/main",
            message,
            new_tree_id,
            parents,
        )?;

        Ok(CommitInfo {
            hash: commit_id.to_hex().to_string(),
            short_hash: commit_id.to_hex().to_string()[..7].into(),
            message: message.into(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            author: self.committer_name.clone(),
        })
    }

    /// 여러 파일을 한 번에 커밋.
    pub fn commit_files(&self, rel_paths: &[&str], message: &str) -> Result<CommitInfo> {
        if !self.enabled { return self.noop_commit(message); }

        let repo = self.repo.lock();
        let head_tree = Self::head_tree_id(&repo)?;
        let mut editor = repo.edit_tree(head_tree)?;

        for rel_path in rel_paths {
            let abs_path = self.root.join(rel_path);
            if abs_path.exists() {
                let content = std::fs::read(&abs_path)?;
                let blob_id = repo.write_blob(&content)?;
                editor.upsert(rel_path, gix_object::tree::EntryKind::Blob, blob_id)?;
            }
        }
        let new_tree_id = editor.write()?;

        let parent = Self::head_commit_id(&repo);
        let signature = Self::signature(&self.committer_name);
        let commit_id = repo.commit_as(
            &signature, &signature,
            "refs/heads/main", message, new_tree_id,
            parent.into_iter().collect::<Vec<_>>(),
        )?;

        Ok(CommitInfo {
            hash: commit_id.to_hex().to_string(),
            short_hash: commit_id.to_hex().to_string()[..7].into(),
            message: message.into(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            author: self.committer_name.clone(),
        })
    }

    /// 파일 삭제 커밋.
    pub fn remove_file(&self, rel_path: &str, message: &str) -> Result<CommitInfo> {
        if !self.enabled { return self.noop_commit(message); }

        let repo = self.repo.lock();
        let head_tree = Self::head_tree_id(&repo)?;
        let mut editor = repo.edit_tree(head_tree)?;
        editor.remove(rel_path)?;
        let new_tree_id = editor.write()?;

        let parent = Self::head_commit_id(&repo);
        let signature = Self::signature(&self.committer_name);
        let commit_id = repo.commit_as(
            &signature, &signature,
            "refs/heads/main", message, new_tree_id,
            parent.into_iter().collect::<Vec<_>>(),
        )?;

        Ok(self.make_commit_info(&repo, &commit_id, message))
    }

    // ── 감사 ────────────────────────────────────

    /// 감사 로그 엔트리를 파일에 append + 커밋.
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

        // 디렉토리 보장
        let dir = self.root.join("audit");
        std::fs::create_dir_all(&dir)?;
        // append
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .write(true)
            .open(self.root.join(&filename))?
            .write_all(entry.as_bytes())?;

        self.commit_file(&filename, &format!("audit: {} {} {}", agent, action, target))?;
        Ok(())
    }

    // ── 태그 ────────────────────────────────────

    /// 태그 생성.
    pub fn tag(&self, name: &str, message: &str) -> Result<()> {
        if !self.enabled { return Ok(()); }

        let repo = self.repo.lock();
        let head_id = Self::head_commit_id(&repo)
            .ok_or_else(|| anyhow::anyhow!("No HEAD commit to tag"))?;
        let signature = Self::signature(&self.committer_name);

        repo.tag(
            name,
            head_id,
            gix_object::Kind::Commit,
            Some(signature),
            message,
            PreviousValue::Any,
        )?;

        Ok(())
    }

    /// 태그 목록.
    pub fn list_tags(&self) -> Result<Vec<String>> {
        let repo = self.repo.lock();
        let mut tags = Vec::new();
        for reference in repo.references()?.all()? {
            let ref_name = reference?.name().shorten().to_string();
            if ref_name.starts_with("tags/") || !ref_name.contains('/') {
                tags.push(ref_name);
            }
        }
        Ok(tags)
    }

    // ── 로그 ────────────────────────────────────

    /// 커밋 로그 조회.
    pub fn log(&self, max_count: usize) -> Result<Vec<LogEntry>> {
        let repo = self.repo.lock();
        let head = repo.head_commit()?;
        let mut entries = Vec::new();
        let mut current = Some(head);

        while let Some(commit) = current {
            if entries.len() >= max_count { break; }
            let id = commit.id;
            entries.push(LogEntry {
                hash: id.to_hex().to_string(),
                short_hash: id.to_hex().to_string()[..7].into(),
                message: commit.message().to_string(),
                timestamp: commit.committer().0.time.to_string(),
                author: commit.author().0.name.to_string(),
            });
            current = commit.parent_ids().next()
                .and_then(|pid| repo.find_commit(pid).ok());
        }

        Ok(entries)
    }

    // ── 복원 ────────────────────────────────────

    /// 특정 파일을 이전 커밋으로 복원.
    pub fn restore_file(&self, rel_path: &str, hash: &str) -> Result<()> {
        let repo = self.repo.lock();
        let commit_id = ObjectId::from_hex(hash.as_bytes())?;
        let commit = repo.find_commit(commit_id)?;
        let tree = commit.tree()?;
        let blob = tree.find_entry(rel_path)
            .ok_or_else(|| anyhow::anyhow!("Path {} not found in commit {}", rel_path, hash))?
            .object()?;

        std::fs::write(self.root.join(rel_path), blob.data)?;
        Ok(())
    }

    // ── 검증 ────────────────────────────────────

    /// 리포지토리 무결성 검증.
    pub fn verify(&self) -> Result<bool> {
        // gix는 fsck을 직접 지원하지 않으므로
        // 모든 참조가 해석 가능한지 확인
        let repo = self.repo.lock();
        let refs = repo.references()?;
        for reference in refs.all()? {
            let r = reference?;
            if r.target().try_id().is_none() {
                return Ok(false);
            }
        }
        // HEAD가 존재하는지 확인
        if repo.head_commit().is_err() && Self::head_commit_id(&repo).is_some() {
            return Ok(false);
        }
        Ok(true)
    }

    // ── 헬퍼 ────────────────────────────────────

    fn head_tree_id(repo: &Repository) -> Result<ObjectId> {
        match Self::head_commit_id(repo) {
            Some(id) => Ok(repo.find_commit(id)?.tree_id()),
            None => Ok(ObjectId::empty_tree(repo.object_hash())),
        }
    }

    fn head_commit_id(repo: &Repository) -> Option<ObjectId> {
        repo.head().ok()
            .and_then(|h| h.try_into_referent().ok())
            .and_then(|r| r.target().try_id().map(|id| id.to_owned()))
    }

    fn signature(name: &str) -> gix_actor::Signature {
        gix_actor::Signature {
            name: name.into(),
            email: format!("{}@oxios", name).into(),
            time: gix_date::Time::now_local_or_utc(),
        }
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

    fn make_commit_info(&self, _repo: &Repository, id: &gix::Id, message: &str) -> CommitInfo {
        CommitInfo {
            hash: id.to_hex().to_string(),
            short_hash: id.to_hex().to_string()[..7].into(),
            message: message.into(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            author: self.committer_name.clone(),
        }
    }
}

const GITIGNORE: &str = r#"# Oxios
*.tmp
*.lock
.env
api-keys.json
container_volumes/
"#;
```

---

## 4. 통합 지점

### 4.1 StateStore → GitLayer (Kernel 수준에서 연결)

StateStore는 **직접 수정하지 않음.** Kernel에서 `save_json` 호출 후 `git_layer.commit_file()` 호출:

```rust
// kernel.rs
pub async fn save_and_commit<T: Serialize>(
    &self,
    category: &str,
    name: &str,
    data: &T,
) -> Result<()> {
    self.state_store.save_json(category, name, data).await?;
    let rel_path = format!("{}/{}.json", category, name);
    self.git_layer.commit_file(&rel_path, &format!("state: {}/{}", category, name))?;
    Ok(())
}
```

Orchestrator, CronScheduler, ContainerManager, MemoryManager은 `kernel.save_and_commit()` 사용.

### 4.2 AccessManager → GitLayer

```rust
// access_manager.rs의 log_access 끝에:
pub fn log_access(&mut self, agent: &str, action: &str, target: &str, allowed: bool, detail: Option<&str>) {
    // 기존 인메모리 로그...
    self.audit_log.push(entry);

    // GitLayer에 감사 기록 (필드가 있으면)
    if let Some(git) = &self.git_layer {
        let _ = git.log_action(agent, action, target, allowed, detail);
    }
}
```

`git_layer: Option<Arc<GitLayer>>` 필드를 `AccessManager`에 추가.

### 4.3 Orchestrator → GitLayer

```rust
// seed 저장 후 태그
self.state_store.save_json("seeds", &key, seed).await?;
if let Some(git) = &self.git_layer {
    git.commit_file(&format!("seeds/{}.json", key), &format!("seed: {}", seed.goal))?;
    git.tag(&format!("seed-{}", seed.id), &format!("Seed: {}", seed.goal))?;
}
```

### 4.4 Kernel 초기화

```rust
// kernel.rs의 KernelBuilder::build()
let git_layer = Arc::new(GitLayer::new(
    workspace_dir.clone(),
    config.git.auto_commit,
)?);

// StateStore는 그대로
let state_store = Arc::new(StateStore::new(workspace_dir)?);

// GitLayer를 Kernel에 저장
Kernel {
    git_layer: git_layer.clone(),
    orchestrator: { /* orchestrator에 git_layer 주입 */ },
    access_manager: { /* access_manager에 git_layer 주입 */ },
    // ...
}
```

---

## 5. 설정

```toml
[git]
auto_commit = true
```

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GitConfig {
    #[serde(default = "default_true")]
    pub auto_commit: bool,
}

impl Default for GitConfig {
    fn default() -> Self {
        Self { auto_commit: true }
    }
}
```

리뷰 M3 반영: `separate_audit_commits` 제거.

---

## 6. API 엔드포인트

| Method | Path | 설명 |
|--------|------|------|
| `GET` | `/api/git/log` | 커밋 로그 (최대 100개) |
| `GET` | `/api/git/tags` | 태그 목록 |
| `GET` | `/api/git/status` | 현재 HEAD 정보 |
| `POST` | `/api/git/restore` | 파일 복원 |
| `POST` | `/api/git/verify` | 무결성 검증 |

리뷰 M2 반영: `restore_all` 제거. `restore_file`만.

---

## 7. 파일 구조

```
crates/oxios-kernel/src/
├── git_layer.rs          # GitLayer (신규, ~300줄)
├── config.rs             # GitConfig 추가
├── access_manager.rs     # git_layer 필드 + log_action에서 호출
├── orchestrator.rs       # git_layer 필드 + 태그 생성
└── lib.rs                # pub mod git_layer; + exports

src/kernel.rs             # git_layer 필드 + save_and_commit()
channels/oxios-web/src/routes/
├── git_routes.rs         # API 핸들러 (신규, ~80줄)
└── mod.rs                # 라우트 등록
```

---

## 8. 리뷰 반영 요약

| # | 원래 이슈 | 해결 방법 |
|---|----------|----------|
| C1 | save_json마다 fork 오버헤드 | gix in-process → fork 없음 |
| C2 | parking_lot::Mutex + .await | gix는 동기 API → .await 없음 |
| C3 | StateStore 54개 호출 지점 수정 | StateStore 수정 없음, Kernel에서 연결 |
| M1 | *.log가 audit 제외 | audit 확장자를 .audit로 변경 |
| M2 | restore_all 위험 | restore_file만 제공 |
| M3 | separate_audit_commits 불필요 | 설정에서 제거 |

---

## 9. 테스트 계획

| 테스트 | 대상 |
|--------|------|
| `test_init_new_repo` | 빈 디렉토리 → git init |
| `test_init_existing_repo` | 이미 .git 있으면 open |
| `test_commit_file` | 파일 쓰기 → 커밋 → 로그에 나타남 |
| `test_commit_files_batch` | 여러 파일 동시 커밋 |
| `test_commit_no_changes` | 변경 없으면 커밋 스킵 (또는 noop) |
| `test_remove_file` | 파일 삭제 커밋 |
| `test_log_action` | 감사 로그 기록 + 커밋 |
| `test_log_query` | log() 로그 조회 |
| `test_tag_create_list` | 태그 생성 + 목록 |
| `test_restore_file` | 이전 커밋에서 파일 복원 |
| `test_verify` | 무결성 검증 |
| `test_disabled_noop` | enabled=false면 커밋 스킵 |

---

## 10. 크기 추정

| 항목 | 라인 수 |
|------|---------|
| `git_layer.rs` | ~300 |
| `git_routes.rs` | ~80 |
| 통합 (kernel, orchestrator, access_manager) | ~60 |
| config | ~15 |
| 테스트 | ~180 |
| **총계** | **~635** |

소요: 1일
