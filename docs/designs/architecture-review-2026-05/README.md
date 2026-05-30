# 아키텍처 리뷰 개선 설계 모음

> **리뷰 날짜:** 2026-05-26
> **범위:** 전체 아키텍처 분석 → UX 일관성 평가 → 개선 설계
> **총 평점:** 6.4/10 — 아키텍처 비전은 명확하나, 채널 간 UX 일관성이 가장 큰 약점

## 문서 인덱스

| # | 문서 | 우선순위 | 범위 | 상태 |
|---|------|----------|------|------|
| 1 | [RFC-013: Gateway Event-Driven 마이그레이션](rfc-013-gateway-event-driven.md) | **P0** | gateway, 모든 채널 | ✅ 구현 완료 |
| 2 | [RFC-014: 채널 UX 통일](rfc-014-channel-ux-unification.md) | **P0-P1** | web, cli, telegram | ✅ 구현 완료 |
| 3 | [RFC-015: 보안 모델 통합](rfc-015-security-unification.md) | **P0-P1** | access_manager, tools, exec | ✅ 구현 완료 |
| 4 | [RFC-016: Frontend 정리 및 패턴 통일](rfc-016-frontend-cleanup.md) | **P1** | web frontend | 📝 설계 |
| 5 | [RFC-017: 메모리 시스템 모듈 활성도 정리](rfc-017-memory-simplification.md) | **P2** | memory | ✅ 구현 완료 |
| 6 | [RFC-018: 설정 UX 개선](rfc-018-configuration-ux.md) | **P2** | config, cli, onboarding | 📝 설계 |
| 7 | [RFC-019: Ouroboros Evolution 루프 활성화](rfc-019-ouroboros-cleanup.md) | **P2** | ouroboros, orchestrator | ✅ 구현 완료 |

## 개정 이력

| 날짜 | RFC | 변경 사항 |
|------|-----|----------|
| 2026-05-27 | RFC-017 | "복잡도 축소" → "활성도 정리". 검증 결과 대부분의 모듈이 활성 코드였음. proactive/sona는 미연결 활성화 대상으로 재분류 |
| 2026-05-27 | RFC-019 | "Dead Code 정리" → "Evolution 루프 활성화". evaluate/evolve가 완전 구현되어 있었음. 연결만 필요 |

## 우선순위 정의

| 등급 | 의미 | 기준 |
|------|------|------|
| **P0** | 즉시 수정 필요 | 보안 취약점, 데이터 손실, 전체 시스템 블로킹 |
| **P1** | 다음 릴리스 포함 | UX 품질 저하, 사용자 불편, 유지보수 부담 |
| **P2** | 향후 계획 | 기술 부채, 복잡도 관리, 개발자 경험 |

## 의존 관계

```
RFC-013 (Gateway) ← RFC-014 (채널 UX) 의존
RFC-015 (보안) — 독립 실행 가능
RFC-016 (Frontend) — 독립 실행 가능
RFC-017 (메모리) — 독립 실행 가능
RFC-018 (설정) — 독립 실행 가능
RFC-019 (Ouroboros) — 독립 실행 가능
```

## 리뷰에서 발견된 주요 이슈 요약

### 🔴 Critical (4)
1. Gateway write lock + Telegram 30초 블로킹 → 전체 채널 응답 불가
2. `ExecTool::new()` 권한 bypass → production 코드에서 무제한 실행 가능
3. Frontend WebSocket 이중 구현 → 데드 코드 + 유지보수 혼란
4. Chat 사이드바 auth 헤더 누락 → 인증 필요 시 조용히 실패

### 🟡 Important (8)
1. Ouroboros 5단계 중 2.5단계만 연결 (dead code)
2. RBAC ↔ Agent Permissions 이중 분리 → 실행 간격
3. CLI fire-and-forget 응답 → 순서 보장 없음
4. 채널 간 에러 포맷 불일치
5. Always-on 도구가 AccessManager 우회
6. `oxios config set` 9/120 필드만 지원
7. Frontend error boundary 없음
8. 설정 기본값 불일치 (TOML vs Rust)

### 🟢 Minor (6)
1. Capability 추론 로직 중복
2. Scheduler O(n log n) task lookup
3. 하드코딩된 `'ko-KR'` 로케일
4. i18n `t()` fallback 사용 불일치
5. Persistence 패턴 불일치 (manual vs persist middleware)
6. AGENTS.md 포트 정보 outdated
