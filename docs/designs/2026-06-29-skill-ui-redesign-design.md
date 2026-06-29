# Skills UI — 저작·임포트·인스펙터 리디자인 설계

> **상태**: 설계 (v1)
> **날짜**: 2026-06-29
> **범위**: `web/src/routes/skills.tsx`, `web/src/components/skills/*`, `crates/oxios-kernel/src/skill/manager.rs`, `src/api/routes/{workspace,mod}.rs`
> **선례**: Claude(Anthropic) Skills 설정 UX, `docs/design-knowledge-ui.md`(포맷), `docs/designs/2026-06-15-settings-redesign.md`(3-zone + mutation-driven 패턴)

---

## 1. 동기 (Motivation)

사용자 피드백: *"Claude Web UI의 스킬 설정 부분은 스킬에 관련한 UI/UX를 아주 훌륭하게 지원한다. 직접 등록할 수도 있고 .skill 파일로 직접 임포트할 수도 있다. 우리도 필요하다. 새로고침 버튼 따위는 필요 없다."*

현재 `/skills` 화면이 안고 있는 문제들:

1. **저작(작성) 불가** — 백엔드에 `POST /api/skills`(name/description/content)이 있지만 **프론트엔드엔 '스킬 만들기' 버튼이 단 하나도 없다.** 직접 등록 경로가 UI에 존재하지 않는다.
2. **임포트 불가** — `.skill` / `.zip` / `.md` 파일 업로드, 텍스트 붙여넣기, URL 가져오기 어느 것도 없다. 마켓플레이스 설치(ClawHub/Skills.sh)만 유일한 '설치' 경로다.
3. **콘텐츠 열람 불가** — `GET /api/skills/{name}/content`(SKILL.md 본문) 엔드포인트가 있지만 `SkillDetail` 패널은 메타데이터·요구사항·config 체크만 보여주고 **실제 지시문(SKILL.md body)을 전혀 렌더링하지 않는다.** 스킬이 실제로 뭘 하는지 UI에서 알 수 없다.
4. **편집 불가** — 인라인 편집 UI가 없다. 설령 UI를 만들더라도 `create_skill`로 라우팅하면 **파괴적**이다(§7.1).
5. **새로고침 버튼** — 헤더에 수동 `RefreshButton` + 30초 폴링이 공존. 모든 뮤테이션은 이미 `qc.invalidateQueries(['skills'])`를 호출하므로 버튼은 중복이다.

> 핵심: 백엔드는 '읽기·토글·삭제'와 '파괴적 생성'만 있고, 프론트엔드는 '목록·설치'만 있다. Claude급 스킬 관리 경험을 만들려면 **저작·임포트·열람·편집** 4개 축이 모두 열려야 한다.

---

## 2. 백엔드 역량 인벤토리 (검증 완료)

> 이 설계의 분기점. UI 전용 작업인지, 백엔드 델타가 필요한지를 이 표가 결정한다.

### 2.1 이미 있는 것 (HAS)

| 엔드포인트 | 핸들러 | 용도 |
|---|---|---|
| `GET /api/skills` | `handle_skills_list` (workspace.rs:453) | 전체 스킬 목록 |
| `GET /api/skills/{name}` | `handle_skill_get` (workspace.rs:463) | 단일 스킬 상세 |
| `GET /api/skills/{name}/content` | `handle_skill_content` (workspace.rs:508) | **SKILL.md 원문** — 열람(뷰어)용으로 이미 사용 가능 |
| `POST /api/skills` | `handle_skill_create` (workspace.rs:540) | 생성(name/description/content, 64KB 제한) |
| `POST /api/skills/{name}/enable\|disable` | (workspace.rs:474/491) | 토글 |
| `DELETE /api/skills/{name}` | `handle_skill_delete` (workspace.rs:571) | 삭제 |

**재사용 가능 커널 조각(검증됨):**

