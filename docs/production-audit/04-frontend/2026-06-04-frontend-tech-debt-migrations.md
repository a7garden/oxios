# Frontend 기술 부채 — 마이그레이션 설계

**날짜:** 2026-06-04
**전제:** [`2026-06-04-web-ui-stack-evaluation.md`](./2026-06-04-web-ui-stack-evaluation.md)에서 식별된 4개 이슈의 작업 설계
**대상 이슈:** D-1, D-2, D-3, D-4 (D-5 i18n CI는 본 설계에서 제외)
**방법론:** 정적 코드 분석 + transitive dep lock 확인 + 외부 라이브러리 마이그레이션 가이드 교차 검증

---

## 0. 우선순위 및 의존성 그래프

```
D-1 (d3 정리)        ────────────► [즉시 가능, 독립]
D-4 (swagger-ui 게이팅) ─────────► [즉시 가능, 독립]
D-2 (ReactFlow v12)  ────────────► [단독 PR 가능, e2e 영향]
D-3 (CM5/HyperMD)    ────────────► [가장 신중, 다단계]
    ├─ Phase 1: CM6 평탄화 (자동저장·단축키·헤딩 강제)
    ├─ Phase 2: 위젯 (이미지·코드 폴드, 링크 클릭)
    └─ Phase 3: 토큰 숨김 + Mermaid (가장 큰 작업)
```

**권장 실행 순서:** D-1 → D-4 → D-2 → D-3 Phase 1 → D-3 Phase 2 → D-3 Phase 3
- D-1, D-4는 하루 안에 끝낼 수 있어 빠른 가치 회수
- D-2는 단독 PR, D-2 PR 머지된 후 React Flow v12 위에 CM6 위젯 작성
- D-3은 3개 PR로 분할 — 각 단계가 망가지면 그 전 단계로 롤백 가능

---

# D-1. d3 잔존 의존성 정리

**대상:** `surface/oxios-web/web/package.json`
**영향:** 빌드 시간, 디스크, 번들 사이즈
**소요:** 10분 (실험 포함 30분)
**리스크:** **낮음** — 단, `d3-drag`은 transitive로 reactflow가 사용 중이므로 검증 필요

## 1.1 현재 상태

```bash
$ grep -rE "from ['\"]d3-drag|from ['\"]d3-scale-chromatic" src/
(매치 없음 — 코드에서 직접 import 0건)
```

| 모듈 | 직접 사용 | Transitive 사용자 | 제거 가능? |
|------|-----------|-------------------|------------|
| `d3-drag` | ❌ | `@reactflow/core@11.11.4`, `@reactflow/node-resizer@2.2.14` | ⚠️ **transitive 검증 후** |
| `d3-scale-chromatic` | ❌ | `d3@7.9.0` (recharts의 sub-dep) | ✅ 안전 |
| `d3-force` | ✅ embedding-canvas | — | 보존 |
| `d3-selection` | ✅ embedding-canvas | `@reactflow/core@11.11.4` | 보존 |
| `d3-zoom` | ✅ embedding-canvas | `@reactflow/core@11.11.4` | 보존 |

## 1.2 변경 계획

### Step 1.1: `d3-scale-chromatic` 제거 (안전)

```bash
bun remove d3-scale-chromatic
bun install
bun run typecheck  # 회귀 검증
```

- **이유:** 직접 사용처 0건, transitive(`d3@7` → `d3-scale-chromatic@3`)는 d3 패키지 안에 포함됨 (hoisting 시 별도 모듈로도 남지만 미사용 상태)
- **영향:** recharts는 자체 `d3-scale`만 쓰고 chromatic은 안 씀
- **검증:** `grep -r "d3-scale-chromatic" .` 결과 0건이어야 함

### Step 1.2: `d3-drag` 직접 의존 제거 (검증 후)

```bash
bun remove d3-drag
bun install
# 런타임 검증
cd web && bun run build
cd e2e && bunx playwright test a2a-topology  # A2A topology (reactflow 사용)
```

- **이유:** `bun.lock`상 `@reactflow/core@11.11.4`가 `d3-drag: ^3.0.0`을 dep으로 요구. npm/bun은 reactflow의 nested dep을 hoisting하여 root에 단일 인스턴스만 둠. 직접 dep을 제거해도 reactflow는 hoisted 버전을 그대로 사용.
- **검증 항목:**
  - `bun run build` 성공
  - `bunx playwright test e2e/a2a-topology.spec.ts` 통과
  - A2A topology 페이지에서 노드 드래그 정상 동작
  - `bun.lock`에서 `"d3-drag":` 검색 → `@reactflow/core` 항목에 nested로만 등장
- **롤백:** `git checkout package.json bun.lock && bun install`로 1분 롤백

### Step 1.3 (선택): `@types/d3-drag` 제거

