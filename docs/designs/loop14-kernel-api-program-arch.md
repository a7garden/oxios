# Loop 14: Kernel API → Program Architecture

> **버전:** v0.2.0-alpha  
> **작성일:** 2026-05-06  
> **원칙:** "Kernel provides syscalls. Programs compose them."

---

## 1. Unix vs Oxios 구조 비교

### Unix

```
Kernel (syscalls)           Program (compositions)
─────────────────           ─────────────────────
open/read/write    →  cat:  read file → write stdout
fork/exec/wait     →  sh:   fork → exec → pipe → wait
socket/bind/listen →  httpd: accept → fork → read → write → close
mmap/brk           →  malloc: mmap → manage heap
```

Kernel은 **기본 연산**만 제공. Program은 그걸 **조합**해서 고급 동작 생성.

### Oxios (현재)

```
Kernel modules              Direct usage only
─────────────────           ─────────────────────
save_and_commit    →  (직접 호출만)
spawn_agent        →  (직접 호출만)
schedule_task      →  (직접 호출만)
git_commit         →  (직접 호출만)
```

**문제:** Program이 kernel 내부 구조를 다 알아야 함. 진정한 OS-like 구조가 아님.

### Oxios (목표)

```
Kernel API (syscalls)                Programs (compositions)
─────────────────────                ───────────────────────
kernel.save()             →  git-sync: cron → save → commit → tag
kernel.spawn()            →  code-review: spawn → remember → commit
kernel.schedule()         →  deploy: exec_container → schedule → audit
kernel.remember()         →  monitor: resource_check → budget → alert
kernel.commit()           →  
kernel.send()             →  pipeline: spawn(A) → send → spawn(B) → send
kernel.exec()             →  
kernel.audit()            →  guardian: audit → verify → alert on tamper
kernel.query_memory()     →  
```

---

## 2. Kernel Syscall API

Kernel이 **오직** 이 API만 노출. 내부 구조는 숨김.

```rust
impl Kernel {
    // ── State ────────────────────────────────────────
    /// Save data with version control.
    async fn save(&self, category: &str, name: &str, data: &Value) -> Result<()>;
    
    /// Load data.
    async fn load(&self, category: &str, name: &str) -> Result<Option<Value>>;
    
    /// Delete data.
    async fn delete(&self, category: &str, name: &str) -> Result<bool>;
    
    // ── Agent ────────────────────────────────────────
    /// Spawn an agent with a task.
    async fn spawn(&self, task: &str, persona: Option<&str>) -> Result<AgentId>;
    
    /// Send message to a running agent.
    async fn send(&self, agent_id: &AgentId, message: &str) -> Result<()>;
    
    /// Wait for agent completion.
    async fn wait(&self, agent_id: &AgentId) -> Result<AgentResponse>;
    
    /// Kill a running agent.
    async fn kill(&self, agent_id: &AgentId) -> Result<()>;
    
    // ── Scheduling ───────────────────────────────────
    /// Schedule a recurring task.
    async fn schedule(&self, cron_expr: &str, task: &str, persona: Option<&str>) -> Result<String>;
    
    /// Cancel a scheduled task.
    async fn unschedule(&self, job_id: &str) -> Result<bool>;
    
    // ── Memory ───────────────────────────────────────
    /// Store a memory entry.
    async fn remember(&self, category: &str, content: &str, tags: Vec<&str>) -> Result<String>;
    
    /// Query memory.
    async fn recall(&self, query: &str, limit: usize) -> Result<Vec<MemoryEntry>>;
    
    // ── Git ──────────────────────────────────────────
    /// Commit current changes.
    fn commit(&self, message: &str) -> Result<()>;
    
    /// Tag current state.
    fn tag(&self, name: &str, message: &str) -> Result<()>;
    
    /// Restore a file from history.
    fn restore(&self, path: &str, hash: &str) -> Result<()>;
    
    /// Get commit log.
    fn log(&self, max: usize) -> Result<Vec<LogEntry>>;
    
    // ── Container ────────────────────────────────────
    /// Execute command in container.
    async fn exec(&self, container: &str, command: &str) -> Result<ExecResult>;
    
    // ── Communication ────────────────────────────────
    /// Send event to all subscribers.
    fn broadcast(&self, event: KernelEvent);
    
    /// Subscribe to kernel events.
    fn subscribe(&self) -> broadcast::Receiver<KernelEvent>;
    
    // ── Audit ────────────────────────────────────────
    /// Audit an action.
    fn audit(&self, action: AuditAction, resource: &str) -> HashDigest;
    
    /// Verify audit chain.
    fn verify_audit(&self) -> Result<bool>;
    
    // ── Resources ────────────────────────────────────
    /// Get current resource snapshot.
    fn resources(&self) -> ResourceSnapshot;
    
    /// Check if budget allows operation.
    fn check_budget(&self, agent_id: &AgentId, tokens: u64) -> bool;
    
    // ── Programs ─────────────────────────────────────
    /// Install a program.
    async fn install_program(&self, source: &str) -> Result<String>;
    
    /// List installed programs.
    fn list_programs(&self) -> Vec<ProgramMeta>;
    
    /// Run a program.
    async fn run_program(&self, name: &str, args: &Value) -> Result<Value>;
}
```

---

## 3. Program 실행 모델

### 프로그램 정의

```rust
/// A program that composes kernel syscalls.
pub trait Program {
    /// Program metadata.
    fn meta(&self) -> &ProgramMeta;
    
    /// Execute the program with kernel access and arguments.
    async fn run(&self, kernel: &Kernel, args: &Value) -> Result<Value>;
}
```

### 예: git-sync 프로그램