| 함수 | 위치 | 역할 |
|---|---|---|
| `parse_skill(content, dir)` | `frontmatter.rs:311` | frontmatter/body 분리 + YAML 검증 + **4 포맷 자동 감지**(Oxios/OpenClaw/ClaudeCode/AgentSkills). `.skill` 임포트 검증기 그 자체. |
| `load_skill_entry(file, bundled)` | `manager.rs:217` | SKILL.md 읽기 → `parse_skill` → 요구사항 체크 → `SkillEntry` 재인덱스. 편집 후 reindex 경로. |
| `extract_archive` + `find_skill_root` | `clawhub/installer.rs:294,346` | zip 안에서 `SKILL.md` 마커 탐지 + 루트 prefix 스트리핑. `.skill` zip 업로드에 그대로 재사용. |
| `is_safe_relative_path` | `crate::skill::` | Zip Slip / 경로 탈출 방어(두 인스톨러가 이미 사용). 임포트에도 필수. |
| 출처(provenance) 패턴 | `.clawhub/origin.json`, `.skills_sh/origin.json` | 설치 출처 추적. `.skill` 파일 임포트에 `.imported/origin.json`로 재사용. |

### 2.2 없는 것 (MISSING) — 백엔드 델타

| 기능 | 이유 | §7에서 정의 |
|---|---|---|
| **원문 보존 쓰기** | `create_skill`(manager.rs:134)는 frontmatter를 `name`+`description`만으로 **재합성**하여 덮어쓴다 → requires/install/allowed-tools/autonomous/primaryEnv 같은 풍부한 메타데이터가 전부 소거됨. | `write_skill_raw` (§7.1) |
| **`.skill`/zip 임포트** | multipart 업로드 엔드포인트 자체가 없음. | `POST /api/skills/import` (§7.2) |
| **인라인 편집** | PUT/update 엔드포인트 없음. `create_skill`로 우회하면 frontmatter 파괴. | `PUT /api/skills/{name}/content` (§7.3) |

> ⚠️ **절대 금지**: 임포트나 편집을 `create_skill`로 라우팅하지 말 것. 검증(manager.rs:137-141) — `format!("---\nname: {name}\ndescription: {description}\n---\n\n{content}")`로 전체 파일을 재작성한다. 마켓플레이스 스킬의 rich frontmatter가 조용히 소거된다.

---

## 3. 설계 목표 (vs Claude)

| 목표 | Claude가 하는 일 | Oxios 설계 |
|---|---|---|
| **직접 등록** | claude.ai Settings > Features에서 커스텀 스킬 업로드 | **F1**: 풀스크린 `SkillEditorDialog` — name/description/body + live frontmatter 미리보기 |
| **임포트** | zip 파일 업로드 | **F2**: `ImportDialog` 3-모드 — 파일 업로드(.md/.zip/.skill) / 텍스트 붙여넣기 / URL 가져오기 |
| **열람** | (코드 실행 환경에서 progressive disclosure) | **F4**: `SkillDetail`에 Content 섹션 — 렌더링된 SKILL.md + "원문 보기" 토글 |
| **편집** | Claude Code(filesystem) / API 업로드 | **F3**: Content 섹션의 [편집] → 인라인 에디터 → `PUT /content`(frontmatter 보존) |
| **새로고침 버튼 제거** | 수동 버튼 없음, 자동 동기화 | **F5**: `RefreshButton` 제거 + mutation-driven `invalidateQueries` (+ 저비용 폴링 유지) |

**Claude의 progressive disclosure(3단계 로딩) 원칙을 UI에 반영**: 메타데이터(항상) → 본문(트리거 시) → 리소스(필요 시). 인스펙터는 기본적으로 메타데이터만 보여주고, 본문은 Content 섹션 전개 시 로드한다(`GET /content`를 클릭 시 fetch — 모든 스킬의 본문을 목록 로드 시 미리 가져오지 않음).

---

## 4. 정보 아키텍처 / 새 레이아웃

