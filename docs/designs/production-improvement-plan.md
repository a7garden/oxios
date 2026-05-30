# 프로덕션 개선 설계서

> **날짜:** 2026-05-30
> **기준 버전:** v0.4.0
> **목표:** 프로덕션 품질 격상 — 기능은 완성, 품질·일관성·유지보수성 개선에 집중
> **CI 제외:** 로컬 빌드/테스트로 검증, GitHub CI는 의도적 비활성화
> **검증 방법:** 커밋 메시지가 아닌 실제 코드 분석으로 확인

---

## 0. 현재 상태 요약

| 영역 | 평가 | 검증 근거 |
|------|------|-----------|
| **빌드** | ✅ 깨끗 | `cargo build` 성공, 36.25s |
| **테스트** | ✅ 1,159 passed / 0 failed | 29 test suite 전부 통과 |
| **Clippy** | ⚠️ 82 warnings | 14 auto-fixable + 34 missing_docs + 34 수동 |
| **핵심 기능** | ✅ 전부 구현 완료 | 코드 레벨 검증 완료 (아래 §4 참조) |
| **보안** | ✅ 4-layer Gate + AuditTrail | `access_manager/gate.rs` 860줄 |
| **레거시 정리** | ⚠️ release.yml 구경로 | `channels/oxios-web` → `surface/oxios-web` 미갱신 |

---

## 1. 즉시 수정 (Hot Fix)

### 1.1 Release Workflow 경로 수정

**문제:** `.github/workflows/release.yml` L19, L24가 `channels/oxios-web/web` 참조 → 실제 경로는 `surface/oxios-web/web`

**수정:**
```yaml
# Before
working-directory: channels/oxios-web/web
cd channels/oxios-web/web/dist

# After
working-directory: surface/oxios-web/web
cd surface/oxios-web/web/dist
```

**영향:** 다음 릴리스 태그 푸시 시 빌드 실패 방지
**난이도:** 🟢 1분

---

### 1.2 AGENTS.md 워크스페이스 멤버 목록 업데이트

**문제:** AGENTS.md 디렉토리 트리가 `channels/oxios-web` 표시 → 실제는 `surface/oxios-web`

**수정:** AGENTS.md에서 `channels/oxios-web` → `surface/oxios-web` 변경

**난이도:** 🟢 2분

---

### 1.3 ARCHITECTURE.md 버전 업데이트

**문제:** `Version: 0.1.0` — 실제 v0.4.0

**난이도:** 🟢 1분

---

## 2. 코드 품질 정리 (Quality Pass)

### 2.1 Clippy Auto-fix (22건 자동 수정)

**범위:** `oxios-kernel` (14), `oxios-bench` (4), `oxios` bin (4)

```bash
cargo clippy --fix --lib -p oxios-kernel --allow-dirty
cargo clippy --fix --lib -p oxios-bench --allow-dirty
cargo clippy --fix --bin "oxios" -p oxios --allow-dirty
```

**수정 항목:**
- redundant closure (2건)
- `map_or` 단순화 (3건)
- `impl` derive 가능 (4건)
- `to_string` 불필요 (2건)
- `format!` 불필요 (1건)
- explicit auto-deref (4건)
- casting 동일 타입 (1건)

**난이도:** 🟢 5분 (자동)

### 2.2 Clippy 수동 수정 (34건)

**위험도별 분류:**

| 심각도 | 항목 | 파일 | 설명 |
|--------|------|------|------|
| 🔴 | MutexGuard across await | `memory/sona.rs:247` | 데드락 위험. `tokio::sync::Mutex`로 교체 또는 drop 후 await |
| 🟡 | 함수 인자 과다 (10/7) | `agent_runtime.rs:374` | Builder 패턴 또는 struct 매개변수로 리팩토링 |
| 🟡 | 함수 인자 과다 (8/7) | `surface/oxios-web/src/routes/chat.rs:475` | Request struct 도입 |
| 🟡 | identical if blocks | kernel 내 | 조건문 분기가 동일 — 로직 오류 가능성 |
| 🟡 | if let collapsible | kernel 내 | 중첩 match/if-let 단순화 |
| 🟡 | clamp-like pattern | kernel 내 | `clamp()` 사용 |
| 🟡 | transmute without annotation | kernel 내 | 안전한 캐스팅으로 교체 |
| 🟢 | Default 구현 누락 | `AgentPool`, `RecallTiming`, `TelegramPlugin` | `#[derive(Default)]` 추가 |
| 🟢 | sort_by_key / for loop 제안 | kernel 내 | 가독성 개선 |