`@types/d3-drag`도 직접 사용 0건. typescript는 transitive types를 자동 hoist하므로 직접 dep은 불필요.

```bash
bun remove @types/d3-drag
```

## 1.3 예상 효과

- `bun install` 시간: 수 초 단축 (네트워크 왕복 2건)
- 번들 사이즈: `d3-drag` ~3KB, `d3-scale-chromatic` ~14KB minified → 토탈 **~17KB 절감**
- `package.json` 라인 수 2줄 감소, 의존성 그래프 단순화

## 1.4 롤백 계획

- 전체 작업이 단일 PR, 단일 commit. 문제 시 PR revert.
- 검증은 e2e test 통과 여부가 게이트.

---

# D-2. ReactFlow 11 → @xyflow/react 12 마이그레이션

**대상:** `surface/oxios-web/web/src/components/a2a/`
**사용처:** 2 파일 (`interactive-topology.tsx`, `agent-node.tsx`)
**소요:** 4-6시간 (코드 + e2e 검증)
**리스크:** **중간** — API 변경이 일부 있음. v12 마이그레이션 가이드에 명시된 변경점이 Oxios 사용 부분에 해당되는지 확인 필요

## 2.1 현재 상태 — 왜 v11을 유지했는가?

`interactive-topology.tsx:33-34`에 코드 주석으로 명시:
```typescript
// reactflow v11.11.4 chosen for React 19 compat (v12 / @xyflow/react not yet validated).
// See RFC-T1-A §6.
```

**즉, 의도적 결정**이다. v12 / `@xyflow/react` 출시는 2024년이지만 React 19 호환성 검증이 Oxios 측에서 끝나지 않아 보수적으로 v11을 고수. RFC-T1-A §6이 이 결정의 기록.

## 2.2 마이그레이션 영향 매트릭스

| v11 API (현재 사용) | v12 / @xyflow/react | 변경점 | Oxios 영향 |
|---------------------|---------------------|--------|-----------|
| `import from 'reactflow'` | `import from '@xyflow/react'` | **패키지명 변경** | import 2곳 |
| `ReactFlow` | `ReactFlow` | 변경 없음 | ✅ |
| `Background, BackgroundVariant, Controls, MiniMap` | 동일 | 변경 없음 | ✅ |
| `Handle, Position` | 동일 | 변경 없음 | ✅ |
| `Node, Edge, NodeProps, NodeMouseHandler` | `Node, Edge, NodeProps, OnNodeClick` | `NodeMouseHandler` → `OnNodeClick` | ⚠️ 타입 rename |
| `ReactFlowInstance` | 동일 | 변경 없음 | ✅ |
| `ReactFlowProps` | 동일 | 변경 없음 | ✅ |
| `ReactFlowProvider` | 동일 | 변경 없음 | ✅ |
| `reactflow/dist/style.css` | `@xyflow/react/dist/style.css` | 경로 변경 | ⚠️ |

### v12 마이그레이션 가이드에서 Oxios에 영향 있는 추가 변경점 (xyflow 공식)

1. **`fitViewOptions` 타입 강화** — 현재 미사용. 무관.
2. **`Node.origin` 필드 추가** — 노드 앵커 위치. 현재 `transform` 직접 설정 안 함. 무관.
3. **`Connection` 모드 추가** — 현재 사용 안 함. 무관.
4. **`useReactFlow()` 훅 추가** — 현재 `useRef<ReactFlowInstance>` 패턴. 무관.
5. **CSS 변수 변경** — `--xy-` prefix 도입. **테마 토큰에 영향 가능** → 검증 필요
6. **React 18/19 concurrent mode 개선** — Oxios의 React 19 사용과 더 잘 맞음

## 2.3 변경 계획

### Step 2.1: 의존성 교체

```bash
bun remove reactflow
bun add @xyflow/react@^12.11.0
```

### Step 2.2: import 경로 일괄 교체

```typescript
// src/components/a2a/interactive-topology.tsx
-import ReactFlow, { Background, BackgroundVariant, Controls, type Edge, MiniMap,
-  type Node, type NodeMouseHandler, type ReactFlowInstance,
-  type ReactFlowProps, ReactFlowProvider } from 'reactflow'
-import 'reactflow/dist/style.css'
+import ReactFlow, { Background, BackgroundVariant, Controls, type Edge, MiniMap,
+  type Node, type OnNodeClick, type ReactFlowInstance,
+  type ReactFlowProps, ReactFlowProvider } from '@xyflow/react'
+import '@xyflow/react/dist/style.css'

// src/components/a2a/agent-node.tsx
-import { Handle, type NodeProps, Position } from 'reactflow'
+import { Handle, type NodeProps, Position } from '@xyflow/react'
```

### Step 2.3: `NodeMouseHandler` → `OnNodeClick` 시그니처

