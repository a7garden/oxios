# Oxios Web UI 기술 스택 평가

**날짜:** 2026-06-04
**평가 범위:** `surface/oxios-web/` 전체 (Rust 백엔드 + Vite/React 프론트엔드)
**방법론:** 코드 정적 분석 + 외부 최신 동향(2026 Q1–Q2) 교차 검증
**결론:** **✅ 전체적으로 2026년 모범 사례(baseline)에 부합.** 핵심 아키텍처 결정은 모두 정당. 다만 마이너한 **드리프트(drift) 3건**과 **잔존 레거시 1건**이 있음.

---

## 1. 한눈에 보기

| 영역 | 선택 | 평가 | 비고 |
|------|------|------|------|
| **백엔드 프레임워크** | Axum 0.8 | ✅ Best-in-class | Tokio 팀 공식, Tower 미들웨어 호환, 2026 Rust 웹 1위 |
| **정적 파일 임베드** | `rust-embed` 8 | ✅ 표준 패턴 | 단일 바이너리 배포에 필수 |
| **OpenAPI / Swagger** | `utoipa` 5 + `utoipa-swagger-ui` 9 | ✅ 표준 | Rust 생태계 de-facto |
| **WebSocket** | Axum `ws` feature | ✅ 적절 | 이벤트 스트리밍·에이전트 로그용 |
| **프론트엔드 번들러** | Vite 8.0.12 | ✅ 최신 | 2026-03-12 stable 출시 |
| **프레임워크** | React 19.2.6 | ✅ Latest | 2026 baseline |
| **언어** | TypeScript 6.0.3 | ✅ Latest | strict + `noUncheckedIndexedAccess` |
| **라우팅** | TanStack Router 1.170 + file-based codegen | ✅ 정답 | type-safe SPA 표준 |
| **데이터 페칭** | TanStack Query 5 | ✅ 정답 | server-state 황금 조합 |
| **UI 키트** | shadcn/ui + Tailwind CSS v4 | ✅ 2026 default | OKLCH, New York, Zinc base |
| **아이콘** | lucide-react 1.16 | ✅ Latest (1.17) | 1.x 메이저 전환 완료 |
| **차트** | Recharts 3.8 | ✅ 적절 | React dashboard default |
| **그래프/플로우** | reactflow 11 | ⚠️ **드리프트** | v12 + `@xyflow/react` 마이그레이션 필요 |
| **마크다운 에디터** | CodeMirror 5 + HyperMD | ⚠️ **레거시** | CM5는 deprecated, CM6 마이그레이션 미완 |
| **상태 관리 (클라)** | Zustand 5 | ✅ 적절 | TanStack Query와 역할 분리 명확 |
| **i18n** | i18next 26 + react-i18next 17 | ✅ Latest | ko/en 두 로케일 완전 채움 |
| **린터/포맷터** | Biome 2.4 | ✅ Rust 통합 | ESLint+Prettier 50× 빠름, Rust 프로젝트와 톤 일치 |
| **테스트 (unit)** | Vitest 4.1 + Testing Library 16 | ✅ 적절 | |
| **테스트 (e2e)** | Playwright 1.60 | ✅ 적절 | |
| **모킹** | MSW 2.14 | ✅ 모범 사례 | 네트워크 경계 표준 |
| **D3 직접 사용** | d3-force / selection / zoom (+ drag, scale-chromatic) | ⚠️ **잔존 2개 불필요** | |

---

## 2. 아키텍처 정합성 (Unix/Ouroboros 원칙)

### 2.1 ✅ 단일 바이너리 배포 (Rust ↔ JS 경계)

```toml
# Cargo.toml
rust-embed = { version = "8", features = ["mime-guess"] }
# exclude = ["frontend/target/*", "static/wasm/*.wasm", "static/assets/*.wasm"]
# include  = ["src/**", "web/dist/**", ...]
```

