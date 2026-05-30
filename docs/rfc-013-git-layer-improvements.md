# RFC-013: GitLayer 개선 — 버그 수정, 에이전트 추적, Diff 기능

> **날짜**: 2026-05-30
> **상태**: 초안
> **범위**: `crates/oxios-kernel/src/git_layer.rs`, 호출처 6개
> **관련**: AGENTS.md `GitLayer`, Ouroboros evaluate 단계

---

## 1. 배경

### 현재 문제

GitLayer는 581줄짜리 gix 기반 버전 관리 계층으로, Supervisor·Orchestrator·MemoryManager·CronScheduler·SecurityApi·StateApi에서 사용합니다.

분석 결과 **2개 버그**와 **3개 설계 개선점**이 있습니다.

#### 버그

| # | 문제 | 심각도 | 증상 |
|---|------|--------|------|
| B1 | `self_signature_ref()` 타임스탬프가 프로세스당 1회 고정 | **P0** | 모든 커밋이 동일한 타임스탬프. 오디트·디버깅 불가 |
| B2 | `restore_file()`이 중첩 경로(`audit/2024-05.audit`)를 처리 못함 | **P0** | `log_action()`이 쓴 audit 파일 복원 불가 |

#### 설계 개선

| # | 개선 | 우선순위 | 근거 |
|---|------|----------|------|
| D1 | 에이전트별 author 식별 | **P1** | 멀티 에이전트에서 누가 뭘 커밋했는지 추적 불가 |
| D2 | Diff 기능 | **P1** | Ouroboros evaluate에서 변경 비교 불가 |
| D3 | 사소한 수정 (hex 왕복, tag 필터) | **P3** | 코드 품질 |

### 호출처 현황

```
commit_file() / commit_files() 호출 경로:
├── Orchestrator::save_seed()          → "ourobors: save seed"
├── Orchestrator::delegate_via_lifecycle() → "orchestrator: save group"
├── MemoryManager::git_commit()        → 메모리 저장
├── CronScheduler::save_jobs()         → "cron: update jobs"
├── SecurityApi::flush()               → "audit trail flush"
└── StateApi::commit_all()             → 일반 상태 저장
```

---

## 2. 설계

### B1: 타임스탬프 버그 수정

**원인**: `self_signature_ref()`가 `OnceLock<String>`에 최초 1회만 타임스탬프를 캐시합니다.

```rust
// 현재 (버그)
fn self_signature_ref() -> gix::actor::SignatureRef<'static> {
    static TIME_BUF: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    let time_str = TIME_BUF.get_or_init(|| gix::date::Time::now_local_or_utc().to_string());
    // ...
}
```

`SignatureRef<'a>`의 필드는 `&'a str`이라 직접 로컬 변수를 참조할 수 없습니다. `commit_as_inner` 시그니처는 `SignatureRef<'_>` (어떤 라이프타임이든 허용)이므로, 소유권 구조체를 만들면 됩니다.

**해결**: 소유권 기반 `Signature` 래퍼 도입.

```rust
/// 소유권 기반 서명. 생성 시점의 타임스탬프를 캡처합니다.
struct Signature {
    name: String,
    email: String,
    time: String,
}

impl Signature {
    fn new(name: impl Into<String>, email: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            email: email.into(),
            time: gix::date::Time::now_local_or_utc().to_string(),
        }
    }

    /// gix API에 전달할 수 있는 레퍼런스를 생성합니다.
    /// self가 살아있는 동안만 유효합니다.
    fn as_ref(&self) -> gix::actor::SignatureRef<'_> {
        gix::actor::SignatureRef {
            name: self.name.as_str().into(),
            email: self.email.as_str().into(),
            time: &self.time,
        }
    }
}
```

**사용법**:

```rust
// 기존
let _sig = self_signature_ref();
repo.commit_as(self_signature_ref(), self_signature_ref(), ...)?;

// 변경 후
let sig = Signature::new(&self.committer_name, &self.committer_email);
repo.commit_as(sig.as_ref(), sig.as_ref(), ...)?;
// sig가 이 스코프 안에 살아있으므로 라이프타임 안전
```

`SignatureRef<'static>` 반환 함수 `self_signature_ref()`는 제거합니다.

**영향**: `create_initial_commit`, `commit_file`, `commit_files`, `remove_file`, `tag` — 총 5개 함수. 모두 동일 패턴으로 수정.

---

### B2: 중첩 경로 복원

**원인**: `restore_file()`이 단일 tree의 entries만 평면 검색합니다.

```rust
// 현재 (버그) — "audit/2024-05.audit"을 찾지 못함
let entry = decoded_tree.entries.iter().find(|e| e.filename == rel_bytes);
```

Git의 tree는 계층 구조입니다. `audit/2024-05.audit`은:
1. 최상위 tree → `audit` (subtree entry)
2. `audit` subtree → `2024-05.audit` (blob entry)

**해결**: 경로 컴포넌트별로 tree를 순회하는 헬퍼 추가.

```rust
/// 경로 컴포넌트를 따라 tree를 순회하여 blob ObjectId를 찾습니다.
///
/// `audit/2024-05.audit` → ["audit", "2024-05.audit"]
/// 1. 최상위 tree에서 "audit" subtree 찾기
/// 2. "audit" subtree에서 "2024-05.audit" blob 찾기
fn find_blob_in_tree(
    repo: &gix::Repository,
    tree_id: ObjectId,
    rel_path: &str,
) -> Result<ObjectId> {
    let components: Vec<&BStr> = Path::new(rel_path)
        .iter()
        .map(|c| BStr::new(c.to_str().expect("valid UTF-8 path")))
        .collect();

    ensure!(!components.is_empty(), "Empty path: {}", rel_path);

    let mut current_tree_id = tree_id;

    for (i, component) in components.iter().enumerate() {
        let tree = repo.find_tree(current_tree_id)?;
        let decoded = tree.decode()?;
        let entry = decoded
            .entries
            .iter()
            .find(|e| &e.filename == component)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Path component '{}' not found (in {})",
                    component,
                    rel_path
                )
            })?;

        if i == components.len() - 1 {
            // 마지막 컴포넌트 — blob
            return Ok(entry.oid.to_owned());
        } else {
            // 중간 컴포넌트 — subtree
            current_tree_id = entry.oid.to_owned();
        }
    }

    unreachable!()
}
```

**동일한 헬퍼를 `commit_file`/`commit_files`의 `editor.upsert`에도 적용**:
gix의 `tree::Editor::upsert`는 이미 중첩 경로를 지원합니다 (확인 결과 `editor.upsert("audit/2024-05.audit", ...)`가 정상 동작). 따라서 커밋 쪽은 수정 불필요, 복원만 수정.

---

### D1: 에이전트 식별

**목표**: 커밋 작성자를 `"oxios"`가 아닌 실제 주체로 식별.

**설계**: `commit_file`/`commit_files`에 선택적 `author` 컨텍스트 추가.

