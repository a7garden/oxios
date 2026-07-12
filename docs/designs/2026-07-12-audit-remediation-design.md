# Audit Remediation Design — v1.22.0 (2026-07-12)

> 본 문서는 2026-07-12 코드 감사(`fdd4617`→`333b5af5`)가 발견한 15개 취약점(F-1 … F-15)에 대한 구체적 수정 설계다. 각 항목은 근인과·수정 설계(파일/심볼 단위)·검증 방법을 갖는다. 감사 보고서는 `/tmp/code-audit/oxios-code-audit-report.html` 참조.

## 설계 원칙

1. **보안 통제는 chokepoint에서 강제한다.** 호출자가 아닌 실제 실행 지점(client.rs spawn, tool-call 게이트)에서 검증. "정의됐으나 실행 경로에서 우회되는" 결함(F-1)을 근원적으로 막는다.
2. **fail-closed가 기본.** 보안 판독·검증 실패는 허용이 아니라 거부.
3. **최소 변경·첨가 우선.** 재작성 제안은 현재 접근이 명백히 깨졌을 때만.
4. **각 수정은 독립 검증 가능.** 회귀 없음을 증명하는 행동 테스트 동반.

---

## Wave 1 — P0 보안 (1~2주차, 출시 블로커)

### F-1 [심각] MCP 서버 spawn이 셸 블랙리스트를 우회 → chokepoint 강제

**근인:** `validate_mcp_command()`(`src/api/routes/infra.rs:306`, FORBIDDEN 메타문자·`..`·`BLOCKED_MCP_SHELLS` 차단)는 HTTP 등록 경로(`infra.rs:352,505`)에서만 호출. 부팅 `init_mcp_bridge`(`src/kernel.rs:1831`)와 실제 spawn `crates/oxios-mcp/src/client.rs:111`(`Command::new(&self.server.command).args(...).envs(&self.server.env).spawn()`)는 검증 없음.

**수정 설계 — chokepoint 강제 + 환경변수 필터:**
1. `validate_mcp_command` + `BLOCKED_MCP_SHELLS` + `FORBIDDEN`를 `oxios-mcp` 크레이트의 공개 모듈로 이동: 신규 `crates/oxios-mcp/src/validation.rs` (`pub fn validate_mcp_command(&str) -> Result<(), McpSpawnError>`, `pub const BLOCKED_MCP_SHELLS`, `pub fn sanitize_env(env: &HashMap<String,String>) -> HashMap<String,String>` — `LD_PRELOAD`/`LD_LIBRARY_PATH`/`DYLD_*`/`PYTHONPATH`/`PYTHONHOME`/`SHLIB_PATH` 제거).
2. `McpClient::initialize()`(`client.rs:102`) 시작부에 `validate_mcp_command(&self.server.command)?` 호출 + spawn 전 `let safe_env = sanitize_env(&self.server.env);` 적용. **이게 단일 chokepoint** — HTTP·부팅·환경변수 어느 호출자도 우회 불가.
3. `src/api/routes/infra.rs`는 `oxios_mcp::validation::{validate_mcp_command, ...}`를 재사용(중복 제거). HTTP 경로는 사용자 친화 에러를 위해 그대로 유지하되 구현은 위임.
4. `init_mcp_bridge`(`kernel.rs:1831`)에서 `def.command`를 `McpServer::new` 전에 검증(이중 방어; chokepoint가 이미 막지만 부팅 시 거부가 진단에 유용).

**검증:** `tests/mcp_spawn_validation.rs` — (a) `OXIOS_MCP_X_COMMAND="bash"` 부팅 시 거부, (b) `OXIOS_MCP_X_COMMAND="npx" OXIOS_MCP_X_ENV="LD_PRELOAD=/tmp/x.so"`가 spawn되는 자식의 환경에 `LD_PRELOAD` 부재 확인(`/proc/self/environ` 또는 자식이 환경을 에코), (c) 정상 `npx` 명령은 여전히 spawn.

**노력:** 0.5일. **영향:** `crates/oxios-mcp/src/{lib.rs,client.rs,validation.rs(신규)}`, `src/api/routes/infra.rs`, `src/kernel.rs`.

