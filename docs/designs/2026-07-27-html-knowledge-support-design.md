# HTML 지식 베이스 지원 설계

> **상태**: 설계
> **날짜**: 2026-07-27
> **목적**: `.html` 파일을 `~/.oxios/knowledge/`에 저장, 조회, 편집할 수 있도록 확장

---

## 1. 문제

현재 지식 베이스는 `.md` 파일만 지원한다. 사용자는 HTML 문서도 지식 베이스에 저장하고 웹 브라우저처럼 렌더링해서 보고 싶어한다.

**핵심 차이**:
| 특성 | `.md` | `.html` |
|------|-------|---------|
| 편집 | CodeMirror 기반 에디터 (HyperMD) | **편집 불가** (렌더링만) |
| 미리보기 | 분할 에디터 좌측: raw, 우측: rendered | 전체 화면 렌더링 |
| 저장 포맷 | 마크다운 원문 | HTML 원문 |

---

## 2. 현재 상태 분석

### 2.1 백엔드: VirtualFs (oxios-markdown)

VirtualFs는 **확장자에 무관**하다. 모든 I/O (`read_path`, `write_path`, `delete_path`, `files_and_dirs`)는 파일명만 받고 `.md` 필터링이 없다.

**`.md` 전제가 있는 곳만**:
- `all_md_files()` / `collect_md_files()` / `collect_md_paths()` — `.md`만 수집
- `only_user_md_files()` — `.md` 필터
- `display_name()` — `.md` extension stripping
- `build_stem_index()` — wikilink 인덱스는 `.md`만 대상

→ raw I/O 레이어는 이미 HTML에 열려 있다. 제한은 **의도적**이다(검색/백링크/위키링크는 마크다운 전용 기능).

### 2.2 프론트엔드

| 위치 | `.md` 가정 | 영향 |
|------|-----------|------|
| `editor-panel.tsx` | **직접 `MarkdownEditor`를 하드코딩** | HTML 파일을 에디터에서 열면 깨짐 |
| `editor-toolbar.tsx` | `.replace(/\.md$/, '')` | HTML 파일명이 잘못 표시됨 |
| `file-tree.tsx` | 인라인 리네임이 `.md` 자동 추가 | HTML 파일 리네임 시 확장자 깨짐 |
| `knowledge-home.tsx` | `'New file.md'` 기본값 | HTML 생성 불가 |
| `file-preview-view.tsx` | `.md` → `MarkdownPreview`, 나머지 → `<pre>` | HTML이 raw `<pre>`로만 보임 |
| `note-rename.ts` | `isProtectedPath`가 `.md` 아닌 파일 보호 | HTML은 **이미 보호됨** (의도와 일치) |
| `store/knowledge.ts` | 확장자 인식 없음 | 영향 없음 |

### 2.3 API

`/api/knowledge/file/{*path}` — 확장자 무관. 변경 불필요.

---

## 3. 설계

### 3.1 아키텍처 결정

**원칙**: HTML 파일은 **read-only 렌더링 결과물**이다. 편집 불가, H1 rename 불가, backlink/semantic search 불가.

```
User creates "design.html"
  → VirtualFs.write_path("design.html", "<html>…</html>")
  → User clicks on it
  → EditorPanel detects ".html"
  → HtmlRenderer (iframe with srcdoc)
```

```
      currentFilePath?
         │
    ┌────┴────┐
    │ ends with .md?  ──→  MarkdownEditor (editable + preview)
    │ ends with .html? ──→  HtmlRenderer (iframe srcdoc, read-only)
    │ other?           ──→  (future: FilePreviewView? for now: empty)
    └─────────
```

#### A. Backend — `oxios-markdown`

**1. `display_name()` — extension stripping**

```rust
// fs.rs
pub fn display_name(filename: &str) -> String {
    let trimmed = filename.trim();
    let without_ext = trimmed
        .strip_suffix(".md")
        .or_else(|| trimmed.strip_suffix(".html"))
        .unwrap_or(trimmed);
    let mut chars = without_ext.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().chain(chars).collect(),
    }
}
```

**2. `only_user_md_files()` — generalize**

```rust
// fs.rs
/// Filter: only user content files (exclude system files, dirs, non-text).
pub fn only_user_text_files(files: &[FileEntry]) -> Vec<FileEntry> {
    files
        .iter()
        .filter(|f| {
            !f.is_dir
                && (f.name.ends_with(".md") || f.name.ends_with(".html"))
                && !SYSTEM_FILES.contains(&f.name.as_str())
        })
        .cloned()
        .collect()
}
```

