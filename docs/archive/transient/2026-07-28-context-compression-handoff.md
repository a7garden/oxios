# 컨텍스트 압축(Context Compression) — 작업 핸드오프 브리프

> **목적**: 별도 세션에서 이어서 진행할 작업의 범위·참고 구현·결정 사항을 담은 브리프. 설계 문서가 아님.
> **작성일**: 2026-07-28
> **상위 맥락**: LobeHub 채팅 패턴 차용 작업의 후속. Round 1(가상화·shiny 추론) 완료, Round 2(shiny 도구·atom 아이콘·CompressedGroup 탭) 설계 완료. 이 문서는 Round 2에서 **대체재로 처리한 Summary 탭을 "진짜 LLM 요약"으로 구현**하는 별도 작업이다.

---

## 1. 목표

긴 대화의 오래된 메시지를 **LLM이 요약**해 압축하고, 웹 UI의 접힌 그룹(CompressedGroup) **Summary 탭에서 그 요약을 보여준다.** 현재 Oxios의 CompressedGroup은 단순 시각적 접기(40개 초과 시 오래된 메시지 숨김)일 뿐, 요약 기능이 없다.

## 2. 배경 / 왜

- LobeHub의 CompressedGroup은 두 탭: **Summary**(LLM 생성 요약, `message.content`) + **History**(원본 메시지, `compressedMessages`).
- Oxios는 이 백엔드 기능이 없어, Round 2 설계(`docs/superpowers/plans/2026-07-28-lobehub-chat-borrow-2.md`)는 Summary 탭을 **클라이언트 통계 다이제스트**로 대체하기로 했음.
- 사용자가 "B(진짜 LLM 요약)"를 선택 → 이 작업으로 분리.

## 3. 현재 상태 (Oxios)

- `web/src/components/chat/compressed-group.tsx` — 제어형 접기 토글 (`{count, expanded, onToggle}`). Round 1에서 리팩터됨.
- `web/src/routes/chat.tsx` — virtua `VList` + `buildChatRows` 행 모델. 접기 바는 `collapse-bar` 행. `COLLAPSE_THRESHOLD=40`, `VISIBLE_TAIL=20`.
- `web/src/lib/chat-rows.ts` — 순수 행 모델.
- **백엔드**: 컨텍스트 압축/요약 기능 없음. 세션은 `StateStore`에 user_messages/agent_responses/trajectory/reasoning 저장.
- Round 2 계획에 `buildCompressedDigest`(통계 다이제스트)가 포함되어 있으나, **이 작업(B)에서는 그것이 아니라 LLM 요약을 구현**한다. (통계 다이제스트는 요약 생성 중/실패 시 폴백으로 쓸 수 있음.)

## 4. LobeHub 참고 구현 (읽을 것)

`/tmp/lobehub` (없으면 `git clone --depth 1 https://github.com/lobehub/lobehub.git`):

| 파일 | 내용 |
|---|---|
| `src/features/Conversation/Messages/CompressedGroup/index.tsx` | Summary/History 탭 UI, 요약 스트리밍 렌더 |
| `src/features/Conversation/Messages/CompressedGroup/logic.ts` | `isCompressionSummaryGenerating`, `shouldShowCompressedGroupPanel` |
| `src/store/chat/agents/transports/ClientCompressionTransport.ts` | 압축 실행 트랜스포트 (LLM 호출) |
| `src/store/chat/utils/compression.ts` | 메시지를 압축 그룹으로 묶는 로직 |
| `src/store/chat/slices/operation/types.ts` | `generateSummary` / `contextCompression` 연산 타입 |
| `src/store/chat/slices/agentRun/actions/entries/conversationLifecycle.ts` | 압축 라이프사이클 진입점 |
| `src/services/message/index.ts` | 압축 관련 메시지 서비스 API |
| `packages/types/src/message/ui/chat.ts` | `compressedMessages`, `CompressionGroupMetadata` 타입 |

## 5. 해야 할 일 (고수준)