**난이도:** 🟡 1~2시간

### 2.3 Missing Docs 정리 (34건)

**전략:** `#[warn(missing_docs)]` 활성화 상태. 커밋 `e13b02a`에서 280+ → 0으로 줄였으나 새 모듈 추가로 34건 재발생.

**접근:**
1. `cargo clippy --workspace 2>&1 | grep "missing documentation"`로 전체 위치 파악
2. Public API에만 doc comment 추가 (`///`)
3. 내부 struct field는 `#[allow(missing_docs)]` 고려

**난이도:** 🟢 30분

---

## 3. 레거시 정리 (Housekeeping)

### 3.1 문서 내 경로 갱신

**문제:** 여러 RFC/설계 문서가 `channels/oxios-web` 참조

**대상 파일:**
- `docs/rfc-003-web-dashboard-audit.md`
- `docs/design-web-ui-ts.md`
- `docs/design-knowledge-ui.md`
- `docs/designs/architecture-review-2026-05/rfc-013-gateway-event-driven.md`
- `docs/designs/architecture-review-2026-05/rfc-014-channel-ux-unification.md`
- `docs/designs/architecture-review-2026-05/rfc-016-frontend-cleanup.md`

**작업:** `channels/oxios-web` → `surface/oxios-web` 치환

**주의:** `archive/`, `analysis/`는 역사적 기록이므로 변경 불필요

**난이도:** 🟢 10분

### 3.2 RFC 설계 문서 상태 갱신

**문제:** 이미 구현 완료된 RFC의 설계 문서가 여전히 `📝 설계` 상태

**갱신 대상:**

| 파일 | 현재 상태 | 실제 상태 |
|------|-----------|-----------|
| `rfc-013-gateway-event-driven.md` | 📝 설계 | ✅ 구현 완료 |
| `rfc-014-channel-ux-unification.md` | 📝 설계 (개정) | ✅ 구현 완료 |
| `rfc-015-security-unification.md` | 📝 설계 (v2) | ✅ 구현 완료 |
| `rfc-017-memory-simplification.md` | 📝 설계 | ✅ 구현 완료 |
| `rfc-019-ouroboros-cleanup.md` | 📝 설계 (v2) | ✅ 구현 완료 |
| `rfc-020-proactive-sona-activation.md` | 📝 설계 (v2) | ✅ 구현 완료 |

**난이도:** 🟢 5분

---

## 4. RFC 구현 상태 — 코드 검증 결과

> 아래는 커밋 메시지가 아닌 실제 코드 분석으로 확인한 결과.