---

### F-13 [높음→P0] `auth_enabled` 기본값 false → 기본 true + 옵트아웃

**근인:** `share/default-config.toml`에서 `[auth] enabled`가 주석 처리(= false). `crates/oxios-kernel/src/config.rs`의 `AuthConfig::default()`.

**수정 설계:**
1. `AuthConfig::Default` → `enabled: true`(`config.rs`).
2. `share/default-config.toml`: `enabled = true`로 명시 + 주석 "기본 인증. 로컬 단일 사용자는 false로 끌 수 있음".
3. **마이그레이션:** 기존 사용자 중 인증 없이 쓰던 경우를 고려해 첫 부팅 시 `auth.toml`에 키가 없으면 자동 키 생성 + 한 번 안내. `cmd_doctor`(`main.rs:1300`)에서 `auth_enabled == false`면 워닝 라인 추가.
4. 웹 UI: `auth_enabled` 토글 시 현재 세션에 영향 주지 않도록(변경 후 재인증 유도).

**검증:** `config.rs` 단위 테스트 `default_auth_enabled_is_true`; `cmd_doctor`가 `auth_enabled=false`일 때 워닝 출력(문자열 매칭 테스트).

**노력:** 1시간 + 마이그레이션 0.5일. **영향:** `config.rs`, `share/default-config.toml`, `main.rs`(doctor).

---

### F-7 [높음→P0] cargo audit 비차단 + 24 RUSTSEC 무시 + cargo-deny 부재

**근인:** `.github/workflows/ci.yml`의 audit 스텝이 `continue-on-error: true`; `.cargo/audit.toml:8-33`이 24개 무시; `deny.toml` 없음.

**수정 설계:**
1. **`deny.toml` 도입:** `[advisories]`(`vulnerability = "deny"`, `unmaintained = "workspace"`), `[licenses]`(allow MIT/Apache-2.0/BSD/ISC, copyleft deny), `[bans]`(multiple-versions warn). 초안은 `cargo deny init` 후 편집.
2. **RUSTSEC 무시 목록 재검토:** 24개 각각 (a) 의존 경로, (b) 실제 영향, (c) 해결(업그레이드/대체) 가능성 조사. 해결 가능한 건 ignore 해제 + 의존성 업그레이드; 불가능한 것만 `ignore`에 사유 주석과 함께 남김(wasmtime 관련 optional, rsa no-fix 등).
3. **CI:** `cargo audit`를 `continue-on-error: false`(차단)로 전환; `cargo deny check advisories licenses bans` 스텝 추가(차단).
4. 프론트엔드: `web/`에 `bun audit --severity=high` 스텝 추가(또는 `bunx npm-audit-html`). `package.json` trustedDependencies 점검.

**검증:** CI에서 `cargo deny check` 통과; `cargo audit`이 0(또는 사유 명시된 ignore만). 로컬 `cargo deny check bans`로 중복 버전 경고 확인.

**노력:** deny.toml 0.5일 + RUSTSEC 재검토 1~2일(의존성 업그레이드 포함). **영향:** `deny.toml(신규)`, `.cargo/audit.toml`, `.github/workflows/ci.yml`.

---

### F-3 [높음→P0] 자격증명·API 키 평문 저장 → 암호화 at-rest

**근인:** `crates/oxios-kernel/src/credential.rs:148` `fs::write(serde_json::to_string_pretty(&map))`로 평문 JSON. 파일 권한 `0o600`만.

**수정 설계 — OS 키링 우선, 파일 폴백은 머신 키 암호화:**
1. 의존성 추가: `keyring = "3"` (workspace `[workspace.dependencies]`). 크로스플랫폼(macOS Keychain/Linux Secret Service/Windows Credential Manager).
2. `CredentialStore` 추상화: `trait CredentialBackend { fn get/set/delete }` — `KeyringBackend`(우선) + `EncryptedFileBackend`(폴백).
3. **폴백 암호화:** 키링 사용 불가 시(헤드리스 Linux에 DBus 없음 등), 머신 고유 키로 AES-GCM 암호화. 키 파생: 머신 ID(`/etc/machine-id` 또는 macOS IOPlatformUUID) + 고정 salt → HKDF(`sha2`+`hkdf`). `crates/oxios-kernel/src/credential.rs`에 `encrypt(plaintext) -> Vec<u8>`, `decrypt(ciphertext) -> String` 추가(aes-gcm 의존성).
4. **마이그레이션:** 첫 로드 시 기존 평문 파일 감지 → 암호화해서 재접두사(`ENC1:`)로 재저장. 백업 후 평문 삭제.
5. 평문 평문 경로(`extract_credential`)는 복호화 후 동작.

