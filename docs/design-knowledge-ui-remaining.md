# Knowledge UI — Phase 3/4 완성 설계

> **상태**: 설계 (Phase 1-2 구현 완료 기준)
> **날짜**: 2026-05-20
> **목표**: 남은 모든 기능을 끝까지 구현하기 위한 완전한 명세

---

## 0. 현재 구현 상태

### 2061 lines, 16개 파일

| 파일 | lines | 구현 상태 |
|------|------:|-----------|
| `types/knowledge.ts` | 104 | ✅ 완료 |
| `stores/knowledge.ts` | 106 | ✅ 완료 |
| `hooks/use-knowledge.ts` | 295 | ✅ 완료 (29개 API 전체) |
| `knowledge-layout.tsx` | 40 | ✅ 완료 |
| `knowledge-sidebar.tsx` | 86 | ✅ 완료 |
| `file-tree.tsx` | 96 | ✅ 완료 |
| `resize-handle.tsx` | 43 | ✅ 완료 |
| `editor-panel.tsx` | 43 | ⚠️ 스플릿 열기 방법 없음 |
| `editor-toolbar.tsx` | 63 | ⚠️ 스플릿/인포패널 버튼 없음 |
| `markdown-editor.tsx` | 104 | 🔲 textarea 스텁 (HyperMD 아님) |
| `split-editor.tsx` | 27 | 🔲 컨테이너만 있음 |
| `knowledge-chat.tsx` | 538 | ✅ 완료 |
| `search-modal.tsx` | 297 | ✅ 완료 |
| `move-modal.tsx` | 173 | ✅ 완료 |
| `info-panel.tsx` | 46 | ⚠️ 백링크만, 그래프 없음 |

### 완성도

| Phase | 상태 | 남은 작업 |
|-------|------|-----------|
| Phase 1: 뼈대 | ✅ | — |
| Phase 2: 채팅+검색 | ✅ | — |
| Phase 3: 에디터 고급 | 🔲 10% | HyperMD, 자동완성, 링크클릭, 이미지, 스플릿 |
| Phase 4: Oxios 확장 | 🔲 5% | 그래프, 코파일럿, 습관, 통계, 설정 |

---

## 1. Phase 3: 에디터 고급

### 1.1 `markdown-editor.tsx` — HyperMD 통합

**현재**: 순수 `<textarea>` + 자동 저장 + ⌘B/I
**목표**: HyperMD (CodeMirror 5 기반) React 래퍼

#### npm 의존성 추가

```
hypermd@^3.0
codemirror@^5.65
```

HyperMD은 CM5에 의존. CM6과 충돌 가능하므로 dynamic import로 지연 로드.

#### 구현

```typescript
// lib/hypermd-setup.ts

// Dynamic import — CM5를 전역에 로드
import 'codemirror/lib/codemirror.css'
import 'hypermd/theme/hypermd-light.css'
import 'hypermd/mode/hypermd'

// 에디터 필수 addon
import 'codemirror/addon/edit/continuelist'
import 'codemirror/addon/selection/active-line'

// HyperMD fold addon
import 'hypermd/fold'
import 'hypermd/fold-link'
import 'hypermd/fold-image'
import 'hypermd/fold-emoji'

// HyperMD 클릭 핸들러 (링크 클릭 → 파일 열기)
import 'hypermd/click'

// 코드 블록 하이라이트
import 'codemirror/mode/javascript/javascript'
import 'codemirror/mode/python/python'
import 'codemirror/mode/go/go'
import 'codemirror/mode/shell/shell'
import 'codemirror/mode/php/php'

// 자동완성
import 'codemirror/addon/hint/show-hint'
import 'codemirror/addon/hint/show-hint.css'

import CodeMirror from 'codemirror'
```

#### `markdown-editor.tsx` 재작성 명세