```
┌─────────────────────────────────────────────────────────────────────┐
│ Skills                                                                │
│ 스킬을 관리·작성·가져옵니다                       [12 · ✓9 · ⚠2 · ◯1] │  ← 상태 요약 pill (버튼 아님)
├─────────────────────────────────────────────────────────────────────┤
│                                                                       │
│  ┌─ Primary actions ─────────────────────────────────────────────┐  │
│  │  [＋ 스킬 만들기]   [↓ 가져오기 ▾]                              │  │  ← 항상 노출 헤드라인 액션
│  │                          ├─ 파일에서(.md/.zip/.skill)           │  │
│  │                          ├─ 텍스트 붙여넣기                      │  │
│  │                          └─ URL에서 가져오기                     │  │
│  └────────────────────────────────────────────────────────────────┘  │
│                                                                       │
│  [Installed (12)] [Marketplace]          🔍 검색   [all▼ ready⎵ …]   │  ← 탭 + 필터
│                                                                       │
│  ┌─ 스킬 카드 목록 ─────────────────┬─ Skill Inspector ──────────┐  │
│  │ ┌ Card ┐ ┌ Card ┐ ┌ Card ┐        │  [개요] [콘텐츠]            │  │
│  │ │ name │ │ name │ │ name │        │  ────────────────────────  │  │
│  │ └──────┘ └──────┘ └──────┘        │  status · format · 버전     │  │
│  │  ...(그리드/리스트 토글)           │  요구사항 · config 체크      │  │
│  │                                    │                            │  │
│  │                                    │  ▸ Content (펼치면 렌더링)  │  │
│  │                                    │    [원문] [편집] [복제]      │  │
│  │                                    │                            │  │
│  └────────────────────────────────────┴────────────────────────────┘  │
│                                                                       │
│  <SkillEditorDialog/>   <ImportDialog/>   (전역 오버레이)              │
└─────────────────────────────────────────────────────────────────────┘
```

**핵심 IA 결정**:
- **Primary actions를 헤더 바로 아래에 배치** — 저작·임포트는 '발견' 경로가 아니라 '헤드라인' 경로여야 한다. Claude처럼 항상 보이는 진입점.
- **RefreshButton 제거** — 상태 요약 pill(`12 · ✓9 · ⚠2 · ◯1`)로 대체. 클릭하면 필터로 동작(예: ⚠2 클릭 → needs_setup 필터). 정보 제공 + 인터랙션, 버튼은 아님.
- **Skill Inspector는 탭 2개**(개요 / 콘텐츠) — 개요는 기존 `SkillDetail` 내용, 콘텐츠는 본문 렌더링 + 편집 진입점.

---

## 5. 핵심 기능 설계

### F1. 직접 등록 — `SkillEditorDialog`

**목적**: 백엔드 `POST /api/skills`가 이미 있으므로 **UI 전용 작업**. 빈 스킬을 처음부터 작성.

**구조** (풀스크린 모달 / 드로어):
```
┌─ 새 스킬 만들기 ──────────────────────────────── [취소] ─┐
│                                                          │
│  이름 *        [_______________________]                 │  ← kebab-case 검증
│  설명 *        [___________________________________]      │  ← "언제 쓰는지" 포함 권장
│                (최대 1024자 · 스킬이 언제 트리거되는지)    │
│                                                          │
│  ┌─ 본문 에디터 ──────────┬─ 미리보기 ──────────────┐    │
│  │ # 제목                  │ <h1>제목</h1>            │    │  ← 분할 뷰
│  │                         │                          │    │
│  │ ## 지시사항             │ <h2>지시사항</h2>        │    │
│  │ ...                     │ ...                      │    │
│  └─────────────────────────┴──────────────────────────┘    │
│                                                          │
│  ▾ 생성될 SKILL.md 미리보기 (frontmatter 자동)            │
│    ---                                                   │
│    name: <입력값>                                         │
│    description: <입력값>                                  │
│    ---                                                   │
│                                                          │
│                                  [취소] [저장하고 활성화] │
└──────────────────────────────────────────────────────────┘
```

**검증 규칙** (Claude authoring 규칙 + Oxios frontmatter 준수):
- `name`: 소문자/숫자/하이픈만, 최대 64자, 빈 값 불가. 정규식 `^[a-z0-9][a-z0-9-]*$`.
- `description`: 비어있지 않음, 최대 1024자.
- `content`: 최대 64KB(`handle_skill_create`의 `MAX_SKILL_CONTENT`와 일치).
- 클라이언트 검증 후 `POST /api/skills`. 실패 시 `toast.error`.

**저장 후**: `qc.invalidateQueries(['skills'])` → 새 스킬이 목록 상단에 등장(정렬 주의 — 현재는 이름순). 모달 닫기.

> F1은 **create 전용**이므로 `create_skill`의 frontmatter 재합성(name+description)이 정확히 의도된 동작이다. 문제는 F3(편집)에서 발생한다.

### F2. `.skill` 파일 임포트 — `ImportDialog`

**목적**: **백엔드 델타 필요**(§7.2). 3가지 진입 모드.

