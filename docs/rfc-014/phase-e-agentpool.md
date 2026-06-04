# Phase E: AgentPool SDK 도입

> **위험**: 중간 (supervisor 코어 변경)
> **예상 시간**: 2시간
> **선행**: Phase A + oxi-sdk에서 AgentPool 활성화 (dormant 문서 Step 2)

---

## 전제 조건

oxi-sdk의 `lifecycle/agent_pool.rs`가 활성화되어 `oxi_sdk::AgentPool`을
사용할 수 있어야 한다. `export_state()` stub이 실 구현으로 교체되어야
oxios의 기능이 온전히 보존된다.

→ oxi 측 `sdk-dormant-modules-activation.md` Step 2 + Section 6

## 현재 상태

```rust
// supervisor.rs 내부에 AgentPool 정의 (인라인)
pub struct AgentPool {
    agents: RwLock<HashMap<AgentId, Arc<Agent>>>,
}
```

AgentPool은 supervisor.rs에 종속적이고, supervisor는 AgentRuntime, EventBus,
ResourceMonitor에 결합되어 있다.

## 변경 내용

### 1. SDK AgentPool로 교체

```rust
// supervisor.rs
// 변경 전
use crate::supervisor::AgentPool;

// 변경 후
pub use oxi_sdk::AgentPool;
```

### 2. Oxios의 AgentId 타입과 SDK의 String 키 변환

oxios의 `AgentId`는 `Uuid` 타입이다. SDK의 `AgentPool`은 `String` 키를 사용한다.

```rust
// 변환 헬퍼
impl BasicSupervisor {
    fn pool_key(id: &AgentId) -> String {
        id.to_string()
    }
}
```

### 3. export_state / import_state 동등성 확인

SDK의 AgentPool이 `Agent::export_state()`를 호출하는지 확인.
oxios의 기존 동작:

```rust
// oxios 기존 — 대화 이력 전체 직렬화
pub fn export_state(&self, id: &AgentId) -> Option<serde_json::Value> {
    self.agents.read().get(id).and_then(|agent| agent.export_state().ok())
}
```

SDK에서 stub이 실 구현으로 교체되면 동일하게 동작함.

### 4. Supervisor는 oxios 고유 구현 유지

`BasicSupervisor`는 `AgentRuntime`, `EventBus<KernelEvent>`,
`ResourceMonitor`에 결합되어 있으므로 SDK의 `AgentSupervisor`로
교체하지 않는다. AgentPool만 SDK에서 가져온다.

## 변경 파일

| 파일 | 변경 |
|------|------|
| `supervisor.rs` | `AgentPool` 정의 삭제, SDK re-export |

## 검증 기준

- [ ] `cargo test -p oxios-kernel` 통과
- [ ] `AgentPool::export_state()`가 실제 대화 이력을 반환
- [ ] `AgentPool::import_state()`가 상태를 올바르게 복원
