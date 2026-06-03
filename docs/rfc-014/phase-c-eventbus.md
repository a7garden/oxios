# Phase C: EventBus 제네릭 전환

> **위험**: 중간 (10개 파일 import 경로 변경)
> **예상 시간**: 2시간
> **선행**: Phase A + oxi-sdk에서 EventBus 활성화 (dormant 문서 Step 1)

---

## 전제 조건

oxi-sdk의 `event_bus.rs`가 활성화되어 `oxi_sdk::EventBus<E>`를 사용할 수 있어야 한다.
→ oxi 측 `sdk-dormant-modules-activation.md` Step 1이 완료되어야 함.

## 현재 상태

```rust
// crates/oxios-kernel/src/event_bus.rs (595줄)
// 자체 구현: tokio::broadcast 기반 + 20개 KernelEvent variant + audit 연동
```

## 목표 상태

```rust
// crates/oxios-kernel/src/event_bus.rs (~120줄)
// SDK의 EventBus<KernelEvent>를 사용, KernelEvent 정의만 유지

use oxi_sdk::EventBus;

pub type KernelEventBus = EventBus<KernelEvent>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KernelEvent {
    // 20개 variant 그대로
    AgentCreated { id: AgentId, name: String },
    AgentStarted { id: AgentId },
    AgentStopped { id: AgentId },
    AgentFailed { id: AgentId, error: String },
    MessageReceived { from: String, content: String },
    AgentOutput { session_id: String, agent_id: String, output: String },
    ApprovalRequested { id: String, action: String, resource: String, reason: String },
    ApprovalResolved { id: String, approved: bool },
    SeedCreated { seed_id: Uuid },
    EvaluationComplete { seed_id: Uuid, passed: bool },
    PhaseStarted { session_id: String, phase: String },
    PhaseCompleted { session_id: String, phase: String, success: bool },
    EvolutionStarted { session_id: String, generation: u32 },
    EvolutionMaxReached { session_id: String },
    AgentGroupCreated { group_id: String, agent_count: usize },
    AgentGroupMemberCompleted { group_id: String, agent_id: String },
    ProjectCreated { project_id: String, name: String },
    ProjectActivated { project_id: String },
    MemoryStored { agent_id: String, entry_id: String },
    MemoryRecalled { agent_id: String, query: String, count: usize },
}
```

## 변경 파일 목록

| 파일 | 변경 |
|------|------|
| `event_bus.rs` | 전면 재작성 (SDK EventBus 사용) |
| `kernel_handle/mod.rs` | import 경로 변경 |
| `kernel_handle/agent_api.rs` | import 경로 변경 |
| `kernel_handle/infra_api.rs` | import 경로 변경 |
| `orchestrator.rs` | import 경로 변경 |
| `agent_lifecycle.rs` | import 경로 변경 |
| `a2a/mod.rs` | import 경로 변경 |
| `project/manager.rs` | import 경로 변경 |
| `tools/a2a_tools.rs` | import 경로 변경 |
| `supervisor.rs` | import 경로 변경 |

## audit 연동 처리

현재 event_bus.rs에는 `attach_audit_trail()` 기능이 있다.
SDK의 EventBus는 제네릭이라 이 기능이 없다.

해결: oxios-kernel에 audit 연동 헬퍼를 별도로 둔다.

```rust
// event_bus.rs에 추가
use crate::audit_trail::AuditTrail;

/// AuditTrail에 커널 이벤트를 기록하는 백그라운드 태스크를 시작한다.
pub fn attach_audit_trail(bus: &KernelEventBus, trail: Arc<AuditTrail>) {
    let mut rx = bus.subscribe();
    tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            if let Some(action) = kernel_event_to_audit_action(&event) {
                trail.append("kernel".into(), action, "event_bus".into());
            }
        }
    });
}

fn kernel_event_to_audit_action(event: &KernelEvent) -> Option<crate::audit_trail::AuditAction> {
    match event {
        KernelEvent::AgentCreated { name, .. } => Some(
            crate::audit_trail::AuditAction::AgentSpawn { task_type: name.clone() }
        ),
        KernelEvent::AgentFailed { error, .. } => Some(
            crate::audit_trail::AuditAction::AgentExit { reason: error.clone() }
        ),
        // ... 나머지 매핑
        _ => None,
    }
}
```

## 검증 기준

- [ ] `cargo build -p oxios-kernel` 성공
- [ ] `cargo test -p oxios-kernel` 통과
- [ ] `KernelEvent` 직렬화/역직렬화 동일 (serde 호환)
- [ ] 기존 event_bus 구독자가 정상 동작
