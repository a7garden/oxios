# Knowledge File Tree Redesign

> **상태**: 설계
> **날짜**: 2026-07-12
> **선행 문서**: [design-knowledge-ui.md](../design-knowledge-ui.md) §4.2 (FileTree 원본 설계)

---

## 0. 문제 요약

지식 베이스 파일 트리가 설계 문서의 의도대로 동작하지 않는다. **코스메틱 수준이 아니라 근원 구조 결함**이다.

| # | 근원 원인 | 위치 | 영향 |
|---|---------|------|------|
| **R1** | 모듈 수준 전역 `expandedDirs = new Set()` + `forceUpdate` 해킹 | `file-tree.tsx:34,66` | 비반응형 상태 → 확장/축소 불안정, 리마운트 시 깨짐, persist 불가 |
| **R2** | 폴더 확장마다 `useKnowledgeTree(dir)` 개별 fetch | `file-tree.tsx:144` | N+1 쿼리, 확장마다 스피너, 전역 검색/필터 불가 |
| **R3** | `note_move()` 백엔드 구현 완료(원자적 rename + 백링크 재인덱싱) but **HTTP 라우트 미노출** → MoveModal이 write+delete 2단계로 가짜 move 우회 | `knowledge.rs:295` ↔ `move-modal.tsx:93-95` | 비원자적(중간 실패 시 파일 복제), **git 히스토리 연속성 단절**(delete+create로 인식), 백링크 미갱신, 전체 내용 재기록 |
| **R4** | API 핸들러가 `FileEntry`의 `ctime`, `display_name`, `has_content` 버리고 `size: 0` 하드코딩 | `knowledge_routes.rs:459` | 최근 수정 정렬 불가, 빈 파일 구분 불가 |
| **R5** | 설계 §4.2의 컨텍스트 메뉴(rename/delete/new file), DnD, 깜박임 효과 전부 미구현 | `file-tree.tsx` 전체 | 파일 조작 기능 부재 |
| **R6** | `handleNewFile`이 항상 "New file.md" 생성 → 이름 충돌 시 덮어쓰기 | `sidebar.tsx:275` | 데이터 손실 위험 |

---

## 1. 아키텍처 결정

### D1: 전체 트리 1회 패치 (Full Tree Fetch) → 지연 로딩 폐기

```
Before (N+1):                    After (1 request):
GET /tree (root)                 GET /tree?recursive=true
GET /tree?dir=brain              → [{ name, is_dir, children: [...] }]
GET /tree?dir=journal              
GET /tree?dir=projects
... (N requests)
```

**근거**: 개인 지식 베이스는 수십~수백 파일 규모. 백엔드에 이미 `VirtualFs::all_md_files()` (전체 순회)가 있다. 1회 패치로:

- 폴더 확장 **즉시 렌더링** (스피너 없음)
- 트리 내 **인크리멘탈 검색/필터** 가능
- TanStack Query 캐시 1건으로 전체 트리 커버
- `forceUpdate` 해킹 불필요 (데이터가 props로 들어옴)

**트레이드오프**: KB가 10,000+ 파일로 커지면 응답 크기가 문제. 그때는 가상 스크롤 + 딥 레이지 로딩으로 전환. 현재 규모에서는 오버엔지니어링.

### D2: 확장 상태를 Zustand persist로 이동

```
Before:                              After:
// file-tree.tsx (module scope)      // stores/knowledge.ts
const expandedDirs = new Set()       expandedPaths: string[]   // persist: true
function toggleDir(path) { ... }     toggleExpand: (path) => void
                                     expandPath: (path) => void
// FileTreeItem                      collapseAll: () => void
const [, forceUpdate] = useState(0)  
```

- 파일 열 때 **부모 디렉토리 자동 확장**
- 새로고침 후에도 확장 상태 유지
- `collapseAll()` 액션으로 전체 축소

### D3: `POST /api/knowledge/move` 엔드포인트 추가

백엔드에 `KnowledgeBase::note_move(old, new)`가 이미 완전 구현되어 있다 (rename + 백링크 재인덱싱 + 변경 알림). API 라우트만 추가하면 된다.

```
POST /api/knowledge/move
Body: { "from": "brain/Rust.md", "to": "notes/Rust.md" }
→ note_move("brain/Rust.md", "notes/Rust.md")
  → VirtualFs::rename_path() (atomic on same filesystem)
  → BacklinkIndex::remove_file(old) + index_file(new)
  → FileChange::Moved notification
```

MoveModal의 write+delete 우회를 이 엔드포인트로 교체. `rename_path()`는 같은 파일 시스템 내에서 `rename(2)` 시스콜을 사용하므로 **git이 이동을 rename으로 인식** (유사도 기반 rename detection) — delete+create가 아니라 파일 히스토리가 연속된다.

---

## 2. 백엔드 변경

### 2.1 새 엔드포인트: `POST /api/knowledge/move`

```rust
// src/api/routes/knowledge_routes.rs

#[derive(Debug, Deserialize)]
pub(crate) struct KnowledgeMoveBody {
    pub from: String,
    pub to: String,
}

/// POST /api/knowledge/move — move/rename a note atomically.
pub(crate) async fn handle_knowledge_move(
    state: State<Arc<AppState>>,
    Json(body): Json<KnowledgeMoveBody>,
) -> Result<Json<serde_json::Value>, AppError> {
    state
        .kernel
        .knowledge
        .note_move(&body.from, &body.to)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({ "from": body.from, "to": body.to })))
}
```

라우트 등록:
```rust
// mod.rs
.route("/api/knowledge/move", post(handle_knowledge_move))
```

### 2.2 트리 API: 재귀 모드 + 메타데이터 패스스루

```rust
// GET /api/knowledge/tree?recursive=true

// 새 응답 타입 (중첩)
#[derive(Debug, Serialize)]
pub(crate) struct KnowledgeTreeNode {
    pub name: String,
    pub path: String,           // "brain/Rust.md" (full relative path)
    pub is_dir: bool,
    pub size: i64,              // 실제 파일 크기 (더 이상 0 하드코딩)
    pub ctime: i64,             // 수정 시간 (epoch ms)
    pub display_name: String,    // "Rust" (확장자 제거, 대문자화)
    pub has_content: bool,
    pub oxios_quality: Option<String>,
    pub children: Vec<KnowledgeTreeNode>,  // is_dir인 경우만
}
```