```
Props:
  filePath: string
  initialContent: string
  onSave: (content: string) => void
  isSplit?: boolean  // true면 절반 폭

동작:
  1. useRef<HTMLTextAreaElement> 로 textarea 참조
  2. useEffect에서 HyperMD.fromTextArea(textarea, options) 호출
  3. CM6가 아닌 CM5 인스턴스이므로 ref로 에디터 조작

HyperMD 옵션 (editor.js에서 포팅):
  mode: { name: 'hypermd', math: false }
  lineNumbers: false
  dragDrop: false
  viewportMargin: 10
  hmdClick: true
  styleActiveLine: true

이벤트:
  onChange → debounce(1000ms) → onSave(content)
  onBlur → 즉시 저장 (dirty면)

커스텀 hmdResolveURL (링크 클릭 처리):
  - .md 경로 → store.openFile(path)
  - http(s) → window.open(url, '_blank')
  - media/* → 이미지 프리뷰

커스텀 hmdReadLink (위키 링크 읽기):
  - .md 파일 → store.openFile(path)

`[` 입력 → 자동완성 (lib/autocomplete-link.ts):
  - 파일 목록: 캐시된 tree 데이터
  - 정렬: 최근 수정 순
  - 선택 → `[filename](path)` 삽입

이미지 붙여넣기:
  - clipboard image → PUT /api/knowledge/file/media/{timestamp}.png
  - `![](media/{filename})` 삽입

서식 단축키:
  ⌘B → **bold** 토글
  ⌘I → *italic* 토글
  ⌘Y → ✅ 삽입

제목 라인 강제:
  첫 줄 항상 `# ` 로 시작 (onChange 감시)
```

#### cleanup

```
useEffect return:
  editor.toTextArea()  // CM5 인스턴스 정리
```

---

### 1.2 `lib/autocomplete-link.ts` — 위키 링크 자동완성

files.md 원본: `lib/autocomplete-link.js` + `app.js` → `createAutocompleteDict()`

```
입력: 캐시된 KnowledgeTreeEntry[] (tree API 결과)
출력: CodeMirror hint function

로직:
  1. tree를 평탄화해서 파일 목록 생성
     - 각 파일: { key: "filename without .md", path: "dir/file.md", lastModified }
  2. `[` 입력 시 hint 활성화
  3. 타이핑 → fuzzy match (startsWith + includes)
  4. 결과: `[filename](path)` 삽입
  5. 최대 20개 결과
  6. 시스템 디렉토리 (archive, media) 제외
  7. 현재 편집 중인 파일 제외
  8. sort: lastModified 역순 (최근 수정 우선)
```

---

### 1.3 `markdown-editor.tsx` — 이미지 붙여넣기

```
editor.on('paste', async (cm, event) => {
  const items = event.clipboardData?.items
  for (const item of items) {
    if (item.kind === 'file' && item.type.startsWith('image/')) {
      event.preventDefault()
      const file = item.getAsFile()
      const filename = `${Date.now()}.${ext}`  // png, jpg, etc.
      
      // Read as ArrayBuffer → PUT /api/knowledge/file/media/{filename}
      const arrayBuffer = await file.arrayBuffer()
      await fetch(`/api/knowledge/file/media/${filename}`, {
        method: 'PUT',
        body: arrayBuffer,
        headers: { 'Content-Type': file.type }
      })
      
      // Insert markdown
      cm.replaceSelection(`![](media/${filename})\n`)
    }
  }
})
```

주의: 현재 PUT 엔드포인트는 `body: String`을 기대하지만,
이미지는 바이너리. 별도 엔드포인트가 필요할 수 있음.
Phase 3에서는 PUT으로 텍스트 전송 (base64?) 하거나
별도 멀티파트 엔드포인트 추가 필요.

→ 일단 Phase 3에서는 **이미지 붙여넣기를 TODO로 남기고**
나머지 HyperMD 기능부터 완성.

---

### 1.4 `editor-toolbar.tsx` — 버튼 추가

```
현재: 뒤로/앞으로/파일명/닫기
추가:
  - 스플릿 에디터 열기 버튼 (Columns2 아이콘)
    → 클릭 시 store.openSplit(currentFilePath)
  - 인포 패널 토글 버튼 (PanelRight 아이콘)
    → 클릭 시 store.toggleInfoPanel()
  - ⌘S 저장 버튼 (Save 아이콘) — manual save
```

---

### 1.5 `split-editor.tsx` — 스플릿 에디터 열기

현재: 컨테이너만 있고, 열 방법이 없음.

변경:
1. `editor-toolbar.tsx`에 스플릿 버튼 추가
2. 스플릿 열리면 현재 파일을 두 번째 패널에 복사
3. 다른 파일을 열면 split에 고정
4. ⌘W 또는 X 버튼으로 닫기

---

### 1.6 `knowledge-sidebar.tsx` — 파일 삭제

```
우클릭 또는 ⌘D → 삭제 확인 (confirm) → DELETE /api/knowledge/file/{path}
```

---

### 1.7 `knowledge-layout.tsx` — 글로벌 키보드 단축키

```
현재: SearchModal과 MoveModal이 각자 ⌘K, ⌘M 리스너를 가짐.
추가 (knowledge-layout.tsx에 통합):
  ⌘N          → 새 파일
  ⌘⇧N         → 새 폴더
  ⌘D          → 파일 삭제 (editor 모드일 때)
  ⌘Enter      → chat 모드 전환
  ⌘⇧Enter     → chat 모달 토글 (editor 위에 떠 있는 채팅)
  ⌘~ / ⌘§     → 사이드바 토글
  ⌘W          → 스플릿 닫기
  Escape      → 스플릿 닫기 / 에디터 포커스
```

---

## 2. Phase 4: Oxios 확장

### 2.1 `info-panel.tsx` — 링크 그래프 추가

```
현재: 백링크 목록만
추가:
  - TanStack Query: useKnowledgeGraph()
  - 미니 그래프 시각화 (SVG)
    - nodes: 원으로 표시, label 표시
    - edges: 선으로 연결
    - 현재 파일 하이라이트
  - 클릭 → 파일 열기

의존성: recharts (이미 package.json에 있음)
또는 간단한 force-directed SVG (추가 의존성 없이)
```

---

### 2.2 `copilot.tsx` — AI 코파일럿

```
위치: InfoPanel 내부에 탭으로 통합 (Backlinks | Copilot)

API: POST /api/knowledge/copilot
  요청: { question, context_path }
  응답: { content, referenced_notes }

UI:
  - 질문 입력 필드
  - 답변 표시 (마크다운 렌더링 또는 plain text)
  - 참조된 노트 목록 (클릭 → 열기)
  - 로딩 인디케이터 (스트리밍은 API가 지원 안 하므로 일반 POST)

Hooks:
  useKnowledgeCopilot() — 이미 use-knowledge.ts에 구현됨
```

---

### 2.3 `habits.tsx` — 습관 트래커

```
라우트: /knowledge/habits (또는 사이드바 버튼으로 열기)

API:
  GET /api/knowledge/habits?year=2026
  GET /api/knowledge/habits/last-week

UI:
  - 연도 선택
  - 습관별 연간 그리드 (GitHub contribution 스타일)
    - ⚪️ 스킵 = 회색
    - 🟢 완료 = 초록
    - 🟡 주말 완료 = 노란색
  - 이번 주 요약
  - Mood 트래커 (⚪️🤕😔😐🙂😊)

의존성: recharts (이미 있음)

타입: HabitsData (유연한 [key: string]: unknown)
  실제 구조는 oxios_markdown::types::Habits에서 직렬화됨
  프론트엔드에서는 any로 받아서 렌더링
```

---

### 2.4 `today-stats.tsx` — 오늘의 통계

```
위치: 대시보드 내부 또는 사이드바 하단 패널

API:
  GET /api/knowledge/stats/today
  GET /api/knowledge/stats/done-today

UI:
  - 오늘 완료한 항목 수
  - 완료한 파일 목록
  - 간단한 카드 형태
```

---

### 2.5 `knowledge-settings.tsx` — 설정

```
라우트: /knowledge/settings (또는 사이드바 하단 톱니바퀴)

API:
  GET  /api/knowledge/config
  PUT  /api/knowledge/config

폼 필드 (KnowledgeConfig 타입):
  - language: string (select: ko, en, ...)
  - timezone: string (text input)
  - move_to_commands: string[] (태그 입력)
  - pomodoro_duration_in_minutes: number
  - quick_commands: string[] (태그 입력)
  - two_emojis_enabled: boolean (switch)
  - mode: string (select: chat, full, tasks, notes, journal)
  - quick_habits_enabled: boolean (switch)

UI:
  - shadcn/ui 폼 컴포넌트
  - React Hook Form + Zod 검증
  - 저장 버튼 → PUT API 호출
```

---

### 2.6 `link-graph.tsx` — 전체 그래프 페이지

```
라우트: /knowledge/graph (또는 InfoPanel 내부)

API: GET /api/knowledge/graph
  응답: { nodes: [...], edges: [...] }

UI:
  - 전체 화면 그래프 시각화
  - Force-directed layout (SVG or Canvas)
  - 노드 클릭 → 파일 열기
  - 그룹(디렉토리)별 색상 구분
  - 줌/팬 지원

의존성: d3-force (경량) 또는 순수 SVG 구현
→ 순수 SVG로 구현 (의존성 추가 최소화)
```

---

## 3. 파일별 변경 요약

### 수정 (기존 파일 재작성)

| 파일 | 변경 내용 |
|------|-----------|
| `markdown-editor.tsx` | **전면 재작성**. textarea → HyperMD/CM5. 자동완성, 링크클릭, 북마크, 단축키 |
| `editor-toolbar.tsx` | 스플릿 열기 버튼, 인포패널 토글, 수동 저장 버튼 추가 |
| `info-panel.tsx` | 탭 추가: Backlinks \| Copilot \| GraphMini |
| `knowledge-layout.tsx` | 글로벌 키보드 단축키 (⌘N, ⌘D, ⌘Enter 등) |
| `knowledge-sidebar.tsx` | 파일 삭제 기능 (⌘D 또는 우클릭) |
| `package.json` | hypermd, codemirror(@5) 추가 |

### 신규 파일

| 파일 | 내용 |
|------|------|
| `lib/hypermd-setup.ts` | HyperMD/CM5 import 및 초기화 설정 |
| `lib/autocomplete-link.ts` | `[` 입력 시 파일 링크 자동완성 로직 |
| `components/knowledge/habits.tsx` | 습관 트래커 (연간 그리드) |
| `components/knowledge/today-stats.tsx` | 오늘의 통계 카드 |
| `components/knowledge/knowledge-settings.tsx` | 설정 폼 |
| `components/knowledge/link-graph.tsx` | 전체 링크 그래프 시각화 |
| `components/knowledge/copilot.tsx` | AI 코파일럿 패널 |
| `routes/knowledge/graph.tsx` | 그래프 전체 페이지 라우트 |
| `routes/knowledge/habits.tsx` | 습관 트래커 라우트 |
| `routes/knowledge/settings.tsx` | 설정 라우트 |
| `hooks/use-knowledge-shortcuts.ts` | 글로벌 키보드 단축키 훅 |

---

## 4. 의존성 변경

```diff
// package.json
+ "hypermd": "^3.0",
+ "codemirror": "^5.65"
```

주의: codemirror@6이 이미 설치되어 있을 수 없음 (프로젝트에 CM 없음).
CM5는 독립적으로 작동하므로 충돌 없음.

---

## 5. 구현 순서 (작업 단위)

### Step 1: HyperMD 에디터 전환
- `package.json`에 hypermd, codemirror 추가
- `lib/hypermd-setup.ts` 작성
- `markdown-editor.tsx` 전면 재작성
- `lib/autocomplete-link.ts` 작성
- → **HyperMD 에디터 + `[` 자동완성 작동**

### Step 2: 에디터 고급 기능
- 링크 클릭 → 파일 열기 (hmdResolveURL 커스텀)
- 서식 단축키 (⌘B/I/Y)
- 제목 라인 강제
- 이미지 북여넣기 (TODO — 바이너리 API 필요)
- → **files.md 에디터 경험 완성**

### Step 3: 툴바 + 스플릿 + 단축키
- `editor-toolbar.tsx` 버튼 추가
- `split-editor.tsx` 동작 연결
- `hooks/use-knowledge-shortcuts.ts` 작성
- `knowledge-layout.tsx`에 단축키 훅 연결
- `knowledge-sidebar.tsx` 삭제 기능 추가
- → **files.md 전체 워크플로우 완성**

### Step 4: InfoPanel 확장
- `info-panel.tsx`에 탭 추가
- `copilot.tsx` 작성
- `link-graph.tsx` 작성 (미니 버전)
- → **Oxios 확장: 코파일럿 + 그래프**

### Step 5: 독립 페이지들
- `habits.tsx` 작성 + 라우트
- `today-stats.tsx` 작성
- `knowledge-settings.tsx` 작성 + 라우트
- `link-graph.tsx` 전체 페이지 버전 + 라우트
- → **Oxios 확장: 습관/통계/설정/그래프**

### Step 6: 정리
- AGENTS.md 업데이트 (Knowledge UI 섹션)
- design-knowledge-ui.md 최종 상태 업데이트
- 모든 페이지 수동 테스트

---

## 6. API 연결 완전 매핑 (29개 엔드포인트)

| # | API | Hook (use-knowledge.ts) | 컴포넌트 | Phase |
|---|-----|------------------------|----------|-------|
| 1 | GET /tree | ✅ useKnowledgeTree | KnowledgeSidebar, FileTree, SearchModal, MoveModal | 1 |
| 2 | GET /file/{path} | ✅ useKnowledgeFile | MarkdownEditor, SplitEditor, MoveModal | 1 |
| 3 | PUT /file/{path} | ✅ useWriteFile | MarkdownEditor, KnowledgeSidebar, MoveModal | 1 |
| 4 | DELETE /file/{path} | ✅ useDeleteFile | KnowledgeSidebar, MoveModal | 3 |
| 5 | POST /search | ✅ useKnowledgeSearch | SearchModal | 2 |
| 6 | GET /backlinks | ✅ useKnowledgeBacklinks | InfoPanel | 1 |
| 7 | GET /graph | ✅ useKnowledgeGraph | LinkGraph, InfoPanel | 4 |
| 8 | POST /copilot | ✅ useKnowledgeCopilot | Copilot | 4 |
| 9 | POST /checklist/items | ✅ useChecklistItems | (Phase 4에서 사용) | 4 |
| 10 | POST /checklist/add | ✅ useChecklistAdd | KnowledgeChat | 2 |
| 11 | POST /checklist/complete | ✅ useChecklistComplete | (Phase 4에서 사용) | 4 |
| 12 | POST /checklist/remove | ✅ useChecklistRemove | (Phase 4에서 사용) | 4 |
| 13 | POST /chat/append | ✅ useChatAppend | KnowledgeChat | 2 |
| 14 | GET /chat/messages | ✅ useChatMessages | KnowledgeChat | 2 |
| 15 | POST /chat/delete | ✅ useChatDelete | KnowledgeChat | 2 |
| 16 | POST /chat/move | ✅ useChatMove | KnowledgeChat (To File) | 2 |
| 17 | POST /journal/add | ✅ useJournalAdd | KnowledgeChat | 2 |
| 18 | POST /journal/emoji | ✅ useJournalAddEmoji | KnowledgeChat | 4 |
| 19 | GET /journal/today | ✅ useJournalToday | KnowledgeSidebar | 4 |
| 20 | GET /habits | ✅ useKnowledgeHabits | Habits | 4 |
| 21 | GET /habits/last-week | ✅ useKnowledgeHabitsLastWeek | Habits | 4 |
| 22 | GET /stats/today | ✅ useKnowledgeStatsToday | TodayStats | 4 |
| 23 | GET /stats/done-today | ✅ useKnowledgeDoneToday | TodayStats | 4 |
| 24 | GET /config | ✅ useKnowledgeConfig | KnowledgeSettings | 4 |
| 25 | PUT /config | ✅ useKnowledgeConfigUpdate | KnowledgeSettings | 4 |
| 26 | POST /worker/nightly | ✅ useNightlyCleanup | KnowledgeSettings | 4 |
| 27 | POST /worker/scheduled | ✅ useScheduledTasks | KnowledgeSettings | 4 |
| 28 | POST /convert/html | ✅ useConvertHtml | MarkdownPreview (Phase 4) | 4 |
| 29 | GET /emoji | ✅ useAutoEmoji | MarkdownEditor (자동완성) | 3 |

### 사용처 없는 hooks (Phase 4에서 연결)
- #9, #11, #12: 체크리스트 관리 (독립 체크리스트 뷰에서)
- #18: 저널 이모지 (Journal 페이지에서)
- #26, #27: Worker (설정 페이지에서 수동 트리거)
- #28: MD→HTML (미리보기 토글)

---

## 7. 라우트 구조 (최종)

```
/knowledge/              → KnowledgeLayout (files.md 스타일 SPA)
  기본: chat 모드
  파일 열기: store.openFile() → editor 모드
  (내부 상태로 관리, URL 라우팅 없음)

/knowledge/graph         → 전체 링크 그래프 페이지
/knowledge/habits         → 습관 트래커
/knowledge/settings      → 설정
```

Knowledge는 기본적으로 **상태 기반 SPA** (store로 mode/path 관리).
graph, habits, settings만 별도 라우트.

---

## 8. HyperMD npm 패키지 검증 필요

```
npm info hypermd versions
```

hypermd이 npm에 존재하는지, CM5와 호환되는지 확인.
만약 없거나 오래되었으면:
- 대안 A: CodeMirror 6 + @codemirror/lang-markdown
- 대안 B: 원본 lib/ 파일들을 그대로 복사 (vendor)

이 검증은 Step 1에서 수행.
