# Progress

## Status
In Progress

## Tasks

### Phase 2: EngineApi + Backend 웹 API 라우트
- [x] 1. `engine_api.rs` 생성 — 읽기 전용 퍼사드 + 쓰기 작업 (providers, models, search, config, set_model, set_api_key, set_provider_options)
- [x] 2. `KernelHandle`에 `EngineApi` 추가 — `engine` 필드, 생성자 15번째 인자
- [x] 3. `engine_routes.rs` 생성 — 7개 핸들러 (GET providers/models/config, PUT model/api-key/provider-options, POST validate-key)
- [x] 4. `routes/mod.rs`에 라우트 등록 — engine_routes 모듈 + build_routes()에 7개 엔진 라우트
- [x] 5. `kernel.rs` — KernelHandle 생성 시 EngineApi 전달 (2곳: handle() + build())
- [x] cargo build 통과 확인

### Phase 3: Frontend — Engine 설정 UI
- [x] 1. types/engine.ts 생성 — ProviderInfo, ModelInfo, EngineConfig, ProviderOptions, ApiKeySource types
- [x] 2. hooks/use-engine.ts 생성 — TanStack Query hooks: useProviders, useModels, useEngineConfig, useSetModel, useSetApiKey, useSetProviderOptions
- [x] 3. components/engine/ 디렉토리 생성
  - [x] provider-select.tsx — 카테고리별 그룹 드롭다운, has_key 상태 아이콘
  - [x] model-select.tsx — reasoning ✦, vision 👁 아이콘, context window, 가격 정보
  - [x] api-key-input.tsx — 상태 표시 (env/auth_store/config/none), 마스킹 입력
  - [x] provider-options.tsx — Anthropic/OpenAI/Google 동적 렌더링
- [x] 4. routes/settings.tsx 수정 — EnginePanel 컴포넌트로 Engine 탭 교체

## Files Changed
- `crates/oxios-kernel/src/kernel_handle/engine_api.rs` — NEW: EngineApi facade with providers(), models(), search_models(), config(), set_model(), set_api_key(), set_provider_options(), validate_key()
- `crates/oxios-kernel/src/kernel_handle/mod.rs` — MODIFIED: Added engine_api module, EngineApi field to KernelHandle, 15th constructor arg
- `crates/oxios-kernel/src/lib.rs` — MODIFIED: Re-export EngineApi, EngineConfigResponse, ModelInfo, ProviderInfo, ValidateKeyResult
- `channels/oxios-web/src/routes/engine_routes.rs` — NEW: 7 axum handlers for /api/engine/* endpoints
- `channels/oxios-web/src/routes/mod.rs` — MODIFIED: Added engine_routes module, re-exports, and 7 routes in build_routes()
- `src/kernel.rs` — MODIFIED: Added config_path to Kernel struct, EngineApi creation in both KernelHandle::new() sites
- `channels/oxios-web/web/src/types/engine.ts` — NEW: Engine-related TypeScript types
- `channels/oxios-web/web/src/hooks/use-engine.ts` — NEW: TanStack Query hooks for engine config
- `channels/oxios-web/web/src/components/engine/provider-select.tsx` — NEW: Provider selection dropdown with category groups
- `channels/oxios-web/web/src/components/engine/model-select.tsx` — NEW: Model selection with reasoning/vision icons and pricing
- `channels/oxios-web/web/src/components/engine/api-key-input.tsx` — NEW: API key input with source status
- `channels/oxios-web/web/src/components/engine/provider-options.tsx` — NEW: Per-provider advanced options (Anthropic/OpenAI/Google)
- `channels/oxios-web/web/src/routes/settings.tsx` — MODIFIED: Engine tab now uses EnginePanel with rich UI components

## Notes
- cargo build passes cleanly (no errors, only pre-existing warnings)
- EngineApi only references config (Arc<RwLock<OxiosConfig>>) + config_path — no Oxi instance, Supervisor, or runtime references
- validate_key does basic sanity checks (provider exists, models available, key non-empty) — full API call validation is a future enhancement
- ProviderOptions are per-request in oxi-sdk; set_provider_options is a no-op placeholder for future config.toml persistence
- The Kernel struct now stores config_path (PathBuf) so EngineApi can persist config changes
- Frontend hooks can now hit real /api/engine/* endpoints (previously used static fallback)