Rename callers from `only_user_md_files` to `only_user_text_files`.

**No changes needed**:
- `all_md_files()` / `collect_md_files()` / `collect_md_paths()` — semantic search and wikilink index are markdown-only
- BacklinkIndex — HTML files have no `[[wikilink]]` syntax
- `build_stem_index()` — HTML files are skipped (not md)
- `MAX_TEXT_SIZE` — applies to HTML as well

#### B. Frontend

**1. `HtmlRenderer` component (new)**

`web/src/components/knowledge/html-renderer.tsx`:

- Renders via `<iframe sandbox srcdoc={content}>`
- Sandbox: blocks everything, `allow-popups` only (links open in new tab)
- Full width/height, padding
- Loading / error states

```tsx
// html-renderer.tsx
export function HtmlRenderer({ content }: { content: string }) {
  return (
    <iframe
      sandbox="allow-popups"
      className="w-full h-full border-0"
      title="HTML preview"
      srcDoc={content}
    />
  )
}
```

> **Security rationale**: Knowledge base files may be authored by agents, making
> malicious `<script>` or inline event handlers possible. `sandbox=""` blocks all
> scripts/forms/same-origin. `allow-popups` permits link clicks to open in a new
> tab. `dangerouslySetInnerHTML` + DOMPurify is riskier — a missed sanitize or
> polyfill failure is a live XSS. iframe srcdoc has no such gap.

**2. `EditorPanel` — extension branching**

```tsx
// editor-panel.tsx
if (currentFilePath?.endsWith('.html')) {
  return <HtmlRenderer key={editorSessionId} filePath={currentFilePath} content={content ?? ''} />
}
if (currentFilePath?.endsWith('.md')) {
  return <MarkdownEditor ... />
}
```

**3. `EditorToolbar` — `.html` extension stripping**

```tsx
const fileName = currentFilePath
  ?.split('/')
  .pop()
  ?.replace(/\.md$/, '')
  ?.replace(/\.html$/, '') ?? ''
```



**5. No "New HTML" creation flow**

HTML files are **not** created through the UI. They enter the knowledge base
when an AI agent writes them or the user places them directly on disk. The
file tree discovers and displays them, and the editor panel renders them.
No dropdown, no "New HTML" button, no HTML-specific i18n keys needed.

**6. `FilePreviewView`** — unchanged (see rationale below)

**Unchanged**:
- `note-rename.ts` — `isProtectedPath` already refuses non-`.md` files; HTML
  files are correctly excluded from H1-driven rename
- `knowledge store` — no extension awareness. EditorPanel handles branching
- `search-modal.tsx` — search is extension-agnostic (filename search)

## 4. 미변경 목록

| 영역 | 이유 |
|------|------|
| `all_md_files()` | Semantic search 대상은 `.md`만 |
| `BacklinkIndex` | `[[wikilink]]`는 마크다운 전용 문법 |
| `build_stem_index()` | wikilink 대상은 `.md`만 |
| `html.rs` 모듈 | Markdown→HTML 변환기. HTML 원문과 무관 |
| API routes | 이미 path-agnostic |
| Git integration | `.html`도 Git이 정상 처리 |
| System files (Chat.md 등) | 계속 `.md` 유지 |
| KnowledgeLens (semantic index) | HTML은 AI 검색 불필요 |
| KnowledgeSettings | HTML 관련 설정 없음 |
| MarkdownEditor | HTML 파일은 편집 불가이므로 접근 불필요 |

---

## 5. 구현 순서

1. 백엔드: `display_name()` 확장자 무관화
2. 백엔드: `only_user_md_files()` → `only_user_text_files()` 리네임 + `.html` 허용
3. 프론트: `HtmlRenderer` 컴포넌트 생성
4. 프론트: `EditorPanel` 확장자 분기
5. 프론트: `EditorToolbar` 파일명 표시 수정
6. 프론트: `FileTree` 인라인 리네임 확장자 인식
7. Smoke test: `.html` 파일을 knowledge 디렉토리에 수동 배치 → tree 표시 → 렌더링 확인

## 6. 검증

- `.html` 파일을 knowledge 디렉토리에 배치 → tree에 표시
- `.html` 파일 열기 → iframe 렌더링
- `.md` 파일 열기 → 여전히 MarkdownEditor
- 인라인 리네임 → 기존 확장자 유지
- Semantic search → `.html` 파일 제외
- Backlinks → `.html` 파일 제외