**모드 A — 파일 업로드**:
- 드래그&드롭 존 + 파일 선택 버튼
- 허용: `.md`(단일 SKILL.md, frontmatter 포함 필수) / `.zip` / `.tar.gz` / `.skill`
- `.zip`/`.skill` → `extract_archive`로 압축 해제 → `find_skill_root`가 SKILL.md 마커 탐지 → `parse_skill` 검증
- 파싱 결과 미리보기(감지된 포맷/이름/요구사항) → 사용자 확인 → 설치

**모드 B — 텍스트 붙여넣기**:
- textarea — `parse_skill`로 즉시 검증, frontmatter 누락 시 에러 표시
- 검증 통과 시 `write_skill_raw`로 원문 그대로 저장

**모드 C — URL에서 가져오기**:
- URL 입력 → 백엔드가 fetch → `parse_skill` 검증 → 설치
- Claude Code의 `skill add <url>`와 동일한 패턴

**임포드 결과**: 출처 추적을 위해 `.imported/origin.json` 작성(provenance 패턴 재사용):
```json
{ "source": "file", "filename": "my-skill.skill", "importedAt": "...", "format": "claude_code" }
```

### F3. 인라인 편집 — Content 섹션 → 에디터 전환

**목적**: **백엔드 델타 필요**(§7.3). 기존 스킬의 본문을 편집하되 **frontmatter를 보존**.

**흐름**:
1. 인스펙터 콘텐츠 탭 → `GET /api/skills/{name}/content` 로드 → 렌더링
2. [편집] 클릭 → 에디터 모드 전환(textarea + 라이브 프리뷰)
3. 에디터는 **전체 원문**(frontmatter 포함)을 편집 — 사용자가 메타데이터까지 고칠 수 있도록
4. 저장 → `PUT /api/skills/{name}/content` → `write_skill_raw`(원문 그대로 저장 + `parse_skill`/`load_skill_entry`로 reindex)

**마켓플레이스 스킬 편집 경고**: 출처가 ClawHub/Skills.sh인 스킬을 편집하면 배너 표시:
> *"마켓플레이스 스킬을 편집했습니다. update-all 시 변경사항이 덮어씌워집니다."*

### F4. SKILL.md 콘텐츠 뷰어

**목적**: **UI 전용**(`GET /content` 이미 존재). `SkillDetail`에 Content 섹션 추가.

- 기본: 접힌 상태(`▸ Content`) — 클릭 시 fetch + 렌더링(lazy 로드, 목록 로드 시 미리 가져오지 않음)
- 렌더링: 마크다운 → HTML(oxios-markdown `html::markdown_to_html` 또는 프론트엔드 라이브러리)
- [원문] 토글: 렌더링 뷰 ↔ raw textarea(읽기 전용)
- [편집]: F3 에디터로 전환
- [복제]: 현재 스킬을 새 이름으로 복제(create 플로우 재사용)

### F5. 새로고침 버튼 제거

**목적**: **UI 패턴 변경만**. 백엔드 변경 없음.

**현재**(skills.tsx:131, 224-230): 30초 `refetchInterval` + 헤더 `RefreshButton`.
**변경**:
- `RefreshButton` 컴포넌트 제거 + import 정리
- 상태 요약 pill로 대체(정보 + 필터 인터랙션)
- 모든 뮤테이션(create/edit/import/toggle/delete/install)은 이미 `qc.invalidateQueries(['skills'])` 호출 — 이것이 1차 신선도 소스
- 폴링은 **유지하되** 간격을 30s → 60s로 완화(저비용 안전망; 외부 변경 — 예: CLI에서 스킬 추가 — 감지용). 설정 가능하게 하되 기본 켜짐.

---

## 6. 컴포넌트 구조

### 6.1 신규 컴포넌트

| 컴포넌트 | 파일 | 역할 |
|---|---|---|
| `SkillEditorDialog` | `components/skills/skill-editor-dialog.tsx` | 풀스크린 작성/편집 모달. name/description/body 에디터 + 라이브 프리뷰 + frontmatter 미리보기. create/edit 양쪽에서 재사용(mode prop). |
| `ImportDialog` | `components/skills/import-dialog.tsx` | 3-모드 임포트(파일/텍스트/URL). 드래그&드롭 존, 검증 미리보기, 출처 표시. |
| `SkillInspector` | `components/skills/skill-inspector.tsx` | `SkillDetail`을 대체/확장하는 탭 인스펙터. 개요 탭 + 콘텐츠 탭. |
| `SkillContent` | `components/skills/skill-content.tsx` | 콘텐츠 탭 본체. lazy fetch `/content` + 렌더링 + raw 토글 + 편집/복제 액션. |
| `SkillActionBar` | `components/skills/skill-action-bar.tsx` | 헤더 Primary actions(만들기/가져오기 드롭다운). |
| `SkillSummaryPill` | `components/skills/skill-summary-pill.tsx` | 상태 요약 pill(`12 · ✓9 · ⚠2 · ◯1`), 클릭 시 필터 연동. |

