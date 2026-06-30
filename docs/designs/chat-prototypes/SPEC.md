# Chat Prototype — Shared Spec (read FIRST)

Every prototype renders the IDENTICAL content payload below, using the IDENTICAL
design tokens below. Do NOT invent different content. Do NOT change the tokens.
The ONLY thing that differs across prototypes is the SPATIAL PARADIGM.

## The real problem we are solving

Today the assistant's rich output — ReactMarkdown with code highlighting, tables,
`ToolCallCard`, `ActivityTimeline`, error cards — is crammed into an 80%-width
`bg-muted` bubble (`web/src/components/chat/message-bubble.tsx:63-135`).
A 30-line code block or a 6-column table inside that narrow bubble is the actual
wart the user means by "doesn't need to be bubbles." Every prototype MUST render
the code block and the table at FULL readable width, never squeezed into a chat
bubble. If your layout puts the code block inside a rounded bubble <70% width,
you have failed.

## Canonical content payload (render VERBATIM)

### Turn 1 — user prompt
```
Oxios 커널이 oxi-sdk에 의존하는 부분과 자체 구현하는 부분을 구분해줘. 그리고 왜 커널을 단일 크레이트로 유지하는지도.
```

### Turn 1 — assistant turn (rich)

**Reasoning step 1**
> 사용자가 커널↔oxi-sdk 의존성 경계를 묻고 있음. ARCHITECTURE.md §10(모놀리식 근거)과 워크스페이스 Cargo.toml을 확인해야 함.

**Tool call 1** — `read_file`
- input: path = `docs/ARCHITECTURE.md`
- output excerpt:
  > §10 Monolithic Rationale — The kernel is intentionally a single crate. Star topology around `AgentId` / `EventBus` / `StateStore` prevents circular deps. Internal boundaries via `pub(crate)` + directory mod files. Do not propose splitting.
- meta: 2340 lines · 198 ms

**Reasoning step 2**
> star topology 확인. oxi-sdk는 crates.io 전용 정책 — path dep 금지, 재구현 금지.

**Tool call 2** — `grep`
- input: pattern = `oxi-sdk`, glob = `**/Cargo.toml`
- output:
  ```
  Cargo.toml:31:             oxi-sdk = "0.45"
  crates/oxios-kernel/Cargo.toml:12: oxi-sdk = { workspace = true }
  crates/oxios-memory/Cargo.toml:8:  oxi-sdk = { workspace = true }
  ```
- meta: 3 matches · 42 ms

**Answer body (markdown — render with real styling)**
```
## 의존성 경계 — oxi-sdk vs 자체 구현

Oxios 커널(`oxios-kernel`)은 의도적으로 **단일 모놀리식 크레이트**다.
`oxi-sdk`는 crates.io 전용 의존성이라 재구현이 금지되며, 커널은 이를
*래핑*만 하고 자체 수명 주기·보안·메모리를 얹는 구조다.

**oxi-sdk에서 가져옴** — `AgentConfig`, tool-calling loop, `Oxi`, `ModelCatalog`
**커널이 자체 구현** — `Supervisor`, `EventBus`, `StateStore`, `AccessManager`, `OxiosEngine`(래퍼)
```

Then this Rust code block (render with syntax highlighting, FULL width):
```rust
// crates/oxios-kernel/src/lib.rs — star topology around AgentId
use oxios_memory::MemoryManager;
use oxios_ouroboros::Orchestrator;
use oxi_sdk::{AgentConfig, Oxi};

pub struct Kernel {
    bus: EventBus,          // pub/sub across all subsystems
    state: StateStore,      // durable, keyed by AgentId
    supervisor: Supervisor, // fork / exec / wait / kill
    access: AccessManager,  // RBAC + path sandbox + Merkle audit
    memory: MemoryManager,  // tiered: Hot / Warm / Cold
    engine: OxiosEngine,    // wraps oxi_sdk::Oxi + ModelCatalog
}

impl Kernel {
    /// Fork a new agent process — Unix semantics.
    pub fn fork(&self, parent: AgentId, spec: AgentSpec) -> AgentId {
        let id = self.state.next_id();
        self.supervisor.spawn(id, spec);
        self.bus.emit(Event::Forked { parent, child: id });
        id
    }
}
```

Then this table (render at FULL width, no horizontal squeeze):
| Component | Source | Why |
|---|---|---|
| `AgentConfig`, tool loop, `Oxi`, `ModelCatalog` | `oxi-sdk` (crates.io) | 업스트림 재구현 금지 |
| `OxiosEngine` (wrapper) | kernel | 프로바이더/모델 해석 + 핫스왑 |
| `Supervisor`, `EventBus`, `StateStore` | kernel (자체) | 프로세스 수명 주기 — Unix 핵심 |
| `AccessManager` | kernel (자체) | RBAC + 샌드박스 + Merkle 감사 |
| `MemoryManager` | `oxios-memory` 크레이트 | 계층 메모리, Dream, HNSW |
| `Orchestrator` | `oxios-ouroboros` 크레이트 | assess → crystallize → execute → review |

