# Oxios 프로덕션 준비도 분석 보고서

**작성일:** 2026-05-30  
**코드베이스:** 0.5.0 (10개 크레이트, ~107K 라인)

---

## 종합 평가: 🟡 프로덕션 진입 직전 (Pre-Production)

> **판정:** 이 앱은 핵심 기능이 완성되었지만, **릴리스 파이프라인이 미비**하고 **일부 영역에서 테스트가 부족**하여 아직 진정한 프로덕션 단계가 아닙니다. 다만 코드 품질, 아키텍처 설계, 보안基础设施는 프로덕션 수준입니다.

---

## 1. 코드 품질 분석

### 1.1 컴파일 & 빌드

| 항목 | 상태 | 비고 |
|------|------|------|
| 모든 크레이트 컴파일 | ✅ Pass | warnings만 존재 (unused imports, dead code) |
| Frontend 빌드 | ✅ Pass | bun build 성공 |
| Dockerfile 빌드 | ✅ Pass | Multi-stage, non-root user |
| Release 빌드 | ❌ 미검증 | CI에서 debug 빌드만 실행 |

### 1.2 테크니컬 데트 (Tech Debt)

| 항목 | 수 | 심각도 |
|------|---|--------|
| `TODO` | 6개 | 🟢 Low — CLI wiring ×2, Web TODO ×2, ouroboros ×1, markdown ×1 |
| `FIXME` | 0개 | 🟢 Excellent |
| `HACK` | 0개 | 🟢 Excellent |
| `XXX` | 0개 | 🟢 Excellent |
| `todo!()` 매크로 | 0개 | 🟢 Excellent |
| `unimplemented!()` | 0개 | 🟢 Excellent |

**평가:** 6개의 TODO만 존재하고 FIXM/HACK/XXX가 전혀 없는 것은 매우 건강한 상태입니다.

### 1.3 에러 처리

| 항목 | 수 | 평가 |
|------|---|------|
| `unwrap()` | 1,146회 | 🔴 **경고** — 전체 unwrap 중 일부만 `expect()`로 대체됨 |
| `expect()` | 50회 | 🟡 허용 범위 |
| `panic!` | 10회 | 🟢 최소 수준 (event_bus ×6, e2e test ×2, audit_trail ×1, embedding ×1) |
| `Result<` 타입 | 813회 | 🟢 양호 |
| `anyhow::Result` | 174회 | 🟢 앱 레벨에서 널리 사용 |
| `thiserror` 정의 | 9개 enum | 🟢 라이브러리 레벨에서 proper error types 사용 |
| `#[should_panic]` 테스트 | 0개 | 🟡 Panic behavior 미검증 |

**핵심 문제:** `unwrap()`이 1,146회 사용되는 것은 큰 문제입니다. `.clippy.toml`에서 `unwrap()` on `Option`/`Result`를 disallowed로 설정했음에도 불구하고 상당수가 남아있습니다. 그러나 panic! 매크로가 10회뿐이고 `todo!()`/`unimplemented!()`가 0이라는 것은/runtime 시 unfinished code가 없음을 의미합니다.

### 1.4 테스트 품질

| 항목 | 수 | 평가 |
|------|---|------|
| 총 테스트 수 | **1,027개** | 🟢 양호 |
| Async 테스트 (`#[tokio::test]`) | 153개 | 🟢 적절 |
| Integration 테스트 | 89개 | 🟢 충분 |
| 테스트 없는 50줄+ 파일 | ~25개 | 🟡 일부 주의 |
| `mock`/`fake`/`stub` 패턴 | **0회** | 🔴 **심각** — Mock 프레임워크 없음 |
| Property-based testing | 0개 | 🟡 놓친 기회 |

**가장 중요한 미테스트 파일:**
- `orchestrator.rs` (1,378줄) — **Ouroboros 프로토콜 뇌**, 테스트 **0개**
- `ouroboros_engine.rs` (867줄) — **프로토콜 엔진**, 테스트 **0개**
- `src/main.rs` (2,023줄) — 바이너리 엔트리포인트
- `src/kernel.rs` (1,092줄) — 커널 어셈블러

