# LobeHub Image Generation Port — Design Document

> **Date:** 2026-07-24
> **Scope:** 에이전트가 이미지를 생성할 수 있는 능력(backend provider client + kernel tool + chat rendering + 설정)을 Oxios에 additive로 추가. LobeHub의 16-provider runtime / tRPC server lambda / DB 기반 generation topic·batch·asset 지속화 / 전용 워크스페이스 UI는 포팅하지 않는다.
> **Source:** lobehub/lobehub (구 lobe-chat), canary HEAD `e698f8da`, 2026-07-24 clone. 분석은 ref-porter 보고서(본 세션)에 근거.
> **Companion to:**
> - `docs/designs/2026-07-21-lobehub-chat-port-design.md` (chat UX transplant — shipped)
> - `docs/designs/2026-07-21-lobehub-backend-streaming-design.md` (WS chunk protocol — shipped)
> - `docs/designs/2026-07-21-lobehub-port-remaining-work.md` (남은 작업 — 이미지 생성은 **미포함**, 본 문서가 신규 스코프)

---

## 0. TL;DR

| 질문 | 답 |
|---|---|
| 판정 | **부분 이식(Port partially).** "에이전트가 이미지를 생성한다"는 핵심 가지만 가져온다. LobeHub의 풀스택(72MB runtime + Postgres + serverless)은 Oxios 아키텍처(단일 Rust daemon, DB 없음)와 충돌. |
| 첫 provider | **OpenAI images (`/v1/images/generations`)**. 동기 호출, 기존 `[engine]` provider 키 재사용, 폴링 불필요. 모델(`gpt-image-1`/`dall-e-3`/…)은 **사용자 config 선택** — 클라이언트가 URL·b64_json 양쪽 응답을 견고히 처리해 항상 URL로 정규화(b64는 로컬 저장+서빙). fal.ai는 Phase 2(모델 다양성 + 비동기 폴링). |
| oxi-sdk 지원? | **없음(확정).** `oxi-sdk` 0.58.0의 `Provider` trait은 `stream()`/`name()`만, `OutputModality` 없음. 단, `oxi-ai`에 `ImageGenerationRequest`/`ImageGenerationResponse`/`ImagesApi` **dead-code 타입이 이미 정의**되어 있음(구현체 0, 미재-export). → Oxios 타입을 이 dead 타입에 **맞춰 두면** 향후 oxi-sdk 구현 시 교체 무비용. |
| 툴 결과는 어떻게? | `AgentToolResult.output`은 `String`이므로 JSON 직렬화 → 에이전트가 마크다운 `![](url)`로 재출력(systemRole이 유도). LobeHub `systemRole.ts:15`와 동일 패턴. |
| 백엔드 의존성 | `reqwest` 단 하나. 이미 `oxios-kernel`에서 사용 중(`Cargo.toml:98`). |
| MVP 범위 | [high] 3건: provider 클라이언트 + `generate_image` 툴 + 채팅 렌더링. 설정(#4)은 즉시 뒤따름. 비동기 폴링(#5)은 fal.ai 추가 시점에만. |

---

## 1. 배경 및 분석 결론

### 1.1 Oxios 현재 상태

이미지 생성 기능이 **전무**하다:

- **백엔드 없음.** `register_all_kernel_tools()`(`crates/oxios-kernel/src/tools/builtin/mod.rs:55`)에 14개 툴 등록, 이미지 툴 없음. `pub mod` 목록(linen 18-30)에도 없음.
- **출력 모달리티 개념 부재.** `OxiosEngine`(`crates/oxios-kernel/src/engine.rs`)은 oxi-sdk `Oxi` 래핑. `InputModality`는 `Text`/`Image`(비전 입력)만 있고 `OutputModality` 없음(`crates/oxios-kernel/src/kernel_handle/engine_api.rs:665-719`).
- **프론트엔드 스텁만 존재.** `web/src/routes/settings.tsx:868`에 `<ImageGenerationSettings defaultImageNum={4} onDefaultImageNumChange={() => {}} />` — LobeHub에서 가져온 **no-op** 설정 UI.
- **채팅 렌더링에 이미지 단계 없음.** `AssistantMessage.tsx:82-107` 파이프라인(Reasoning → SearchGrounding → FileChunks → MarkdownMessage → ToolCallList → FollowUpChips). 주석 line 7에만 "Images"가 미래 단계로 언급.

### 1.2 oxi-sdk dead-code 발견 (Phase 9 교차 검증)

`oxi-ai` 0.58.0의 `src/types.rs:335-395`에 다음 타입이 **정의되어 있으나 구현체 0, endpoint 호출 0, oxi-sdk 미재-export**:

```rust
pub enum ImagesApi { OpenRouter }
pub struct ImageGenerationRequest { pub prompt: String, pub model: Option<String>, /* … */ }
pub struct ImageGenerationResponse { pub images: Vec<Vec<u8>>, pub revised_prompt: Option<String> }
```

`Provider` trait(`oxi-ai/src/providers/trait_def.rs:12-28`)은 `fn stream(...)` + `fn name()`만. `image_generation_call`은 OpenAI Responses API의 wire-format 분류용 문자열일 뿐(`openai_responses.rs:723-740`).

**의미:** oxi진영도 이미지 생성을 염두에 두고 타입부터 정의했으나 아직 미구현. Oxios가 자체 클라이언트로 먼저 가되, **타입 shape을 위 dead 타입에 정렬**하면 향후 oxi-sdk가 구현·재-export할 때 교체가 거의 무비용이다. 단, oxi_ai는 `Vec<Vec<u8>>`(바이트)를 반환하지만 Oxios는 에이전트 소비를 위해 **URL**이 필요하다 → §4.3에서 이 간극을 처리한다.

### 1.3 LobeHub 구조 요약 (가져올 것 / 버릴 것)

```
LobeHub 이미지 생성 스택                      → Oxios 포팅?
─────────────────────────────────────────    ──────────────
builtin-tool-image-generation (4-API tool)    → 가져온다 (action 기반 툴로)
model-runtime (16 providers, createImage)     → 버린다 (1-2 provider 직접 구현)
model-bank (이미지 모델 카탈로그/파라미터/가격) → 버린다 (Oxios은 config 기반)
tRPC server lambda + Postgres (topic/batch)   → 버린다 (Oxios은 DB 없음, 세션 기반)
src/store/image (Zustand)                     → 버린다 (Oxios은 툴 결과 기반, 별도 store 불필요)
src/routes/(create)/image (전용 워크스페이스)   → 버린다 (채팅 내 툴 결과로 충분)
client/Render/GenerateImage.tsx (이미지 렌더)  → 가져온다 (tool-render registry에)
ExecutionRuntime 비동기 폴링                    → Phase 2에서만 (fal.ai 추가 시)
```

---

## 2. 아키텍처

### 2.1 LobeHub 데이터 흐름 (참고용)

```
Agent → builtin tool(generateImage) → ExecutionRuntime → imageService(tRPC lambda)
      → server → LobeFalAI.createImage → fal.subscribe(endpoint) → async task
      ← poll getImageGenerationStatus (3s 간격, 175s 타임아웃) ← imageUrl
  UI: store/image(generationConfig/Batch/Topic) → /(create)/image 라우트
```

### 2.2 Oxios 목표 데이터 흐름

```
Agent → generate_image 툴(action="generate")
      → ImageGenClient::generate(req) → reqwest POST {provider}/images/generations
      ← { image_url, width, height }  (동기; OpenAI는 즉시 반환)
      → AgentToolResult.output = JSON { images:[{url,width,height}], prompt, model }
      → 에이전트가 마크다운 ![](url) 재출력 (systemRole 유도)
  UI: tool-renders/ImageGeneration.tsx가 툴 결과 렌더 (이미지 그리드 + 다운로드)
```

Oxios는 **별도 store가 불필요**하다 — 이미지는 툴 호출 결과로 스트림에 포함되고 `StreamProcessor`가 `toolCalls[].result`에 저장한다. LobeHub의 전용 `store/image`는 DB 지속화 + 전용 워크스페이스가 필요해서 존재하는 것인데, Oxios은 둘 다 버렸으므로 툴 결과 렌더링으로 충분하다.

---

## 3. Provider 선택

| 후보 | 호출 방식 | 반환 | 모델 | Oxios 키 재사용 | Phase |
|---|---|---|---|---|---|
| **OpenAI images** | 동기(`/v1/images/generations`) | URL 또는 b64_json | `gpt-image-1`, `dall-e-3` | ✅ `[engine]` provider 키 / `~/.oxi/auth.json` | **1** |
| **fal.ai** | 비동기(subscribe + poll) | URL | Flux, SDXL, Seedream, Nano Banana 등 다수 | ❌ 별도 fal 키 | 2 |
| Replicate | 비동기(prediction + poll) | URL | 다수 | ❌ 별도 키 | (후보) |

**권장: OpenAI images를 Phase 1로.** 이유: (1) 동기 호출 → 폴링 로직(#5) 생략 가능, (2) Oxios 사용자는 이미 OpenAI 호환 provider를 `[engine]`에 설정 중이므로 추가 자격증명 불필요, (3) API가 가장 단순. fal.ai는 모델 다양성이 필요해질 때 Phase 2로 추가(이때 비동기 폴링 #5가 함께 들어온다).

> **모델 선택은 사용자 config 영역이다.** API 키를 보유한 사용자가 `gpt-image-1`, `dall-e-3`, 또는 OpenAI 호환 엔드포인트의 임의 모델을 고른다. 아키텍처 결정은 모델이 아니라 **provider 클라이언트가 URL 반환 모델과 b64_json 반환 모델을 모두 견고히 처리하는 것**이다(§4.3). 모델별 반환 포맷이 달라(예: `gpt-image-1`은 항상 b64, `dall-e-3`은 URL), 클라이언트는 응답을 항상 URL로 정규화한다 — b64면 로컬 저장 후 서빙. 따라서 로컬 저장/서빙은 Phase 1 요구사항이다.

---

## 4. 백엔드 설계 — `image_gen` 모듈

### 4.1 모듈 구조

```
crates/oxios-kernel/src/image_gen/
├── mod.rs              # pub trait ImageGenProvider + re-exports
├── types.rs            # ImageGenRequest, ImageGenResult, GeneratedImage
├── openai.rs           # OpenAiImageProvider (Phase 1)
└── fal.rs              # FalImageProvider (Phase 2)
```

`oxios-kernel`에 추가. 의존성: `reqwest`(이미 workspace dep), `serde`, `tokio`.

### 4.2 Provider trait + 타입 (oxi_ai dead 타입 정렬)

```rust
// crates/oxios-kernel/src/image_gen/types.rs
// NOTE: 필드명/shape은 oxi-ai의 dead 타입(ImageGenerationRequest/Response,
// oxi-ai/src/types.rs:335-395)에 정렬. 향후 oxi-sdk가 구현·재-export 시 교체 무비용.

use serde::{Deserialize, Serialize};

/// oxi-ai ImageGenerationRequest에 대응 (prompt + model + provider params).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageGenRequest {
    pub prompt: String,
    pub model: Option<String>,        // None = provider 기본값
    pub n: u8,                         // 생성할 이미지 수 (1-8)
    pub size: Option<ImageSize>,       // None = provider 기본값
    pub quality: Option<String>,       // "standard" | "hd" (OpenAI)
    pub reference_image_url: Option<String>, // image-to-image (Phase 2)
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ImageSize {
    #[serde(rename = "1024x1024")] Square1024,
    #[serde(rename = "1792x1024")] Landscape1792,
    #[serde(rename = "1024x1792")] Portrait1792,
}

/// oxi-ai ImageGenerationResponse.images: Vec<Vec<u8>> (바이트)와 다르게
/// Oxios는 URL 기반으로 에이전트가 마크다운으로 소비하게 한다.
/// 바이트만 주는 provider는 openai.rs에서 URL 우선, b64는 로컬 저장 후 URL 서빙(§4.3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageGenResult {
    pub images: Vec<GeneratedImage>,
    pub revised_prompt: Option<String>,
    pub provider: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedImage {
    pub url: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}
```

```rust
// crates/oxios-kernel/src/image_gen/mod.rs
use async_trait::async_trait;
use crate::error::KernelError;

#[async_trait]
pub trait ImageGenProvider: Send + Sync {
    /// provider 식별자 ("openai" | "fal" | …)
    fn name(&self) -> &'static str;

    /// 동기 생성. OpenAI는 즉시 반환.
    /// fal/replicate는 Phase 2에서 내부 폴링 후 반환(최대 타임아웃까지).
    async fn generate(
        &self,
        req: &ImageGenRequest,
        api_key: &str,
    ) -> Result<ImageGenResult, KernelError>;
}

pub mod types;
mod openai;
// mod fal;  // Phase 2

pub use openai::OpenAiImageProvider;
pub use types::*;
```

### 4.3 응답 정규화 — 항상 URL로 (Phase 1 요구사항)

에이전트는 마크다운 `![](url)`로 이미지를 소비하므로, provider 클라이언트는 모델·provider에 무관하게 **항상 URL을 반환**해야 한다. 모델별 반환 포맷이 다르기 때문에(사용자가 어떤 모델을 고르든 처리해야 한다):

- **provider가 `url`을 반환** → 그대로 사용. 단, OpenAI URL은 ~1시간 만료이므로 툴 결과에 만료 시각 메타데이터 포함("빨리 저장하라" 안내).
- **provider가 `b64_json`을 반환**(`gpt-image-1` 등) → `~/.oxios/workspace/images/<uuid>.png`로 디코딩 저장 후 `GET /api/images/<uuid>` 서빙 URL로 정규화.

이 정규화는 Phase 1에 포함된다 — b64만 반환하는 모델을 사용자가 선택할 수 있으므로 로컬 저장/서빙은 Phase 2가 아니다. oxi_ai `ImageGenerationResponse.images: Vec<Vec<u8>>`(바이트) shape는 이 로컬 저장 경로와 자연스럽게 정렬된다 — 향후 oxi-sdk가 바이트 기반으로 구현되더라도 Oxios의 store→serve 레이어가 호환 adapter 역할을 한다.

### 4.4 OpenAI provider 구현 개요

```rust
// crates/oxios-kernel/src/image_gen/openai.rs (요약)
#[derive(Deserialize)]
struct OpenAiImageResponse {
    data: Vec<OpenAiImage>,
    // revised_prompt는 dall-e-3만; gpt-image-1 미지원
}
#[derive(Deserialize)]
struct OpenAiImage { url: Option<String>, b64_json: Option<String> }

pub struct OpenAiImageProvider { client: reqwest::Client, base_url: String }

#[async_trait]
impl ImageGenProvider for OpenAiImageProvider {
    fn name(&self) -> &'static str { "openai" }

    async fn generate(&self, req: &ImageGenRequest, api_key: &str)
        -> Result<ImageGenResult, KernelError>
    {
        // model은 사용자 config 기본값(config.rs) 또는 툴 인자로 override.
        // 어떤 모델이든 응답은 url | b64_json → 항상 URL로 정규화(§4.3).
        let body = serde_json::json!({
            "model": req.model,             // None이면 config 기본값 사용
            "prompt": req.prompt,
            "n": req.n.clamp(1, 8),
            "size": req.size.map(|s| s.as_str()).unwrap_or("1024x1024"),
        });
        let resp: OpenAiImageResponse = self.client
            .post(format!("{}/images/generations", self.base_url))
            .bearer_auth(api_key)
            .json(&body)
            .send().await.map_err(map_http_err)?
            .error_for_status().map_err(map_http_err)?
            .json().await.map_err(map_http_err)?;

        // 모델 무관 정규화: url이면 그대로, b64_json이면 로컬 저장 + 서빙 URL(§4.3)
        let images = resp.data.into_iter().map(|img| match (img.url, img.b64_json) {
            (Some(url), _) => Ok(GeneratedImage { url, width: None, height: None }),
            (None, Some(b64)) => persist_b64(&b64)        // → /api/images/<uuid>
                .map(|u| GeneratedImage { url: u, width: None, height: None }),
            (None, None) => Err(KernelError::ImageProvider("no url/b64 in response".into())),
        }).collect::<Result<_, _>>()?;

        Ok(ImageGenResult {
            images,
            provider: "openai".into(),
            model: req.model.clone().unwrap_or_default(),
            revised_prompt: None,
        })
    }
}
```

> **정규화는 모델 무관.** `gpt-image-1`(항상 b64)이든 `dall-e-3`(URL)이든 사용자가 config에서 고른 모델이 무엇이든, `match` 분기가 URL로 정규화하므로 클라이언트/툴/프론트엔드는 URL만 다룬다. b64 분기의 `persist_b64`는 §4.3의 로컬 저장(`~/.oxios/workspace/images/<uuid>.png`) + `/api/images/<uuid>` 서빙을 수행한다.

---

## 5. 커널 툴 설계 — `generate_image`

### 5.1 툴 구조 (ProjectTool 패턴 준용)

```rust
// crates/oxios-kernel/src/tools/builtin/image_generation_tool.rs
use oxi_sdk::{AgentTool, AgentToolResult, ToolContext};
// pub struct ImageGenerationTool { kernel: Option<Arc<KernelHandle>> }
// impl AgentTool — action 기반 디스패치 (project_tool.rs:50-232 패턴)
```

action 파라미터로 LobeHub의 4-API를 매핑:

| `action` | LobeHub API | 설명 |
|---|---|---|
| `generate` | `generateImage` | 이미지 생성 (핵심) |
| `list_models` | `listImageModels` | 사용 가능한 provider/모델 목록 (config 기반 정적) |
| `get_status` | `getImageGenerationStatus` | 비동기 provider 상태 확인 (Phase 2 전용) |

`getImageModelParameters`(`getImageModelParameters`)는 Oxios에서 생략 — 모델 파라미터 스키마가 고정(`ImageGenRequest`)이므로 툴 description에 명시한다.

### 5.2 파라미터 스키마

```json
{
  "type": "object",
  "properties": {
    "action": { "type": "string", "enum": ["generate", "list_models", "get_status"] },
    "prompt": { "type": "string", "description": "Text-to-image prompt (action=generate 필수)" },
    "model": { "type": "string", "description": "provider/model (생략 시 기본값)" },
    "n": { "type": "integer", "minimum": 1, "maximum": 8, "default": 1 },
    "size": { "type": "string", "enum": ["1024x1024", "1792x1024", "1024x1792"] },
    "quality": { "type": "string", "enum": ["standard", "hd"] }
  },
  "required": ["action"]
}
```

### 5.3 툴 결과 shape + systemRole

`AgentToolResult.output`(`String`)에 JSON 직렬화:

```json
{
  "action": "generate",
  "images": [
    { "url": "https://.../img.png", "width": 1024, "height": 1024 }
  ],
  "prompt": "a cat",
  "provider": "openai",
  "model": "gpt-image-1"
}
```

systemRole은 에이전트에게 이 결과를 마크다운으로 재출력하도록 지시한다 — LobeHub `systemRole.ts:15`("copying the markdown image tags returned by generateImage exactly. Do not rewrite, shorten, translate, or rebuild the image URLs")를 그대로 준용:

```text
When generation completes, show the generated images by emitting markdown
image tags ![](url) for each image URL in the tool result. Copy URLs exactly
— do not rewrite, shorten, or translate them. Include a brief caption only.
```

### 5.4 등록

```rust
// crates/oxios-kernel/src/tools/builtin/mod.rs
pub mod image_generation_tool;          // line ~30에 추가
// ...
pub fn register_all_kernel_tools(...) {
    // ...
    registry.register(ImageGenerationTool::from_kernel(kernel));  // line ~89에 추가
}
```

---

## 6. 프론트엔드 설계

### 6.1 tool-render registry 등록

`web/src/components/chat/tool-renders/`에 신규 컴포넌트 + registry 등록. 기존 패턴(`registry.tsx:51` `registerToolRender`) 준용:

```tsx
// web/src/components/chat/tool-renders/ImageGeneration.tsx
import type { ToolRenderProps } from './registry'

export function ImageGenerationRender({ args, result, isRunning }: ToolRenderProps) {
  // result(JSON 문자열) 파싱 → images[].url → 그리드 렌더
  // LobeHub builtin-tool-image-generation/src/client/Render/GenerateImage.tsx 참조
  //  - LoadingState: isRunning 시 스피너 + 경과 시간
  //  - SuccessState: 이미지 그리드 + 다운로드 버튼
  //  - ErrorState: 에러 메시지 + 재시도(args 그대로)
}
```

등록 위치: registry 초기화 파일에서 `registerToolRender('image_generation', ImageGenerationRender)` (또는 툴이 노출하는 identifier). ToolCallList/ToolInspector가 `getToolRender(toolName)`으로 조회(`registry.tsx:67`).

### 6.2 AssistantMessage 파이프라인

`AssistantMessage.tsx:82-107`의 주석 line 7 "Images" 단계는 툴 결과 렌더링(tool-renders)으로 흡수된다 — 별도 파이프라인 단계 불필요. 마크다운 `![](url)`은 이미 `MarkdownMessage`(remark-gfm)가 `<img>`로 렌더하므로, 툴 inspector 내 그리드 + 본문 마크다운 이미지가 함께 표시된다.

> **확인 필요(§9.3):** `markdown-message.tsx`의 rehype-sanitize 설정이 `<img>`를 허용하는지. 허용하지 않으면 sanitize 스키마에 `img: ['src', 'alt']` 추가.

### 6.3 설정 UI 실연결

`web/src/routes/settings.tsx:868`의 dead 스텁을 실 config API에 연결:

```tsx
// AS-IS (no-op):
<ImageGenerationSettings defaultImageNum={4} onDefaultImageNumChange={() => {}} />

// TO-BE: apiClient로 config 읽기/쓰기
const [cfg, setCfg] = useState(/* GET /api/config/image-gen */)
<ImageGenerationSettings
  defaultImageNum={cfg.default_num}
  provider={cfg.provider}
  model={cfg.model}
  onDefaultImageNumChange={(n) => api.patch('/api/config/image-gen', { default_num: n })}
  onProviderChange={(p) => api.patch('/api/config/image-gen', { provider: p })}
/>
```

`image-generation-settings.tsx` 컴포넌트 자체는 LobeHub에서 이미 가져왔으므로, prop을 no-op에서 실 핸들러로 교체 + provider/model 셀렉트 추가.

---

## 7. 설정 설계

### 7.1 config.toml (token-maxing 패턴 미러링)

```toml
# ── Image Generation ───────────────────────────────────────────────────────
# 에이전트가 generate_image 툴로 이미지를 생성한다.
# 비활성화하면 툴이 등록되지 않는다.
[image-gen]
enabled = false            # 옵트인
default_provider = "openai"
default_model = ""         # 빈 = provider 기본값 (openai → gpt-image-1)
default_num = 1
# 생성된 이미지 로컬 저장 (만료 URL 보존). 빈 = ~/.oxios/workspace/images/
# storage_path = ""

# provider별 오버라이드. token-maxing의 [[token-maxing.providers]] 패턴 준용.
[[image-gen.providers]]
provider = "openai"
# model 오버라이드 (선택). 빈 = default_model 사용.
base_url = "https://api.openai.com/v1"   # OpenAI 호환 엔드포인트
# quality_default = "standard"
```

API 키는 `[secrets.providers]`(`default-config.toml:22-26`)에 추가 — 환경변수 폴백 포함:

```toml
[secrets.providers]
openai = ""   # OXIOS_OPENAI_API_KEY 폴백; [engine].api_key / ~/.oxi/auth.json 상속
```

우선순위: `[image-gen]` 명시 → `[secrets.providers]` → `~/.oxi/auth.json` → 환경변수. 이는 기존 config 우선순위 규칙(`default-config.toml:7`)과 일치.

### 7.2 config Rust 구조체

```rust
// crates/oxios-kernel/src/config.rs (기존 Config에 병합)
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ImageGenConfig {
    pub enabled: bool,
    pub default_provider: String,
    pub default_model: String,
    pub default_num: u8,
    pub storage_path: Option<String>,
    pub providers: Vec<ImageGenProviderConfig>,
}
```

---

## 8. 구현 페이즈

| Phase | 범위 | 산출물 | 검증 |
|---|---|---|---|
| **1a** | provider 클라이언트(`image_gen/`) + OpenAI 구현 + **응답 정규화(URL/b64 → URL, b64 로컬 저장 + `/api/images/<uuid>` 서빙)** + 단위 테스트(mock HTTP) | `OpenAiImageProvider` + `persist_b64` | `cargo test -p oxios-kernel image_gen` (mock 서버로 url/b64 양쪽 파싱 assertion) |
| **1b** | `generate_image` 툴 + 등록 + systemRole | `ImageGenerationTool` | 단위 테스트(출력 JSON shape) + smoke `oxios run --json "고양이 그림 그려줘"` |
| **1c** | config(`[image-gen]`) + secret 연동 + API 엔드포인트 | config.rs + `/api/config/image-gen` | config 직렬화/역직렬화 테스트 + secret 마스킹 |
| **1d** | 프론트엔드: tool-render `ImageGeneration.tsx` + registry 등록 + 설정 UI 실연결 | `tool-renders/ImageGeneration.tsx` + settings wiring | `bun run typecheck && test` + 브라우저 smoke(툴 결과 렌더 확인) |
| **2** | fal.ai provider + 비동기 폴링(모델 다양성) | `FalImageProvider` | 폴링 타임아웃 테스트 |

Phase 1a-d가 MVP(단일 동기 provider, 응답 정규화 포함, 채팅 내 이미지 생성). Phase 2는 fal.ai 모델 다양성 + 비동기 폴링.

---

## 9. 리스크 및 검증

### 9.1 [high] 응답 정규화 — 모델별 URL/b64 포맷 차이

모델은 사용자 config 선택이므로 클라이언트는 어떤 모델이 오든 URL·b64_json 양쪽을 처리해야 한다. `gpt-image-1`은 항상 `b64_json`(만료 없음, 디코딩 필요), `dall-e-3`은 `url`(~1시간 만료). OpenAI 호환 엔드포인트(로컬 SD/WebUI 등)는 또 다른 포맷일 수 있다.

**처리:** §4.3의 정규화 — `match (url, b64_json)`로 URL을 항상 반환(b64는 로컬 저장 + 서빙). 이것이 아키텍처 결정이지 "기본 모델 선택"이 아니다. **검증:** `cargo test -p oxios-kernel image_gen`에서 mock 서버가 (a) `url` 응답, (b) `b64_json` 응답 두 케이스를 각각 반환하도록 하고, 두 경우 모두 `ImageGenResult.images[].url`이 유효한 URL(로컬 서빙 URL 포함)로 나오는지 assertion. `clippy`는 잡지 못한다.

### 9.2 [high] 툴 결과 → 에이전트 마크다운 재출력 신뢰성

`AgentToolResult.output`이 `String`이라 에이전트가 JSON을 해석해 마크다운으로 다시 쓰는 단계가 있다. 모델이 URL을 변형/누락할 수 있다.

**검증:** 통합 smoke에서 응답 마크다운에 툴이 반환한 URL이 정확히 포함되는지 assertion. systemRole에 "URL을 그대로 복사하라"는 지시(LobeHub 검증된 문구)로 완화. 최후의 보루: 툴 결과를 그대로 UI에서도 렌더(tool-render)하므로, 에이전트가 마크다운을 생략해도 사용자는 툴 inspector에서 이미지를 본다.

### 9.3 [medium] rehype-sanitize `<img>` 허용 여부

`markdown-message.tsx`의 sanitize 스키마를 확인하지 않았다. `<img>`가 차단되면 마크다운 이미지가 깨진다. **검증:** `grep img markdown-message.tsx` 및 sanitize 스키마 리딩 → 필요 시 `img: ['src', 'alt', 'title']` 추가.

### 9.4 [medium] content-policy / 과금 에러

이미지 생성 provider는 프롬프트 정책 위반 시 400/403을 반환. 이를 그대로 에이전트에 전달하면 무한 재시도할 수 있다. LobeHub `systemRole.ts:17`("do not retry the unchanged request automatically") 패턴 줸용 — systemRole에 정책 에러 시 재시도 금지 지시.

### 9.5 [low] oxi_ai dead 타입 정렬 유지

Oxios 타입명(`ImageGenRequest`/`ImageGenResult`)이 oxi_ai(`ImageGenerationRequest`/`ImageGenerationResponse`)와 다르다. 향후 oxi-sdk 재-export 시 일괄 리네임 필요. 완화: 이름을 지금부터 `ImageGenerationRequest`/`ImageGenerationResponse`로 맞추되, 응답은 `Vec<GeneratedImage { url, .. }>`(URL 기반)로 두어 oxi_ai 바이트 shape와 의도적으로 다르게 — 호환 시점에 adapter 작성.

### 9.6 검증 게이트

```bash
# Phase 1 전체 완료 후
cargo clippy -D warnings -p oxios-kernel
cargo test -p oxios-kernel image_gen          # provider 단위 테스트
cargo test --workspace --doc
oxios run --json "고양이 그림 그려줘"           # 통합 smoke
cd web && bun run typecheck && bun run test && bun run build
```

---

## 10. 미해결 질문 (구현 전 결정)

> 모델 선택은 사용자 config 영역이므로 설계 질문에서 제외한다(§3, §4.3). 응답 정규화로 어떤 모델이든 처리한다.

1. **툴 식별자 이름:** `image_generation` vs `generate_image` vs `image_gen`? 기존 툴 이름 규칙(`*_tool`, action 기반)과 정렬 필요.
2. **`getImageModelParameters` 생략 확정?** Oxios은 파라미터가 고정이라 툴 description으로 충분하지만, provider별 차이(quality 지원 여부 등)를 동적으로 노출할지.

이 2개에 답이 오면 Phase 1a부터 구현에 들어간다.