Then this section:
```
### 왜 모놀리식 단일 크레이트인가

순환 의존성을 원천 차단하고, `pub(crate)` 경계로 모듈을 분리한다.
8개 크레이트로 쪼개면 빌드 시간은 늘고 인지 비용만 커진다.
커널은 "의도적으로 모놀리식"이다 — 분할 제안은 거부한다.
```

**Turn footer meta** (render verbatim): `in 1,240 tok · out 380 tok · claude-sonnet-4.5 · $0.0089 · 4.2s`

### Turn 2 — failure scenario

User prompt:
```
트레이드오프 비교표도 만들어줘
```

Assistant turn FAILS. Model selected was `gpt-4o`; provider token quota exhausted.

Error card copy (Korean — this is the bilingual web UI, conversational copy is
Korean-allowed):
- title: `선택한 프로바이더에 토큰이 남아있지 않습니다.`
- detail: `gpt-4o (openai) — quota_exceeded. 다른 모델로 전환하거나 토큰을 충전한 뒤 다시 시도하세요.`
- action button: `다시 시도` (with a retry icon)

## Design tokens (embed VERBATIM — these are Oxios's real OKLCH tokens)

```css
:root {
  --radius: 0.625rem;
  --background: oklch(0.99 0 0);
  --foreground: oklch(0.141 0.005 285.823);
  --card: oklch(1 0 0);
  --muted: oklch(0.967 0.001 286.375);
  --muted-foreground: oklch(0.552 0.016 285.938);
  --primary: oklch(0.23 0.025 265);
  --primary-foreground: oklch(0.985 0 0);
  --secondary: oklch(0.967 0.001 286.375);
  --accent: oklch(0.967 0.003 265);
  --accent-foreground: oklch(0.23 0.025 265);
  --border: oklch(0.92 0.004 286.32);
  --ring: oklch(0.45 0.04 265);
  --success: oklch(0.596 0.145 163);
  --warning: oklch(0.669 0.162 70);
  --error: oklch(0.577 0.245 27.325);
  --info: oklch(0.623 0.214 259.815);
  --shadow-sm: 0 1px 2px oklch(0 0 0 / 0.04);
  --shadow-md: 0 2px 8px oklch(0 0 0 / 0.06);
  --shadow-lg: 0 4px 16px oklch(0 0 0 / 0.08);
}
.dark {
  --background: oklch(0.13 0.005 285.823);
  --foreground: oklch(0.985 0 0);
  --card: oklch(0.19 0.008 265);
  --muted: oklch(0.274 0.006 286.033);
  --muted-foreground: oklch(0.705 0.015 286.067);
  --primary: oklch(0.91 0.03 265);
  --primary-foreground: oklch(0.15 0.02 265);
  --border: oklch(1 0 0 / 10%);
  --ring: oklch(0.6 0.05 265);
  --success: oklch(0.723 0.219 149.579);
  --warning: oklch(0.769 0.188 70.08);
  --error: oklch(0.704 0.191 22.216);
  --info: oklch(0.685 0.196 259);
  --shadow-sm: 0 1px 2px oklch(0 0 0 / 0.2);
  --shadow-md: 0 2px 8px oklch(0 0 0 / 0.3);
}
```

Body font: **Geist** (CDN: `https://cdn.jsdelivr.net/npm/@fontsource/geist`). Mono: **Geist Mono**.

## Shared rules for ALL prototypes

1. Single standalone `.html` file — inline CSS/JS, only external CDN for fonts.
2. Light + dark both work (auto via `prefers-color-scheme`, plus a toggle button).
3. Render the FULL content payload above — code block AND table AND both tool
   calls AND both reasoning steps AND the error card. Nothing may be omitted or
   stubbed.
4. Code block + table render at FULL readable width. No narrow bubble around them.
5. A composer/input area at the bottom styled to match the paradigm.
6. Include a small "model chip" somewhere (e.g. `claude-sonnet-4.5`) and the
   turn footer meta — Oxios routes across providers, so model identity matters.
7. Bilingual copy is fine (Korean assistant replies are correct for the web UI).

## How you will be judged

- Does the 30-line Rust block + 6-row table breathe at full width? (must)
- Is the paradigm STRUCTURALLY distinct from the other three? (must — no reskins)
- Does it lean into Oxios = "Agent OS" (fork/exec/wait/kill) identity? (should)
- Is it visually polished, not generic AI-slop? (must)