```rust
pub struct GitSyncProgram;

impl Program for GitSyncProgram {
    fn meta(&self) -> &ProgramMeta {
        &ProgramMeta {
            name: "git-sync",
            version: "1.0.0",
            description: "Periodically commit and tag state snapshots",
            tools: {},  // Uses kernel API, not tools
            host_requirements: HostRequirements::default(),
        }
    }
    
    async fn run(&self, kernel: &Kernel, args: &Value) -> Result<Value> {
        let interval_mins = args["interval_mins"].as_u64().unwrap_or(60);
        let tag_prefix = args["tag_prefix"].as_str().unwrap_or("auto-sync");
        
        // Schedule recurring commit
        let cron_expr = format!("*/{} * * * *", interval_mins);
        let job_id = kernel.schedule(&cron_expr, "git-auto-sync", None).await?;
        
        // Commit all pending changes
        kernel.commit(&format!("auto-sync: {}", chrono::Utc::now()))?;
        
        // Tag with timestamp
        let tag = format!("{}-{}", tag_prefix, chrono::Utc::now().format("%Y%m%d-%H%M%S"));
        kernel.tag(&tag, "automatic sync tag")?;
        
        // Audit
        kernel.audit(AuditAction::GitCommit { message: "auto-sync".into() }, "git-sync");
        
        Ok(json!({ "job_id": job_id, "tag": tag }))
    }
}
```

### 예: code-review 프로그램

```rust
pub struct CodeReviewProgram;

impl Program for CodeReviewProgram {
    fn meta(&self) -> &ProgramMeta {
        &ProgramMeta {
            name: "code-review",
            version: "1.0.0",
            description: "Spawn an agent to review code changes, remember feedback",
            tools: {},
            host_requirements: HostRequirements::default(),
        }
    }
    
    async fn run(&self, kernel: &Kernel, args: &Value) -> Result<Value> {
        let repo_path = args["repo_path"].as_str().unwrap_or(".");
        
        // Get recent changes
        let log = kernel.log(10)?;
        
        // Spawn reviewer agent
        let agent_id = kernel.spawn(
            &format!("Review the following recent commits: {:?}", log),
            Some("code-reviewer"),
        ).await?;
        
        // Wait for review
        let response = kernel.wait(&agent_id).await?;
        
        // Remember the review
        let mem_id = kernel.remember("reviews", &response.content, vec!["review", "code"]).await?;
        
        // Commit the memory
        kernel.commit("code-review: save review memory")?;
        
        // Audit
        kernel.audit(AuditAction::AgentSpawn { task_type: "code-review".into() }, "code-review");
        
        Ok(json!({ "review": response.content, "memory_id": mem_id }))
    }
}
```

---

## 4. vs 현재 ProgramManager

### 현재

```rust
pub struct ProgramManager {
    programs: HashMap<String, Program>,
    program_dir: PathBuf,
}

// Program은 디렉토리 + SKILL.md + program.toml
// ProgramTool이 agent에게 노출됨
```

현재 Program은 **에이전트에게 툴로 노출**되는 것. Kernel API를 조합하는 게 아님.

### 목표

```
현재 Program = "에이전트 툴" (SKILL.md 기반)
목표 Program = "커널 syscall 조합" (Rust trait 기반)
```

둘 다 필요:
- **Tool Programs**: 에이전트가 사용하는 툴 (현재 방식, SKILL.md)
- **System Programs**: 커널 syscall을 조합한 자동화 (새로운 방식, Rust trait)

### 분류

```
Program
├── Tool Program (SKILL.md + bin/)
│   └── agent가 tool로 사용 (예: web-search, file-manager)
├── System Program (impl Program trait)
│   └── kernel이 실행 (예: git-sync, code-review, deploy)
└── WASM Program (.wasm)
    └── 샌드박스에서 실행 (예: untrusted plugin)
```

---

## 5. 구현 계획

### Phase 1: Kernel syscall API 정리 (현재)
- `impl Kernel`에 syscall 메서드 정리
- 내부 필드 노출 최소화 (pub → pub(crate))
- Kernel이 유일한 진입점

### Phase 2: System Program trait
```rust
pub trait SystemProgram: Send + Sync {
    fn meta(&self) -> &ProgramMeta;
    async fn run(&self, kernel: &Kernel, args: Value) -> Result<Value>;
}
```

### Phase 3: 빌트인 System Programs
- `git-sync` — 주기적 커밋/태그
- `code-review` — 에이전트 스폰 + 리뷰 + 메모리
- `deploy` — 컨테이너 빌드 + 실행 + 감사
- `guardian` — 감사 검증 + 예산 체크 + 알림
- `pipeline` — 멀티 에이전트 파이프라인

### Phase 4: Program Registry
```rust
pub struct ProgramRegistry {
    tool_programs: ProgramManager,      // SKILL.md 기반
    system_programs: HashMap<String, Box<dyn SystemProgram>>,
    wasm_programs: WasmSandbox,         // WASM 기반
}

impl ProgramRegistry {
    pub async fn run(&self, name: &str, kernel: &Kernel, args: Value) -> Result<Value>;
}
```

---

## 6. 철학

> **"Kernel은 syscall만 제공한다. Program은 syscall을 조합한다."**
>
> — Unix 철학, Oxios에 적용

이 구조가 주는 이점:
1. **단순성** — Kernel은 작고 테스트 가능한 단위
2. **조합성** — Program = syscall 조합, 무한 확장
3. **격리** — Program은 Kernel API만 본다, 내부 구조 모름
4. **테스트** — Kernel API mock → Program 테스트 용이
5. **보안** — Kernel이 권한 관리, Program은 제한된 API만 사용
