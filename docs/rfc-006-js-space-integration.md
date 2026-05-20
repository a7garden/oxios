# RFC-006: JS 연동 + Space × Knowledge 통합 설계

> **날짜**: 2026-05-20
> **상태**: 초안
> **관련**: RFC-005 (Track D/E 미완료 항목)

---

## 0. 현황

RFC-005의 Track A–C가 완료됐다. 남은 두 항목:

1. **JS 연동 (Track D)**: files.md JS 에디터가 아직 Oxios REST API와 연결되지 않았다.
   현재는 브라우저 File System Access API로 로컬 파일시스템에 직접 접근.
2. **Space × Knowledge (Track E)**: `switch_space()`는 구현됐지만, Space 전환 시
   자동으로 호출되지 않음.

---

## Part 1: JS 연동 — Oxios REST API 브릿지

### 1.1 현재 JS 아키텍처 분석

```
index.html
├── lib/fs.js          ← 파일 I/O: getFileHandle(), read(), write(), remove()
│                         (브라우저 File System Access API 사용)
├── lib/sidebar.js     ← 사이드바 트리 UI (1,562줄)
├── lib/similarity.js  ← fuzzy search
├── files.js           ← 파일 관리: loadLocalFiles, syncCurrentText, openFile, moveFile
│                         (920줄, 메모리 파일 트리 + 동기화 로직)
├── editor.js          ← HyperMD/CodeMirror 에디터 초기화
├── app.js             ← 메인 진입점: init(), post(), 동기화 프로토콜
│                         (870줄)
├── chat.js            ← Chat.md 조작 (920줄, 순수 UI — LLM 없음)
├── modals.js          ← 모달 UI
├── welcome.js         ← 초기 Welcome 파일
├── oxios-adapter.js   ← Oxios API 어댑터 (현재 미사용)
└── lib/md.js, lib/emoji.js 등
```

### 1.2 핵심 발견: 파일 I/O의 3개 경로

files.md의 JS 코드에는 **3개의 파일 I/O 경로**가 공존한다:

#### 경로 A: 브라우저 File System Access API (현재 활성)

```javascript
// lib/fs.js — 브라우저 네이티브 API
async function getFileHandle(path, create = false) {
    let currentDirHandle = await getRootDirHandle(); // ← FileSystemDirectoryHandle
    // ... 디렉토리 순회 ...
    fileHandle = await currentDirHandle.getFileHandle(filename, {create});
    return fileHandle;
}

async function read(path) {
    let fileHandle = await getFileHandle(path);
    let file = await fileHandle.getFile();
    return await file.text();
}

async function write(path, content) {
    let fileHandle = await getFileHandle(path, true);
    const writable = await fileHandle.createWritable();
    await writable.write(content);
    await writable.close();
}
```

이 경로는 **사용자가 로컬 폴더를 열었을 때** 사용된다.
`window.showDirectoryPicker()`로 `FileSystemDirectoryHandle`을 얻고,
그 핸들로 직접 파일을 읽고 쓴다.

#### 경로 B: 메모리 파일시스템 (isMemFS)

```javascript
// app.js
let isMemFS = false;

// init()에서:
if (hasSavedLocalDir) {
    isMemFS = false;
} else {
    isMemFS = true;  // ← 로컬 폴더를 안 열었을 때
}
```

`isMemFS = true`일 때는 `loadLocalFiles()`가 빈 메모리 트리를 만들고,
`openFile('/🪴 Welcome.md')`로 Welcome 파일을 연다.
이때 `memFile.content`를 직접 사용.

#### 경로 C: 서버 동기화 프로토콜 (현재 no-op)

```javascript
// files.js — Oxios 모드에서 no-op 처리됨
async function syncTextsWithServer() {
    log('Oxios mode: syncTextsWithServer no-op');
    return;
}

async function syncLocalFileWithServer(path) {
    log('Oxios mode: syncLocalFileWithServer no-op for', path);
    return;
}
```

### 1.3 설계 결정: 경로 B (isMemFS)를 활용하라

**핵심 통찰**: Oxios에서는 **로컬 폴더를 열 필요가 없다**.
파일은 이미 서버(`~/.oxios/workspace/knowledge/`)에 있다.
`isMemFS = true` 모드로 시작하고, Oxios REST API에서 파일을 로드/저장한다.

이렇게 하면:
- `lib/fs.js`의 `read()`, `write()`, `remove()`만 교체하면 된다
- `loadLocalFiles()`를 REST API 기반으로 교체
- `syncCurrentText()`의 저장 로직을 REST API 호출로 교체
- sidebar.js, editor.js, chat.js는 **수정 불필요**