```typescript
// v11
const handleNodeClick: NodeMouseHandler = useCallback(
  (_event, node) => { onNodeSelect?.(node.id) },
  [onNodeSelect],
)
<ReactFlow onNodeClick={handleNodeClick} ... />

// v12 — onNodeClick의 시그니처는 (event, node) → 동일, 타입 이름만 변경
const handleNodeClick: OnNodeClick = useCallback(
  (_event, node) => { onNodeSelect?.(node.id) },
  [onNodeSelect],
)
```

### Step 2.4: RFC-T1-A §6 갱신

- 결정을 "v12 + React 19 검증 완료"로 변경
- 새 RFC-T1-A 또는 동일 문서의 §6에 검증 결과 기록

### Step 2.5: e2e 검증

```bash
cd web
bun run build
bunx playwright test e2e/a2a-topology.spec.ts
# 통과 시: bun run test:all
```

## 2.4 위험과 완화

| 위험 | 확률 | 완화 |
|------|------|------|
| CSS 변수 prefix 변경으로 디자인 깨짐 | 낮음 | visual diff e2e (`a2a-topology.spec.ts`) |
| React 19 concurrent mode 변경으로 hydration 문제 | 매우 낮음 | Oxios는 SPA (SSR 없음) |
| `NodeMouseHandler` → `OnNodeClick` rename 누락 | 중간 | TypeScript strict가 잡음 |
| 기존 v11 only API 미발견 사용 | 낮음 | grep으로 사용 export 전수 확인 완료 |

## 2.5 롤백 계획

- 단일 PR, 단일 commit. PR revert로 5분 롤백.
- e2e test 게이트. `a2a-topology.spec.ts` 통과 필수.

## 2.6 예상 효과

- 1-2년 후 xyflow 메이저 v13 / v14 출시 시 마이그레이션 부담 감소
- React 19 concurrent mode 최적화 자동 적용
- 패키지 deprecation 위험 제거
- 새 API (`useReactFlow`, `Node.origin`) 활용 가능

---

# D-3. HyperMD / CodeMirror 5 → CodeMirror 6 (가장 신중한 분석)

**대상:** `surface/oxios-web/web/src/components/knowledge/markdown-editor.tsx` + `lib/hypermd-*` 3개 + `types/hypermd.d.ts`
**소요:** **다단계, 각 단계 1-3일** (총 1-2주)
**리스크:** **높음** — 단순 의존성 교체가 아닌 **기능 재구현**이 필요

## 3.1 진짜 문제: HyperMD는 의존성이 아니라 "WYSIWYG 동작 모음"

먼저 사용자 강조대로 **"현재 동작을 위해 어쩔 수 없는 의존 구조"인지** 솔직하게 답합니다.

### 3.1.1 HyperMD가 Oxios에 제공하는 동작 목록

| 동작 | HyperMD가 활성화하는 방식 | Oxios 자체 코드에서 처리? |
|------|---------------------------|---------------------------|
| **(a) 마크다운 토큰 시각적 숨김** — `**` `*` `#` `[]` `()` 등 비활성 라인에서 안 보이게 | `hmdHideToken: { enabled: true }` (CM5 addon) | ❌ HyperMD 전담 |
| **(b) 이미지 인라인 폴드** — `![alt](path)` → 실제 이미지 렌더 | `hmdFold.image: true` (CM5 addon) | ❌ HyperMD 전담 |
| **(c) 코드 블록 신택스 색 + 폴드** — ```js 코드 → 색상 + 접힘 토글 | `hmdFold.code: true` | ❌ HyperMD 전담 |
| **(d) 링크/위키링크 클릭 → 라우터 호출** | `hmdClick` + `hmdReadLink` (CM5 addon) | ⚠️ **override는 Oxios가 직접**, **이벤트 발생은 HyperMD** |
| **(e) Mermaid 인라인 다이어그램** — ```mermaid 코드 → SVG | `hmdFoldCode` 컨트랙트 (CM5 addon) | ❌ HyperMD fold + Oxios renderer |
| **(f) Markdown 자동 들여쓰기/연속 리스트** | `continuelist` addon | ❌ CM5 addon |
| **(g) 자동완성 (링크/이모지)** | `show-hint` addon (CM5) | ⚠️ **데이터는 Oxios**, **표시는 CM5 hint** |
| **(h) Tailwind 다크/라이트** | `hypermd-light.css` + 커스텀 `hypermd-dark.css` | ❌ HyperMD 전담 (테마 CSS) |
| (i) 헤딩 1 강제 | 자체 처리 (이벤트 핸들러) | ✅ **HyperMD 무관** |
| (j) 자동 저장 (디바운스) | 자체 처리 | ✅ HyperMD 무관 |
| (k) 키보드 단축키 (⌘B ⌘I ⌘Y) | `extraKeys` 옵션 | ✅ **CM5에 의존하지만 CM6에도 동등** |
| (l) 다크/라이트 모드 동기화 | OKLCH 변수 + CSS | ✅ HyperMD 무관 |
| (m) 변경 알림 (`isDirty`) | 자체 처리 | ✅ HyperMD 무관 |