### 6.2 변경 컴포넌트

| 컴포넌트 | 변경 |
|---|---|
| `skills.tsx` (SkillsPage) | `RefreshButton` 제거 → `SkillActionBar` + `SkillSummaryPill`. Inspector 교체. 모달 상태 추가. |
| `skill-detail.tsx` | → `SkillInspector`로 흡수(개요 탭으로 재구성) 또는 그대로 두고 Inspector가 조합. |
| `SkillCard` | [편집] 액션 추가(카드 인라인 액션에 연필 아이콘 → 에디터 모달). |

### 6.3 제거

- `RefreshButton` import 및 헤더 사용(skills.tsx:26, 224-230)
- `refetch` 프로퍼티 전달(더 이상 수동 refetch 버튼 없음)

---

## 7. 백엔드 델타 (구현 명세)

> 작은 델타. 재사용 가능한 조각이 이미 검증됐으므로(§2.1) 새 코드를 짜지 말고 기존 함수에 연결한다.

### 7.1 `SkillManager::write_skill_raw(name, raw)` — 신규 메서드

**목적**: frontmatter를 보존하며 원문을 그대로 저장. **F2(임포트)와 F3(편집) 양쪽에 서비스**.

```rust
// crates/oxios-kernel/src/skill/manager.rs
/// 스킬 원문(SKILL.md 전체, frontmatter 포함)을 그대로 저장하고 재인덱스.
/// create_skill과 달리 frontmatter를 재합성하지 않는다 → rich 메타데이터 보존.
pub async fn write_skill_raw(&self, name: &str, raw: &str) -> Result<SkillEntry> {
    let dir = self.skills_dir.join(name);
    tokio::fs::create_dir_all(&dir).await?;
    let skill_file = dir.join("SKILL.md");
    tokio::fs::write(&skill_file, raw).await?;
    // parse_skill(load_skill_entry 내부)가 포맷 감지 + 요구사항 체크 수행
    let entry = Self::load_skill_entry(&skill_file, false)?;
    self.installed.write().await.insert(name.to_string(), entry.clone());
    Ok(entry)
}
```

**확장 API 래퍼**: `ExtensionApi::write_skill_raw`(kernel_handle/extension_api.rs) 추가. 기존 `create_skill`(extension_api.rs:62) 옆.

**왜 새 메서드인가**: `create_skill`은 `format!("---\nname:..description:..---\n\n{content}")`로 재합성(manager.rs:137-141) → 임포트된 풍부한 frontmatter(`requires`/`install`/`allowed-tools`/`autonomous`/`primaryEnv`) 소거. `write_skill_raw`는 `parse_skill`이 감지한 원문을 보존.

### 7.2 `POST /api/skills/import` — 신규 엔드포인트

**목적**: multipart 파일 임포트(.md / .zip / .tar.gz / .skill).

**핸들러**(src/api/routes/workspace.rs):
```rust
// POST /api/skills/import — multipart(.md | .zip | .skill)
pub(crate) async fn handle_skill_import(
    state: State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, AppError> {
    // 1. multipart에서 파일 추출(최대 크기 제한 — 예: 16MB)
    // 2. 확장자 분기:
    //    .md  → raw = file_content; parse_skill 검증
    //    .zip/.skill → extract_archive(재사용) → SKILL.md 찾기 → raw 읽기
    //    .tar.gz    → tar 해제(신규 헬퍼, extract_archive 패턴 모방)
    // 3. parse_skill(raw, dir) 검증 → 실패 시 400 + 에러 메시지
    // 4. name = parsed.name 또는 파일명(kebab-case 정규화)
    // 5. write_skill_raw(name, raw)
    // 6. .imported/origin.json 작성(출처 추적)
    // 7. 응답: { status, name, format, requirements }
}
```