**검증:** `tests/credential_encryption.rs` — (a) 저장 후 파일이 평문이 아님(휴리스틱: `sk-` 미포함, 접두사 `ENC1:`), (b) 재로드 시 올바른 값, (c) 다른 머신 키로는 복호 실패(키 파생이 머신 종속). 키링 가용 시 키링 경로 통합 테스트.

**노력:** 1.5~2일. **영향:** `credential.rs`, `Cargo.toml`(keyring/aes-gcm), `config.rs`(backend 선택).

---

## Wave 2 — P1 견고성 (3~4주차)

### F-2 [높음] 라이브 에이전트 루프 인젝션 방어 부재 → 도구 호출 게이트 판독

**근인:** `crates/oxios-kernel/src/agent_runtime.rs`가 외부 콘텐츠를 `untrusted_content`로 격리 없이 LLM 출력 → 도구 호출로 직결. `persona_tool.rs:442 security_review()`는 잘 설계됐으나 작성 시점(`:256,332`)에만, 에러 시 fail-open(`:269,345`).

**수정 설계 — 게이트 내장 판독(fail-closed):**
1. `security_review`를 일반화: `crates/oxios-kernel/src/security/injection_guard.rs`(신규)로 추출 — `pub async fn judge_tool_call(engine, tool_name, args, context) -> Verdict { Safe, Suspicious, Blocked }`.
2. `agent_runtime.rs` 도구 호출 루프(각 `AgentTool::execute` 호출 직전)에 게이트 삽입: 외부 소스(파일 읽기·웹 fetch·MCP 결과·이전 도구 출력)가 인자에 포함된 경우 판독. **fail-closed:** 판독 에러 시 도구 호출 거부(사용자에게 알림)하고 다음 턴으로.
3. 성능: 모든 도구 호출이 아니라 외부 콘텐츠 포함 시에만(휴리스틱: 인자 크기 임계치·출처 태그). 일반 계산 도구(현재 시각 등)는 패스스루.
4. `persona_tool.rs`의 `security_review`는 이 공용 함수를 호출하도록 리팩터(중복 제거). persona 경로는 fail-closed로 전환(에러 시 거부, 사용자에게 "검토 불가, 재시도" 안내).
5. 사용자 알림: 판독이 `Blocked`/`Suspicious`면 `AgentToolResult` + 이벤트 버스로 사용자에게 노출(투명성).

**검증:** `tests/agent_runtime_injection.rs` — (a) 도구 인자에 숨겨진 ignore-previous 지시문 → 판독이 `Blocked`, 도구 미실행, (b) 정상 도구 호출은 패스스루, (c) 판독 엔진 에러 시 fail-closed(거부). 기존 `oxios-in-tool-llm-judge` 패턴 재사용(`skill://oxios-in-tool-llm-judge`).

**노력:** 3~5일(판독 프롬프트 튜닝 + 벤치마크 포함). **영향:** `agent_runtime.rs`, 신규 `security/injection_guard.rs`, `persona_tool.rs`, `tools/kernel_bridge.rs`.

---

### F-6 [높음] 핵심 실행 경로 행동 테스트 부재 + 커버리지 미측정

**근인:** `agent_runtime.rs`(1,904 LOC) 행동 테스트 0건; `web/package.json`의 `test:coverage`가 CI에서 미호출.