```rust
pub(crate) async fn handle_knowledge_tree(
    state: State<Arc<AppState>>,
    Query(params): Query<KnowledgeTreeParams>,
) -> Result<Json<...>, AppError> {
    let recursive = params.recursive.unwrap_or(false);

    if recursive {
        // 전체 트리 재귀 구성
        let tree = build_recursive_tree(&state.kernel.knowledge, "")?;
        Ok(Json(tree))
    } else {
        // 기존 단일 디렉토리 모드 (하위 호환)
        // ...
    }
}

fn build_recursive_tree(
    kb: &KnowledgeBase,
    dir: &str,
) -> Result<Vec<KnowledgeTreeNode>> {
    let entries = kb.note_tree(dir)?;
    entries.iter().filter(|e| !e.name.starts_with('.')).map(|e| {
        let path = if dir.is_empty() || dir == "/" {
            e.name.clone()
        } else {
            format!("{}/{}", dir, e.name)
        };
        let children = if e.is_dir {
            build_recursive_tree(kb, &path)?
        } else {
            vec![]
        };
        // quality는 파일에만, ctime/size/display_name 패스스루
        Ok(KnowledgeTreeNode { ... })
    }).collect()
}
```

**기존 단일 디렉토리 모드(`?dir=`)는 하위 호환을 위해 유지.**

### 2.3 품질 배지 — recursive 응답에 포함 (결정)

**결정: recursive 트리 응답에 `oxios_quality`를 포함한다.**

현재 `handle_knowledge_tree`가 각 파일의 프론트매터를 읽기 위해 `note_read()`를 N회 호출한다. 재귀 트리에서도 동일한 비용이 발생하지만, **개인 KB 규모(수십~수백 파일)에서는 무시 가능한 비용**이다 (각 `note_read`는 단순 파일 읽기 + YAML 파싱).

lazy-load를 배제한 이유:
- 별도의 batch quality 엔드포인트 설계/구현/캐싱 비용 > 단순 N회 파일 읽기 비용
- 트리 로드 시점에 quality를 알아야 §8.5 품질 배지가 렌더링됨 (lazy-load하면 배지가 나중야 나타남 → 깜빡임)
- `note_read`는 이미 백링크 인덱싱 시에도 사용되는 경량 작업

최적화(나중에 필요하면): 프론트매터만 읽는 `read_frontmatter(path)` 경량 함수를 `oxios-markdown`에 추가 — 전체 파일이 아닌 첫 N바이트만 읽음.

---

## 3. 프론트엔드 변경

### 3.1 타입 변경

```typescript
// types/knowledge.ts

export interface KnowledgeTreeNode {
  name: string
  path: string                    // 전체 상대 경로
  is_dir: boolean
  size: number
  ctime: number                   // epoch ms
  display_name: string
  has_content: boolean
  oxios_quality?: 'raw' | 'curated' | 'refined' | null
  children: KnowledgeTreeNode[]
}

// 기존 KnowledgeTreeEntry는 레거시 (단일 dir 모드용)
```

### 3.2 Zustand 스토어: 확장 상태 추가

```typescript
// stores/knowledge.ts

interface KnowledgeState {
  // ... 기존 필드 ...

  // Tree expansion (D2)
  expandedPaths: string[]
  toggleExpand: (path: string) => void
  expandPath: (path: string) => void      // 멱등
  collapseAll: () => void
  expandToPath: (filePath: string) => void // 부모 디렉토리들 자동 확장

  // Keyboard focus (D6 — roving tabindex, §9.3)
  focusedPath: string | null
  setFocus: (path: string | null) => void
```

```typescript
expandToPath: (filePath) => {
  const dirs = filePath.split('/').slice(0, -1)
  let current = ''
  const toExpand: string[] = []
  for (const dir of dirs) {
    current = current ? `${current}/${dir}` : dir
    toExpand.push(current)
  }
  set((s) => ({
    expandedPaths: [...new Set([...s.expandedPaths, ...toExpand])]
  }))
}
```

`openFile` 호출 시 `expandToPath(path)` 자동 실행.

### 3.3 `file-tree.tsx` 재작성

```typescript
interface FileTreeProps {
  nodes: KnowledgeTreeNode[]         // ← prop으로 받음 (더 이상 내부에서 fetch하지 않음)
  onFileSelect: (path: string) => void
  onFileMove?: (from: string, to: string) => void
  onFileRename?: (path: string) => void
  onFileDelete?: (path: string) => void
  currentPath: string | null
  depth?: number                     // 들여쓰기 깊이
}

export function FileTree({ nodes, onFileSelect, currentPath, depth = 0 }: FileTreeProps) {
  // 정렬: 디렉토리 먼저, 그 다음 이름순
  const sorted = useMemo(() =>
    [...nodes].sort((a, b) =>
      Number(b.is_dir) - Number(a.is_dir) || a.name.localeCompare(b.name)
    ), [nodes])

  return (
    <ul className="space-y-0.5">
      {sorted.map((node) => (
        <FileTreeNode
          key={node.path}
          node={node}
          depth={depth}
          onFileSelect={onFileSelect}
          currentPath={currentPath}
        />
      ))}
    </ul>
  )
}
```

핵심 변화:
- **더 이상 내부에서 `useKnowledgeTree` 호출하지 않음** — 데이터는 prop으로
- **`SubDirectory` 컴포넌트 삭제** — children이 이미 노드에 포함됨
- **`expandedDirs` 전역 Set + `forceUpdate` 삭제** — Zustand `expandedPaths` 사용
- **`depth` 기반 들여쓰기** — `paddingLeft: depth * 16 + 8`로 깊이별 정렬

### 3.4 트리 노드 컴포넌트

> **C6 수정**: 더블클릭 rename 제거. 브라우저에서 `dblclick`은 `click` 2회를 먼저 발생시키므로 싱글클릭 open과 충돌. Rename은 F2 / 컨텍스트 메뉴로만 트리거.
> **C2 수정**: `bg-accent` → `bg-sidebar-accent` (트리는 사이드바 내부).