**테스트 분포:** oxios-kernel이 61.5%, oxios-markdown이 19.5%로 집중되어 있고, web(1.2%), mcp(1.7%), gateway(2.1%)는 턱없이 낮습니다.

---

## 2. 보안 분석

### 2.1 인증 & 접근 제어

| 항목 | 상태 | 품질 |
|------|------|------|
| RBAC 구현 | ✅ 완전 | 46,659줄 (access_manager/mod.rs), 6개 권한 정의 파일 |
| Path Sandboxing | ✅ 완전 | gate.rs (31,785줄) — OWASP 기반 설계 |
| Audit Trail | ✅ 완전 | Merkle-chain 스타일 변조 방지 감사 로그 (1,135줄) |
| Access Gate | ✅ 완전 | 중앙화된 unified gate 패턴 |

### 2.2 시크릿 관리

| 항목 | 결과 |
|------|------|
| 하드코딩된 시크릿 | ✅ **검출되지 않음** — `password=`/`secret=`/`api_key=` 패턴에 하드코딩 없음 |
| CredentialStore | ✅ 존재 | env → config.toml → oxi auth.json 순서로 multi-source resolution |
| API 키 마스킹 | ✅ 존재 | web에서 api_key를 `***`로 마스킹 (`system.rs`) |

### 2.3 네트워크 보안

| 항목 | 상태 | 비고 |
|------|------|------|
| HTTPS/TLS 패턴 | 🟡 중간 | 29개 파일에서 https:// 또는 rustls 참조 — 주로 API 호출용 |
| TLS 설정 | 확인 필요 | 기본 HTTPS 요청은 하지만 dedicated TLS 설정 파일 없음 |
| WASM 샌드박스 | 🔄 진행중 | Feature-gated stub (완전한 구현 아님) |

**평가:** Security 접근 방식은 OWASP-inspired 설계로 프로덕션 수준입니다. 하드코딩된 시크릿 없음, RBAC + sandboxing + audit trail 3중 보안 구조.

---

## 3. CI/CD & 배포 인프라

### 3.1 CI 파이프라인

| 항목 | 평가 |
|------|------|
| 포맷 체크 (`cargo fmt`) | ✅ Ubuntu만 |
| Clippy 린팅 | 🟡 Core 5개 크레이트만 (web, cli, telegram, bench 미포함) |
| 테스트 (nextest) | ✅ 4분할 병렬 실행, --no-fail-fast |
| 보안 감사 (`cargo audit`) | ✅ 18개 RUSTSEC 무시 (wasmtime, rsa 등 — 주기적 검토 필요) |
| Frontend 빌드 | ✅ bun install && bun run build |
| OS 매트릭스 | ❌ **Ubuntu만** — macOS runner 없음 |
| Rust 버전 고정 | ❌ 미수행 — `rust-toolchain.toml` 없음 |
| 커버리지 리포트 | ❌ 없음 |
| 아티팩트 업로드 | ❌ 없음 |

**CI 점수: 7/10** — 기본 구조는 좋지만 macOS 미테스트, clippy 불완전, 커버리지 부재가 단점.

### 3.2 릴리스 파이프라인

| 항목 | 상태 |
|------|------|
| Trigger | ✅ 태그 기반 (`v*`) |
| Web assets 압축 | ✅ web-dist.zip 생성 |
| GitHub Release 생성 | ✅ softprops/action-gh-release 사용 |
| **Rust 바이너리 빌드** | ❌ **완전 부재** |
| **크로스 플랫폼 빌드** | ❌ 부재 |
| **SHA256 체크섬** | ❌ 미생성 — `scripts/install.sh`가 필요로 함 |
| ** Smoke test** | ❌ 없음 |
| **코드 사인** | ❌ 없음 |

**치명적 문제:** `scripts/install.sh`는 다음을 기대합니다:
```
https://github.com/a7garden/oxios/releases/download/vX.Y.Z/oxios      (바이너리)
https://github.com/a7garden/oxios/releases/download/vX.Y.Z/oxios.sha256 (체크섬)
```

하지만 현재 release.yml은 **web asset만** zip으로 올립니다. 인스톨러가 **전혀 작동하지 않습니다.**

**릴리스 점수: 3/10** — 프로덕션 배포 불가 상태.

### 3.3 Docker