### 1.4 변경 계획

#### 파일 1: `lib/fs.js` — REST API 백엔드 추가

현재 `lib/fs.js`는 File System Access API만 사용.
Oxios 모드에서는 REST API를 사용하도록 조건 분기 추가.

```javascript
// lib/fs.js 상단에 추가:

const OXIOS_API = '/api/knowledge';
const OXIOS_MODE = true;  // Oxios에 임베드되어 있으므로 항상 true

// Oxios REST API 버전의 read/write/remove
async function oxiosRead(path) {
    const relPath = path.replace(/^\//, '');  // "/brain/Rust.md" → "brain/Rust.md"
    const resp = await fetch(`${OXIOS_API}/file/${encodeURIComponent(relPath)}`, {
        credentials: 'include',
        headers: { 'Accept': 'text/plain' }
    });
    if (resp.status === 404) throw new Error('File not found: ' + relPath);
    if (!resp.ok) throw new Error('Read error: ' + resp.status);
    return await resp.text();
}

async function oxiosWrite(path, content) {
    const relPath = path.replace(/^\//, '');
    const resp = await fetch(`${OXIOS_API}/file/${encodeURIComponent(relPath)}`, {
        method: 'PUT',
        credentials: 'include',
        headers: { 'Content-Type': 'text/plain' },
        body: content
    });
    if (!resp.ok) throw new Error('Write error: ' + resp.status);
}

async function oxiosRemove(path) {
    const relPath = path.replace(/^\//, '');
    const resp = await fetch(`${OXIOS_API}/file/${encodeURIComponent(relPath)}`, {
        method: 'DELETE',
        credentials: 'include'
    });
    if (!resp.ok && resp.status !== 404) throw new Error('Delete error: ' + resp.status);
}

async function oxiosExists(path) {
    const relPath = path.replace(/^\//, '');
    try {
        const resp = await fetch(`${OXIOS_API}/file/${encodeURIComponent(relPath)}`, {
            method: 'HEAD',
            credentials: 'include'
        });
        return resp.ok;
    } catch {
        return false;
    }
}

async function oxiosGetTree(dir = '') {
    let url = `${OXIOS_API}/tree`;
    if (dir) url += `?dir=${encodeURIComponent(dir)}`;
    const resp = await fetch(url, {
        credentials: 'include',
        headers: { 'Accept': 'application/json' }
    });
    if (!resp.ok) return [];
    return await resp.json();
}
```

그리고 기존 `read()`, `write()`, `remove()` 함수를 **오버라이드**:

```javascript
// Oxios 모드에서는 기존 File System Access API 대신 REST API 사용
if (OXIOS_MODE) {
    // 기존 함수들을 백업 (필요시 복원 가능)
    window._origRead = read;
    window._origWrite = write;
    window._origRemove = remove;

    // 오버라이드
    read = oxiosRead;
    write = oxiosWrite;
    remove = oxiosRemove;
}
```

**영향 분석**: `read()`, `write()`, `remove()`는 `lib/fs.js`의 최상위 함수이며,
`files.js`와 `app.js`가 이 함수들을 직접 호출한다.
오버라이드하면 모든 파일 I/O가 자동으로 REST API로 라우팅된다.

#### 파일 2: `files.js` — loadLocalFiles 교체

`loadLocalFiles(rootDirHandle)`는 `FileSystemDirectoryHandle`을 받아서
재귀적으로 파일 트리를 구축한다. Oxios 모드에서는 REST API로 트리를 로드.

```javascript
// files.js — loadLocalFiles 상단에 Oxios 분기 추가:

async function loadLocalFiles(rootDirHandle, slowMode = false) {
    if (isLoadingLocalFiles) return;
    isLoadingLocalFiles = true;

    // Oxios: REST API에서 파일 트리 로드
    if (OXIOS_MODE) {
        files = await loadTreeFromAPI();
        isLoadingLocalFiles = false;
        return files;
    }

    // ... 기존 File System Access API 코드 (그대로 유지) ...
}

async function loadTreeFromAPI() {
    let tree = {};
    let rootEntries = await oxiosGetTree('');

    async function loadDir(entries, dirPath) {
        let current = tree;
        if (dirPath !== '/') {
            let parts = dirPath.split('/').filter(Boolean);
            for (let part of parts) {
                let key = part + '/';
                if (!current[key]) current[key] = {};
                current = current[key];
            }
        }

        for (let entry of entries) {
            if (entry.is_dir) {
                let key = entry.name + '/';
                current[key] = {};
                let subEntries = await oxiosGetTree(entry.name);
                await loadDir(subEntries, dirPath + entry.name + '/');
            } else if (entry.name.endsWith('.md')) {
                current[entry.name] = {
                    isFile: true,
                    path: dirPath + entry.name,
                    // content는 openFile 시점에 REST API에서 로드
                };
            }
        }
    }

    await loadDir(rootEntries, '/');
    return tree;
}
```