| RFC | 항목 | 상태 | 코드 근거 |
|-----|------|------|-----------|
| **RFC-013** | Gateway Event-Driven | ✅ 완료 | `gateway.rs`: `tokio::select!` + `mpsc::channel(1024)` + `Semaphore(32)`. 각 채널이 `register()`에서 `tokio::spawn`으로 receive → shared mpsc push. 폴링 완전 제거 |
| **RFC-014** | 채널 UX 통일 | ✅ 완료 | `format.rs` 공통 모듈 (cli/telegram/web 각각 구현). `ErrorKind` 분류 (`error_classify.rs`). `ResponseMeta` typed metadata. `ChannelFormatter` trait |
| **RFC-015** | 보안 모델 통합 | ✅ 완료 | `access_manager/gate.rs` 860줄: 4-layer Gate (CSpace → RBAC → Permissions → ExecConfig). `audit_sink.rs`, `context.rs` (AgentContext). short-circuit 평가 |
| **RFC-016** | Frontend 정리 | ❌ 미구현 | `getToken()` 여전히 chat.ts에서 localStorage 직접 읽기 (L67). `ko-KR` 하드코딩 (chat.tsx L305, L348). localStorage persist 불일치 (theme/sidebar 수동 vs chat persist 미들웨어) |
| **RFC-017** | 메모리 정리 | ✅ 완료 | `reasoning_bank.rs`, `rvf_store.rs` 파일 미존재. `lateral.rs`, `regression.rs` 제거됨 |
| **RFC-018** | 설정 UX 개선 | ❌ 미구현 | `main.rs:421`에서 여전히 `toml::to_string_pretty()`로 전체 재직렬화 (코멘트 파괴). `config set` match arm 9개만 지원. `max_agents` 기본값은 10으로 통일됨 |
| **RFC-019** | Ouroboros Evolution | ✅ 완료 | `orchestrator.rs`: `should_evaluate()` (L719), `evaluate()` + `evolve()` 루프 (L753-816). `EvolutionConfig`가 `OrchestratorConfig`에서 `max_evolution_iterations` 읽음 |
| **RFC-020** | Proactive Recall & Sona | ✅ 완료 | `3451110` 커밋. proactive recall, sona 학습 엔진 활성화 |

**실제 남은 작업: RFC-016 (Frontend), RFC-018 (설정 UX) 단 2개**

---

## 5. 실행 계획

### Phase 1: 즉시 (오늘) — ~40분

| # | 작업 | 예상 시간 |
|---|------|-----------|
| 1 | release.yml 경로 수정 | 1분 |
| 2 | AGENTS.md 경로 갱신 | 2분 |
| 3 | ARCHITECTURE.md 버전 0.4.0 | 1분 |
| 4 | `cargo clippy --fix` 자동 수정 (22건) | 5분 |
| 5 | RFC-013,014,015,017,019,020 설계 문서 상태 `✅` 갱신 | 5분 |
| 6 | Missing docs 보강 (34건) | 20분 |
| 7 | 문서 내 `channels/oxios-web` → `surface/oxios-web` 치환 | 10분 |

### Phase 2: 단기 (이번 주) — 2~3시간

| # | 작업 | 예상 시간 |
|---|------|-----------|
| 8 | MutexGuard across await 수정 (`sona.rs:247`) | 30분 |
| 9 | 함수 인자 과다 리팩토링 (2건) | 30분 |
| 10 | 나머지 Clippy 수동 수정 (~20건) | 1시간 |

### Phase 3: RFC-016 Frontend 정리 — 1~2일

| # | 작업 | 비고 |
|---|------|------|
| 11 | `getToken()` → auth store 통일 | `stores/chat.ts:67` |
| 12 | raw fetch → `api` 클라이언트 통일 | `routes/chat.tsx` |
| 13 | `ko-KR` → i18n locale 사용 | `routes/chat.tsx:305,348` |
| 14 | localStorage persist 패턴 통일 | theme/sidebar/chat stores |

### Phase 4: RFC-018 설정 UX — v0.5.0

| # | 작업 | 비고 |
|---|------|------|
| 15 | `toml_edit` 기반 config set (코멘트 보존) | `main.rs:421` |
| 16 | config set field coverage 확대 (9 → 전체) | `main.rs:426-504` |
| 17 | config list / describe / reset 서브커맨드 | ConfigAction 확장 |

---

## 6. 완료 기준

| 기준 | 측정 방법 |
|------|-----------|
| Clippy warnings 0건 | `cargo clippy --workspace 2>&1 \| grep -c "^warning:"` = 0 |
| 모든 테스트 통과 유지 | `cargo test --workspace` — 1,159+ passed, 0 failed |
| release.yml 경로 정확 | `surface/oxios-web/web` 참조 |
| 문서에 구경로 잔존 없음 | `grep -r "channels/oxios-web" docs/` = 0 |
| RFC 설계 문서 상태 정확 | 구현 완료 6개 `✅`, 미구현 2개 `📝` |