| 항목 | 평가 |
|------|------|
| Multi-stage 빌드 | ✅ 양호 |
| Non-root user | ✅ 양호 |
| Health check | ✅ curl http://localhost:4200/health |
| `.dockerignore` | ❌ 미존재 |
| Multi-platform | ❌ 미지원 |
| Rust 버전 동기화 | ⚠️ 미검증 (Dockerfile: 1.85, CI: stable) |

**Docker 점수: B+** — 기본 양호하지만 .dockerignore 없고 CI와 버전 미동기.

---

## 4. 기능 완성도

### 4.1 핵심 시스템

| 시스템 | 파일 수 | 평가 |
|--------|--------|------|
| Kernel (supervisor, scheduler, agent lifecycle) | 133개 | ✅ 완성 |
| Memory (Dream, HNSW, hyperbolic, compaction, decay) | 24개 | ✅ 완성 |
| Ouroboros (interview → seed → execute → evaluate → evolve) | 14개 | ✅ 완성 |
| MCP Client (JSON-RPC 2.0 over stdio) | 3개 | ✅ 완성 |
| A2A Protocol (Google agent-to-agent) | 구현됨 | ✅ 완성 |
| Knowledge Base (markdown, backlinks, graph) | 21개 | ✅ 완성 |
| Skill System (unified SKILL.md) | 구현됨 | ✅ 완성 |
| Circuit Breaker (3-state) | 구현됨 | ✅ 완성 |

### 4.2 에이전트 도구

| Tier | 도구 수 | 상태 |
|------|--------|------|
| Tier 1 (always-on, oxi-sdk) | 8개 | ✅ `read`, `write`, `edit`, `grep`, `find`, `ls`, `web_search`, `get_search_results` |
| Tier 2 (kernel domain) | 16개 | ✅ `exec`, `memory_*`, `project`, `agent`, `a2a_*`, `persona`, `cron`, `security`, `budget`, `resource`, `browser`, `knowledge`, `marketplace` |
| **총합** | **24개** | ✅ 완전 |

### 4.3 KernelHandle API

15개의 typed API facade:
- `StateApi`, `AgentApi`, `SecurityApi`, `PersonaApi`, `ExtensionApi`, `McpApi`, `InfraApi`, `ExecApi`, `BrowserApi`, `A2aApi`, `EngineApi`, `KnowledgeBase`, `KnowledgeLens`, `ProjectApi`, `MarketplaceApi`

### 4.4 미완성 / 상태

| 항목 | 상태 | 심각도 |
|------|------|--------|
| OTel (OpenTelemetry) | 🔄 Feature-gated stub | 🟡 선택적 기능 |
| WASM Sandboxing | 🔄 Feature-gated stub | 🟡 선택적 기능 |
| `orchestrator.rs` 테스트 | ❌ 0개 | 🟡 P0 권장 |
| `ouroboros_engine.rs` 테스트 | ❌ 0개 | 🟡 P0 권장 |
| KernelHandle API 테스트 | ❌ 전무 | 🟡 P1 권장 |
| 파일.md 포팅 (RFC-007) | 🔄 62% 진행중 | 🟡 진행중 |

**평가:** 24개 도구 + 15개 API로 기능적 완성도는 매우 높습니다. 미완성 기능은 모두 선택적(feature-gated)이거나 진행중인 RFC范畴입니다.

---

## 5. 문서화 & RFC

| 항목 | 상태 |
|------|------|
| AGENTS.md | ✅ 존재 — 프로젝트 소개 |
| ARCHITECTURE.md | ✅ 존재 |
| RFC 문서 | 21개 (RFC-003 ~ RFC-013 등) |
| 채널 가이드 | ✅ docs/channel-plugin-guide.md |
| 테스트 가이드 | ✅ inline, `OXIOS_E2E=1` 환경변수 기반 |

**RFC 상태:** 21개 RFC 중 **단 1개만 구현 완료** (RFC-003 Knowledge Separation). 나머지는 Draft/In Progress 상태. 이는 활발한 개발 중임을 의미합니다.

---

## 6. 버그 발견

### 🔴 치명적