```rust
/// 커밋 메타데이터. 호출처에서 에이전트/시스템 정보를 제공합니다.
#[derive(Default)]
pub struct CommitContext {
    /// 에이전트 ID (있으면 "agent-{short_id}", 없으면 "oxios")
    pub agent_id: Option<AgentId>,
    /// 시드 ID (있으면 커밋 메시지에 포함)
    pub seed_id: Option<uuid::Uuid>,
    /// 추가 태그 (예: "memory", "audit", "cron")
    pub tag: Option<&'static str>,
}

impl CommitContext {
    /// 기본 시스템 커밋 (에이전트 컨텍스트 없음)
    pub fn system() -> Self {
        Self::default()
    }

    /// 에이전트 커밋
    pub fn agent(agent_id: AgentId, seed_id: Option<uuid::Uuid>) -> Self {
        Self {
            agent_id: Some(agent_id),
            seed_id,
            tag: None,
        }
    }

    /// 태그가 있는 커밋
    pub fn tagged(tag: &'static str) -> Self {
        Self {
            tag: Some(tag),
            ..Default::default()
        }
    }

    /// 작성자 이름 생성
    fn author_name(&self) -> String {
        match &self.agent_id {
            Some(id) => {
                let hex = id.to_string();
                format!("agent-{}", &hex[..8])
            }
            None => "oxios".to_string(),
        }
    }

    /// 커밋 메시지 접두사 생성
    fn message_prefix(&self) -> String {
        let mut parts = Vec::new();
        if let Some(tag) = self.tag {
            parts.push(format!("[{}]", tag));
        }
        if let Some(ref seed) = self.seed_id {
            let hex = seed.to_string();
            parts.push(format!("[seed-{}]", &hex[..8]));
        }
        if parts.is_empty() {
            String::new()
        } else {
            format!("{} ", parts.join(" "))
        }
    }
}
```

**API 변경** (하위 호환):

```rust
impl GitLayer {
    /// 기존 API (하위 호환) — 시스템 커밋
    pub fn commit_file(&self, rel_path: &str, message: &str) -> Result<CommitInfo> {
        self.commit_file_with(rel_path, message, CommitContext::default())
    }

    /// 확장 API — 컨텍스트 지정
    pub fn commit_file_with(
        &self,
        rel_path: &str,
        message: &str,
        ctx: CommitContext,
    ) -> Result<CommitInfo> {
        // ... 기존 로직 + ctx.author_name()으로 Signature 생성
    }

    /// 기존 API (하위 호환) — 시스템 배치 커밋
    pub fn commit_files(&self, rel_paths: &[&str], message: &str) -> Result<CommitInfo> {
        self.commit_files_with(rel_paths, message, CommitContext::default())
    }

    /// 확장 API — 컨텍스트 지정 배치 커밋
    pub fn commit_files_with(
        &self,
        rel_paths: &[&str],
        message: &str,
        ctx: CommitContext,
    ) -> Result<CommitInfo> {
        // ...
    }
}
```

**호출처 변경** (점진적):

| 호출처 | 현재 | 변경 |
|--------|------|------|
| `Orchestrator::save_seed` | `commit_file(path, msg)` | `commit_file_with(path, msg, CommitContext::tagged("seed"))` |
| `Orchestrator::delegate_via_lifecycle` | `commit_file(path, msg)` | `commit_file_with(path, msg, CommitContext::tagged("group"))` |
| `MemoryManager::git_commit` | `commit_file(path, msg)` | 향후 에이전트 ID 전달 가능 |
| `CronScheduler` | `commit_file(path, msg)` | `CommitContext::tagged("cron")` |
| `SecurityApi::flush` | `commit_file(path, msg)` | `CommitContext::tagged("audit")` |
| `StateApi::commit_all` | `commit_file(path, msg)` | `CommitContext::default()` |

모든 기존 `commit_file(path, msg)` 호출은 하위 호환으로 동작 (기본값 = "oxios").

---

### D2: Diff 기능

**목표**: 두 커밋 간 변경 사항을 비교하여 Ouroboros evaluate와 디버깅에 활용.

**gix 의존성**: 현재 `features = ["tree-editor"]`만 활성화. diff를 위해 `blob-diff` feature가 필요합니다.

```toml
# Cargo.toml 변경
gix = { version = "0.83", features = ["tree-editor", "blob-diff"] }
```

**설계**:

