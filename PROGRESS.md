# Progress

## Status
In Progress

## Tasks
- [x] Phase 1: default-config.toml — 위험 바이너리 제거 + host + allowlist_mode
- [x] Phase 1: config.rs — AllowlistMode enum + is_binary_allowed() + tests

## Files Changed
- `share/default-config.toml` — gateway host 127.0.0.1, exec allowlist_mode=enforced, 위험 바이너리 제거, security 경고 주석
- `crates/oxios-kernel/src/config.rs` — AllowlistMode enum, ExecConfig.allowlist_mode field, is_binary_allowed() mode-aware logic, 3 updated tests

## Notes
- Phase 1 완료: osascript, open, shortcuts, gh 제거; 24개 안전 바이너리만 허용
- Phase 1 config.rs 완료: AllowlistMode::Enforced 기본값, Permissive/Enforced 모드 분기, 9개 테스트 통과