- `cargo build` 한 번으로 `web/dist/`가 임베드된 단일 실행 파일 생성
- `web_dist: Option<PathBuf>` 필드로 hot-reload UI까지 지원 (`server.rs`)
- Rust → JS 경계의 표준 패턴. Oxios의 "no containers, direct host execution" 원칙과 일치

### 2.2 ✅ 채널로서의 web surface

`oxios-web`은 `oxios-gateway`를 의존성으로 가지는 **채널** (AGENTS.md §Quick Facts):
- `WebChannelHandle` → `AppState.channel`로 gateway에 메시지 전달
- `KernelHandle` facade의 13개 API에 직접 접근
- 즉 web UI는 "하나의 채널 인스턴스"일 뿐, 다른 CLI/Telegram과 동등. **이것이 옳다.**

### 2.3 ✅ Hot-reload 가능한 설정

`config: Arc<RwLock<OxiosConfig>>` + `reload_config()`. Unix 철학의 런타임 재구성.

---

## 3. 프론트엔드 아키텍처 평가

### 3.1 라우팅: TanStack Router — 정답

**왜 React Router v7이 아니라 TanStack인가?**
- RR v7의 진보한 type safety는 **framework mode (Remix)**에서만 작동. SPA library 모드에서는 옛 RR 수준
- TanStack은 **library 자체가 처음부터 end-to-end type-safe** — search params, params, loader 결과 모두 추론
- File-based routing + `routeTree.gen.ts` 자동 생성 (Vite plugin) — 수동 동기화 부담 없음
- 2026년 가장 type-safe한 React SPA 라우터 (LogRocket, noqta.tn, ekino 다수 평가 일치)

**리스크:** 30+ 라우트, `routeTree.gen.ts` 700+ lines 자동 생성. **OK** — 생성 파일이므로 추적 부담 없음.

### 3.2 데이터 페칭: TanStack Query 5 — 정답

- 30+ `use-*.ts` hooks가 일관되게 `useQuery` / `useMutation` 사용
- `staleTime: 30000, retry: 1` — 보수적 합리적 default
- `mutation onError` → `oxios:mutation-error` CustomEvent → Sonner 토스트 — 깔끔한 cross-cutting error UX
- **WebSocket/SSE** 통합이 `useGlobalEvents`, `useEvents` 등으로 명시적으로 분리됨 (TanStack 공식 권장 패턴)

### 3.3 디자인 시스템: shadcn/ui + Tailwind v4 + OKLCH — 2026 default

- `components.json`에 명시된 New York + Zinc + OKLCH
- `index.css`에 다크/라이트 양 방향 OKLCH 토큰
- 메시지 종류별 edge color도 OKLCH로 정의 (A2A 토폴로지용)
- **shadcn은 "distribution이 아니라 source generator"** → `components/ui/`에 코드 소유, fork 자유. Oxios의 통제 지향 철학과 정합
- `tailwind-merge`, `clsx`, `cva` 조합이 정석

### 3.4 상태 관리: Zustand 5 + TanStack Query 역할 분리

- Server state → TanStack Query (캐시, 재검증, dedup)
- Client state (UI, 노트, 토픽) → Zustand
- 이것이 **2026 정석 분리**. 흔한 "Zustand로 API도 캐싱" 안티패턴 회피

### 3.5 차트 & 그래프: 적절한 선택

- **Recharts 3.8** — React dashboard의 default. logrocket/chartts 다수 비교에서 "실용적 default"
- **reactflow 11** — ⚠️ 아래 이슈 참조

### 3.6 테스트 스택: 모범 사례

- Vitest + Testing Library + jsdom + MSW + Playwright
- MSW로 네트워크 모킹 → component test가 진짜 API 계약 검증
- e2e + unit 분리 명확
- `bun.lock` 168K로 보아 의존성 lock이 건강

### 3.7 린터/포맷터: Biome 2.4 — Rust 프로젝트와의 정합