```rust
/// 단일 파일의 변경 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDiff {
    /// 파일 경로
    pub path: String,
    /// 이전 상태 (None = 새로 추가됨)
    pub old_hash: Option<String>,
    /// 이후 상태 (None = 삭제됨)
    pub new_hash: Option<String>,
    /// 변경 종류
    pub kind: DiffKind,
    /// Unified diff 텍스트 (binary면 None)
    pub patch: Option<String>,
}

/// 변경 종류
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DiffKind {
    /// 새 파일
    Added,
    /// 삭제
    Deleted,
    /// 내용 변경
    Modified,
    /// 이름 변경 (선택적 감지)
    Renamed,
}

/// 두 커밋 간 전체 diff
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitDiff {
    /// 비교 출발 커밋
    pub from_hash: String,
    /// 비교 도착 커밋
    pub to_hash: String,
    /// 파일별 diff
    pub files: Vec<FileDiff>,
    /// 통계
    pub stats: DiffStats,
}

/// Diff 통계
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffStats {
    pub files_changed: usize,
    pub additions: usize,
    pub deletions: usize,
}
```

**핵심 메서드**:

```rust
impl GitLayer {
    /// 두 커밋 간 diff.
    pub fn diff_commits(&self, from_hash: &str, to_hash: &str) -> Result<CommitDiff> {
        // 1. 두 커밋의 tree ObjectId 획득
        // 2. 재귀적으로 tree를 비교하여 변경 파일 탐지
        //    - 양쪽 tree를 동시에 순회
        //    - 같은 경로, 다른 blob hash → Modified
        //    - 한쪽에만 있음 → Added/Deleted
        // 3. 변경된 blob pair에 대해 unified diff 생성
        // 4. 통계 집계
    }

    /// HEAD와 워킹 디렉토리 간 diff (단일 파일).
    pub fn diff_working(&self, rel_path: &str) -> Result<FileDiff> {
        // 1. HEAD tree에서 해당 파일의 blob 획득 (find_blob_in_tree)
        // 2. 워킹 디렉토리에서 파일 읽기
        // 3. 두 내용을 비교
    }

    /// 특정 파일의 특정 커밋에서의 내용 조회.
    pub fn file_at_commit(&self, rel_path: &str, hash: &str) -> Result<Vec<u8>> {
        // restore_file과 동일 로직이나, 파일을 쓰지 않고 내용 반환
    }
}
```

**Tree 비교 알고리즘** (gix 직접 사용):

```rust
/// 두 tree를 재귀적으로 비교하여 변경된 파일 목록을 수집합니다.
fn diff_trees(
    repo: &gix::Repository,
    old_tree: ObjectId,
    new_tree: ObjectId,
    prefix: &str,     // 현재 경로 접두사 (예: "audit/")
    changes: &mut Vec<FileDiff>,
) -> Result<()> {
    let old_decoded = repo.find_tree(old_tree)?.decode()?;
    let new_decoded = repo.find_tree(new_tree)?.decode()?;

    let old_entries: HashMap<&BStr, &gix::objs::tree::Entry> =
        old_decoded.entries.iter().map(|e| (&e.filename, e)).collect();
    let new_entries: HashMap<&BStr, &gix::objs::tree::Entry> =
        new_decoded.entries.iter().map(|e| (&e.filename, e)).collect();

    // 추가 + 변경 감지
    for (name, new_entry) in &new_entries {
        let path = format!("{}{}", prefix, name);
        match old_entries.get(name) {
            None => {
                // 새 파일 또는 새 하위 디렉토리
                if new_entry.mode.is_tree() {
                    // 빈 tree로 diff (모든 파일이 Added)
                    let empty = ObjectId::empty_tree(repo.object_hash());
                    diff_trees(repo, empty, new_entry.oid.to_owned(), &format!("{}/", path), changes)?;
                } else {
                    changes.push(FileDiff { path, old_hash: None, new_hash: Some(new_entry.oid.to_hex().to_string()), kind: DiffKind::Added, patch: None });
                }
            }
            Some(old_entry) => {
                if old_entry.oid == new_entry.oid {
                    continue; // 동일 — 변경 없음
                }
                if new_entry.mode.is_tree() && old_entry.mode.is_tree() {
                    // 양쪽 다 subtree — 재귀
                    diff_trees(repo, old_entry.oid.to_owned(), new_entry.oid.to_owned(),
                               &format!("{}/", path), changes)?;
                } else {
                    // Blob 변경
                    changes.push(FileDiff { path, old_hash: Some(old_entry.oid.to_hex().to_string()),
                        new_hash: Some(new_entry.oid.to_hex().to_string()),
                        kind: DiffKind::Modified, patch: None });
                }
            }
        }
    }

    // 삭제 감지
    for (name, old_entry) in &old_entries {
        if !new_entries.contains_key(name) {
            let path = format!("{}{}", prefix, name);
            changes.push(FileDiff { path, old_hash: Some(old_entry.oid.to_hex().to_string()),
                new_hash: None, kind: DiffKind::Deleted, patch: None });
        }
    }

    Ok(())
}
```