```typescript
interface FileTreeNodeProps {
  node: KnowledgeTreeNode
  depth: number
  currentPath: string | null
  onFileSelect: (path: string) => void
  onRename: (oldPath: string, newName: string) => void   // S1: prop으로 명시
  onContextMenu: (node: KnowledgeTreeNode, x: number, y: number) => void  // S1
}

function FileTreeNode({ node, depth, currentPath, onFileSelect, onRename, onContextMenu }: FileTreeNodeProps) {
  const { expandedPaths, toggleExpand, focusedPath, setFocus } = useKnowledgeStore()
  const moveFile = useMoveFile()  // S1: hook에서 직접 호출
  const isExpanded = expandedPaths.includes(node.path)
  const isActive = currentPath === node.path
  const isFocused = focusedPath === node.path
  const [renaming, setRenaming] = useState(false)

  // S2: 재귀 카운트 — 하위 폴더의 파일까지 모두 합산
  const fileCount = useMemo(() => countFilesRecursive(node), [node])

  const startRename = useCallback(() => setRenaming(true), [])
  const submitRename = useCallback((newName: string) => {
    const parentDir = node.path.includes('/') ? node.path.split('/').slice(0, -1).join('/') : ''
    const newPath = parentDir ? `${parentDir}/${newName}` : newName
    if (newPath !== node.path) {
      moveFile.mutateAsync({ from: node.path, to: newPath })  // S1: 정의된 핸들러
    }
    setRenaming(false)
  }, [node.path, moveFile])

  const handleContextMenu = (e: React.MouseEvent) => {
    e.preventDefault()
    onContextMenu(node, e.clientX, e.clientY)  // S1: prop 콜백 호출
  }

  if (node.is_dir) {
    return (
      <li
        role="treeitem"
        aria-expanded={isExpanded}
        aria-level={depth + 1}
        aria-selected={isActive}
        aria-label={`${node.name}, ${fileCount} files`}
        tabIndex={isFocused ? 0 : -1}
      >
        <div
          className="group flex items-center ..."
          style={{ paddingLeft: depth * 16 + 8 }}
          onClick={() => toggleExpand(node.path)}
          onContextMenu={handleContextMenu}
          onKeyDown={(e) => handleTreeKeyDown(e, node, { startRename })}  // §9.2
        >
          <ChevronRight className={cn('h-3 w-3 shrink-0 transition-transform', isExpanded && 'rotate-90')} />
          {isExpanded ? <FolderOpen className="h-4 w-4 shrink-0 text-muted-foreground" /> : <Folder className="h-4 w-4 shrink-0 text-muted-foreground" />}
          <span className="truncate">{node.name}</span>
          {fileCount > 0 && (
            <span className="ml-auto text-2xs text-muted-foreground/60 shrink-0">
              {fileCount}
            </span>
          )}
        </div>
        {isExpanded && node.children.length > 0 && (
          <div role="group">
            <FileTree
              nodes={node.children}
              depth={depth + 1}
              currentPath={currentPath}
              onFileSelect={onFileSelect}
              onRename={onRename}
              onContextMenu={onContextMenu}
            />
          </div>
        )}
      </li>
    )
  }

  // File node — onDoubleClick 없음 (C6). Rename은 F2/컨텍스트 메뉴만.
  return (
    <li
      role="treeitem"
      aria-level={depth + 1}
      aria-selected={isActive}
      aria-label={node.display_name}
      tabIndex={isFocused ? 0 : -1}
    >
      <div
        className={cn(
          'group relative flex items-center gap-2 py-1.5 rounded-lg text-xs w-full text-left select-none transition-all',
          isActive
            ? 'bg-sidebar-accent text-sidebar-accent-foreground font-medium'
            : 'text-sidebar-foreground/70 hover:bg-sidebar-accent/50 hover:text-sidebar-foreground',
        )}
        style={{ paddingLeft: depth * 16 + 8 }}
        onClick={() => onFileSelect(node.path)}
        onContextMenu={handleContextMenu}
        onKeyDown={(e) => handleTreeKeyDown(e, node, { startRename })}
      >
        {/* 활성 액센트 바 (§10.4) */}
        {isActive && <span className="absolute left-0 top-1 bottom-1 w-0.5 rounded-r bg-primary" />}
        <span className="w-4 shrink-0" /> {/* chevron spacer — 폴더와 정렬 */}
        <File className={cn('h-4 w-4 shrink-0 text-amber-500', !node.has_content && 'opacity-30')} />
        {renaming ? (
          <InlineRenameInput
            currentName={node.display_name}
            onSubmit={submitRename}
            onCancel={() => setRenaming(false)}
          />
        ) : (
          <span className="truncate flex-1">{node.display_name}</span>
        )}
        {node.oxios_quality && !renaming && <QualityBadge quality={node.oxios_quality} />}
      </div>
    </li>
  )
}

// S2: 재귀 파일 카운트 헬퍼
function countFilesRecursive(node: KnowledgeTreeNode): number {
  if (!node.is_dir) return 0
  return node.children.reduce((sum, child) => {
    return sum + (child.is_dir ? countFilesRecursive(child) : 1)
  }, 0)
}
```

### 3.5 컨텍스트 메뉴

```typescript
// components/knowledge/file-tree-context-menu.tsx

interface ContextMenuState {
  x: number
  y: number
  node: KnowledgeTreeNode
}

function FileTreeContextMenu({ state, onClose, onRename, onMove, onDelete, onNewFile }) {
  return (
    <div
      className="fixed z-50 ..."
      style={{ left: state.x, top: state.y }}
    >
      {state.node.is_dir && (
        <MenuItem icon={<FilePlus />} onClick={() => onNewFile(state.node.path)}>
          New file here
        </MenuItem>
      )}
      <MenuItem icon={<Pencil />} shortcut="F2" onClick={() => onRename(state.node)}>
        Rename
      </MenuItem>
      <MenuItem icon={<ArrowRightLeft />} shortcut="⌘M" onClick={() => onMove(state.node)}>
        Move to…
      </MenuItem>
      <Separator />
      <MenuItem icon={<Trash2 />} variant="destructive" shortcut="⌫" onClick={() => onDelete(state.node)}>
        Delete
      </MenuItem>
    </div>
  )
}
```

### 3.6 인라인 이름 변경

> **C3 수정**: Escape 후 onBlur 재실행 방지 (ref로 cancel 상태 추적).
> **C4 수정**: `.md` 이중 append 방지 (입력값이 이미 `.md`로 끝나면 추가 안 함).
> **C7 수정**: 빈 입력 검증 (빈 값이면 submit하지 않고 cancel).

```typescript
function InlineRenameInput({ currentName, onSubmit, onCancel }: {
  currentName: string
  onSubmit: (newName: string) => void
  onCancel: () => void
}) {
  const [value, setCurrent] = useState(currentName)
  const cancelledRef = useRef(false)  // C3: Escape 후 blur 무시

  const handleSubmit = () => {
    const trimmed = value.trim()
    // C7: 빈 입력 → 취소
    if (!trimmed) {
      onCancel()
      return
    }
    // C4: .md가 없으면 추가, 있으면 그대로
    const newName = trimmed.endsWith('.md') ? trimmed : `${trimmed}.md`
    onSubmit(newName)
  }

  return (
    <input
      autoFocus
      selectAll  // 텍스트 전체 선택 (확장자 제외 이름만 편집 용이)
      className="flex-1 rounded bg-sidebar/80 px-1 text-xs outline-none ring-1 ring-ring"
      value={value}
      onChange={(e) => setCurrent(e.target.value)}
      onKeyDown={(e) => {
        if (e.key === 'Enter') {
          e.preventDefault()
          cancelledRef.current = false
          handleSubmit()
        } else if (e.key === 'Escape') {
          cancelledRef.current = true  // C3: blur 시 submit 방지
          onCancel()
        }
      }}
      onBlur={() => {
        if (!cancelledRef.current) handleSubmit()  // C3: Escape가 아니면 submit
      }}
    />
  )
}
```

