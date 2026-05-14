# Oxios Agent OS — 프로젝트 분석 및 평가 보고서

> **분석 일자:** 2026-05-14  
> **버전:** 0.2.0-alpha  
> **분석 대상:** 152개 Rust 소스 파일, 41,329줄 코드  

---

## 1. 프로젝트 개요

Oxios는 Rust로 구축된 **Agent Operating System**입니다. Unix 철학(작은 도구의 조합)과 Ouroboros 방법론(스펙-퍼스트 워크플로우)을 결합하여, AI 에이전트가 사용자를 대신해 실제 작업을 수행하는 OS를 목표로 합니다.

### 아키텍처

```
Gateway (채널 무관한 메시지 허브)
    ↓
Kernel (감독자 + Ouroboros + oxi-agent)
    ├── Supervisor      — 에이전트 생명주기 (fork/exec/wait/kill)
    ├── Orchestrator    — Ouroboros 5단계 프로토콜 조정
    ├── AgentRuntime    — oxi-agent 기반 도구 호출 루프
    ├── AccessManager   — RBAC + 워크스페이스 샌드박스
    ├── MemoryManager   — 벡터 검색 + 하이퍼볼릭 임베딩
    ├── AuditTrail      — 블레이크3 해시 체인 무결성
    ├── BudgetManager   — 토큰/호출 예산 관리
    ├── Scheduler       — 우선순위 기반 작업 큐
    ├── CronScheduler   — 시간 기반 자동 실행
    ├── ProgramManager  — OS 수준 프로그램 설치/관리
    └── A2A Protocol    — 에이전트 간 통신
```

### 크레이트 구조

| 크레이트 | 역할 | 라인 수(추정) |
|----------|------|-------------|
| `oxios` (바이너리) | 진입점, CLI, 커널 조립 | ~1,250 |
| `oxios-kernel` | 핵심 라이브러리 (30+ 모듈) | ~33,000 |
| `oxios-ouroboros` | 스펙-퍼스트 프로토콜 | ~800 |
| `oxios-gateway` | 채널 무관 메시지 라우팅 | ~400 |
| `oxios-web` | HTTP 채널 (Axum) | ~2,500 |
| `oxios-cli` | CLI 채널 | ~100 |
| `oxios-telegram` | 텔레그램 채널 | ~200 |

---

## 2. 정량 분석

### 2.1 코드 규모

| 지표 | 값 |
|------|-----|
| Rust 소스 파일 | 152개 |
| 총 코드 라인 | 41,329줄 |
| 테스트 파일 포함 | 46개 파일에 `#[cfg(test)]` |
| 총 테스트 케이스 | 459개 |
| 테스트/코드 비율 | ~11% (라인 기준), 모듈별 40–59% (핵심 모듈) |
| Cargo 크레이트 | 8개 (workspace) |
| 커널 공개 모듈 | 30개 |

### 2.2 모듈별 규모 (Top 15)

| 파일 | 라인 수 | 역할 |
|------|--------|------|
| `program/mod.rs` | 1,282 | 프로그램 수명주기 관리 |
| `access_manager/mod.rs` | 1,278 | RBAC + 보안 |
| `integration_tests.rs` | 1,142 | 통합 테스트 |
| `audit_trail.rs` | 1,128 | 감사 추적 |
| `scheduler.rs` | 994 | 작업 스케줄러 |
| `config.rs` | 914 | 설정 (22개 설정 구조체) |
| `orchestrator.rs` | 891 | Ouroboros 오케스트레이터 |
| `exec_tool.rs` | 874 | 실행 도구 |
| `a2a.rs` | 870 | 에이전트 간 통신 |
| `memory/store.rs` | 850 | 메모리 스토어 |
| `e2e_kernel.rs` | 848 | E2E 테스트 |
| `agent_runtime.rs` | 810 | 에이전트 런타임 |
| `memory/auto_memory_bridge.rs` | 776 | 자동 메모리 브릿지 |
| `cron.rs` | 747 | 크론 스케줄러 |
| `main.rs` | 731 | 메인 바이너리 |

### 2.3 테스트 커버리지 평가