**Unified diff 생성**: `similar` 크레이트(이미 gix의 의존성)를 사용하거나, 간단한 라인 기반 diff를 직접 구현.

```rust
/// 두 blob의 unified diff를 생성합니다.
fn compute_unified_diff(old: &[u8], new: &[u8], path: &str) -> Option<String> {
    let old_str = std::str::from_utf8(old).ok()?;
    let new_str = std::str::from_utf8(new).ok()?;

    // similar::TextDiff (gix가 이미 의존하는 similar 크레이트)
    use similar::{TextDiff, ChangeTag};
    let diff = TextDiff::from_lines(old_str, new_str);

    let mut output = String::new();
    output.push_str(&format!("--- a/{}\n+++ b/{}\n", path, path));

    for hunk in diff.unified_diff().iter_changes() {
        match hunk.tag() {
            ChangeTag::Delete => output.push_str(&format!("-{}", hunk)),
            ChangeTag::Insert => output.push_str(&format!("+{}", hunk)),
            ChangeTag::Equal => output.push_str(&format!(" {}", hunk)),
        }
    }

    Some(output)
}
```

> **참고**: `similar`는 gix의 전이적 의존성이지만 직접 의존으로 명시해야 할 수 있습니다. 대안으로 간단한 라인 비교를 직접 구현해도 충분합니다 (Oxios의 diff는 주로 JSON/metadata 파일로, 파일 크기가 작음).

---

### D3: 사소한 수정

#### D3a: hex 왕복 제거

```rust
// 현재 (불필요한 변환)
let decoded = commit.decode()?;
let oid = ObjectId::from_hex(decoded.tree)?;  // ObjectId → bytes → hex string → 다시 ObjectId?

// 수정 — gix commit의 tree 필드 타입에 따라 직접 사용
// decoded.tree는 이미 &ObjectId 또는 변환 가능한 타입
let tree_id = decoded.tree;  // 또는 적절한 메서드
```

> gix 0.83의 `commit_decode()`가 반환하는 구조체의 `tree` 필드 타입을 확인하여 최소 변환 경로를 사용합니다.

#### D3b: tag 필터링 수정

```rust
// 현재 (부정확 — HEAD 같은 non-tag ref도 포함)
if name.starts_with("tags/") || (!name.contains('/') && !name.is_empty()) {

// 수정
// reference.name().shorten()은 "refs/tags/v1" → "tags/v1" 형태로 반환
if let Some(tag_name) = name.strip_prefix("tags/") {
    tags.push(tag_name.to_string());
}
```

---

## 3. API 요약

### 새로운 공개 타입

```rust
// git_layer.rs
pub struct CommitContext { ... }        // 커밋 메타데이터
pub struct FileDiff { ... }            // 파일 diff 결과
pub struct CommitDiff { ... }          // 커밋 간 diff 결과
pub struct DiffStats { ... }           // diff 통계
pub enum DiffKind { ... }              // 변경 종류
```

### 새로운 공개 메서드