`POST /api/knowledge/move` 호출: `{ from: oldPath, to: parentDir + '/' + newName }`

### 3.7 드래그 앤 드롭 리페어런팅

HTML5 DnD API 사용 (외부 라이브러리 불필요 — 트리 규모가 작음).

> **S3 수정**: 순환 이동 가드 + dragLeave 깜빡임 방지 추가.

```typescript
// File: draggable
<div
  draggable
  onDragStart={(e) => {
    e.dataTransfer.setData('text/knowledge-path', node.path)
    e.dataTransfer.effectAllowed = 'move'
  }}
>

// Folder: drop target
function FolderDropZone({ node, onFileMove }: { ... }) {
  const [dropTarget, setDropTarget] = useState(false)
  const dragCounter = useRef(0)  // S3: 자식 요소 진입/이탈 깜빡임 방지

  // S3: 순환 이동 가드 — 폴더를 자신 또는 자손에게 드롭 금지
  const isCircularDrop = (from: string, toDir: string): boolean => {
    if (from === toDir) return true
    return from.startsWith(toDir + '/')  // from이 toDir의 상위면 순환
  }
  // 주의: from이 "brain"이고 toDir이 "brain/sub"인 경우,
  // brain을 brain/sub로 옮기면 순환이 발생 → 가드 필요
  // 하지만 from이 "brain/Rust.md"이고 toDir이 "brain/sub"이면 OK
  // 따라서 from이 디렉토리 경로인지만 확인

  return (
    <div
      onDragOver={(e) => {
        e.preventDefault()
        e.dataTransfer.dropEffect = 'move'
      }}
      onDragEnter={() => {
        dragCounter.current++
        setDropTarget(true)
      }}
      onDragLeave={() => {
        dragCounter.current--
        if (dragCounter.current <= 0) {
          dragCounter.current = 0
          setDropTarget(false)
        }
      }}
      onDrop={(e) => {
        e.preventDefault()
        dragCounter.current = 0
        setDropTarget(false)
        const from = e.dataTransfer.getData('text/knowledge-path')
        const filename = from.split('/').pop()!
        const to = `${node.path}/${filename}`
        // S3: 같은 위치이거나 순환이면 무시
        if (to === from) return
        // 순환 검사: from이 디렉토리이고 toDir가 from의 하위인 경우
        if (node.path === from || node.path.startsWith(from + '/')) {
          toast.error('Cannot move a folder into itself')
          return
        }
        onFileMove(from, to)  // POST /api/knowledge/move
      }}
      className={cn(dropTarget && 'bg-primary/10 ring-1 ring-primary/30')}
    >
  )
}
```

### 3.8 MoveModal 단순화

```typescript
// Before: write + delete (2 requests, non-atomic, no backlink update)
await writeFile.mutateAsync({ path: newPath, content: currentContent })
await deleteFile.mutateAsync(currentFilePath)

// After: single move API call (atomic, backlinks updated)
await moveFile.mutateAsync({ from: currentFilePath, to: newPath })
```

```typescript
// hooks/use-knowledge.ts
export function useMoveFile() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ from, to }: { from: string; to: string }) =>
      api.post('/api/knowledge/move', { from, to }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['knowledge', 'tree'] })
      qc.invalidateQueries({ queryKey: ['knowledge', 'backlinks'] })
    },
  })
}
```

또한 MoveModal은 루트 디렉토리만 표시하는데 (`extractDirectories` — root level only), 이를 전체 디렉토리 트리로 확장해야 한다. 재귀 트리 데이터에서 모든 디렉토리 경로를 재귀 추출:

```typescript
// S7: MoveModal 디렉토리 브라우저 — 전체 디렉토리 트리 표시
function extractAllDirs(nodes: KnowledgeTreeNode[], prefix = ''): string[] {
  return nodes
    .filter(n => n.is_dir)
    .flatMap(n => {
      const path = prefix ? `${prefix}/${n.name}` : n.name
      return [path, ...extractAllDirs(n.children, path)]
    })
}
// 결과: ['brain', 'brain/rust', 'brain/go', 'journal', 'projects', ...]
```
사용자가 타이핑하면 이 전체 목록에서 prefix 필터링. `dir?` prop 없이 루트에서 `useKnowledgeTree(recursive=true)` 1회 호출로 모든 디렉토리 확보.

### 3.9 새 파일 생성 개선

> **C5 수정**: `generateUniqueName`이 `defaultName`에서 접두사를 추출하여 충돌 검사에 사용.

```typescript
// sidebar.tsx
const handleNewFile = useCallback(async (dir?: string) => {
  const basePath = dir ? `${dir}/` : ''
  const name = generateUniqueName(entries, basePath, 'New file.md')
  await writeFile.mutateAsync({ path: `${basePath}${name}`, content: '# New file\n\n' })
  openFile(`${basePath}${name}`)
  expandToPath(`${basePath}${name}`)  // 부모 폴더 자동 확장
}, [entries, writeFile, openFile, expandToPath])

function generateUniqueName(
  entries: KnowledgeTreeNode[],
  basePath: string,
  defaultName: string,
): string {
  const existing = new Set(
    flattenTree(entries)
      .filter(n => !n.is_dir)
      .map(n => n.path),
  )
  const fullPath = `${basePath}${defaultName}`
  if (!existing.has(fullPath)) return defaultName

  // C5: defaultName에서 기본 이름과 확장자 분리
  const dotIdx = defaultName.lastIndexOf('.')
  const stem = dotIdx > 0 ? defaultName.slice(0, dotIdx) : defaultName
  const ext = dotIdx > 0 ? defaultName.slice(dotIdx) : ''

  let i = 2
  while (existing.has(`${basePath}${stem} ${i}${ext}`)) i++
  return `${stem} ${i}${ext}`
}
// "New file.md" → 충돌 시 "New file 2.md", "New file 3.md", ...
// "Notes.md" → 충돌 시 "Notes 2.md", "Notes 3.md", ...
```

`handleNewFolder`의 `prompt()` → 인라인 입력 필드 또는 다이얼로그로 교체.

---

## 4. 데이터 흐름

```
┌─────────────────────────────────────────────────────────────┐
│                    Sidebar (KnowledgeNav)                    │
│                                                              │
│  useKnowledgeTree(recursive=true)                            │
│    └→ GET /api/knowledge/tree?recursive=true                 │
│       └→ build_recursive_tree(kb, "")                        │
│          └→ note_tree("") → note_tree("brain") → ...         │
│             └→ 1회 전체 순회, 중첩 트리 반환                   │
│                                                              │
│  FileTree (props: nodes, depth=0)                            │
│    └→ FileTreeNode (depth=0)                                 │
│         ├→ [dir] expanded? → FileTree(children, depth=1)     │
│         └→ [file] active? → highlight                        │
│                                                              │
│  Zustand store:                                              │
│    expandedPaths: ['brain', 'brain/rust']  (persist: true)   │
│    expandToPath('brain/rust/Ownership.md')                   │
│      → ['brain', 'brain/rust']                               │
│                                                              │
│  Actions:                                                    │
│    move  → POST /api/knowledge/move {from, to}               │
│    rename → POST /api/knowledge/move {from, to: dir/newname} │
│    delete → DELETE /api/knowledge/file/{path}                │
│    create → PUT /api/knowledge/file/{path}                   │
└─────────────────────────────────────────────────────────────┘
```

