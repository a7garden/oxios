# Loop 15: Application Layer + Production Hardening

> **목표:** Application 레이어 실질화 + 프로덕션 검증
> **전제:** ARCHITECTURE.md 4-layer 구조 기반

---

## 1. 현재 상태 요약

```
Layer        Score   Issues
───────────  ──────  ───────────────────────────────────────
Terminal      70%    Telegram basic, CLI 최소
Application   30%    Built-in 프로그램 0개, System Call 조합 없음
Kernel        85%    WASM/OTel stub, inner_xxx getter 15개 잔존
Runtime       90%    충분
Engine        95%    외부 의존성
```

---

## 2. 설계: Built-in Applications

### 2.1 Application = program.toml + SKILL.md

현재 ProgramManager는 이미 이 구조를 지원한다. 필요한 건 **실제 프로그램 파일**을 만드는 것뿐이다.

```
programs/
├── code-review/
│   ├── program.toml
│   └── SKILL.md
├── deploy/
│   ├── program.toml
│   └── SKILL.md
└── guardian/
    ├── program.toml
    └── SKILL.md
```

### 2.2 code-review 프로그램

```toml
# programs/code-review/program.toml
[program]
name = "code-review"
version = "1.0.0"
description = "Deep code review with memory and audit trail"
author = "oxios"

[requires_tools]
names = ["read", "grep", "bash"]

[host_requirements]
required = []
optional = ["gh"]

[kernel_api]
allow = ["git_log", "spawn", "wait", "remember", "recall", "commit", "audit", "save"]
```

```markdown
# Code Review

## Purpose
Automated code review that analyzes recent changes, generates feedback,
and stores results in agent memory for future reference.

## Usage
When the user asks to review code:
1. Use `git_log` to get recent commits (max 10)
2. Analyze the changes — look for bugs, style issues, security concerns
3. Provide structured feedback with severity levels
4. Remember the review results for future context

## Workflow
1. Read recent commit history
2. For each commit, examine changed files
3. Review code quality, security, performance
4. Generate structured review with findings
5. Save review to memory under "reviews" category
6. Commit the review artifact

## Review Criteria
- **Critical**: Security vulnerabilities, data loss risks
- **Important**: Logic errors, race conditions, resource leaks
- **Minor**: Style inconsistencies, naming, documentation gaps
- **Nit**: Formatting preferences
```

### 2.3 deploy 프로그램

```toml
# programs/deploy/program.toml
[program]
name = "deploy"
version = "1.0.0"
description = "Safe deployment with pre-flight checks and rollback"
author = "oxios"

[requires_tools]
names = ["bash", "read"]

[host_requirements]
required = []
optional = ["gh", "docker"]

[container]
minimal_tools = ["bash", "curl", "jq"]
```

```markdown
# Deploy

## Purpose
Safe deployment workflow with automated testing, tagging, and rollback capability.

## Usage
When the user asks to deploy:
1. Run tests in container
2. Verify all tests pass
3. Create a git tag with version
4. Build and deploy
5. Audit the deployment

## Workflow
1. Run test suite via container exec
2. Verify test results
3. Tag the current state with version
4. Execute deployment steps
5. Save deployment record
6. Audit the entire deployment

## Rollback
If deployment fails:
1. Use `git_restore` to revert to previous tag
2. Audit the rollback
3. Notify via the channel
```

### 2.4 guardian 프로그램

```toml
# programs/guardian/program.toml
[program]
name = "guardian"
version = "1.0.0"
description = "Background daemon for audit verification, resource monitoring, and budget enforcement"
author = "oxios"

[requires_tools]
names = []

[host_requirements]
required = []
optional = []

# Guardian is special — it runs as a background daemon, not an agent tool
[daemon]
interval_secs = 300
```

```markdown
# Guardian

## Purpose
Background daemon that periodically verifies system integrity,
monitors resources, and enforces budgets.

## Checks (every 5 minutes)
1. Audit chain integrity: verify_audit()
2. Resource overload: is_overloaded()
3. Budget status: check_budget() for all agents
4. Git state: git_verify()

## Alerts
If any check fails, broadcast a KernelEvent alert.
All check results are audit-logged.

## Not an Agent Tool
Guardian runs as a tokio::spawn background task.
It does not need agent tools — it only uses Kernel System Calls.
```

---

## 3. 설계: Guardian Daemon

Guardian은 특수한 Application — 백그라운드 데몬.