**수정 설계:**
1. **행동 테스트 추가:** `crates/oxios-kernel/tests/agent_runtime_test.rs` — `MockEngine`(`EngineProvider` 구현, 스크립트된 응답)으로 `handle_agent_event` 전 주기를 검증: 프롬프트 → 도구 호출 → 결과 환입 → 종료. 실패 경로(모델 에러·도구 패닉·예산 초과) 포함.
2. **CI 커버리지 게이트:** `cargo llvm-cov nextest --workspace --lcov` 스텝 추가(또는 `cargo-tarpaulin`). `codecov` 업로드. `agent_runtime.rs` 라인 커버리지 임계치(예: 70%)를 `ci.yml`에서 게이트(초기에는 워닝→점진 차단).
3. 프론트엔드: `bun run test:coverage`를 `frontend` 스텝에 추가; `vitest` 임계치.
4. `#[ignore]` 3개(GGUF/catalog)는 스케줄된 워크플로(주간) 또는 `--run-ignored` 옵션으로 주기 실행.

**검증:** 커버리지 리포트가 `agent_runtime.rs` 임계치 충족; 새 행동 테스트가 의도된 버그(MockEngine이 도구 패닉 반환)에서 실패.

**노력:** 테스트 3일 + CI 게이트 0.5일. **영향:** `tests/agent_runtime_test.rs(신규)`, `.github/workflows/ci.yml`.

---

### F-4 [높음] panic=abort + 복구 경로 unwrap/expect → 제거 + catch_unwind

**근인:** 릴리스 `panic=abort`(`Cargo.toml:28`) + 출하 `unwrap` 1,358 + `expect` 115, 복구 경로에 일부. `catch_unwind` 전무 → 단일 패닉이 데몬 즉시 종료.

**수정 설계:**
1. **복구 경로 우선 제거:** `src/supervisor.rs:168,368,406`(WebSurface expect), `token_maxing/session.rs:163`(HashMap expect), `mount/detection.rs:130`(`expect("non-empty")`), `api/routes/chat.rs:737` — 각각 `?` 또는 명시적 에러(`KernelError::Internal`)로 교체. `auth.rs:210`(getrandom expect)는 OS RNG 실패=치명이므로 `expect` 유지하되 메시지 명확화(의도적).
2. **`catch_unwind` 격리:** 크리티컬 백그라운드 태스크(guardian, recalibration, cron, dream)의 본체를 `std::panic::catch_unwind`로 감싸 — 패닉 시 워닝 로그 + 태스크 재시작(supervisor가 이미 track_critical하므로 가능). `panic=abort`와 `catch_unwind`는 **충돌**(abort면 unwind 안 됨) → 주의: `panic=abort`를 유지하려면 catch_unwind 불가. **설계 결정:** 크리티컬 겔더리는 `panic=unwind`로 가거나, abort를 유지하면서 "태스크 자체가 절대 패닉하지 않도록" unwrap을 전부 제거하는 것이 본질. **권장:** abort 유지 + 복구 경로 unwrap 전면 제거(근본 해결)가 더 단순하고 성능에 이득. catch_unwind는 옵션.
3. **clippy 게이트:** `clippy::unwrap_used`, `clippy::expect_used`를 workspace `[lints]`에서 `warn`(초기)→`deny`(점진). `#[cfg(test)]`는 허용.

**검증:** `cargo clippy --workspace -- -D clippy::unwrap_used`가 출하 코드에서 0(테스트 제외). 복구 경로 테스트: WebSurface 상태 부재 시 abort가 아니라 graceful 에러.

**노력:** 2~3일(1500+ 개체 중 복구 경로 위주; 전수 제거는 장기). **영향:** `supervisor.rs`, `token_maxing/session.rs`, `mount/detection.rs`, `chat.rs`, `Cargo.toml`(`[lints]`), `clippy.toml`.

---

## Wave 3 — P2 (5~8주차)

### F-5 [높음] HTTP API가 커널 내부 직접 임포트 → 파사드 일원화

**근인:** 7개 라우트 파일이 `KernelHandle` 파사드를 우회해 커널 구현 모듈 직접 사용(`events.rs:71`, `chat.rs:1401`, `budget_routes.rs:9`, `infra.rs:9-10`, `workspace.rs:1105`, `system.rs:1335`, `a2a.rs:11`).