---

## 5. 구현 순서

> **P1 수정**: Workspace/ChatSession 마이그레이션은 "선택적 후속 작업"으로 분리. 지식 트리 설계의 핵심 범위가 아님.
> **P2 수정**: 공통 컴포넌트 대신 CSS 프리미티브 + 인덴트 유틸만 공유. 각 트리는 독립 유지.
> **P3 수정**: 테스트 단계 추가.

### Phase 1: 백엔드 (R3, R4 해결)
1. `POST /api/knowledge/move` 라우트 + 핸들러 추가 (`note_move` 노출)
2. `GET /api/knowledge/tree?recursive=true` 재귀 모드 + `KnowledgeTreeNode` 응답 타입
3. 품질 배지 포함 — `oxios_quality` 패스스루 (§2.3 결정)
4. 정렬: 백엔드 정렬 제거, 프론트엔드에서 통일 정렬 (S6)

### Phase 2: 상태 관리 (R1 해결)
5. `stores/knowledge.ts`에 `expandedPaths`, `focusedPath`, `setFocus` + 액션 추가 (persist)
6. `file-tree.tsx`에서 전역 `expandedDirs` Set + `forceUpdate` 삭제
7. `openFile` 시 `expandToPath` 자동 호출

### Phase 3: 지식 파일 트리 재작성 (R2, R5 해결)
8. `lib/tree-utils.ts` — 공유 유틸: `indentStyle(depth)`, `fileTint(name)`, `countFilesRecursive(node)` (P2: 컴포넌트가 아닌 유틸만 공유)
9. `FileTree` + `FileTreeNode` 재작성 — prop 구동, `sidebar-accent` 토큰 (C2)
10. `SubDirectory` 삭제 — children 직접 렌더링
11. `KnowledgeNav`에서 `useKnowledgeTree(recursive=true)` 호출
12. ARIA: `role="tree/treeitem"`, `aria-level/expanded/selected` (D6)
13. 인라인 필터 입력창 (§8.6)
14. 빈 상태 `EmptyState` (§8.9)
15. 품질 배지 (§8.5: h-3.5 + 툴팁)

### Phase 4: 파일 조작 (R3, R5, R6 해결)
16. `useMoveFile` hook 추가 → `POST /api/knowledge/move`
17. MoveModal을 move API로 교체 + 전체 디렉토리 브라우저 (S7)
18. 컨텍스트 메뉴 (§8.7)
19. 인라인 이름 변경 (§3.6: C3/C4/C7 수정 적용)
20. DnD 리페어런팅 (§3.7: S3 순환 가드 + 깜빡임 방지)
21. 새 파일 이름 충돌 방지 (§3.9: C5 수정 적용)
22. `handleNewFolder` `prompt()` → 인라인 입력

### Phase 5: 접근성 + 폴리시
23. `useFlattenedVisibleTree(nodes, expandedPaths)` 훅 (S5: 키보드 탐색용)
24. 키보드 탐색 구현 (§9.2: Arrow/Enter/Space/Home/End/F2/Delete/Escape)
25. roving tabindex (§9.3: `focusedPath` Zustand 추적)
26. 새 파일 생성 시 깜박임 애니메이션
27. `design-knowledge-ui.md` §4.2 갱신

### Phase 6: 테스트 (P3)
28. 백엔드: `handle_knowledge_move` 통합 테스트 (move + 백링크 재인덱싱 검증)
29. 백엔드: `build_recursive_tree` 유닛 테스트 (중첩 구조, 빈 폴더, 숨김 파일)
30. 프론트엔드: `generateUniqueName` 유닛 테스트 (충돌, 커스텀 이름, 빈 트리)
31. 프론트엔드: `InlineRenameInput` 테스트 (Escape+blur, 빈 입력, .md 이중 append)
32. 프론트엔드: DnD 순환 가드 테스트 (폴더→자손, 폴더→자기 자신, 파일→폴더)
33. E2E: 파일 생성 → 이름변경 → 이동 → 삭제 전체 워크플로우

### 선택적 후속 작업 (P1 — 본 설계 범위 외)
- Workspace 트리를 공유 유틸로 정렬
- ChatSession 트리를 공유 유틸로 정렬
- 3개 트리의 시각적 토큰 통일 감사

---

## 6. design-knowledge-ui.md과의 관계

본 문서는 `design-knowledge-ui.md` §4.2 FileTree 섹션의 **갭 분석 + UI 재설계**이다.

| design-knowledge-ui.md §4.2 명세 | 현재 구현 | 본 설계 |
|----------------------------------|----------|---------|
| 디렉토리 확장/축소 (지연 로딩) | ✅ 구현됨 (N+1) | 재귀 1회 패치로 교체 (D1) |
| 파일 클릭 → openFile(path) | ✅ | 유지 |
| 우클릭 컨텍스트 메뉴 (이름 변경, 삭제, 새 파일) | ❌ 미구현 | Phase 5 (§8.7) |
| 시스템 디렉토리 숨김 | ✅ (하드코딩) | 백엔드 IGNORED_NAMES에 의존 |
| 새로 생성된 파일 깜박임 | ❌ 미구현 | Phase 6 |
| 드래그 리사이즈 핸들 | ❌ 미구현 | 본 설계 범위 외 (별도 이슈) |
| [+New File] / [+New Dir] 버튼 | ✅ (충돌 버그) | 이름 충돌 방지 (Phase 5) |
| ARIA 트리 / 키보드 탐색 | ❌ (원본에 명시 없음) | Phase 3+6 (§9 — D6) |
| 공통 트리 프리미티브 | ❌ (3개 트리 파편화) | Phase 3 (§7 — D4) |

`design-knowledge-ui.md`의 §4.2는 본 설계 구현 후 갱신된다.

---

## 7. UI 설계 — 시각적 일관성 감사

"파일 트리가 제대로 구성이 안 되어 있다"는 느낌의 근원에는 **같은 앱에 3개의 트리 컴포넌트가 각각 다른 패턴으로 구현되어 있다**는 사실이 있다. 사용자는 무의식적으로 일관성을 기대하는데, 3개 트리가 시각적/동작적 파편화를 일으킨다.

### 7.1 3개 트리 컴포넌트 감사