- Rust 코드(BE) + JS/TS 코드(FE) 둘 다 **Rust 도구체인** 위에서 lint 가능
- ESLint+Prettier 대비 ~50× 빠름 (anhtu.dev 2026 벤치)
- 일부 a11y 규칙이 의도적으로 off → `biome.json`에 명시 (오버라이드 의도 보존). 좋음.

---

## 4. 발견된 이슈

### ⚠️ D-1 (P2) ReactFlow 11 → @xyflow/react 12 마이그레이션

**현상:** `package.json`에 `reactflow: ^11`. 2024년 v12 출시, 패키지명이 `@xyflow/react`로 변경됨. 2026-06 기준 latest 12.11.0.

**사용처:** `src/components/a2a/interactive-topology.tsx`, `src/components/a2a/agent-node.tsx`

**영향:**
- 신규 API (SSR, dark mode, computing flows) 미사용
- 패키지명이 곧 deprecated 예정
- TS 타입/번들 사이즈 손해

**권장:** v12 + `@xyflow/react`로 마이그레이션. A2A 토폴로지는 핵심 시각화 → 한 번은 손볼 가치 있음. (의존성 변경만으로 끝나면 1 PR, 그래프 API 변경까지면 더 큼)

**근거:** xyflow.com 2026-06, reactflow.dev migrate-to-v12 가이드 존재.

---

### ⚠️ D-2 (P2) HyperMD / CodeMirror 5 잔존

**현상:** `src/components/knowledge/markdown-editor.tsx`가 `codemirror: ^5` + `hypermd: ^0.3.11` 사용. 그러나 **다른 곳은 모두 CodeMirror 6** (`@codemirror/state`, `@codemirror/view`, `@codemirror/lang-*`, `@codemirror/commands` 등). 5/6 혼재.

**왜 문제:**
- CodeMirror 5는 공식적으로 **deprecated** (2018년부터 권장 종료)
- HyperMD 0.3.x 마지막 release가 2019년 — **9년 묵음**, 메인테이너 비활성
- 5/6 API가 완전히 달라 두 시스템의 모듈이 섞일 수 없음
- `legacy-modes`, `@types/codemirror: ^5.60.17` 도 legacy 화

**사용처:** knowledge/markdown-editor.tsx, lib/hypermd-mermaid.ts, lib/autocomplete-link.ts, lib/cm6-language.ts

**영향:** 보안 패치·React 19 호환성 등 향후 마이그레이션 강제될 때 큰 작업. 지금 표류하면 더 늦어짐.

**옵션:**
1. **CM6로 마이그레이션** — `@uiw/react-codemirror` (4.25, 1주일 전 릴리즈) 또는 `codemirror-rich-markdoc` 류. 지식 베이스 위키 편집에 필요한 핵심 기능(링크 자동완성, 머메이드, 키보드 단축키)은 직접 CM6 extension으로 재작성
2. **Tiptap/Lexical/ProseMirror 검토** — 2026년 가장 활발한 rich-text/markdown 통합 스택
3. **현상 유지 + 격리** — knowledge 모듈 안에서만 CM5, 외부 표면 차단. 단기 OK, 중기 리스크

**권장:** 1번. Oxios의 "unified skill/knowledge" 비전과 직접 연결되는 부분이라 도구 자체가 죽으면 곤란. knowledge 디자인(`docs/design-knowledge-ui.md`) 작업과 묶어 진행.

---

### ⚠️ D-3 (P3) d3 모듈 잔존 (불필요 의존성 2개)

**현상:** `package.json`에 5개 d3 모듈, 실제 사용은 3개.

| 모듈 | 사용처 | 평가 |
|------|--------|------|
| `d3-force` | memory/embedding-canvas.tsx | ✅ 사용 |
| `d3-selection` | memory/embedding-canvas.tsx | ✅ 사용 |
| `d3-zoom` | memory/embedding-canvas.tsx | ✅ 사용 |
| `d3-drag` | — (코드 내 import 0건) | ❌ 불필요 |
| `d3-scale-chromatic` | — (코드 내 import 0건) | ❌ 불필요 |