#### 파일 3: `files.js` — syncCurrentText 저장 로직 교체

`syncCurrentText()`의 파일 저장 부분이 File System Access API의
`createWritable()`을 사용한다. Oxios 모드에서는 REST API로 저장:

```javascript
// syncCurrentText() 내부의 저장 로직:
// 현재:
//   const writable = await file.handle.createWritable();
//   await writable.write(freshContent);
//   await writable.close();

// Oxios 변경:
if (OXIOS_MODE && !file.handle) {
    // REST API로 저장
    await write(path, freshContent);  // oxiosWrite으로 라우팅됨
    currentEditor.markClean();
} else if (file.handle) {
    // 기존 File System Access API 로직 유지
}
```

하지만 이 변경은 `syncCurrentText`의 250줄 코드 깊숙이 파고들어야 한다.
더 안전한 방법은 `write()` 함수를 오버라이드하는 것이다 —
`syncCurrentText`는 결국 `write()`를 통해 저장하므로,
이미 `lib/fs.js`에서 오버라이드했다면 자동으로 REST API가 사용된다.

**문제**: `syncCurrentText`는 `write()`를 직접 호출하지 않고
`file.handle.createWritable()`을 직접 사용한다.

**해결**: `syncCurrentText`의 저장 분기를 Oxios 모드에서 간소화:

```javascript
// syncCurrentText() 내부, 저장 부분:
} else if (!currentEditor.isClean()) {
    isSaving = true;
    try {
        const freshContent = getCurrentContent();
        currentEditor.markClean();

        if (OXIOS_MODE) {
            // Oxios: REST API로 직접 저장
            await write(currentEditor.path, freshContent);
        } else {
            // 기존: File System Access API
            const file = getMemFile(currentEditor.path);
            if (file && file.handle) {
                const writable = await file.handle.createWritable();
                await writable.write(freshContent);
                await writable.close();
            }
        }
    } catch (error) {
        // ... 기존 에러 처리 ...
    }
    isSaving = false;
}
```

#### 파일 4: `app.js` — init() 수정

```javascript
async function init() {
    // Oxios: 인증/토큰 불필요
    markServerOk();

    // Oxios: 로컬 폴더 열기 건너뛰기
    isMemFS = true;

    // Oxios: REST API에서 파일 로드
    files = await loadLocalFiles(null);

    initChat();
    renderSidebar();

    // Oxios: 동기화 불필요
    // await syncTextsWithServer();  ← 이미 no-op
    // await syncMediaFiles();       ← 이미 no-op

    // 첫 파일 열기
    await openFile('/Chat.md');
}
```

#### 파일 5: `app.js` — openFile 시 content 로드

현재 `openFile()`은 `memFile.handle.getFile()`로 파일 내용을 읽는다.
Oxios 모드에서는 handle이 없으므로 REST API에서 로드:

```javascript
// openFile() 내부:
let content = '';
if (OXIOS_MODE && !memFile.handle) {
    // Oxios: REST API에서 파일 내용 로드
    content = await read(path);
} else if (memFile.handle !== undefined) {
    const file = await memFile.handle.getFile();
    content = await file.text();
} else {
    content = memFile.content;
}
```

#### 파일 6: `oxios-adapter.js` — 삭제

`oxios-adapter.js`는 아무도 호출하지 않는다.
`lib/fs.js`의 오버라이드가 그 역할을 대체하므로 삭제.

### 1.5 수정 범위 요약

| 파일 | 변경 유형 | 줄 수 | 설명 |
|------|----------|-------|------|
| `lib/fs.js` | 수정 | +60줄 | `oxiosRead/Write/Remove/GetTree` + 오버라이드 |
| `files.js` | 수정 | ~20줄 | `loadLocalFiles` Oxios 분기, `syncCurrentText` 저장 분기, `openFile` content 로드 |
| `app.js` | 수정 | ~15줄 | `init()` Oxios 모드 활성화 |
| `oxios-adapter.js` | 삭제 | - | 미사용 |
| `lib/sidebar.js` | 수정 없음 | - | `files` 객체만 읽음, API 무관 |
| `editor.js` | 수정 없음 | - | CodeMirror만 담당 |
| `chat.js` | 수정 없음 | - | Chat.md 조작만 담당 |