| 패턴 | Workspace (`workspace/index.tsx`) | Knowledge (`file-tree.tsx`) | ChatSession (`chat-session-nav.tsx`) |
|------|-----------------------------------|------------------------------|---------------------------------------|
| **들여쓰기** | `depth * 16 + 8` px | `pl-4` + `w-4` spacer (불일치) | DOM 중첩 (명시적 depth 없음) |
| **ARIA** | `role="treeitem"` `tabIndex={0}` | ❌ 없음 | ❌ 없음 |
| **키보드** | Enter/Space → 활성화 | ❌ 없음 | ❌ 없음 |
| **파일 아이콘** | `fileTint()` 확장자별 색상 | 단색 `text-muted-foreground` | N/A |
| **폴더 아이콘** | `Folder` (경고색) | `Folder`/`FolderOpen` (회색) | `FolderKanban` |
| **펼침 아이콘** | `ChevronDown`/`ChevronRight` | `ChevronRight` 회전 | `ChevronRight` 회전 |
| **크기 표시** | KB/B (`ml-auto`) | ❌ (size 항상 0) | 세션 카운트 (`text-2xs`) |
| **선택 하이라이트** | `bg-primary/10 text-primary` | `bg-accent font-medium` | store 기반 |
| **호버** | `hover:bg-muted/50` | `hover:bg-accent/50` | `hover:bg-sidebar-accent` |
| **확장 상태** | 로컬 `useState<Set>` | **전역 모듈 Set + forceUpdate** | 로컬 `useState<Set>` |
| **DnD** | ❌ | ❌ | ✅ (세션 → 프로젝트) |
| **컨텍스트 메뉴** | ❌ | ❌ | ✅ (DropdownMenu) |
| **빈 상태** | `EmptyState` 컴포넌트 | ❌ | ❌ |

### 7.2 통합 원칙

3개 트리가 공통으로 따라야 할 규칙을 정의한다. Knowledge 파일 트리 재설계가 이 규칙의 **첫 구현체**가 되고, 이후 Workspace/ChatSession이 따라온다.

**D4: 공유 유틸 + CSS 프리미티브 (컴포넌트 강제 병합 아님)**

> **P2 수정**: 13-prop `<TreeNode>` 컴포넌트는 과추상화. 대신 **유틸 함수 + CSS 클래스만 공유**하고 각 트리 컴포넌트는 독립 유지. 일관성은 토큰/유틸로 보장.

```typescript
// lib/tree-utils.ts — 3개 트리가 공유하는 유틸

/** 들여쓰기 스타일 계산 (D5) */
export function indentStyle(depth: number): React.CSSProperties {
  return { paddingLeft: `${depth * 16 + 8}px` }
}

/** 파일 타입별 아이콘 색상 — Workspace 패턴 재사용 */
export function fileTint(name: string): string {
  const ext = name.split('.').pop()?.toLowerCase() ?? ''
  if (['md', 'txt'].includes(ext)) return 'text-amber-500'
  if (['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp'].includes(ext)) return 'text-pink-500'
  if (['rs', 'ts', 'tsx', 'js', 'py', 'go'].includes(ext)) return 'text-blue-500'
  return 'text-muted-foreground'
}

/** 재귀 파일 카운트 (S2) */
export function countFilesRecursive<T extends { is_dir: boolean; children?: T[] }>(node: T): number {
  if (!node.is_dir) return 0
  return (node.children ?? []).reduce((sum, child) =>
    sum + (child.is_dir ? countFilesRecursive(child) : 1), 0)
}
```
각 트리 컴포넌트(Knowledge, Workspace, Chat)는 이 유틸을 import하여 자체 렌더링. 공통 스타일은 `sidebar-accent` 토큰 + 동일한 Tailwind 클래스로 통일. 컴포넌트 구조가 다른 도메인(세션 vs 파일)을 강제로 하나로 묶지 않는다.

**D5: 들여쓰기 단위 통일 → `depth * 16 + 8` px**

Workspace 트리의 `depth * 16 + 8`을 공통 단위로 채택. 16px는 한 글자 너비에 가까워 깊이 구분이 명확하고, 8px 기본 패딩은 사이드바 `p-2`(8px)와 정렬된다.

```
depth 0:  ████████░░ name          (8px)
depth 1:  ████████████████░░ name  (24px)
depth 2:  ████████████████████████░░ name  (40px)
```

현재 Knowledge의 `pl-4`(16px) + `w-4`(16px) spacer는 depth 1에서 32px가 되어 비대칭.

**D6: ARIA 트리 패턴 채택**

```html
<div role="tree" aria-label="Knowledge files">
  <div role="treeitem" aria-expanded="true" aria-level="1" aria-selected="false"
       tabIndex={0}>
    brain/
  </div>
  <div role="group">
    <div role="treeitem" aria-level="2" aria-selected="true" tabIndex={0}>
      Ownership.md
    </div>
  </div>
</div>
```

WAI-ARIA Tree View 패턴. 스크린 리더가 "확장됨, 레벨 1, 폴더, 3개 항목"을 읽어준다.

---

## 8. UI 설계 — 지식 파일 트리 상세

### 8.1 사이드바 전체 구조 (개선 전/후)

```
개선 전 (현재):                          개선 후:
┌────────────────────────┐               ┌────────────────────────┐
│ Oxios            ◀ ▶  │               │ Oxios            ◀ ▶  │
│ [Console][Knowledge]  │               │ [Console][Knowledge]  │
│ ─────────────────────  │               │ ─────────────────────  │
│                        │               │                        │
│ 🏠 Home                │               │ 🏠 Home                │
│ 💬 Quick Notes         │               │ 💬 Quick Notes    [3]  │
│ 📖 Journal             │               │ 📖 Journal             │
│ 🔗 Graph               │               │ 🔗 Graph               │
│ ─────────────────────  │               │ ─────────────────────  │
│ FILES                  │               │ Files    🔍 ⬚          │
│                        │               │ ┌──────────────────┐   │
│  📁 brain              │               │ │ (인라인 필터)      │   │
│   📄 Rust              │               │ └──────────────────┘   │
│   📄 Go                │               │                        │
│  📄 Chat.md            │               │  ▸ 📁 brain       3    │
│  📄 Later.md           │               │  ▸ 📁 journal          │
│  📁 journal            │               │  · 📄 Chat.md          │
│                        │               │  · 📄 Later.md    ◆    │
│                        │               │  · 📄 Notes            │
│                        │               │  · 📄 Done          ◐  │
│                        │               │                        │
│ ─────────────────────  │               │ ─────────────────────  │
│ [➕][📁][🗑] [📊][⚙] ⌘K│               │ [+File][+Folder]  ⌘K  │
└────────────────────────┘               └────────────────────────┘
```