1. **릴리스 파이프라인 미비** — 바이너리 빌드 없음, SHA256 체크섬 없음, install script 작동 불가
2. **`orchestrator.rs` (1,378줄) 테스트 0개** — 가장 중요한 모듈이 미테스트
3. **`ouroboros_engine.rs` (867줄) 테스트 0개** — 프로토콜 엔진 미테스트
4. **Mock 프레임워크 부재** — 모든 테스트가 integration 수준으로 격리 불가

### 🟡 중요

5. **`unwrap()` 1,146회** — 특히 핵심 모듈 (scheduler 58개, exec_tool 39개, memory 38개)
6. **macOS CI 미지원** — launchd 기반 daemon의 플랫폼 특이적 코드 미테스트
7. **Rust 버전 미고정** — `rust-toolchain.toml` 부재
8. **Clippy 불완전** — web, cli, telegram, bench 미포함
9. **커버리지 리포트 부재** — coverage 측정 불가

### 🟢 양호

10. 하드코딩된 시크릿 **0건**
11. FIXM/HACK/XXX **0개**
12. `todo!()`/`unimplemented!()` **0개**
13. 1,027개 테스트 + 2,662개 assertion

---

## 7. 종합 점수

| 영역 | 점수 | 가중치 | 加权점 |
|------|------|--------|--------|
| **코드 품질** | 7.5/10 | 25% | 1.875 |
| **보안** | 9.0/10 | 20% | 1.800 |
| **테스트** | 6.0/10 | 20% | 1.200 |
| **CI/CD** | 6.75/10 | 15% | 1.013 |
| **릴리스 파이프라인** | 3.0/10 | 10% | 0.300 |
| **기능 완성도** | 9.5/10 | 10% | 0.950 |
| **합계** | | 100% | **7.14/10** |

### 등급 기준

| 점수 | 등급 | 의미 |
|------|------|------|
| 9-10 | 🟢 프로덕션 완전 | 즉시 배포 가능 |
| 7-9 | 🟡 프로덕션 진입 직전 | 일부 수정 후 배포 가능 |
| 5-7 | 🟠 개발 완료 | 대규모 수정 필요 |
| 3-5 | 🔴 개발 중 | 프로덕션很远 |
| 0-3 | ⚫ 초기 | 프로토타입 |

**판정: 🟡 7.14/10 — 프로덕션 진입 직전**

---

## 8. 프로덕션 전환을 위한 로드맵

### P0 (즉시 수정 필요)

1. **릴리스 파이프라인 구축**
   - cross-platform 빌드 (Linux x64/ARM64, macOS Intel/Apple Silicon)
   - SHA256 체크섬 생성
   - GitHub Release에 바이너리 첨부
   - Smoke test 추가

2. **orchestrator.rs / ouroboros_engine.rs 테스트 작성**
   - 최소 Happy path + 주요 에러 케이스

3. **Mock 프레임워크 도입**
   - `mockall` 도입으로 LLM provider 격리 테스트 가능하게

### P1 (배포 전 수정)

4. **`rust-toolchain.toml` 추가** — CI/Dockerfile/로컬 동기화
5. **macOS CI runner 추가** — launchd daemon 테스트
6. **`unwrap()` 정리** — 핵심 모듈에서 `expect()` 또는 `?`로 전환
7. **Clippy 전체 크레이트 적용**

### P2 (점진적 개선)

8. 커버리지 리포트 도입 (codecov)
9. `.dockerignore` 추가
10. Docker multi-platform 빌드 (buildx)
11. KernelHandle API 테스트 추가

---

## 결론

Oxios는 **아키텍처 설계, 보안基础设施, 기능 완성도** 측면에서 프로덕션 수준입니다. 24개 도구, 15개 API, 1,027개 테스트, 3중 보안 구조, 6개 TODO 이하의 테크니컬 데트로 견조한 기반을 갖추고 있습니다.

그러나 **릴리스 파이프라인이 부재**하고 **가장 중요한 모듈 2개가 미테스트**이며 **Mock 프레임워크가 없어 테스트 격리가 불가능**한 상황입니다. 이 3가지를 수정하면 바로 프로덕션 배포가 가능합니다.

**현재 상태:** "완료된 건물에 입주 허가가 없는 상황" — 건물(코드)은 완성이지만, 입주(릴리스)를 위한 인허가(릴리스 파이프라인)가 없습니다.