**결론:** **8개 동작은 HyperMD/CM5에 의존, 5개는 Oxios 자체 처리.** 즉 "어쩔 수 없는 의존"이 맞다 — **그러나 8개 동작은 모두 분해 가능하고, 각각이 CM6 위에서 독립적으로 재구현 가능한 표준 패턴**이다.

### 3.1.2 CM5 vs CM6 — API 차이

| 측면 | CM5 | CM6 |
|------|-----|-----|
| 기본 단위 | textarea + 인스턴스 | EditorView (DOM 트리 직접) |
| 확 장 | 옵션 객체 + addon 모듈 | **Extension** 조합 |
| State 변경 | 명령형 (`setValue`) | **StateEffect** / **StateField** |
| 렌더링 | 라인 단위 (전체 다시 그리기) | **ViewPlugin** + **Decoration** (효율적) |
| 자동완성 | `showHint` addon | **`@codemirror/autocomplete`** (완전히 다른 API) |
| 입력 | textarea 뒤에 숨김 | contenteditable DOM |
| 모듈 | 글로벌 등록 (UMD) | ESM (트리쉐이킹) |

**가장 큰 차이:** CM5는 "DOM 위에 textarea를 얹는 메타포", CM6는 "DOM 자체가 에디터". CM5 → CM6는 단순 라이브러리 교체가 아니라 **아키텍처 재작성**.

### 3.1.3 Oxios의 HyperMD가 CM5에서 "예쁘게" 작동하는 이유 (잠재 함정)

`hypermd-setup.ts`의 주석이 핵심 단서:
> Vite 8 (Rolldown) processes HyperMD's UMD as CJS, so `window.HyperMD` is never populated.
> We therefore hardcode the essential suggestedEditorConfig

**즉, Oxios는 HyperMD의 UMD가 Vite에서 안 풀리는 문제(workaround)를 이미 한 번 풀었다.** 이건 향후 CM6 마이그레이션 시 "HyperMD의 UMD 가정"이 더 이상 발목을 잡지 않는다는 좋은 신호.

또한 `v8(Rolldown)`이 CM5의 `window.CodeMirror` 가정도 위와 같이 패치. **이미 Vite 모듈 시스템 우회 코드가 들어간 상태** → CM6로 가면 이 workaround 모두 제거.

## 3.2 옵션 비교

### 옵션 A: 현상 유지 (비권장)

- 장점: 0 effort, 현재 동작 유지
- 단점: HyperMD 9년 묵음, CM5 deprecated, 보안 패치 위험
- **2027년쯤 React 20 / Vite 9에서 강제 마이그레이션** — 그때 더 큰 작업

### 옵션 B: CM6 + 기능 단계적 재구현 (권장, 다단계)

3개 PR로 분할, 각 단계 검증 후 진행.

| 단계 | 작업 | 위험 | 롤백 |
|------|------|------|------|
| **Phase 1** | `@uiw/react-codemirror` + `@codemirror/lang-markdown` 도입. **단순 코드 에디터** (WYSIWYG 포기, 평탄한 마크다운). 자동 저장·단축키·헤딩 강제·링크 자동완성은 이식. | **낮음** | 이전 commit |
| **Phase 2** | ViewPlugin으로 **이미지 폴드 + 코드 폴드** (b, c) 추가. 링크 클릭 핸들러 (d) 이식. | 중간 | Phase 1로 롤백 |
| **Phase 3** | (a) 마크다운 토큰 숨김 + (e) Mermaid + (h) 다크 테마. | **높음** | Phase 2로 롤백 |

### 옵션 C: Tiptap (ProseMirror 기반) 교체

- 장점: 가장 강력한 rich text, 활발한 메인테이너, ProseMirror 위의 마크다운 확장 존재
- 단점: **완전 다른 아키텍처**, 마이그레이션 코드 양 가장 많음, react-tiptap의 React 19 호환 검증 필요
- **Oxios의 use case (마크다운 위주 + 위키링크 + mermaid)에 약간 over-engineering**
- 권장: **Phase 3까지 가서도 WYSIWYG가 부족하면 검토**

### 옵션 D: Plain textarea + read mode (최소)

- 장점: 가장 단순, 의존성 0
- 단점: **WYSIWYG 포기**, 코드 색, 이미지 폴드, mermaid 인라인 모두 포기
- 사용자 경험 큰 다운
- 권장: 안 함 (Obsidian을 안 쓰고 싶은 사람을 위한 옵션)

### 옵션 E: `@uiw/react-markdown-preview` (마크다운 read-only) + 평탄 textarea