**총 수정량**: ~95줄 추가, ~10줄 삭제. 기존 코드는 조건 분기로 보존.

### 1.6 Chat.md 쓰기

`chat.js`의 `sendToChat()`은 Chat.md에 텍스트를 append한다.
현재는 `writeAtEnd()` (File System Access API)를 사용.

`lib/fs.js`에 `writeAtEnd()`의 Oxios 버전을 추가:

```javascript
async function oxiosWriteAtEnd(path, content) {
    const relPath = path.replace(/^\//, '');
    // 먼저 현재 내용 읽기
    let existing = '';
    try { existing = await oxiosRead(relPath); } catch { /* 새 파일 */ }
    const updated = existing + content;
    await oxiosWrite(relPath, updated);
}
```

`writeAtEnd`도 오버라이드:

```javascript
if (OXIOS_MODE) {
    writeAtEnd = oxiosWriteAtEnd;
}
```

### 1.7 미디어 파일

files.md는 `media/` 디렉토리에 이미지를 저장한다.
현재 Oxios API는 `.md`만 다룬다.

**Phase 1**: 미디어 업로드는 제외. 에디터에서 이미지를 붙여넣으면
`/knowledge/media/` 경로로 직접 업로드하는 별도 엔드포인트가 필요.

Phase 1에서는:
- 기존 이미지 표시: `<img src="/knowledge/media/photo.png">` —
  Axum의 `ServeDir`이 직접 서빙
- 새 이미지 업로드: 추후 `POST /api/knowledge/media` 추가

### 1.8 검증 방법

```bash
# 1. Oxios 데몬 시작
cargo run -- --foreground

# 2. 브라우저에서 /knowledge/ 열기
# 3. 사이드바에 파일 트리가 보이는지 확인 (GET /api/knowledge/tree)
# 4. 파일 클릭 → 에디터에 내용 로드 (GET /api/knowledge/file/{path})
# 5. 편집 → 자동 저장 (PUT /api/knowledge/file/{path})
# 6. 백링크 확인 — [[link]] 작성 후 다른 파일에서 역방향 참조
```

---

## Part 2: Space × Knowledge 통합

### 2.1 현재 상태

- `KnowledgeApi::switch_space(&self, space_dir: &Path)` — 구현됨
- `KernelEvent::SpaceActivated { space_id, name }` — 이미 발생함
- `SpaceManager::activate()` — 이벤트 발행함
- **연결 없음**: Space 전환 시 KnowledgeApi에 아무 알림이 가지 않음

### 2.2 설계: EventBus 구독

Oxios의 `EventBus`는 pub/sub 패턴이다.
`SpaceActivated` 이벤트를 KnowledgeApi가 구독하면 된다.

하지만 `KnowledgeApi`는 sync struct다 — async 이벤트 핸들러를 등록할 수 없다.
대신 **KernelHandle 수준**에서 이벤트를 구독하고 KnowledgeApi를 호출.

#### 옵션 A: kernel.rs에서 이벤트 구독

```rust
// src/kernel.rs — build() 후 이벤트 구독 설정

let handle = self.handle();
let event_bus = self.event_bus.clone();

event_bus.subscribe(move |event: KernelEvent| {
    if let KernelEvent::SpaceActivated { space_id, name } = event {
        let workspace_dir = // space_id로 workspace 경로 계산
        if let Err(e) = handle.knowledge.switch_space(&workspace_dir) {
            tracing::warn!(error = %e, "Failed to switch knowledge base");
        }
        // 백링크 재인덱싱
        if let Err(e) = handle.knowledge.index_all() {
            tracing::warn!(error = %e, "Failed to reindex knowledge base");
        }
    }
});
```

**문제**: `switch_space()`와 `index_all()`은 sync인데, `EventBus::subscribe`
콜백이 sync여야 할 수도 있다.

#### 옵션 B: SpaceApi::activate()에서 직접 호출

```rust
// space_api.rs
pub async fn activate(&self, id: &str) -> Result<()> {
    let space_id = uuid::Uuid::parse_str(id).context("Invalid Space ID")?;
    self.space_manager.activate(&space_id).await?;

    // KnowledgeApi에 Space 전환 알림
    if let Some(space) = self.space_manager.current_space() {
        let workspace_dir = self.space_manager.default_workspace_dir(&space_id);
        self.knowledge.switch_space(&workspace_dir)?;
        self.knowledge.index_all()?;
    }

    Ok(())
}
```