**보안**:
- `is_safe_relative_path`로 Zip Slip 방어(extract_archive가 이미 수행)
- 최대 파일 크기 제한(요청 레벨 + 핸들러 레벨)
- `parse_skill` 검증 실패 시 설치 거부

**라우트 등록**(mod.rs, §323 Skills 블록에 추가):
```rust
.route("/api/skills/import", post(handle_skill_import))
```

### 7.3 `PUT /api/skills/{name}/content` — 신규 엔드포인트

**목적**: 기존 스킬 본문 편집(frontmatter 보존).

```rust
// PUT /api/skills/{name}/content — 본문 편집(원문 교체, frontmatter 보존)
pub(crate) async fn handle_skill_content_update(
    state: State<Arc<AppState>>,
    Path(name): Path<String>,
    Json(body): Json<SkillContentUpdate>,  // { content: String }
) -> Result<Json<serde_json::Value>, AppError> {
    // 1. 스킬 존재 확인
    // 2. 크기 검증(64KB)
    // 3. parse_skill 사전 검증(유효하지 않으면 400)
    // 4. write_skill_raw(name, content)
    // 5. 응답: { status: "updated", name, format, requirements }
}
```

**라우트 등록**:
```rust
.route("/api/skills/{name}/content", put(handle_skill_content_update))
```

> `GET /api/skills/{name}/content`(workspace.rs:508)와 짝을 이룸. GET은 이미 존재하므로 PUT만 추가.

### 7.4 (선택) `POST /api/skills/import-url` — URL 임포트

**목적**: F2 모드 C. 백엔드에서 URL fetch → `write_skill_raw`.
- 보안: 허용 스킴(http/https) + 타임아웃 + 크기 제한 + SSRF 방지(사내망 차단 옵션)
- Claude Code의 `skill add <url>` 패턴

---

## 8. 데이터 흐름 / 상태

### 8.1 React Query 키

| 키 | 용도 | 무효화 시점 |
|---|---|---|
| `['skills']` | 목록 | create/edit/import/toggle/delete/install 후 |
| `['skill', name, 'content']` | 단일 스킬 본문 | edit 성공 후; lazy fetch(인스펙터 콘텐츠 탭 전개 시) |

### 8.2 뮤테이션 (신규)

```ts
// create (F1) — POST /api/skills (기존)
// edit   (F3) — PUT  /api/skills/{name}/content (신규)
// import (F2) — POST /api/skills/import (multipart, 신규)
//   또는 import-url — POST /api/skills/import-url { url } (선택)
```

모든 뮤테이션 `onSuccess` → `qc.invalidateQueries({ queryKey: ['skills'] })` + 해당 content 쿼리 무효화 + `toast.success`.

### 8.3 모달 상태 (SkillsPage)

```ts
const [editorState, setEditorState] = useState<
  | { mode: 'create' }
  | { mode: 'edit'; skill: Skill }
  | null
>(null)
const [importOpen, setImportOpen] = useState(false)
```

---

## 9. i18n 키 (skills.* 네임스페이스 추가)

> 기존 키는 변경 없음. ko/en 동기화(AGENTS.md: 웹 UI는 이중언어).