- Oxios는 이미 `react-markdown + remark-gfm + rehype-highlight` 사용 (DESIGN.md 인용)
- **좌우분할 (text + preview) 패턴**이 가장 단순
- 옵션 B의 Phase 1과 거의 같은 결과
- 권장: 옵션 B Phase 1으로 흡수

## 3.3 권장안: 옵션 B (다단계)

### Phase 1: CM6 평탄화 (1-3일)

**목표:** `markdown-editor.tsx`가 CM5/HyperMD 대신 `@uiw/react-codemirror`를 사용. **WYSIWYG 없이 평탄한 마크다운 에디터**. 자동 저장·단축키·헤딩 강제·자동완성·다크 테마는 보존.

```typescript
// 변경 후: src/components/knowledge/markdown-editor.tsx (스케치)
import CodeMirror from '@uiw/react-codemirror'
import { markdown, markdownLanguage } from '@codemirror/lang-markdown'
import { autocompletion, type CompletionContext, type CompletionResult }
  from '@codemirror/autocomplete'
import { EditorView, keymap, lineNumbers } from '@codemirror/view'
import { defaultKeymap, history, historyKeymap } from '@codemirror/commands'

// 자동완성 소스: 위키링크
function linkCompletion(currentFilePath: string, getTreeEntries: () => FileEntry[]) {
  return (ctx: CompletionContext): CompletionResult | null => {
    // [[ 또는 [ 직후 트리거
    // ... (현재 createLinkHintFn 로직 포팅)
  }
}

// 헤딩 1 강제 — ViewPlugin
const headingEnforcer = EditorView.updateListener.of((v) => {
  if (!v.docChanged) return
  const firstLine = v.state.doc.line(1).text
  if (!firstLine.startsWith('# ')) {
    v.dispatch({
      changes: { from: 0, to: firstLine.length,
        insert: `# ${firstLine.replace(/^#*\s*/, '')}` },
    })
  }
})