```rust
// src/main.rs — KernelBuilder::build() 끝에 추가

/// Start background guardian daemon.
fn start_guardian(kernel: &Kernel) {
    let handle = kernel.handle();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(300)).await;
            
            // 1. Audit chain integrity
            if let Ok(valid) = handle.verify_audit() {
                if !valid {
                    handle.audit("guardian", 
                        AuditAction::Other { detail: "AUDIT CHAIN BROKEN".into() }, 
                        "guardian");
                }
            }
            
            // 2. Resource check
            let snapshot = handle.resource_snapshot();
            if handle.is_overloaded() {
                handle.audit("guardian",
                    AuditAction::Other { detail: format!("OVERLOADED: cpu={:.1}%", snapshot.cpu_percent) },
                    "guardian");
            }
            
            // 3. Git integrity
            if let Ok(valid) = handle.git_verify() {
                if !valid {
                    handle.audit("guardian",
                        AuditAction::Other { detail: "GIT REPOSITORY CORRUPTED".into() },
                        "guardian");
                }
            }
            
            // 4. Periodic commit
            let _ = handle.commit_all("guardian: periodic checkpoint");
        }
    });
}
```

---

## 4. 설계: E2E 테스트 파이프라인

### 4.1 테스트 구조

```
tests/
├── e2e_real_pipeline.rs    ← 이미 있음 (LLM 필요)
├── e2e_kernel.rs           ← NEW: Kernel 통합 테스트
├── e2e_system_calls.rs     ← NEW: System Call 전체 검증
└── e2e_git_layer.rs        ← NEW: Git-as-Kernel 검증
```

### 4.2 e2e_kernel.rs

```rust
//! Kernel assembly + System Call integration test.
//! Runs without LLM — uses mock provider.

#[tokio::test]
async fn test_kernel_system_calls() {
    let kernel = build_test_kernel().await;
    
    // State
    kernel.save_and_commit("test", "item", &json!({"key": "value"})).await.unwrap();
    let loaded: Option<Value> = kernel.load("test", "item").await.unwrap();
    assert!(loaded.is_some());
    
    // Git
    let log = kernel.git_log(10).unwrap();
    assert!(!log.is_empty()); // At least the save commit
    
    // Audit
    let count_before = kernel.audit_count();
    kernel.audit("test", AuditAction::Other { detail: "test".into() }, "test");
    assert!(kernel.audit_count() > count_before);
    assert!(kernel.verify_audit().unwrap());
    
    // Budget
    let agent_id = AgentId::new_v4();
    kernel.budget_manager.set_budget(BudgetLimit { ... });
    assert!(kernel.check_budget(&agent_id).tokens_remaining > 0);
    
    // Resources
    let snapshot = kernel.resource_snapshot();
    assert!(snapshot.cpu_percent >= 0.0);
    
    // Cleanup
    kernel.delete_and_commit("test", "item").await.unwrap();
}
```

### 4.3 e2e_system_calls.rs

```rust
//! Verify ALL 133 System Call methods work.

#[tokio::test]
async fn test_all_system_calls_exist() {
    let kernel = build_test_kernel().await;
    let handle = kernel.handle();
    
    // State
    handle.save_and_commit("test", "sc", &json!({})).await.unwrap();
    handle.load::<Value>("test", "sc").await.unwrap();
    handle.list_category("test").await.unwrap();
    handle.delete_and_commit("test", "sc").await.unwrap();
    
    // Git
    handle.git_log(1).unwrap();
    handle.git_tags().unwrap();
    handle.git_verify().unwrap();
    handle.commit_all("test").unwrap();
    
    // Audit
    handle.audit("test", AuditAction::Other { detail: "test".into() }, "test");
    handle.verify_audit().unwrap();
    handle.query_audit(0, 100);
    handle.audit_count();
    
    // Budget
    handle.check_budget(&AgentId::new_v4());
    handle.set_budget(BudgetLimit { agent_id: AgentId::new_v4(), token_budget: 100, calls_budget: 10, window_secs: 3600 });
    handle.is_overloaded();
    handle.resource_snapshot();
    
    // Events
    handle.subscribe();
    
    // System
    handle.get_config();
    handle.uptime();
    handle.list_programs().await;
}
```

---

## 5. 설계: KernelHandle 캡슐화 완성

### 5.1 inner_xxx getter 제거

현재 15개 getter가 내부 구조를 노출. 모두 적절한 System Call로 교체:

