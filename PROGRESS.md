# Progress

## Track B: KnowledgeApi 재설계 — 완료

### 1. VirtualFs POSIX path 메서드 (사전조건)
Track A에서 이미 추가됨:
- `read_path`, `write_path`, `delete_path`, `rename_path`, `exists_path`, `mtime_path` (VirtualFs 메서드)
- `split_posix_path` (자유 함수)

수정: `lib.rs`에서 중복 export 제거

### 2. BacklinkIndex::clear() 추가 ✅
- `crates/oxios-markdown/src/backlinks.rs`에 `clear()` 메서드 추가
- `forward`, `backward`, `details` 필드 모두 초기화

### 3. KnowledgeApi 재작성 ✅
- `crates/oxios-kernel/src/kernel_handle/knowledge_api.rs` 완전 재작성
- 새 구조체: `fs`, `memory`, `backlinks`, `engine`, `default_model`, `agent_writes`
- POSIX path 기반 I/O: `note_read/write/delete/move` → `read_path/write_path/delete_path/rename_path`
- `index_to_memory()` helper (fire-and-forget async)
- `search()` — name-based + semantic via MemoryManager
- `backlinks_for()`, `link_graph()`
- `index_all()` — 전체 knowledge base 인덱싱
- `copilot_chat()` — sync AI-powered copilot (block_in_place)
- `call_engine()` — AI engine 호출
- `switch_space()` — space 전환 시 루트 교체
- `agent_writes` 추적: `mark_agent_write`, `is_agent_write`, `clear_agent_write`
- 새 타입: `CopilotResponse`, `NoteHit`
- 모든 기존 테스트 보존 + 신규 테스트 (agent_write, index_all, switch_space, copilot_chat[ignore])

### 4. kernel.rs 수정 ✅
- 2곳의 `KnowledgeApi::new()` 호출에 `EngineProvider` + `default_model` 주입 추가
  - `Kernel::handle()` 메서드
  - `KernelBuilder::build()` 메서드

### 5. KernelHandle::from_subsystems() 업데이트 ✅
- `crates/oxios-kernel/src/kernel_handle/mod.rs` — KnowledgeApi 생성에 engine/model 추가

### 6. KnowledgeTool 수정 ✅
- `crates/oxios-kernel/src/tools/kernel/knowledge_tool.rs`
- `engine` + `default_model` 필드 추가
- `from_kernel()` — `model_id()` 접근자 사용
- `make_api()` — 4인자 생성자 사용

### 7. 테스트 코드 수정 ✅
- `tools/kernel_bridge.rs` — 테스트 KnowledgeApi 생성에 engine/model 추가
- `supervisor.rs` — 테스트 KnowledgeApi 생성에 engine/model 추가

### 검증 결과
- `cargo check --workspace` ✅
- `cargo test --workspace` ✅ (모든 테스트 통과)