1. **백엔드: 요약 생성** — 세션의 오래된 메시지(또는 전체)를 LLM에 넘겨 요약을 생성하는 기능. Oxios의 `OxiosEngine`/`OxiBuilder`로 모델 호출. 결과(요약 텍스트)를 세션 메타데이터 또는 별도 필드에 저장.
2. **백엔드: 압축 트리거** — 언제 압축할지 (임계값 초과 시 자동 / 사용자 수동 / `/compress` 명령). LobeHub는 자동(임계값) + 수동 취소.
3. **백엔드: API/WS** — 요약 생성 상태를 프론트에 전달 (스트리밍 또는 완료 이벤트). 기존 RFC-015 WS 청크 패턴 참고.
4. **프론트: 세션 데이터** — `loadSession`이 요약 + 원본 메시지(`compressedMessages` 상당)를 받도록.
5. **프론트: CompressedGroup Summary 탭** — 요약을 마크다운으로 렌더 (스트리밍 지원). History 탭은 원본 메시지 목록.
6. **프론트: 폴백** — 요약 생성 중/실패 시 통계 다이제스트(Round 2의 `buildCompressedDigest`) 또는 로딩 표시.

## 6. 결정할 사항 (별도 세션에서)

- **압축 단위/전략**: 전체 대화를 한 요약으로? 슬라이딩 윈도우? LobeHub는 그룹 단위. Oxios 세션 모델에 맞게 결정.
- **트리거**: 자동 임계값(몇 개 메시지?) vs 수동. 둘 다?
- **요약 저장 위치**: 세션 메타데이터 vs 새 필드 vs 별도 레코드. `StateStore` 스키마 영향.
- **원본 보존**: 압축 후 원본 메시지를 유지하나 (History 탭용) vs 삭제. LobeHub는 `compressedMessages`로 보존.
- **요약 모델**: 기본 모델 vs 저렴한 모델 고정.
- **스트리밍 여부**: 요약을 실시간 스트리밍(LobeHub 방식) vs 완료 후 표시.

## 7. 인수 기준

- 긴 대화(임계값 초과)에서 오래된 메시지가 요약으로 압축된다.
- CompressedGroup Summary 탭에 LLM 요약이 (마크다운으로) 보인다.
- History 탭에 원본 메시지가 보인다.
- 요약 생성 중 상태가 UI에 표시된다.
- 새로고침 후에도 요약이 유지된다 (영속화).
- 기존 채팅/가상화/스트리밍 동작에 회귀 없음.

## 8. 제약 / 주의

- **oxi-sdk 재구현 금지** — 모델 호출은 crates.io의 `oxi-sdk`(`OxiosEngine`/`OxiBuilder`) 경유. (`AGENTS.md`)
- **문서 규칙** — 분석/진행 파일을 루트에 만들지 말 것. RFC/설계는 `docs/rfc-NNN-*.md` 또는 `docs/designs/`. (`AGENTS.md`)
- **i18n** — 새 문자열은 `en.json`/`ko.json` 둘 다.
- **CI 게이트** — `cargo fmt && clippy -D warnings && cargo test --workspace` (Rust), `bunx tsc --noEmit && bunx biome check src && bun run vitest run && bun run build` (web).
- **⚠️ 동시 세션 주의** — 이 저장소에는 검색 패널 등을 작업하는 **다른 에이전트 세션**이 동시에 커밋할 수 있음. `web/src/stores/chat.ts`, `web/src/routes/chat.tsx` 등에 동시 편집 충돌이 발생한 전례가 있음. 작업 전 `git pull`/상태 확인, 커밋 전 `git diff`로 자기 변경만 스테이지됐는지 확인.
- **관련 선행 작업 커밋**: `26c5cab`(virtua 가상화), `9bbd2f8`(행 모델), `6a942cc`(CompressedGroup 제어형), `30c73e5`(shiny 추론). Round 2 계획: `docs/superpowers/plans/2026-07-28-lobehub-chat-borrow-2.md`.