| 키 | en | ko | 비고 |
|---|---|---|---|
| `create` | `Create Skill` | `스킬 만들기` | Primary action |
| `createTitle` | `New Skill` | `새 스킬` | 에디터 모달 제목 |
| `editTitle` | `Edit {{name}}` | `{{name}} 편집` | 에디터 모달(편집) |
| `import` | `Import` | `가져오기` | Primary action |
| `importFromFile` | `From File` | `파일에서` | 드롭다운 |
| `importFromText` | `Paste Text` | `텍스트 붙여넣기` | 드롭다운 |
| `importFromUrl` | `From URL` | `URL에서 가져오기` | 드롭다운 |
| `importDropzone` | `Drop .md / .zip / .skill here` | `.md / .zip / .skill 파일을 놓으세요` | 드래그존 |
| `nameLabel` | `Name` | `이름` | 에디터 |
| `nameHint` | `lowercase, numbers, hyphens` | `소문자·숫자·하이픈` | |
| `descriptionHint` | `What it does and when to use it` | `무엇을, 언제 쓰는지` | Claude 규칙 반영 |
| `bodyLabel` | `Instructions` | `지시문` | |
| `frontmatterPreview` | `Generated SKILL.md preview` | `생성될 SKILL.md 미리보기` | |
| `content` | `Content` | `콘텐츠` | 인스펙터 탭 |
| `overview` | `Overview` | `개요` | 인스펙터 탭 |
| `viewRaw` | `View raw` | `원문 보기` | 콘텐츠 토글 |
| `edit` | `Edit` | `편집` | 액션 |
| `duplicate` | `Duplicate` | `복제` | 액션 |
| `editMarketplaceWarning` | `Editing a marketplace skill. Changes will be overwritten on update-all.` | `마켓플레이스 스킬을 편집했습니다. update-all 시 덮어씌워집니다.` | 경고 배너 |
| `importSuccess` | `Imported "{{name}}" successfully.` | `"{{name}}" 가져오기 성공.` | |
| `importFailed` | `Import failed.` | `가져오기 실패.` | |
| `saveSuccess` | `Skill saved.` | `저장됨.` | |
| `summaryReady` | `{{count}} ready` | `{{count}} 준비` | 요약 pill |
| `summaryNeedsSetup` | `{{count}} need setup` | `{{count}} 설정 필요` | |
| `summaryDisabled` | `{{count}} disabled` | `{{count}} 비활성` | |

---

## 10. 검증 계획

### 10.1 백엔드 (cargo)
- `write_skill_raw` 단위 테스트: rich frontmatter 보존 확인(requires/install/allowed-tools 포함 스킬 저장 → 재로드 후 메타데이터 유지). `create_skill`과의 회귀 비교.
- `handle_skill_import` 테스트: .md / .zip / .skill 각 케이스; Zip Slip 악의 경로 거부; parse 실패 시 400.
- `handle_skill_content_update` 테스트: 존재하지 않는 스킬 404; 64KB 초과 413; frontmatter 보존.
- CI: `cargo fmt && clippy -D warnings && cargo test --workspace`

### 10.2 프론트엔드 (bun)
- `tsc` 타입체크
- `biome` 린트
- 단위: 모달 검증(name 정규식, description 길이, content 크기)
- e2e(`e2e/skills.spec.ts`):
  - 스킬 만들기 → 목록 등장
  - .md 파일 임포트 → 파싱된 미리보기 → 설치
  - 콘텐츠 탭 전개 → 렌더링 확인
  - 편집 → 저장 → 본문 갱신
  - 새로고침 버튼 부재 확인
  - 마켓플레이스 스킬 편집 경고 배너

### 10.3 회귀 주의점
- `create_skill`는 **F1(신규 생성)에서만** 사용 — 여기서는 name+description 재합성이 의도됨.
- F2/F3는 반드시 `write_skill_raw` — frontmatter 소거 회귀 방지.
- 출처(provenance): ClawHub/Skills.sh 출처 스킬을 편집하면 origin.json은 유지되되 경고 배너로 사용자 인지.

---

## 11. 구현 순서 (제안)

1. **백엔드**: `write_skill_raw` → `PUT /content` → `POST /import` → (선택) `import-url`. 각 단위 테스트.
2. **프론트엔드 F5**(가장 저위험): RefreshButton 제거 + 요약 pill. 폴링 간격 조정.
3. **프론트엔드 F4**: 콘텐츠 뷰어(GET /content 연결, UI 전용).
4. **프론트엔드 F1**: SkillEditorDialog(create).
5. **프론트엔드 F3**: 편집 모드(PUT /content 연결).
6. **프론트엔드 F2**: ImportDialog(POST /import 연결).
7. i18n 키 ko/en 추가, e2e 작성, 최종 build.

---

## 12. 한계 / 후속

| 항목 | 비고 |
|---|---|
| **URL 임포트 SSRF** | §7.4 — 사내망 차단, 스킴 화이트리스트 필요. v1에서는 파일/텍스트 모드만 ship해도 됨. |
| **충돌 해결** | 동일 이름 스킬 임포트 시 덮어쓰기 정책(확인 다이얼로그 vs 거부). ClawHub installer는 "already installed" bail — 임포트는 사용자 파일이므로 덮어쓰기 허용 권장. |
| **버전 관리** | 편집 이력/스냅샷은 v1 범위 외. 출처(origin.json)로 충분. |
| **리소스 파일 편집** | SKILL.md 외 스크립트/리소스 편집은 v1 외. 본문(text) 편집에 집중. |