(grep -rE "from ['\"]d3-drag|from ['\"]d3-scale-chromatic" src → 매치 0건)

**영향:**
- `bun install` 시간·디스크, 번들 사이즈 (drag 6KB, scale-chromatic 14KB minified)
- `d3-drag`는 reactflow에 이미 transitive dep로 들어감 → 완전 중복 가능성
- `d3-scale-chromatic`는 d3-scale의 color 보충 → 차트 색상에 필요해 보이지만 **현재 직접 사용처 없음** (Recharts 내부적으로 자체 처리)

**권장:** `bun remove d3-drag d3-scale-chromatic`. 사용처 생기면 그때 재설치.

---

### ℹ️ N-1 (참고) i18n 범위

`i18n/locales/{ko,en}.json` 모두 채워져 있음. AGENTS.md는 "user-facing messages — 한국어"라고 명시 → **en은 외부 기여자/문서화용**. OK.

단, 일관성 검증을 위해 CI에서 `ko.json`이 default, `en.json`이 누락 키 검사 정도는 가치 있음. (현 CI에 없음)

---

### ℹ️ N-2 (참고) utoipa Swagger UI

`utoipa-swagger-ui` 9.x + `axum_extras`. Rust OpenAPI 생성기 de-facto. 적절. 다만 `/swagger-ui` endpoint가 프로덕션 빌드에도 노출되는지 (env gating) 확인 필요 — `WebPlugin` 라이프사이클에서 보임.

---

## 5. 외부 의존성 버전 무결성 (2026-06 cutoff 기준)

직접 npm/cargo에 확인한 결과, **모든 버전이 정상 latest**임. (의심스러운 메이저는 없음)

| 패키지 | Oxios 버전 | Latest (2026-06) | 상태 |
|--------|------------|------------------|------|
| react | ^19.2.6 | 19.2.x | ✅ |
| typescript | ^6.0.3 | 6.0.x | ✅ |
| vite | ^8.0.12 | 8.0.x (2026-03 stable) | ✅ |
| vitest | ^4.1.6 | 4.1.x | ✅ |
| tailwindcss | ^4.3.0 | 4.3.x | ✅ |
| @tanstack/react-router | ^1.170.4 | 1.170+ | ✅ |
| @tanstack/react-query | ^5.100.11 | 5.100+ | ✅ |
| lucide-react | ^1.16.0 | 1.17.0 (2026-05-28) | ✅ |
| recharts | ^3.8.1 | 3.8.x | ✅ |
| i18next | ^26.2.0 | 26.x | ✅ |
| react-i18next | ^17.0.8 | 17.0.8 (18일 전 릴리즈) | ✅ |
| @biomejs/biome | ^2.4.15 | 2.4.x | ✅ |
| axum | 0.8 | 0.8 | ✅ |
| utoipa | 5 | 5 | ✅ |

> **잠재 드리프트:**
> - `reactflow: ^11` — v12 + `@xyflow/react`로 갱신 필요 (D-1)
> - `codemirror: ^5` + `hypermd: ^0.3.11` — CM6 + 활성 fork로 갱신 필요 (D-2)

---

## 6. 종합 평가

### 잘한 점 (강점)