**문제**: `SpaceApi`가 `KnowledgeApi`를 알아야 함 → 순환 의존.
현재 `SpaceApi`는 `SpaceManager`만 가지고 있음.

#### 옵션 C (선택): KernelHandle에 편의 메서드 추가

```rust
// kernel_handle/mod.rs에 추가:

impl KernelHandle {
    /// Space를 활성화하고 KnowledgeApi를 함께 전환.
    pub async fn activate_space(&self, id: &str) -> anyhow::Result<()> {
        let space_id = uuid::Uuid::parse_str(id).context("Invalid Space ID")?;

        // 1. Space 활성화
        self.spaces.activate(id).await?;

        // 2. KnowledgeApi 전환
        let space = self.spaces.current_space()
            .context("No active space after activation")?;
        let workspace_dir = self.spaces.workspace_dir(&space_id);
        self.knowledge.switch_space(&workspace_dir)?;
        self.knowledge.index_all()?;

        Ok(())
    }
}
```

이 옵션이 가장 깔끔하다:
- `SpaceApi`와 `KnowledgeApi` 사이의 순환 의존 없음
- `KernelHandle`이 이미 두 API를 모두 가지고 있음
- 호출부는 `state.kernel.activate_space(id)` 하나로 끝
- 기존 `spaces.activate(id)`도 그대로 유지 (KnowledgeApi 전환 없이)

### 2.3 SpaceApi에 workspace_dir 추가

`SpaceApi`에 `workspace_dir` 메서드를 추가해서
KernelHandle이 Space의 workspace 경로를 얻을 수 있게:

```rust
impl SpaceApi {
    /// Space의 workspace 디렉토리 경로 반환.
    pub fn workspace_dir(&self, space_id: &uuid::Uuid) -> PathBuf {
        self.space_manager.default_workspace_dir(space_id)
    }
}
```

### 2.4 Space 라우트에서 활용

```rust
// space_routes.rs의 activate 핸들러:
pub(crate) async fn handle_space_activate(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<SpaceInfo>, AppError> {
    // Before: state.kernel.spaces.activate(&id)
    // After:
    state.kernel.activate_space(&id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let space = state.kernel.spaces.current_space()
        .ok_or_else(|| AppError::NotFound("Space not found".into()))?;

    Ok(Json(space))
}
```

### 2.5 파일 변경 요약

| 파일 | 변경 |
|------|------|
| `crates/oxios-kernel/src/kernel_handle/mod.rs` | `activate_space()` 편의 메서드 추가 |
| `crates/oxios-kernel/src/kernel_handle/space_api.rs` | `workspace_dir()` 메서드 추가 |
| `channels/oxios-web/src/routes/space_routes.rs` | `activate_space()` 사용 |

---

## Part 3: 마일스톤

| Phase | 내용 | 예상 공수 |
|-------|------|----------|
| **Phase 1** | `lib/fs.js`에 REST API 백엔드 추가 | 1시간 |
| **Phase 2** | `app.js` init() + `files.js` loadLocalFiles 분기 | 1시간 |
| **Phase 3** | `files.js` syncCurrentText + openFile 분기 | 1시간 |
| **Phase 4** | E2E 수동 테스트 + 버그 수정 | 1시간 |
| **Phase 5** | Space × Knowledge: `activate_space()` | 30분 |
| **Phase 6** | 미디어 업로드 `POST /api/knowledge/media` | 1시간 |

Phase 1–3은 순차. Phase 5는 독립. Phase 6은 선택.

---

## Part 4: 리스크

### 4.1 File System Access API 의존도

`lib/sidebar.js` (1,562줄)은 `files` 객체만 읽고 API를 직접 호출하지 않는다.
하지만 `openFile()`에서 `getFileHandle()`을 직접 호출하는 부분이 있다.
이 부분을 Oxios 모드에서 스킵해야 할 수도 있다.

### 4.2 동시성

현재 files.md는 브라우저 탭 1개에서만 사용한다고 가정한다.
Oxios에서는 에이전트가 동시에 파일을 수정할 수 있다.
merge 알고리즘이 필요한 시나리오.

**Phase 1에서는**: "마지막 쓰기가 이긴다" 정책.
향후 merge 알고리즘(`oxios-markdown::merge`) 도입.

### 4.3 브라우저 호환성

File System Access API는 Chrome/Edge만 지원.
Safari/Firefox는 `isMemFS = true`로 동작.
Oxios 모드에서는 항상 REST API를 사용하므로 **모든 브라우저에서 동작**.
이것이 오히려 장점이 된다.
