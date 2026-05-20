# Progress

## Status
In Progress

## Tasks

### Part 2: Space × Knowledge 통합 — activate_space() 구현 ✅

1. **SpaceManager.default_workspace_dir 공개화** — `fn` → `pub fn`
2. **SpaceApi.workspace_dir() 추가** — Space의 workspace 디렉토리 경로 반환
3. **KernelHandle.activate_space() 추가** — Space 활성화 + KnowledgeApi 전환을 단일 호출로 통합
4. **space_routes.rs 업데이트** — `spaces.activate()` → `kernel.activate_space()` 교체

## Files Changed

- `crates/oxios-kernel/src/space/manager.rs` — `default_workspace_dir`을 `pub`로 변경
- `crates/oxios-kernel/src/kernel_handle/space_api.rs` — `workspace_dir()` 메서드 추가
- `crates/oxios-kernel/src/kernel_handle/mod.rs` — `activate_space()` 편의 메서드 추가
- `channels/oxios-web/src/routes/space_routes.rs` — `handle_space_activate`이 `activate_space()` 사용하도록 변경

## Verification

- `cargo check --workspace` ✅
- `cargo test --workspace` ✅ (모든 테스트 통과: 540+ 단위 테스트, 40+ e2e 테스트, 22+ 통합 테스트)

## Notes

- `KnowledgeApi`에는 이미 `switch_space()` 및 `index_all()` 메서드가 구현되어 있어 별도 추가 불필요
- `activate_space()`는 Space 활성화 → Knowledge 루트 전환 → 백링크 재색인을 원자적으로 수행