// 키맵
const keymap_ = keymap.of([
  { key: 'Mod-b', run: toggleWrap('**', '**') },
  { key: 'Mod-i', run: toggleWrap('*', '*') },
  { key: 'Mod-y', run: insertCheckmark },
  { key: 'Mod-s', run: () => { document.dispatchEvent(new Event('knowledge:save')); return true } },
])
```

**제거 대상:**
- `package.json`: `codemirror: ^5`, `hypermd: ^0.3.11` (또는 보존)
- `lib/hypermd-setup.ts` → **삭제**
- `lib/hypermd-dark.css` → **`index.css`에 다크 테마 통합**
- `lib/cm6-language.ts` (현재 CM6 lang-* import만 있음) → **확장 또는 삭제**
- `types/hypermd.d.ts` (CM5 addon ambient 모듈 선언) → **삭제**
- `vite.config.ts`의 `optimizeDeps.include`에서 CM5 경로 제거

**유지 대상:**
- `lib/hypermd-mermaid.ts` — **Phase 3에서 재작성**, **Phase 1에서는 사용 안 함** (또는 미리 mermaid dep만 추가하고 비활성)
- `lib/autocomplete-link.ts` — **CompletionSource로 포팅**

**위험과 검증:**
- `e2e/` 디렉토리: knowledge 관련 e2e 있는지 확인 필요 (없으면 추가 작성)
- 자동 저장 동작 (1초 디바운스) 수동 검증
- 헤딩 1 강제 동작 수동 검증
- `Mod-b`/`Mod-i`/`Mod-y` 단축키 수동 검증
- 자동완성 (wiki link, emoji) 수동 검증

### Phase 2: 위젯 (이미지·코드 폴드 + 링크 클릭) (3-5일)

```typescript
// 이미지 폴드: ViewPlugin + widget decoration
const imageFolder = ViewPlugin.fromClass(class {
  decorations: DecorationSet
  constructor(view: EditorView) {
    this.decorations = this.buildDecorations(view)
  }
  update(update: ViewUpdate) {
    if (update.docChanged || update.viewportChanged) {
      this.decorations = this.buildDecorations(update.view)
    }
  }
  buildDecorations(view: EditorView): DecorationSet {
    const builder = new RangeSetBuilder<Decoration>()
    for (const { from, to } of view.visibleRanges) {
      // markdown syntax tree에서 Image 노드 찾기
      // syntaxTree(view.state).iterate({ from, to, enter: (node) => {
      //   if (node.type.name === 'Image') {
      //     builder.add(node.from, node.to, Decoration.replace({
      //       widget: new ImageWidget(view.state.doc.sliceString(node.from, node.to))
      //     }))
      //   }
      // }})
    }
    return builder.finish()
  }
})
```

**CM6 markdown syntax tree** (`@lezer/markdown`)의 노드 타입:
- `Image`: `![alt](path)` 
- `FencedCode`: ```언어 코드```
- `Link`: `[text](url)` — `Autolink`도 있음

**Oxios 특이 케이스:**
- 현재 위키링크 `[[PageName]]` (CM5 HyperMD는 알지만 lezer/markdown은 모름) → **`linkClick` 핸들러는 텍스트 매칭으로 처리**
- 이미지 폴드는 `Image` 노드, 위키폴드는 별도 패턴 매칭

**위험:**
- `@lezer/markdown` AST 순회 시 라인 단위 처리 누락 → 잘못된 위치
- 위키링크 클릭이 마크다운 링크 클릭과 충돌 → 핸들러 우선순위 정의
- **롤백:** Phase 1 상태로 5분 롤백

### Phase 3: 토큰 숨김 + Mermaid + 다크 테마 (5-7일, 가장 큰 작업)

#### 3a. 마크다운 토큰 숨김 (a)

가장 까다로운 작업. CM5의 `hmdHideToken`은 **마크다운 토큰의 위치를 AST로 파악하고 비활성 라인에서만 숨김**.

```typescript
// 핵심 로직 스케치
const tokenHider = ViewPlugin.fromClass(class {
  decorations: DecorationSet
  update(update: ViewUpdate) {
    if (update.docChanged || update.selectionSet || update.viewportChanged) {
      this.decorations = this.computeDecorations(update.view)
    }
  }
  computeDecorations(view: EditorView): DecorationSet {
    const builder = new RangeSetBuilder<Decoration>()
    const { state } = view
    const cursor = syntaxTree(state).cursor()
    do {
      // 현재 커서 라인이 아니면 토큰 숨김
      const tokenLine = state.doc.lineAt(cursor.from).number
      const cursorLine = state.doc.lineAt(state.selection.main.head).number
      if (tokenLine !== cursorLine) {
        // markdown 강조 노드: Emphasis, StrongEmphasis, HeaderMark, LinkMark
        if (['EmphasisMark', 'HeaderMark', 'LinkMark', 'CodeMark'].includes(cursor.type.name)) {
          builder.add(cursor.from, cursor.to, Decoration.replace({}))
        }
      }
    } while (cursor.next())
    return builder.finish()
  }
})
```

**기술적 위험:**
- `Decoration.replace({})`로 토큰을 가리면 **편집 시 커서 위치가 어긋날 수 있음** → ViewPlugin이 selection 변경을 받아 갱신하는지 확인
- `Emphasis` 노드 안의 `EmphasisMark`만 가려야 하는데, AST traverse 순서가 중요
- 비활성 라인만 가리는 로직 + 활성 라인은 즉시 가시화 (CM5와 동일 UX)

#### 3b. Mermaid 인라인 렌더 (e)

`hypermd-mermaid.ts`는 이미 **렌더러 함수**만 정의하면 됨. Phase 3에서:
- mermaid dep을 직접 `package.json`에 추가 (지금은 transitive로 들어옴)
- `mermaidRenderer`를 CM6의 `Decoration.replace({ widget: new MermaidWidget(code) })`로 포팅
- 위젯은 `MermaidWidget extends WidgetType` — `toDOM()`에서 `div` 만들고 lazy load

```typescript
class MermaidWidget extends WidgetType {
  constructor(readonly code: string) { super() }
  toDOM(): HTMLElement {
    const el = document.createElement('div')
    el.textContent = 'Loading mermaid…'
    ensureMermaidLoaded().then((m) => m.render(/* ... */))
    return el
  }
  ignoreEvent() { return false } // 다이어그램 클릭 등 처리
}
```

**mermaid dep 추가:** 현재는 transitive로 들어옴 (HyperMD 안). Phase 3에서 명시화. lazy load 그대로.

#### 3c. 다크 테마 (h)

`hypermd-dark.css`는 CM5용 CSS 변수. CM6용으로 다시 작성:
- `@codemirror/view`의 `EditorView.theme({ ... })`로 다크/라이트 정의
- 또는 CSS 파일로 `.cm-editor.dark { ... }` 정의

**작은 작업** — 단순 스타일 시트 작성.

### 3.4 위험 종합

| 위험 | 확률 | 영향 | 완화 |
|------|------|------|------|
| Phase 1: 자동완성 API 차이로 링크 매칭 실패 | 중간 | UX 저하 | e2e + 수동 테스트 |
| Phase 1: 헤딩 1 강제 시 무한 루프 | 중간 | 에디터 잠금 | `isSettingContent` ref 패턴 재사용 |
| Phase 2: 위키링크 클릭과 일반 링크 충돌 | 낮음 | 잘못된 파일 열림 | 핸들러 우선순위 명시 |
| Phase 2: 이미지 로드 실패 시 폴드 깨짐 | 낮음 | UI 깨짐 | 에러 핸들링 추가 |
| Phase 3: 토큰 숨김 후 커서 위치 어긋남 | **높음** | 편집 불가능 | 충분한 e2e + Playwright 회귀 |
| Phase 3: Mermaid 다이어그램이 성능 저하 | 중간 | 프레임 드롭 | 뷰포트 외부 위젯 unmount |
| Phase 3: 다크 테마 색상 불일치 | 낮음 | 시각적 깨짐 | OKLCH 토큰 재사용 |

### 3.5 검증 계획

각 Phase마다:
1. `bun run typecheck` 통과
2. `bun run build` 통과
3. `bunx playwright test` 전체 통과
4. 수동 검증: `cd web && bun run dev` → 지식 베이스 페이지
   - 헤딩 1 강제
   - 자동 저장
   - 위키링크 자동완성 (`[` 입력)
   - 이모지 자동완성 (`:` 입력)
   - 키보드 단축키 (⌘B ⌘I ⌘Y)
   - 다크/라이트 전환
5. Phase 2+: 이미지/코드 폴드, 링크 클릭
6. Phase 3+: 토큰 숨김, Mermaid, 다크 테마

### 3.6 롤백 계획

- 각 Phase는 단일 PR, 단일 commit
- 문제 시 PR revert → 5분 롤백
- Phase 1, 2, 3가 독립 → 가장 큰 작업인 Phase 3에서 막히면 Phase 2까지 머지된 상태로 멈춤

### 3.7 일정 추정

| Phase | 범위 | 소요 (1인) | 비고 |
|-------|------|-----------|------|
| 1 | 평탄 CM6 에디터 + 자동완성/단축키/자동저장/헤딩 강제 | 1-3일 | e2e + 수동 검증 포함 |
| 2 | 이미지/코드 폴드 + 링크 클릭 | 3-5일 | ViewPlugin 작성 |
| 3 | 토큰 숨김 + Mermaid + 다크 테마 | 5-7일 | 가장 큰 작업 |
| **합계** | | **9-15일** | 작업자 1인 기준 |

### 3.8 "현상 유지"의 숨은 비용

9-15일이 큰 듯 보이지만:
- HyperMD 9년 묵음, CM5 deprecated — **2027년 강제 마이그레이션** 시 코드베이스가 지금보다 크다
- React 20 / Vite 9 호환성 문제 발생 시 **즉시 대응 불가**
- 보안 패치 (XSS in mermaid/latex 등) **즉시 받을 수 없음**
- 지식 베이스는 Oxios의 핵심 기능 — 도구 죽으면 사용자 신뢰 잃음

**즉, 9-15일은 "지금 쓰냐 vs 2-3배 비용으로 나중에 쓰냐"의 선택**이다. 다단계 분할로 위험 분산하면 지금 쓰는 게 합리적.

## 3.9 최종 권장

**Phase 1 → 머지 → 1주일 운영 검증 → Phase 2 → 머지 → 1주일 운영 검증 → Phase 3**

- 각 Phase 사이 운영 검증으로 회귀 조기 발견
- Phase 3에서 막히면 Phase 1+2만 머지하고 "WYSIWYG 폴드백 에디터"로 정착 가능

---

# D-4. Swagger UI 프로덕션 노출 정책

**대상:** `surface/oxios-web/src/plugin.rs:69-77`
**소요:** 1-2시간
**리스크:** **낮음**

## 4.1 현재 상태

```rust
// src/plugin.rs
let openapi = api_docs::build_openapi();
let swagger: Router<()> = utoipa_swagger_ui::SwaggerUi::new("/api-docs")
    .url("/openapi.json", openapi)
    .nest_service("/api-docs", swagger)
```

**문제:**
- `/api-docs` (Swagger UI) **항상 노출**
- `/openapi.json` (스키마) **항상 노출**
- config flag 없음 (`grep "swagger\|openapi\|api_docs" share/default-config.toml crates/oxios-kernel/src/config.rs` → 매치 0건)
- 프로덕션 배포 시 외부에 API 스키마가 그대로 노출 → **공격 표면 확대**
- `utoipa-swagger-ui` 자체의 알려진 이슈: dev server에서 CORS 우회 등

## 4.2 권장안: 환경 변수 기반 게이팅 + 인증 결합

### 4.2.1 변경 계획

```rust
// src/plugin.rs — 수정 후
fn build_router(state: AppState) -> Router {
    let mut app = Router::new()
        .nest("/api", routes::router(state.clone()))
        .with_state(state.clone());

    // Swagger UI: opt-in, dev only by default
    if state.config.read().server.expose_api_docs {
        let openapi = api_docs::build_openapi();
        let swagger = utoipa_swagger_ui::SwaggerUi::new("/api-docs")
            .url("/openapi.json", openapi);
        app = app.nest_service("/api-docs", swagger);
        tracing::info!("API docs exposed at /api-docs (set server.expose_api_docs=false to disable)");
    }

    // Static assets (always)
    // ...
    app
}
```

### 4.2.2 Config 추가

`crates/oxios-kernel/src/config.rs`:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    /// Expose /api-docs and /openapi.json. Default: false (production-safe).
    pub expose_api_docs: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 4200,
            expose_api_docs: false,  // ← 안전한 기본
        }
    }
}
```

`share/default-config.toml`:
```toml
[server]
host = "127.0.0.1"
port = 4200
# expose_api_docs: false  # default: false
```

### 4.2.3 추가 권장: 인증 게이트

단순 boolean보다 **dev에서만 자동 활성**이 더 안전:

```rust
fn should_expose_api_docs(cfg: &OxiosConfig) -> bool {
    cfg.server.expose_api_docs
        && (cfg.server.host.starts_with("127.") || cfg.server.host == "::1")
}
```

- localhost 바인딩이면서 명시적으로 opt-in일 때만 노출
- **프로덕션에서 `0.0.0.0` 바인딩 시 자동으로 비활성** (잊어도 안전)

### 4.2.4 WebUI 반영

`settings.tsx`의 `[server]` 섹션에 `expose_api_docs` 토글 추가 (이미 `docs/production-audit/2026-06-03-webui-config-coverage.md`의 "F-4: 14개 섹션 통째로 폼에 없음" 작업과 묶음).

## 4.3 위험과 완화

| 위험 | 확률 | 완화 |
|------|------|------|
| 기존 사용자가 의존 (curl, API 클라이언트) | 매우 낮음 | default false이지만 opt-in은 가능 |
| config hot-reload 미작동 | 낮음 | `Arc<RwLock<OxiosConfig>>` 이미 지원 — reload_config에 hook 추가 |
| localhost 검사가 IPv6 미커버 | 낮음 | `::1` + `127.0.0.0/8` 모두 체크 |
| E2E 테스트 영향 | 중간 | `e2e/app.spec.ts`에 `/api-docs` 존재 여부 확인 추가 |

## 4.4 검증

```bash
cargo run  # expose_api_docs=false 기본 → /api-docs 404
curl -i http://localhost:4200/api-docs
# HTTP/1.1 404 Not Found  ← OK

# opt-in으로 검증
# ~/.oxios/config.toml 에서 server.expose_api_docs = true
curl -i http://localhost:4200/api-docs
# HTTP/1.1 200 OK  ← OK

# 프로덕션 시뮬레이션: 0.0.0.0 바인딩 + true여도 비노출
# (should_expose_api_docs 로직)
```

## 4.5 롤백

- 단일 PR, config 기본값만 변경. config로 쉽게 원복 가능.

---

# 부록: 작업 일정 (실행 순서)

```
Week 1
  ├─ Day 1: D-1 (d3 정리, 30분) + D-4 (swagger-ui, 2h)
  └─ Day 2-3: D-2 (ReactFlow v12, 4-6h + e2e)

Week 2-3: D-3 Phase 1 (CM6 평탄화)
  ├─ Day 4-5: @uiw/react-codemirror 도입 + Markdown mode
  ├─ Day 6: 자동완성 (링크/이모지) 포팅
  └─ Day 7-8: 단축키/자동저장/헤딩 강제 + e2e

Week 4-5: D-3 Phase 2 (위젯)
  ├─ Day 9-11: 이미지/코드 폴드 ViewPlugin
  └─ Day 12-13: 위키링크 클릭 핸들러 + e2e

Week 6-7: D-3 Phase 3 (토큰 숨김 + Mermaid + 다크)
  ├─ Day 14-17: 토큰 숨김 ViewPlugin (가장 까다로움)
  ├─ Day 18-19: Mermaid 위젯 + lazy load
  └─ Day 20: 다크 테마 + e2e
```

**총 7주, 1인 풀타임 기준.** 다단계 롤백 가능.

---

# 부록: 즉시 결정 필요 사항

| 질문 | 기본 가정 | 다른 옵션 |
|------|-----------|-----------|
| D-1을 묶을 것인가, 분리할 것인가? | 분리 (단일 commit) | 묶으면 D-1 검증 후 즉시 머지 |
| D-2 RFC-T1-A §6 갱신 형식은? | 단일 PR에 RFC 갱신 포함 | 별도 RFC 문서 |
| D-3 Phase 1에서 HyperMD dep을 제거할 것인가? | **즉시 제거** | 호환성 위해 보존 (비권장 — 듀얼 모드 유지보수 비용 큼) |
| D-3 Phase 3가 막힐 때 폴백? | Phase 1+2 머지 후 "WYSIWYG 폴드백" 정착 | Tiptap 검토 |
| D-4의 localhost 자동 게이팅을 1차로 할 것인가? | 예 (보수적) | 단순 boolean opt-in만 |

위 결정에 따라 PR 분리/작업자 할당/일정 미세조정 가능.