| 모듈 | 단위 테스트 | 통합 테스트 | E2E 테스트 | 평가 |
|------|-----------|------------|-----------|------|
| ExecTool | 33+ | — | — | ⭐⭐⭐⭐⭐ |
| ProgramManager | 31+ | 2 | — | ⭐⭐⭐⭐⭐ |
| AuditTrail | — | — | 9 | ⭐⭐⭐⭐ |
| BudgetManager | — | — | 9 | ⭐⭐⭐⭐ |
| GitLayer | — | — | 8 | ⭐⭐⭐⭐ |
| Scheduler | 내장 | 2 | — | ⭐⭐⭐⭐ |
| Memory | 내장(47%) | — | — | ⭐⭐⭐⭐ |
| AccessManager | 내장(57%) | 4 | — | ⭐⭐⭐⭐ |
| EventBus | — | 3 | — | ⭐⭐⭐ |
| StateStore | — | 5 | 5 | ⭐⭐⭐⭐ |
| Hyperbolic | 내장(43%) | — | — | ⭐⭐⭐⭐ |
| FlashAttention | 내장(42%) | — | — | ⭐⭐⭐⭐ |
| HNSW | 내장(43%) | — | — | ⭐⭐⭐⭐ |
| A2A | 5 | — | — | ⭐⭐⭐ |
| MCP Client | **0** | — | — | ⭐ |
| Web Routes | **0** | — | — | ⭐ |
| Supervisor | **0** | — | — | ⭐ |
| Config | **0** | — | — | ⭐ |
| KernelHandle | **0** | — | — | ⭐ |

---

## 3. 아키텍처 평가

### 3.1 강점 ⭐⭐⭐⭐⭐ (5/5)

| 강점 | 설명 |
|------|------|
| **명확한 계층 분리** | 바이너리(조립) → 커널(제공) → 게이트웨이(라우팅) → 채널(입출력)의 명확한 책임 분리 |
| **KernelHandle 퍼사드 패턴** | 7개 도메인 API(State, Agent, Security, Persona, Extension, MCP, Infra)로 우아한 분해 |
| **7단계 도구 등록** | oxi 네이티브 → Exec → Program → MCP → Memory → A2A → Browser의 계층적 도구 등록 |
| **보안-퍼스트 설계** | OWASP 영감 RBAC, 워크스페이스 샌드박스, 셸 메타문자 차단, 환경변수 스트리핑 |
| **감사 추적 무결성** | blake3 해시 체인으로 변조 감지, 12개 감사 액션 타입 |
| **이벤트 기반 아키텍처** | 17개 이벤트 타입의 broadcast 채널, 모든 상태 전이가 이벤트 발행 |
| **기능 게이트 컴파일** | `web`, `cli`, `telegram`, `browser`, `otel`, `wasm-sandbox` 모두 선택적 |
| **레이어드 폴백** | 메모리 검색(HNSW → 무차별 → 키워드), 에이전트 위임(A2A → lifecycle → 단일) |

### 3.2 아키텍처 패턴 사용 현황

| 패턴 | 적용 위치 | 적절성 |
|------|----------|--------|
| Builder | KernelBuilder, AgentRuntime | ✅ 훌륭 |
| Facade | KernelHandle → 7개 Sub-API | ✅ 훌륭 |
| Strategy | Supervisor trait, OuroborosProtocol trait | ✅ 훌륭 |
| Observer/Event | EventBus (broadcast channel) | ✅ 적절 |
| Plugin | ChannelPlugin for channels | ✅ 적절 |
| Circuit Breaker | LLM_CIRCUIT_BREAKER (글로벌) | ✅ 적절 |
| Priority Queue | AgentScheduler | ⚠️ Vec 기반 (개선 필요) |
| Hash Chain | AuditTrail | ✅ 적절 |

### 3.3 의존성 그래프

```
oxios (binary)
├── oxios-kernel
│   ├── oxi-agent (path dep from ../oxi/)
│   │   └── oxi-ai (path dep from ../oxi/)
│   ├── oxios-ouroboros
│   ├── usearch (HNSW 벡터 검색)
│   ├── gix (Git 작업)
│   ├── wasmtime (WASM 샌드박스, optional)
│   ├── oxibrowser-core (헤드리스 브라우저, optional)
│   └── opentelemetry (OTel, optional)
├── oxios-gateway
├── oxios-web (Axum, optional)
├── oxios-cli (optional)
└── oxios-telegram (optional)
```

---

## 4. 코드 품질 평가

### 4.1 Rust 관례 준수 ⭐⭐⭐⭐ (4/5)

| 항목 | 상태 | 비고 |
|------|------|------|
| `#![warn(missing_docs)]` | ✅ | 모든 공개 크레이트에 적용 |
| Error handling | ✅ | 라이브러리: `thiserror`, 바이너리: `anyhow` |
| async/await | ✅ | tokio 런타임 전면 사용 |
| Serialization | ✅ | serde + serde_json (wire), toml (config) |
| Edition 2021 | ✅ | 최신 에디션 |
| `#[allow(dead_code)]` | ⚠️ | kernel.rs에 일부 잔존 |
| 공개 API 문서화 | ✅ | 모듈 수준 + 타입 수준 doc comments |

### 4.2 보안 ⭐⭐⭐⭐⭐ (5/5)