**수정 설계:** 두 가지 경로 — (A) 라우트가 필요한 내부 타입을 `KernelHandle` 파사드 메서드로 노출(메서드 위임 추가) 후 라우트는 `handle().*`만 사용; (B) 일부 타입(`SessionId`, `BudgetLimit`)은 이미 파사드 재노출이므로 import 경로만 `oxios_kernel::` 최상위로 변경. 점진적: 파일당 PR. `state_store::SessionId`는 `KernelHandle`이 이미 노출하므로 `use oxios_kernel::SessionId`.

**검증:** 컴파일 + 각 라우트 파일이 `oxios_kernel::state_store::`/`event_bus::`/`budget::` 직접 import 0건(`grep` 게이트를 CI에 추가 가능).

**노력:** 1~2주(점진). **영향:** `src/api/routes/*`, `kernel_handle/*`.

---

### F-8 [높음] OpenAPI 경로 0건 → 핵심 엔드포인트 어노테이션

**근인:** `src/api/api_docs.rs:18` 빈 `PathsBuilder`; utoipa 어노테이션 없음.

**수정 설계:** 핵심 엔드포인트(chat send/session, mcp servers CRUD, knowledge tree, auth)에 `#[utoipa::path(...)]` 추가. `AppState`/요청/응답 타입에 `ToSchema`. 우선순위: 외부 통합 대상 엔드포인트 10~15개. 나머지는 점진.

**검증:** `build_openapi()`가 ≥10 path 생성; Swagger UI(`/api/docs`)에서 표시.

**노력:** 1주(점진). **영향:** `api_docs.rs`, `api/routes/*`, 요청/응답 타입.

---

### F-9 [중간] 메모리 recall 임베딩 이중 복제 → 참조 전달

**근인:** `crates/oxios-memory/src/memory/sqlite/store.rs:337-338` 후보 벡터 `Vec<Vec<f32>>` 두 번 clone(~3.5MB).

**수정 설계:** `FlashAttention::attention` 시그니처를 `&[&[f32]]`(또는 `&[Vec<f32>]`에서 빌린) 받도록 변경; `recall_with_rerank`는 후보 슬라이스 참조를 빌려 전달. 질의 벡터도 `&[f32]`. `keys`/`values` 소유 벡터 제거.

**검증:** 기존 recall 테스트 통과 + 벤치마크(후보 300개 recall 시 할당 `-90%`). `cargo test` memory 스위트.

**노력:** 0.5일. **영향:** `store.rs`, `embedding.rs`(attention 시그니처).

---

### F-10 [중간] agent_log_db 매 쿼리 SQL 재파싱 → prepared statement 캐싱

**근인:** `agent_log_db.rs:515` 등 매 조회 `format!()` + `conn.prepare()`.

**수정 설계:** `AgentLogDb`에 `HashMap<String, Statement>` 또는 `once_cell` 상수 SQL 템플릿(where 절은 고정 열 조합으로 한정) + `prepare_cached`. where_clause 동적 생성이면 열거형으로 정규화(몇 안 되는 패턴).

**검증:** 동일 쿼리 2회 호출 시 두 번째 `prepare` 미발생(카운터/모의); 페이지네이션 조회 벤치마크.

**노력:** 1~2일. **영향:** `agent_log_db.rs`.

---

### F-11 [중간] 거대 라우트 파일 SRP 위반 → 서브 모듈 분할

**근인:** `chat.rs`(2,029), `system.rs`(2,345), `workspace.rs`(1,907), `knowledge_routes.rs`(1,576) 다관심사.

**수정 설계:** 파일별 관심사 그룹핑 → `routes/chat/{streaming.rs, session.rs, history.rs}` 식 서브 모듈. `mod.rs`에서 `pub use` 재노출. 점진적(한 파일씩, 동작 보존).

**검증:** 분할 후 라우트 등록(`build_routes`) 동일; 통합 테스트 통과.

**노력:** 1~2주. **영향:** `src/api/routes/*`.

---

## Wave 4 — P3 장기/정기

### F-12 [중간] ClawHub SHA-256 재검증 안 함 → 설치/업데이트 양쪽 검증