개선점:
1. **인라인 필터 입력창** 추가 — 트리 내에서 빠르게 파일 검색 (⌘K를 열지 않아도 됨)
2. **Quick Notes 카운트 배지** — 미처리 inbox 개수 표시
3. **폴더 파일 카운트** — 폴더명 우측에 자식 파일 수
4. **품질 배지 가시성 향상** — 더 큰 아이콘 + 툴팁
5. **액션바 단순화** — 파일 조작과 도구 분리

### 8.2 파일 노드 해부도

```
 depth=1, active=false, has_content=true, oxios_quality="refined"

 ┌──┬───┬─────────────────────────┬──────┬───┐
 │  │ ▸ │ 📄  Ownership Rules      │  ◆  │   │
 └──┴───┴─────────────────────────┴──────┴───┘
  ↑   ↑    ↑       ↑                    ↑
  │   │    │       │                    │ 품질 배지 (refined=◆)
  │   │    │       │                    "raw"=🤖 "curated"=✨ "refined"=◆
  │   │    │       └─ 파일명 (display_name, .md 제거)
  │   │    └─ 파일 아이콘 (fileTint: .md=amber)
  │   └─ 펼침/여백 (디렉토리=ChevronRight, 파일=w-4 spacer)
  └─ 들여쓰기 패딩 (depth * 16 + 8 px)
```

### 8.3 폴더 노드 해부도

```
 depth=0, expanded=true, 3 child files

 ┌──┬─────┬────────────┬─────┬──────────┐
 │  │ ▾  │ 📁  brain   │  3  │ ⋯ (hover)│
 └──┴─────┴────────────┴─────┴──────────┘
  ↑   ↑      ↑              ↑      ↑
  │   │      │              │      └─ 호버 시 더보기 메뉴 (rename/move/delete/new)
  │   │      │              └─ 자식 카운트 (text-2xs, muted)
  │   │      └─ 폴더 아이콘 (FolderOpen when expanded)
  │   └─ 펼침 토글 (ChevronRight → 회전 90° = 펼침 / ChevronDown도 OK)
  └─ 들여쓰기 패딩 (depth * 16 + 8 px)
```

### 8.4 상태 매트릭스

| 상태 | 배경 | 텍스트 | 아이콘 | 추가 |
|------|------|--------|-------|------|
| **기본** | transparent | `sidebar-foreground/70` | `muted-foreground` | — |
| **호버** | `sidebar-accent/50` | `sidebar-foreground` | 원래색 | ⋯ 버튼 나타남 |
| **활성** | `sidebar-accent` | `sidebar-accent-foreground` `font-medium` | 원래색 | 좌측 2px 액센트 바 |
| **포커스** | `sidebar-accent/30` | 원래색 | 원래색 | `ring-1 ring-ring` |
| **드래그 중** | `primary/5` | 원래색 | `opacity-50` | 점선 테두리 |
| **드롭 타겟** | `primary/10` | 원래색 | 원래색 | `ring-1 ring-primary/30` |
| **이름변경** | `sidebar-accent` | — | 숨김 | `<input>` 인라인 |
| **빈 파일** | — | `foreground/40` | `opacity-40` | — |
| **새 파일** | `primary/10` | `primary` | 원래색 | blink 애니메이션 (2회) |

### 8.5 품질 배지 설계

```
┌─────────────────────────────────────────┐
│ oxios_quality  아이콘  색상      의미     │
├─────────────────────────────────────────┤
│ (없음)         —      —          사용자 작성 │
│ raw            🤖     muted      AI 초안     │
│ curated        ✨     success    AI 다듬음   │
│ refined        💎     info       AI 정제     │
└─────────────────────────────────────────┘
```

현재: 2.5px 아이콘 → 식별 불가
개선: 3.5px 아이콘 + 호버 시 툴팁 ("AI 정제됨")

### 8.6 인라인 필터

트리 상단에 작은 검색 입력창. 타이핑 즉시 트리를 플랫 리스트로 전환하여 매칭 파일만 표시. 매칭된 파일의 부모 폴더는 자동으로 표시 (경로 컨텍스트 유지).

```
입력 전:                       "rust" 입력 후:
┌──────────────────────┐      ┌──────────────────────┐
│ Files    🔍          │      │ Files    🔍 rust  ✕  │
│                       │      │                       │
│ ▸ 📁 brain       3   │      │ · 📄 brain/Rust.md    │
│ · 📄 Chat.md          │      │ · 📄 brain/Owner….md  │
│ · 📄 Later.md         │      │ · 📄 Rust Patterns    │
│ ▸ 📁 journal          │      │                       │
└──────────────────────┘      └──────────────────────┘
```

특징:
- 입력 시작 → 트리를 플랫 리스트로 전환, 모든 매칭 파일 표시
- 파일명 + 경로 모두 매칭
- Enter → 첫 번째 결과 열기
- Escape → 필터 해제, 트리 복원
- 백엔드 호출 없음 (클라이언트 사이드 필터 — 전체 트리가 이미 메모리에 있음, D1)

### 8.7 컨텍스트 메뉴

```
         우클릭 또는 ⋯ 버튼
              ┌──────────────────────┐
              │ 📄 New file here      │  (폴더에만)
              │ ─────────────────── │
              │ ✏️ Rename        F2   │
              │ ↗  Move to…      ⌘M  │
              │ ─────────────────── │
              │ 🗑  Delete       ⌫   │  (destructive)
              └──────────────────────┘
```

기존 `DropdownMenu` 컴포넌트 재사용 (ChatSession의 패턴과 동일).

### 8.8 드래그 앤 드롭

```
드래그 시작:                    드롭 타겟:
┌──────────────────────┐      ┌──────────────────────┐
│ · 📄 Rust    (반투명) │  →   │ ▸ 📁 brain           │
│ · 📄 Go               │      │   (ring-1 ring-primary│
│ · 📄 Chat.md          │      │    bg-primary/10)    │
└──────────────────────┘      └──────────────────────┘
                              POST /api/knowledge/move
                              { from: "Rust.md", to: "brain/Rust.md" }
```

ChatSessionNav의 DnD 패턴(`window.__draggedSessionId`)과 동일한 구조.
드래그 중인 노드: `opacity-50` + 점선 테두리.
드롭 타겟 폴더: `ring-1 ring-primary/30 bg-primary/10`.

### 8.9 빈 상태

```
┌──────────────────────────┐
│                          │
│      📝                  │
│                          │
│   No notes yet           │
│   Create your first note │
│                          │
│   [+ New file]           │
│                          │
└──────────────────────────┘
```

`EmptyState` 공유 컴포넌트 재사용 (Workspace 패턴과 동일).

---

## 9. 접근성 (Accessibility)

### 9.1 ARIA 트리 패턴