| 보안 기능 | 구현 상태 | 품질 |
|-----------|----------|------|
| 셸 메타문자 차단 | ✅ `SHELL_METACHARS` 상세 목록 | 훌를 |
| 경로 순회 방지 | ✅ `canonicalize()` + `starts_with()` | 훌륭 |
| 바이너리 허용 목록 | ✅ 구조적 실행 모드 | 양호 |
| 환경변수 스트리핑 | ✅ 최소 6개만 유지 | 훌륩 |
| 워크스페이스 샌드박스 | ✅ 에이전트별 격리 | 훌륩 |
| RBAC | ✅ 역할/권한/승인 워크플로우 | 훌륩 |
| 감사 추적 | ✅ blake3 해시 체인 | 훌륩 |
| 타임아웃 강제 | ✅ `max_timeout_secs` 캡 | 양호 |
| WASM 샌드박스 | ✅ wasmtime 기반 (optional) | 양호 |

### 4.3 에러 처리 ⭐⭐⭐⭐ (4/5)

**장점:**
- `KernelError` enum: 9개 변형, HTTP 상태 매핑 내장
- 4개 테스트로 에러 변환/표시 검증
- 웹 레이어: `AppError`로 일관된 에러 응답

**개선점:**
- `Timeout`, `RateLimited` 변형 누락
- `Memory` 변형이 `String` 사용 (타입 정보 손실)

### 4.4 문서화 ⭐⭐⭐⭐ (4/5)

- 모든 파일에 모듈 수준 doc comment 존재
- 공개 타입/함수에 `///` doc comments
- 커널 조립 의도 명확히 서술 (kernel.rs doc)
- AGENTS.md에 상세한 개발 가이드
- 다만: 일부 복잡한 알고리즘(하이퍼볼릭 임베딩)에 수식/논문 참조 부족

---

## 5. 핵심 이슈 및 리스크

### 5.1 🔴 심각 (즉시 수정 필요)

| # | 이슈 | 위치 | 영향 |
|---|------|------|------|
| 1 | **CWD 레이스 컨디션** | `agent_runtime.rs` | 동시 실행 에이전트 간 작업 디렉토리 충돌. `std::env::set_current_dir()`는 프로세스 전역. `spawn_blocking`으로 실행되는 에이전트들이 서로의 CWD를 덮어씀 |
| 2 | **`kill()` 실제 취소 불가** | `supervisor.rs` | 에이전트 상태를 `Stopped`로 변경하지만, 실행 중인 태스크는 계속 실행됨. `CancellationToken` 또는 `JoinHandle::abort()` 필요 |
| 3 | **재귀적 `next_task()`** | `scheduler.rs` | 예산 소진 시 재귀 호출. 다수 에이전트가 예산 소진 시 스택 오버플로우 가능 |

### 5.2 🟡 중간 (단기 개선 권장)

| # | 이슈 | 위치 | 영향 |
|---|------|------|------|
| 4 | **MCP 서버 재연결 없음** | `mcp/client.rs` | 서버 크래시 시 이후 모든 도구 호출 실패 |
| 5 | **프로그램 업그레이드 비원자적** | `program/mod.rs` | uninstall 후 install 실패 시 프로그램 유실 |
| 6 | **감사 추적 자동 정리 시 전체 재해싱** | `audit_trail.rs` | O(N) 체인 재구축, 대규모 트레일에서 성능 저하 |
| 7 | **예산 비영속화** | `budget.rs` | 재시작 시 모든 예산 초기화. `Instant` 타입 직렬화 불가 |
| 8 | **스케줄러 O(N) 삽입** | `scheduler.rs` | 정렬된 Vec 사용. `BinaryHeap` 필요 |
| 9 | **크론 잡 타임아웃/재시도 없음** | `cron.rs` | 장기 실행/실패 잡 관리 불가 |
| 10 | **13개 매개변수 함수** | `agent_runtime.rs` | `run_agent_loop()` 파라미터 과다. 구조체로 리팩토링 필요 |
| 11 | **웹 라우트 핸들러 테스트 없음** | `workspace.rs` | 685 LOC, 0개 테스트 |
| 12 | **MCP 클라이언트 테스트 없음** | `mcp/client.rs` | JSON-RPC 클라이언트 무테스트 |
| 13 | **Supervisor 테스트 없음** | `supervisor.rs` | 핵심 모듈 무테스트 |
| 14 | **Config 테스트 없음** | `config.rs` | 22개 설정 구조체, 0개 테스트 |

### 5.3 🟢 경미 (중기 개선)

| # | 이슈 | 위치 |
|---|------|------|
| 15 | 하이퍼볼릭 k-NN 무차별 대입 (O(N)) | `hyperbolic.rs` |
| 16 | Flash Attention 미통합 | `flash_attention.rs` |
| 17 | AuditEntry 타입 중복 | `access_manager` vs `audit_trail` |
| 18 | 감사 로그 std::thread::spawn (과도한 스레드) | `access_manager/mod.rs` |
| 19 | `ConfigAction::Set` 미구현 | `main.rs` |
| 20 | `DaemonAction::Restart` no-op | `main.rs` |