**근인:** `skill/clawhub/client.rs:42-46` 다운로드 시 해시 계산·저장; `installer.rs` 업데이트 시 미검증.

**수정 설계:** `installer.rs` update 경로에 기존 저장 해시와 다운로드 해시 비교; 불일치 시 거부(또는 사용자 확인). 무결성 체인: lockfile에 해시 기록, 재설치/업데이트 시마다 검증. `fn verify_integrity(path, expected_hash) -> Result<()>` 공용.

**검증:** 변조된 파일(tamper) update 시 거부 테스트.

**노력:** 0.5일. **영향:** `skill/clawhub/{client.rs,installer.rs}`.

---

### F-14 [중간] detached 백그라운드 태스크 JoinHandle 미보관 → 추적 + drain

**근인:** `kernel.rs:664`(daily_health_check), `knowledge_dream.rs:257`, `cron.rs:505`, `event_bus.rs:539`가 fire-and-forget.

**수정 설계:** 각 태스크의 `JoinHandle`을 `TaskSupervisor`에 secondary-critical으로 등록(guardian_task 패턴 `main.rs:3167` 참조). 셧다운 시 drain + 취소. `daily_health_check`는 우선순위.

**검증:** 셧다운 시 백그라운드 태스크가 정리됨(로그/카운터); 핸들 누수 테스트.

**노력:** 1일. **영향:** `kernel.rs`, `knowledge_dream.rs`, `cron.rs`, `event_bus.rs`, `supervisor.rs`.

---

### F-15 [중간] A2A 서킷 브레이커 Relaxed ordering → SeqCst/AcqRel

**근인:** `a2a/circuit_breaker.rs:83-141` state/failure_count가 `Ordering::Relaxed`.

**수정 설계:** 상태 전이(Open/Closed/HalfOpen)는 `SeqCst`; failure_count는 `AcqRel`(fetch_add). `resilience/health.rs` 패턴과 통일. 문서화 주석 추가(왜 강한 순서가 필요한가).

**검증:** 멀티스레드 스트레스 테스트(브레이커 오픈 후 동시 요청이 거부됨).

**노력:** 0.5일. **영향:** `a2a/circuit_breaker.rs`.

---

## 부록 항목 (점진)

`lib.rs:7` `allow(missing_docs)` → `warn` 전환(문서 보강 후) · `Kernel`/`KernelHandle` God Object 분할(관심사별 매니저) · `config.rs` `SETTINGS_VERSION` 도입 · SQLite r2d2 풀(1 writer + N readers, WAL) · 피처 CI 매트릭스(`wasm-sandbox` 재활성화 또는 제거 결정) · RFC-009/-031 상태 동기화 · stale `surface/oxios-web` 경로 약 63개 정리 · SBOM 생성 · 스트리밍 `Arc<String>` · broadcast `Arc<KernelEvent>` · git_layer `spawn_blocking` · 프론트엔드 메시지 가상화 · BM25 `AND memory_type` 푸시다운.

---

## 검증·회귀 전략

- **각 Wave 종료 후:** `cargo fmt && cargo clippy --workspace -- -D warnings && cargo nextest run --workspace` + `cd web && bun run build && bun run test`. 전체 스위트 녹색 필수.
- **보안 수정(F-1,F-3,F-13) 후:** 수동 회귀 — 기존 MCP 서버 정상 기동, 기존 자격증명 마이그레이션, 로컬 단일 사용자 시나리오 인증 옵트아웃.
- **출시 전:** 본 감사 항목 재점검(체크리스트화) + 재감사(회차별 트렌드 비교).

## 추정 총 노력

| Wave | 핵심 | 노력 |
|---|---|---|
| 1 (P0) | F-1, F-13, F-7, F-3 | ~5일 |
| 2 (P1) | F-2, F-6, F-4 | ~10일 |
| 3 (P2) | F-5, F-8, F-9, F-10, F-11 | ~3~4주(점진) |
| 4 (P3) | F-12, F-14, F-15 | ~2일 + 정기 |

P0 4건(약 5일) 완료 시 출시 가능. 나머지는 점진적 품질 향상.
