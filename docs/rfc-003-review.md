# RFC-003 설계 리뷰

> **리뷰 일시:** 2026-05-20  
> **리뷰 대상:** docs/rfc-003-knowledge-separation.md

---

## 심각도 분류

- 🔴 **Critical**: 구현 불가능하거나 설계 근간을 흔드는 문제
- 🟡 **Important**: 구현은 가능하지만 동작에 영향을 주는 문제
- 🟢 **Minor**: 정리/네이밍 수준

---

## 🔴 Issue 1: 이름 충돌 — `KnowledgeBridge`가 이미 존재한다

**현재 코드에 이미 `KnowledgeBridge`가 있다:**

```
crates/oxios-kernel/src/space/knowledge_bridge.rs
```

이건 Space 간 지식 흐름(Reference/Transfer/Synthesis)을 관리하는 완전히 다른 struct다.
`pub use knowledge_bridge::{CrossRefEntry, KnowledgeBridge, KnowledgeFlow}` 로 lib.rs에 re-export까지 되어 있다.

RFC에서 제안한 `kernel_handle::KnowledgeBridge`와 이름이 충돌한다.

**해결:**

제안하는 새 struct의 이름을 바꾼다. 후보:

| 이름 | 의미 |
|------|------|
| `KnowledgeLens` | 에이전트가 지식 베이스를 "들여다보는" 렌즈 |
| `KnowledgeAgent` | 에이전트 전용 지식 인터페이스 |
| `KnowledgePortal` | 에이전트가 지식 세계로 들어가는 포털 |

기존 `space::KnowledgeBridge`는 지금 하는 일(Space 간 메모리 흐름)이 이름과 잘 안 맞으므로 함께 개명 검토:

```
space::KnowledgeBridge  →  space::MemoryBridge  (또는 SpaceBridge)
kernel_handle::KnowledgeBridge  →  KnowledgeLens (새 이름)
```

---

## 🔴 Issue 2: Semantic Index 동기화 누락

**문제:**

현재 `KnowledgeApi.note_write()`는 파일을 쓰면서 `index_to_memory()`로 MemoryManager 인덱스도 갱신한다.

새 구조에서:

```
사용자가 note_write() 호출
  → KnowledgeBase: .md 저장 + 백링크 갱신
  → 끝. semantic index는 아무도 갱신 안 함.
```

KnowledgeBase는 semantic index의 존재를 모른다. KnowledgeBridge가 관리하는데,
파일이 변경된 걸 KnowledgeBridge가 어떻게 알 수 있는가?

**영향:**
- 사용자가 새 노트를 작성해도 semantic search에 안 걸림
- 에이전트가 관련 노트를 못 찾음
- 인덱스가 실제 파일과 점점 동기화가 어긋남

**해결 옵션:**

**A. 이벤트 기반 (권장)**
```rust
// KnowledgeBase에 콜백 훅 추가
pub struct KnowledgeBase {
    fs: RwLock<VirtualFs>,
    backlinks: RwLock<BacklinkIndex>,
    on_file_change: Option<Box<dyn Fn(&str, FileChange) + Send + Sync>>,
}

// KnowledgeBridge가 구독
let kb = KnowledgeBase::new(root)
    .on_file_change(|path, change| {
        bridge.invalidate_index(path);
    });
```

KnowledgeBase는 여전히 kernel 의존 없음. 콜백만 받는다.

**B. Lazy rebuild**
search 시 인덱스 타임스탬프와 파일 mtime 비교해서 stale이면 rebuild.
간단하지만 첫 search가 느릴 수 있음.

**C. 명시적 호출**
web handler에서 `note_write` 후 `bridge.reindex_path()` 직접 호출.
간단하지만 handler가 두 시스템을 다 알아야 함.

**권장: A + C 혼합.** 콜백으로 기본 동기화 보장, 필요 시 수동 reindex도 지원.

---

## 🟡 Issue 3: 기존 `space::KnowledgeBridge`의 역할 변화

**문제:**

기존 `space::KnowledgeBridge`는 Space 간 **메모리 전송**을 담당한다.
지식이 전역 하나가 되면, 이 bridge의 "cross-Space knowledge flow" 개념이 달라진다.

```rust
// 현재: Space 간 메모리 엔트리 전송
pub struct KnowledgeBridge {
    space_manager: Arc<SpaceManager>,
    // MemoryManager를 통해 다른 Space의 메모리를 읽음
}
```

지식(knowledge)은 전역이니 Space 간 전송이 필요 없다.
하지만 **Space 간 메모리(memory) 전송**은 여전히 유효하다
("dev Space에서 배운 패턴을 일상 Space에서도 활용하고 싶다").

**해결:**

1. 이름을 `MemoryBridge`로 변경 (지식이 아닌 메모리 전송임을 명확히)
2. `Space::knowledge_visible` → `memory_visible`로 의미 변경
3. `KernelEvent::SpaceCreated` 등의 이벤트에서 knowledge flow 대신 memory flow로 수정

---

## 🟡 Issue 4: `auto_memory_bridge`의 `MemoryType::Knowledge` 의존

**문제:**

`auto_memory_bridge.rs`가 `MemoryType::Knowledge`를 사용한다:

```rust
// 286라인: Knowledge 타입 리스트를 읽어서 export
.list(MemoryType::Knowledge, 1000)

// 387라인: import 시 Knowledge 타입으로 저장
memory_type: MemoryType::Knowledge,
```

RFC에서 `MemoryType::Knowledge`를 제거하면 auto_memory_bridge가 깨진다.

**auto_memory_bridge의 목적:**
외부 도구(Claude Code)의 MEMORY.md를 Oxios 메모리로 동기화.

**해결:**

auto_memory_bridge의 Knowledge 관련 기능을 재설계:

1. `from-auto`: MEMORY.md → `.md` 파일로 저장 (KnowledgeBase.note_write)
   - MemoryManager에 저장하지 않고 직접 .md로
   - 예: `knowledge/imported/claude-code/패턴-이름.md`

2. `to-auto`: KnowledgeBase에서 `.md` 파일들을 읽어 MEMORY.md 포맷으로 export
   - MemoryManager에서 Knowledge 타입을 찾는 대신
   - KnowledgeBase의 .md 파일들을 순회

즉, auto_memory_bridge가 **MemoryManager 대신 KnowledgeBase**를 사용하도록 변경.
이건 Phase 3에서 KnowledgeBridge 구현 시 함께 처리.

---

## 🟡 Issue 5: agent_runtime에 knowledge recall 연동 누락

**문제:**

RFC는 `KnowledgeBridge.recall_for_context()`를 제안하지만,
현재 `agent_runtime.rs`는 오직 `MemoryManager.recall()`만 호출한다:

```rust
// agent_runtime.rs:243
match memory_manager.recall(&seed.goal).await {
    Ok(memories) => {
        system_prompt = memory_manager.blend_into_prompt(&memories, &system_prompt);
    }
}
```

knowledge recall이 어디에 들어가야 하는지 RFC에 명시가 없다.

**해결:**

agent_runtime의 컨텍스트 조립 순서를 명시:

```rust
// 1. 세션 메모리 recall (기존)
let memories = memory_manager.recall(&seed.goal).await;
system_prompt = memory_manager.blend_into_prompt(&memories, &system_prompt);

// 2. 지식 베이스 recall (추가)
let notes = knowledge_lens.recall_for_context(&seed.goal, 5)?;
if !notes.is_empty() {
    let knowledge_block = notes.iter()
        .map(|(path, content)| format!("### {}\n{}", path, content))
        .collect::<Vec<_>>()
        .join("\n\n");
    system_prompt = format!("{}\n\n## Relevant Notes\n\n{}", system_prompt, knowledge_block);
}
```

이걸 RFC 7.2절(KnowledgeBridge)에 추가해야 함.
그리고 `agent_runtime`이 `KnowledgeLens`에 접근할 수 있도록,
KernelHandle이나 주입된 의존성으로 전달해야 함.

---

## 🟢 Issue 6: 컴파일 의존성 세부 누락

oxios-web이 oxios-markdown을 직접 의존하게 되는데,
현재 oxios-web의 Cargo.toml에는 oxios-markdown이 없다.
kernel을 통해서만 간접 참조.

추가 필요:
```toml
# surface/oxios-web/Cargo.toml
[dependencies]
oxios-markdown = { path = "../../crates/oxios-markdown" }
```

이건 RFC 10절(파일 변경 요약)에 이미 명시되어 있으므로 minor.

---

## 🟢 Issue 7: 테스트 코드 영향

다음 테스트들이 KnowledgeApi를 직접 생성하므로 업데이트 필요:

- `supervisor.rs:423` — 테스트 헬퍼에서 KnowledgeApi::new()
- `tools/kernel_bridge.rs:168` — 테스트에서 KnowledgeApi::new()
- `knowledge_api.rs` 내부 테스트들 (삭제 시 함께 제거)
- `knowledge_routes.rs` 내부 테스트들 (AppState 변경 필요)

RFC Phase 2-3에서 자연스럽게 처리되지만 명시하면 좋음.

---

## 요약

| # | 심각도 | 문제 | 해결 |
|---|--------|------|------|
| 1 | 🔴 | `KnowledgeBridge` 이름 충돌 | 새 이름 사용 (KnowledgeLens). 기존은 MemoryBridge로 개명 |
| 2 | 🔴 | Semantic index 동기화 누락 | KnowledgeBase에 콜백 훅 추가 |
| 3 | 🟡 | space::KnowledgeBridge 역할 변화 | MemoryBridge로 개명, knowledge_visible → memory_visible |
| 4 | 🟡 | auto_memory_bridge의 Knowledge 타입 | KnowledgeBase 직접 사용으로 재설계 |
| 5 | 🟡 | agent_runtime에 knowledge recall 연동 누락 | 컨텍스트 조립 순서에 knowledge recall 추가 |
| 6 | 🟢 | oxios-web Cargo.toml | oxios-markdown 직접 의존 추가 |
| 7 | 🟢 | 테스트 코드 | Phase 2-3에서 자연스럽게 처리 |

**전반적 평가:** 분리 방향은 옳다. 핵심 원칙 6개는 타당하다.
하지만 위 5개 이슈(Critical 2 + Important 3)를 RFC에 반영해야 구현 가능하다.