---

## 6. 알고리즘 및 성능 분석

### 6.1 메모리 시스템

```
검색 파이프라인:
  HNSW (O(log N × D)) → 무차별 (O(N × D)) → 키워드 (O(K × N))
  
임베딩:
  TF-IDF (기본, 언어 무관) → 한국어 음절 보존
  하이퍼볼릭 (Poincaré ball, 선택적)
  
중요도 산출:
  effective_importance = base × (1 + ln(1 + access_count))
  
중복 제거:
  벡터 유사도(>0.95) + 콘텐츠 해시 (이중 가드)
```

### 6.2 리소스 친화도

| 알고리즘 | 메모리 | CPU | 확장성 |
|----------|--------|-----|--------|
| HNSW 검색 | O(N × D) | O(log N) | ~1M 벡터까지 양호 |
| 하이퍼볼릭 k-NN | O(N × D) | O(N) | ~10K 한계 |
| Flash Attention | O(N × D) | O(N² × D) | CPU 한계, 학술적 가치 |
| 감사 해시 체인 | O(N) | O(N) 검증 | 정리 전략 필요 |
| 우선순위 큐 | O(N) | O(N) 삽입 | BinaryHeap 필요 |

---

## 7. 종합 평가

### 7.1 도메인별 점수

| 도메인 | 점수 | 비고 |
|--------|------|------|
| **아키텍처 설계** | ⭐⭐⭐⭐⭐ | 계층 분리, 퍼사드, 이벤트 기반 — 모범 사례 |
| **보안** | ⭐⭐⭐⭐⭐ | OWASP 영감, 다층 방어, 감사 추적 |
| **코드 품질** | ⭐⭐⭐⭐ | 일관된 스타일, 좋은 문서화, 일부 anti-pattern |
| **테스트 커버리지** | ⭐⭐⭐ | 핵심 모듈은 우수(40–59%), 일부 모듈 무테스트 |
| **성능 설계** | ⭐⭐⭐ | HNSW 도입은 훌륭, 스케줄러/하이퍼볼릭은 개선 필요 |
| **에러 처리** | ⭐⭐⭐⭐ | 일관된 패턴, 일부 누락 변형 |
| **확장성** | ⭐⭐⭐⭐⭐ | 기능 게이트, 트레이트 기반 추상화, 플러그인 |
| **운영 준비도** | ⭐⭐⭐ | 감사/백업/지표 있으나, 일부 미완성 기능 |

### 7.2 총평

**Oxios는 alpha 단계(0.2.0-alpha)임에도 상당히 높은 수준의 아키텍처 설계와 코드 품질을 보여줍니다.** 특히 다음이 돋보입니다:

1. **"스펙 없이 실행하지 않는다"** 는 Ouroboros 철학이 코드에 일관되게 반영됨
2. **보안이 1등급 시민** — 모든 실행 경로에 AccessManager, 감사 추적, 샌드박스 적용
3. **Unix 철학의 충실한 구현** — Program 시스템, 7단계 도구 등록, 채널 플러그인
4. **한국어 지원** — 토크나이저, 사용자 대면 메시지, AGENTS.md에 명시

**가장 시급한 개선 영역:**

1. CWD 레이스 컨디션 (동시 에이전트 실행 안전성)
2. 테스트 없는 핵심 모듈 보완 (Supervisor, Config, MCP Client, Web Routes)
3. 스케줄러 자료구조 개선 (Vec → BinaryHeap) 및 재귀 제거

### 7.3 권장 우선순위 로드맵

```
Phase 1 (긴급): CWD 레이스 수정, CancellationToken 도입, 재귀 next_task() 제거
Phase 2 (단기): 핵심 모듈 테스트 보강, MCP 재연결, 프로그램 원자적 업그레이드
Phase 3 (중기): BinaryHeap 스케줄러, 감사 점진적 해싱, 예산 영속화, 크론 잡 타임아웃
Phase 4 (장기): 하이퍼볼릭 HNSW, Flash Attention 통합, 원격 프로그램 보안 강화
```

---

## 8. 메타데이터

| 항목 | 값 |
|------|-----|
| 분석 대상 버전 | 0.2.0-alpha |
| 분석 파일 수 | 152개 Rust 소스 |
| 총 코드 라인 | 41,329줄 |
| 총 테스트 케이스 | 459개 |
| 분석 깊이 | 모든 공개 모듈 + 통합/E2E 테스트 |
| 평가 방법 | 정적 코드 분석, 아키텍처 패턴 검증, 알고리즘 복잡도 분석, 보안 감사 |

---

*이 보고서는 정적 분석 기반으로 작성되었으며, 런타임 프로파일링이나 부하 테스트는 포함하지 않습니다.*
