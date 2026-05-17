# Oxios Recursive Improvement — Progress Log

> 각 세션의 작업 기록. 새 세션은 여기서 마지막 위치를 파악.

---

## Session 1 — 2026-05-17

### 작업 내용
1. **ZAI 엔드포인트 수정** (`engine.rs`)
   - `open.bigmodel.cn/api/paas/v4` → `api.z.ai/api/coding/paas/v4`
   - `ZAI_BASE_URL` 환경변수로 오버라이드 가능하게 변경
   - 빌드 성공, CLI에서 API 응답 수신 확인

2. **벤치마크 실행 시도**
   - Interview phase: ✅ 정상 동작 (15-25초)
   - Seed phase: ✅ 정상 동작 (20-30초)
   - Execute phase: ❌ Seed 생성 후 진입 안 함 (timeout)
   - 원인 미파악 — orchestrator.rs에서 seed 생성 후 execute 호출 로직 확인 필요

3. **계획서 작성**
   - `docs/recursive-improvement-loop.md` 작성
   - 기능 69개 인벤토리화, 테스트 매트릭스 작성
   - 4-Phase 개선 로드맵 수립

### 다음에 할 것 (Phase 1-1)
1. **테스트 컴파일 에러 52개 수정** — `cargo test --workspace` 가 먼저 통과해야 함
2. **Execute Phase 버그 수정** — `orchestrator.rs`에서 seed → execute 흐름 추적
3. 위 둘이 해결되면 `oxios run` E2E 테스트

### 미해결 이슈
- BUG-001: Execute Phase 진입 안 함 (orchestrator.rs)
- BUG-002: 테스트 52개 컴파일 에러
- ZAI_API_KEY를 매번 환경변수로 설정해야 함 (config.toml에 저장 안 됨)

### 커밋
- (아직 커밋 안 함 — 다음 세션에서 Phase 1-1 완료 후 커밋)