```tsx
<div role="tree" aria-label={t('knowledge.files')}>
  {nodes.map(node => <TreeNodeWrapper node={node} level={1} />)}
</div>

// 폴더
<div
  role="treeitem"
  aria-expanded={isExpanded}
  aria-level={depth + 1}
  aria-selected={isActive}
  aria-label={`${node.name} folder, ${childCount} items`}
  tabIndex={isActive ? 0 : -1}
>
  {isExpanded && <div role="group">{children}</div>}
</div>

// 파일
<div
  role="treeitem"
  aria-level={depth + 1}
  aria-selected={isActive}
  aria-label={`${node.display_name}${quality ? `, ${quality}` : ''}`}
  tabIndex={isActive ? 0 : -1}
>
```

### 9.2 키보드 탐색 (WAI-ARIA Tree View)

| 키 | 동작 |
----|------|
| **Arrow Up** | 이전 트리 아이템으로 포커스 |
| **Arrow Down** | 다음 트리 아이템으로 포커스 |
| **Arrow Right** | 폴더 확장 / 확장된 폴더의 첫 자식으로 이동 |
| **Arrow Left** | 폴더 축소 / 축소된 폴더의 부모로 이동 |
| **Enter** | 파일 열기 / 폴더 토글 |
| **Space** | 파일 열기 / 폴더 토글 |
| **Home** | 첫 아이템 |
| **End** | 마지막 아이템 |
| **F2** | 인라인 이름 변경 |
| **Delete** | 삭제 확인 다이얼로그 |
| **Escape** | 이름 변경 취소 / 컨텍스트 메뉴 닫기 |

### 9.3 roving tabindex

포커스 가능한 아이템은 활성 아이템 하나만 `tabIndex={0}`, 나머지는 `tabIndex={-1}`. Tab 키로 트리 진입/이탈, Arrow 키로 트리 내 탐색. 포커스 위치는 Zustand `focusedPath: string | null`로 추적.

### 9.4 가시 트리 플랫 리스트 (S5)

Arrow Up/Down 탐색은 현재 **보이는** 트리 아이템의 순서가 필요하다. 확장 상태에 따라 가시 아이템이 변하므로, 트리를 평탄화하는 훅이 필요하다.

```typescript
// hooks/use-flattened-visible-tree.ts

interface FlatItem {
  node: KnowledgeTreeNode
  depth: number
}

/** 확장 상태에 따라 보이는 노드만 평탄화된 리스트 반환 */
export function useFlattenedVisibleTree(
  nodes: KnowledgeTreeNode[],
  expandedPaths: string[],
): FlatItem[] {
  return useMemo(() => {
    const result: FlatItem[] = []
    const expanded = new Set(expandedPaths)  // O(1) membership
    const walk = (items: KnowledgeTreeNode[], depth: number) => {
      for (const node of items) {
        result.push({ node, depth })
        if (node.is_dir && expanded.has(node.path)) {
          walk(node.children, depth + 1)
        }
      }
    }
    walk(nodes, 0)
    return result
  }, [nodes, expandedPaths])
}
```

키보드 핸들러는 이 리스트의 인덱스로 이동:
```typescript
function handleTreeKeyDown(e, currentNode, flatList, { startRename }) {
  const idx = flatList.findIndex(f => f.node.path === currentNode.path)
  if (e.key === 'ArrowDown') { flatList[idx + 1] && setFocus(flatList[idx + 1].node.path) }
  if (e.key === 'ArrowUp') { flatList[idx - 1] && setFocus(flatList[idx - 1].node.path) }
  // ... ArrowRight/Left, Home, End
}
```

---

## 10. 시각적 사양 (Visual Specs)

### 10.1 타이포그래피

| 요소 | 폰트 | 크기 | 굵기 | 행높이 |
------|------|------|------|--------|
| 파일명 | Geist | `text-xs` (12px) | normal | `leading-tight` |
| 폴더명 | Geist | `text-xs` (12px) | `font-medium` | `leading-tight` |
| 활성 파일명 | Geist | `text-xs` (12px) | `font-medium` | `leading-tight` |
| 카운트 배지 | Geist Mono | `text-2xs` (10px) | normal | — |
| 섹션 헤더 | Geist | `text-xs` (12px) | `font-medium` | `uppercase tracking-wider` |
| 인라인 필터 | Geist | `text-xs` (12px) | normal | — |

### 10.2 간격

```
트리 아이템 높이:    28px (py-1.5 + content)
아이템 간 간격:       2px (space-y-0.5)
아이콘 크기:          h-4 w-4 (16px) — 폴더, 파일
                      h-3 w-3 (12px) — 펼침 화살표
                      h-3.5 w-3.5 (14px) — 품질 배지
아이콘 간 gap:        gap-2 (8px)
좌측 패딩:            depth * 16 + 8 px
우측 패딩:            px-2.5 (10px)
섹션 헤더 하단 간격:  mb-1 (4px)
섹션 간 구분선:       my-2 (8px)
```

### 10.3 색상 매핑 (OKLCH 토큰)

| 요소 | Light | Dark | 토큰 |
|------|-------|------|------|
| 트리 배경 | `oklch(0.978 0.002 265)` | `oklch(0.16 0.008 265)` | `--sidebar` |
| 기본 텍스트 | `oklch(0.141 …)` / 70% | `oklch(0.985 …)` / 70% | `--sidebar-foreground` /70 |
| 호버 배경 | `oklch(0.967 0.003 265)` /50 | `oklch(0.274 0.01 265)` /50 | `--sidebar-accent` /50 |
| 활성 배경 | `oklch(0.967 0.003 265)` | `oklch(0.274 0.01 265)` | `--sidebar-accent` |
| 활성 텍스트 | `oklch(0.23 0.025 265)` | `oklch(0.985 0)` | `--sidebar-accent-foreground` |
| 파일 아이콘 (.md) | `text-amber-500` | `text-amber-500` | Tailwind palette |
| 폴더 아이콘 | `text-muted-foreground` | `text-muted-foreground` | — |
| 카운트 | `muted-foreground` /60 | `muted-foreground` /60 | — |
| 액센트 바 (활성) | `--primary` | `--primary` | 2px 좌측 |
| 구분선 | `oklch(0.92 0.004 286)` | `oklch(1 0 0 / 10%)` | `--sidebar-border` |

### 10.4 활성 파일 액센트 바

현재 활성 파일은 배경색만으로 구분되어 다크 모드에서 거의 보이지 않는다. 좌측 2px 액센트 바를 추가:

```css
.tree-item-active::before {
  content: '';
  position: absolute;
  left: 0;
  top: 4px;
  bottom: 4px;
  width: 2px;
  border-radius: 0 2px 2px 0;
  background: var(--primary);
}
```

VS Code, Notion, Obsidian 모두 사용하는 패턴. 사이드바 너비 60px(=w-60)에서도 활성 파일이 즉시 식별된다.