1. **2026 baseline 정렬** — Vite 8 / React 19.2 / TS 6 / Tailwind v4 / TanStack Router 1.170 / shadcn New York + OKLCH. 외부 가이드의 "React Dashboard: The Complete 2026 Guide" 와 거의 1:1 매핑
2. **Rust ↔ JS 경계의 단일 바이너리 패턴** — rust-embed, dist/ include, web_dist 오버라이드 모두 정석
3. **Server state ↔ Client state 분리** — TanStack Query + Zustand. 흔한 안티패턴 회피
4. **테스트 3단** — Vitest unit / MSW API mocking / Playwright e2e. 각 레이어 역할 명확
5. **모듈 경계 디렉토리** — `components/{ui,layout,agents,knowledge,a2a,...}` — 도메인별 응집. 큰 모노리스에서 살아남는 패턴
6. **설정 검증** — `docs/production-audit/2026-06-03-webui-config-coverage.md`로 BE↔FE 설정 커버리지를 이미 추적 중. 운영감사 문화 있음
7. **Biome 단일 도구** — Rust 코드와 같은 도구체인 톤. 린트/포맷 CI가 빠름

### 개선할 점

| 우선순위 | 항목 | 노력 |
|----------|------|------|
| P2 | ReactFlow 11 → @xyflow/react 12 | S (1–2 PR) |
| P2 | HyperMD/CM5 → CM6 (또는 Tiptap) | M (지식 UI 작업과 묶음) |
| P3 | `d3-drag`, `d3-scale-chromatic` 제거 | XS (10분) |
| P3 | `/swagger-ui` 프로덕션 노출 정책 결정 | XS |
| P4 | i18n CI: ko 누락 키 / en fallback 검사 | S |

### 절대 건드리지 말 것 (지금 잘 되어 있음)

- **TanStack Router + Query 조합** — 바꿀 이유 없음
- **Vite 8 / React 19.2 / TS 6** — 이미 2026 baseline
- **Axum + rust-embed + utoipa** — Rust 웹의 정석
- **Biome** — Rust 프로젝트와 통합된 lint 경험 유지
- **Zustand + TanStack Query 분리** — 흔한 안티패턴 회피 중
- **OKLCH 디자인 토큰** — 미래 표준에 미리 정렬

---

## 7. 출처 (외부 리서치)

- [React Dashboard: The Complete 2026 Guide (React 19 + Vite + shadcn/ui)](https://www.usedatabrain.com/how-to/create-react-dashboard)
- [TypeScript + React in 2026: The Complete Setup Guide](https://webomnizz.com/typescript-react-in-2026-the-complete-setup-guide/)
- [TanStack Router vs React Router v7 — ekino FR (2026)](https://www.ekino.fr/publications/tanstack-router-vs-react-router-v7/)
- [Tanstack Router vs React Router V7 Comparison 2026 — scaled2c](https://www.scaled2c.com/tanstack-router-vs-react-router-v7-comparison-2026)
- [Vite 8.0 is out! (2026-03-12)](https://vite.dev/blog/announcing-vite8)
- [Tailwind v4 + shadcn/ui: Default SaaS Stack 2026](https://starterpick.com/guides/tailwind-v4-shadcn-ui-saas-stack-2026)
- [Biome vs ESLint + Prettier in 2026](https://devtoolbox.blog/biome-vs-eslint-prettier-2026-2/)
- [Rust Web Frameworks in 2026: Axum vs Actix Web vs Rocket vs Warp vs Salvo](https://aarambhdevhub.medium.com/rust-web-frameworks-in-2026-axum-vs-actix-web-vs-rocket-vs-warp-vs-salvo-which-one-should-you-2db3792c79a2)
- [The Best React Chart Libraries for 2026](https://www.usedatabrain.com/blog/react-chart-libraries)
- [@xyflow/react npm](https://www.npmjs.com/package/@xyflow/react)
- [Migrate to React Flow 12](https://reactflow.dev/learn/troubleshooting/migrate-to-v12)
- [lucide-react npm](https://www.npmjs.com/package/lucide-react)
- [react-i18next npm](https://www.npmjs.com/package/react-i18next)
- [5 Best Markdown Editors for React — Strapi](https://strapi.io/blog/top-5-markdown-editors-for-react)
- [Single-binary deployment with Axum — LinkedIn](https://www.linkedin.com/pulse/single-binary-deployment-axum-how-we-embedded-static-perrotta-neto-dxc4f)
