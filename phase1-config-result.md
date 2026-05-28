# Phase 1: config.rs — AllowlistMode enum + is_binary_allowed() 수정

## Status: ✅ 완료

## 변경 내용

### 파일: `crates/oxios-kernel/src/config.rs`

1. **AllowlistMode enum 추가** (ExecConfig struct 위):
   - `Permissive` — 모든 바이너리 허용 (개발 모드)
   - `Enforced` — `allowed_commands`에 있는 바이너리만 허용 (기본값)
   - `#[serde(rename_all = "snake_case")]`, `Serialize`, `Deserialize` derive
   - `Default = Enforced`

2. **ExecConfig struct**에 `allowlist_mode: AllowlistMode` 필드 추가 (`allowed_commands` 뒤, `#[serde(default)]`)

3. **is_binary_allowed()** 메서드 수정:
   - `Permissive`: 빈 리스트 = 모두 허용, 또는 allowlist에 있으면 허용
   - `Enforced`: allowlist에 있는 것만 허용 (빈 리스트 = 아무것도 허용 안 함)

4. **Default for ExecConfig**에 `allowlist_mode: AllowlistMode::default()` 추가

5. **테스트 수정/추가**:
   - `test_exec_config_default_allowed_commands` — 기본 Enforced 모드에서 빈 리스트 = 아무것도 허용 안 함
   - `test_exec_config_permissive_mode` — 신규: Permissive + 빈 리스트 = 모두 허용
   - `test_is_binary_allowed_with_allowlist` — 기존 동작 유지 (Enforced + allowlist)

## 테스트 결과

```
running 9 tests
test config::tests::test_exec_config_permissive_mode ... ok
test config::tests::test_exec_config_default_allowed_commands ... ok
test config::tests::test_is_binary_allowed_with_allowlist ... ok
test config::tests::test_expand_home ... ok
test config::tests::test_default_config_validates ... ok
test config::tests::test_zero_max_agents_error ... ok
test config::tests::test_exec_timeout_validation ... ok
test config::tests::test_invalid_cron_expression ... ok
test config::tests::test_config_serialization_roundtrip ... ok

test result: ok. 9 passed; 0 failed
```

## serialization roundtrip 확인

- `test_config_serialization_roundtrip` 통과 — TOML 직렬화/역직렬화 정상
- `allowlist_mode` 필드는 `#[serde(default)]`이므로 기존 config.toml에서 누락되어도 `Enforced`로 정상 로드