```
제거할 것:                     대체 System Call:
─────────────────────────────  ─────────────────────────────
inner_container_manager()      start/stop/remove/exec_container()
inner_program_manager()        install/uninstall/list/get_program()
inner_memory_manager()         remember/search/memory_stats()
inner_resource_monitor()       snapshot/history/is_overloaded()
inner_scheduler()              scheduler_stats/rate_remaining()
inner_skill_store()            list_skills()
skill_store()                  list_skills()
host_tool_validator()          check_host_tools()
access_manager()               audit_log_action()
persona_manager()              list_personas()
mcp_bridge()                   mcp_list/mcp_init/mcp_call_tool()
auth_manager()                 validate_token/add_key()
state_store_base_path()        workspace_path()
scheduler_stats()              (OK — returns value, not Arc)
scheduler_rate_remaining()     (OK — returns value, not Arc)
```

### 5.2 원칙

```
좋은 System Call:  값을 반환 (String, Vec<T>, bool, Struct)
나쁜 getter:       Arc<Subsystem>을 반환 (내부 구조 노출)
```

---

## 6. 설계: 성능 측정

### 6.1 벤치마크 항목

```
bench/
└── kernel_bench.rs

측정 항목:
1. Cold start:  KernelBuilder::build() 시간
2. State save:  save_and_commit() 1000회 평균
3. Git commit:  commit_file() 100회 평균
4. Audit append: audit() 10000회 평균
5. Memory index: remember() 1000회 평균
6. System Call overhead: handle.method() 라운드트립
```

### 6.2 타겟 수치 (OpenFang 비교)

```
항목             OpenFang    Oxios 타겟
──────────────   ─────────   ──────────
Cold start       180ms       < 200ms
Memory (idle)    40MB        < 50MB
API p99          ?           < 10ms
Git commit       N/A         < 5ms (in-process)
```

---

## 7. 설계: CLI 확장

### 7.1 추가 명령어

```
현재:  run, chat, backup, restore, garden, status, config, pkg

추가:
  oxios agent list         ← list_agents()
  oxios agent kill <id>     ← kill_agent()
  oxios audit verify        ← verify_audit()
  oxios audit log           ← query_audit() 최근 20개
  oxios git log             ← git_log(20)
  oxios git tag <name>      ← git_tag()
  oxios budget <agent>      ← check_budget()
  oxios program run <name>  ← 실행 (Application 레이어)
  oxios daemon start        ← guardian 시작
  oxios daemon status       ← 데몬 상태
```

---

## 8. 구현 계획

### Phase 1: Built-in Applications (P0)

| Task | Files | Effort |
|------|-------|--------|
| T1: code-review 프로그램 | programs/code-review/* | Low |
| T2: deploy 프로그램 | programs/deploy/* | Low |
| T3: guardian 프로그램 | programs/guardian/* | Low |
| T4: Guardian daemon tokio::spawn | src/main.rs | Low |
| T5: 프로그램 자동 설치 (KernelBuilder) | src/kernel.rs | Low |

### Phase 2: KernelHandle 캡슐화 완성 (P0)

| Task | Files | Effort |
|------|-------|--------|
| T6: inner_xxx getter → System Call | kernel_handle.rs | Medium |
| T7: Routes 재마이그레이션 | routes/*.rs | Medium |

### Phase 3: E2E 테스트 (P0)

| Task | Files | Effort |
|------|-------|--------|
| T8: e2e_kernel.rs | tests/e2e_kernel.rs | Medium |
| T9: e2e_system_calls.rs | tests/e2e_system_calls.rs | Medium |
| T10: e2e_git_layer.rs | tests/e2e_git_layer.rs | Low |

### Phase 4: 성능 + CLI (P1)

| Task | Files | Effort |
|------|-------|--------|
| T11: 벤치마크 | bench/kernel_bench.rs | Medium |
| T12: CLI 확장 | src/main.rs | Medium |

### Batches

```
Batch 1 (parallel):  [T1, T2, T3]           — 프로그램 파일 생성
Batch 2 (sequential): [T4, T5]               — 데몬 + 자동 설치
Batch 3 (parallel):  [T6, T8, T9, T10]       — 캡슐화 + E2E 테스트
Batch 4 (parallel):  [T7, T11, T12]           — 라우트 + 벤치마크 + CLI
```

---

## 9. 완료 기준

- [ ] 3개 built-in 프로그램 설치 가능 (code-review, deploy, guardian)
- [ ] Guardian 데몬 백그라운드 실행
- [ ] KernelHandle에 inner_xxx getter 0개
- [ ] E2E 테스트 3개 통과
- [ ] 성능 벤치마크 수치 확보
- [ ] CLI 명령어 9개 추가
- [ ] Dead code warnings 0개
- [ ] `cargo check --workspace` zero errors, zero warnings