```rust
impl GitLayer {
    // D1: 에이전트 식별
    pub fn commit_file_with(&self, rel_path: &str, message: &str, ctx: CommitContext) -> Result<CommitInfo>;
    pub fn commit_files_with(&self, rel_paths: &[&str], message: &str, ctx: CommitContext) -> Result<CommitInfo>;

    // D2: Diff
    pub fn diff_commits(&self, from_hash: &str, to_hash: &str) -> Result<CommitDiff>;
    pub fn diff_working(&self, rel_path: &str) -> Result<FileDiff>;
    pub fn file_at_commit(&self, rel_path: &str, hash: &str) -> Result<Vec<u8>>;
}
```

### 제거

```rust
// B1 수정으로 제거
fn self_signature_ref() -> gix::actor::SignatureRef<'static>;  // → Signature 구조체로 대체
```

### 하위 호환

- `commit_file(path, msg)` 유지 — 내부적으로 `commit_file_with(path, msg, CommitContext::default())` 호출
- `commit_files(paths, msg)` 유지 — 동일
- 기존 호출처는 변경 없이 동작

---

## 4. 구현 순서

```
Phase 1: 버그 수정 (B1 + B2 + D3)
├── B1: Signature 구조체 도입, self_signature_ref 제거
├── B2: find_blob_in_tree 헬퍼, restore_file 수정
├── D3a: head_tree_oid hex 왕복 제거
├── D3b: list_tags 필터 수정
└── 테스트: 타임스탬프 고유성, 중첩 경로 복원, tag 목록 정확성

Phase 2: 에이전트 식별 (D1)
├── CommitContext 타입 정의
├── commit_file_with / commit_files_with 추가
├── 기존 메서드를 래퍼로 변경
├── 호출처에 tag 기반 CommitContext 적용 (Orchestrator, Cron, Security)
└── 테스트: author 필드 검증, 하위 호환

Phase 3: Diff 기능 (D2)
├── Cargo.toml에 "blob-diff" feature 추가
├── DiffKind, FileDiff, CommitDiff, DiffStats 타입 정의
├── diff_trees 재귀 비교 구현
├── compute_unified_diff 구현 (similar 또는 직접 구현)
├── diff_commits, diff_working, file_at_commit 메서드
├── InfraApi에 diff 관련 API 노출
└── 테스트: 커밋 간 diff, 추가/수정/삭제 감지
```

---

## 5. 영향 범위

| 파일 | 변경 유형 | Phase |
|------|-----------|-------|
| `git_layer.rs` | 핵심 구현 (전체) | 1, 2, 3 |
| `Cargo.toml` | gix features 추가 | 3 |
| `kernel_handle/infra_api.rs` | diff API 노출 | 3 |
| `orchestrator.rs` | CommitContext 적용 | 2 |
| `memory/mod.rs` | CommitContext 적용 | 2 |
| `cron.rs` | CommitContext 적용 | 2 |
| `kernel_handle/security_api.rs` | CommitContext 적용 | 2 |

---

## 6. 고려사항

### Mutex 경합

현재 `parking_lot::Mutex<gix::Repository>`로 전체 repo를 보호합니다. Phase 2에서 에이전트별 author를 추가해도 Mutex 경합은 변함없습니다 — 커밋은 이미 에이전트 완료 시점(초~분 단위)에만 발생하므로 실제 병목이 아닙니다.

### similar 의존성

gix의 `blob-diff` feature는 `similar`를 전이적 의존으로 가져옵니다. 직접 `Cargo.toml`에 추가할 필요가 없을 수 있지만, 명시적으로 추가하는 것이 안전합니다.

### Git 호환성

모든 변경은 표준 `.git` 형식을 유지합니다. `git log`, `git diff` CLI로 결과를 확인할 수 있습니다.

### P2: per-agent 브랜치

이 설계에서는 제외합니다. 필요해지면 `GitLayer`에 `create_branch(agent_id)` / `merge_branch(agent_id)` 메서드를 추가하는 것으로 확장 가능합니다.
