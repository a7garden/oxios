# RFC-007: files.md 잔여 포팅 계획서

> **목표**: files.md의 모든 기능을 oxios-markdown + oxios-kernel에 포팅 완료
> **기준**: `/Volumes/MERCURY/PROJECTS/files.md/server/` (Go 20,622줄)
> **현재**: oxios-markdown 2,922줄, 핵심 로직 약 62% 포팅

---

## 현재 상태 요약

### ✅ 완료 (13개 모듈, 2,922줄)

| 모듈 | 줄 수 | 비고 |
|------|-------|------|
| `fs.rs` | 754 | VirtualFs + POSIX path API. Go `fs/` 전체 커버 |
| `types.rs` | 367 | KnowledgeConfig, Schedule, FileEntry 등 |
| `backlinks.rs` | 277 | 양방향 링크 인덱스 (files.md에 없던 신규) |
| `merge.rs` | 176 | LCS 3-way merge. Go `sync/merge.go` 커버 |
| `sync.rs` | 255 | 파일 동기화 엔진. Go `sync/sync.go` 커버 |
| `fslog.rs` | 113 | 동기화 로그. Go `sync/fslog.go` 커버 |
| `parser.rs` | 164 | 텍스트 처리 (일부) |
| `chat.rs` | 149 | Chat.md 블록 파싱/조작 |
| `journal.rs` | 130 | 저널 타임스탬프 기록 |
| `habits.rs` | 157 | 습관 데이터 읽기 (일부) |
| `schedule.rs` | 164 | 스케줄 관리 (일부) |
| `tokens.rs` | 149 | 인증 토큰 관리 |
| `lib.rs` | 67 | 모듈 등록 + re-export |

### ⚠️ 부분 포팅 (함수 누락)

| 모듈 | 누락 함수 | 중요도 |
|------|-----------|--------|
| `parser.rs` | 체크리스트 7개, MarkdownToHTML, 문자열 유틸 8개 | 🔴 높음 |
| `chat.rs` | `appendToChatMsg`, `moveFromChat` | 🟡 중간 |
| `habits.rs` | `LastWeekHabits`, `Write`, `Render` | 🟡 중간 |
| `schedule.rs` | userconfig CRUD (시간대, 모드, 퀵 커맨드 등) | 🟡 중간 |
| `sync.rs` | 미디어 동기화 (`sync_media.go`) | 🟢 낮음 |

### ❌ 미포팅

| 모듈 | Go 줄 수 | 중요도 | 포팅 필요 |
|------|----------|--------|-----------|
| `pkg/txt/md.go` 체크리스트 | ~200 | 🔴 | **필수** — Later/Done/Shop/Watch/Read 전부 체크리스트 기반 |
| `pkg/txt/md.go` MarkdownToHTML | ~150 | 🔴 | **필수** — Telegram/에이전트 메시지 포맷팅 |
| `pkg/txt/str.go` 문자열 유틸 | ~170 | 🟡 | 유틸 함수 — 체크리스트/MarkdownToHTML에서 사용 |
| `pkg/txt/tgtxt.go` | ~186 | 🟢 | Telegram 전용 — Oxios 채널에서만 필요 시 |
| `habits_render.go` | 50 | 🟢 | HTML 템플릿 렌더링 — PNG 습관 차트 |
| `worker.go` | 319 | 🔴 | **필수** — 매일 자정 완료 항목 정리 + 스케줄 실행 |
| `stats/stats.go` | 82 | 🟡 | 오늘 완료 작업 리포트 |
| `plugins/world_clock.go` | 148 | 🟢 | 세계 시계 |
| `i18n/` | 103 | 🟡 | 이모지 자동 매핑 + UI 문자열 상수 |
| `userconfig/` | 471 | 🟡 | `schedule.rs`에 일부 있음, 나머지는 Oxios 기존 Config로 |
| `bot.go` | 3,322 | ⚪ | Telegram 봇 — Oxios 게이트웨이가 대체 |
| `pkg/tg/` | 930 | ⚪ | Telegram API — Oxios 채널이 대체 |
| `db/` | 234 | ⚪ | PostgreSQL — Oxios StateStore가 대체 |
| `sync/webserver.go` | 302 | ⚪ | HTTP 서버 — Oxios Axum이 대체 |
| `sync/autocert.go` | 39 | ⚪ | Let's Encrypt — 불필요 |

---

## 포팅 트랙

### Track 1: 체크리스트 엔진 (🔴 필수)

**Go 원본**: `pkg/txt/md.go` 95~261줄, `pkg/txt/str.go`
**Rust 대상**: `crates/oxios-markdown/src/parser.rs` 또는 신규 `checklist.rs`

files.md의 Later.md, Done.md, Shop.md, Watch.md, Read.md는 **전부 체크리스트**다.
`- [ ] Task` / `- [x] Done` 형식의 항목을 파싱/추가/완료/삭제하는 함수들이 없으면
이 파일들을 프로그래밍적으로 조작할 수 없다.

#### 포팅할 함수 (Go → Rust)

| Go 함수 | Rust 이름 | 설명 |
|---------|-----------|------|
| `ChecklistItems(md)` | `checklist_items(md)` | 체크리스트 파싱 → (항목들, 완료여부 맵) |
| `IncompleteChecklistItems(md)` | `incomplete_checklist_items(md)` | 미완료 항목만 |
| `AddChecklistItem(md, item, checked)` | `add_checklist_item(md, item, checked)` | 항목 추가 |
| `CompleteChecklistItem(md, itemHash)` | `complete_checklist_item(md, item_hash)` | 항목 완료 체크 |
| `RemoveChecklistItem(md, itemOrHash)` | `remove_checklist_item(md, item_or_hash)` | 항목 삭제 |
| `RemoveCompletedChecklistItems(md)` | `remove_completed_checklist_items(md)` | 완료 항목 전체 삭제 → (남은 md, 삭제된 md) |
| `ChecklistItem(md, itemOrHash)` | `checklist_item(md, item_or_hash)` | 단일 항목 조회 |
| `AddHeaderAndText(content, header, text)` | `add_header_and_text(content, header, text)` | 헤더 아래에 텍스트 삽입 |

#### 의존성
- `hash_filename()` — 이미 `fs.rs`에 있음
- `norm_new_lines()` — 이미 `parser.rs`에 있음

#### 예상 줄 수: ~200줄
#### KnowledgeApi 노출:
```rust
pub fn checklist_items(&self, path: &str) -> Result<Vec<(String, bool)>>
pub fn checklist_add(&self, path: &str, item: &str, checked: bool) -> Result<String>
pub fn checklist_complete(&self, path: &str, item_hash: &str) -> Result<String>
pub fn checklist_remove(&self, path: &str, item_or_hash: &str) -> Result<String>
pub fn checklist_remove_completed(&self, path: &str) -> Result<(String, String)>
```

---

### Track 2: Markdown → HTML 변환기 (🔴 필수)

**Go 원본**: `pkg/txt/md.go` 262~432줄, `pkg/txt/str.go` 122~170줄
**Rust 대상**: `crates/oxios-markdown/src/parser.rs` 또는 신규 `html.rs`

Telegram/에이전트 메시지에서 마크다운을 HTML로 변환.
files.md는 파서 콤비네이터(parser combinators)로 구현했다.
Rust에서는 `pulldown-cmark` 크레이트를 사용하면 더 견고하지만,
files.md는 Telegram이 지원하는 HTML 태그만 출력하므로
단순 regex 접근도 가능하다.

#### 포팅할 함수

| Go 함수 | Rust 이름 | 설명 |
|---------|-----------|------|
| `MarkdownToHTML(md)` | `markdown_to_html(md)` | md → Telegram HTML |
| `EscapeHTML(str)` | `escape_html(str)` | &, <, > 이스케이프 |
| `StripHTMLTags(str)` | `strip_html_tags(str)` | HTML 태그 제거 |
| `ReplaceWithPlaceholders(str, regex, placeholder)` | `replace_with_placeholders(str, pattern, placeholder)` | 정규식 매치를 플레이스홀더로 교체 |
| `RestoreFromPlaceholders(str, placeholders)` | `restore_from_placeholders(str, placeholders)` | 플레이스홀더 복원 |

#### 설계 결정 필요
- **옵션 A**: files.md의 파서 콤비네이터를 그대로 포팅 (순수 Rust, 의존성 없음)
- **옵션 B**: `pulldown-cmark` 사용 (견고하지만 Telegram HTML 태그 제한 처리 필요)
- **옵션 C**: regex 기반 단순 변환 (코드는 짧지만 엣지 케이스 가능)

**추천**: 옵션 A — files.md 원본을 충실히 포팅. Telegram 출력이 목적이므로
풀 마크다운 파서는 오버엔지니어링.

#### 예상 줄 수: ~180줄 (파서 콤비네이터 포함)
#### KnowledgeApi 노출:
```rust
pub fn markdown_to_html(&self, md: &str) -> String
```

---

### Track 3: 문자열 유틸 (🟡 필요)

**Go 원본**: `pkg/txt/str.go` 172줄
**Rust 대상**: `crates/oxios-markdown/src/parser.rs`

#### 포팅할 함수

| Go 함수 | Rust 이름 | 설명 |
|---------|-----------|------|
| `SplitTextIntoChunks(text, maxLen)` | `split_text_into_chunks(text, max_len)` | 텍스트 청크 분할 (Telegram 메시지 길이 제한) |
| `IsMultiline(text)` | `is_multiline(text)` | 멀티라인 여부 |
| `Ucfirst(str)` | `ucfirst(str)` | 첫 글자 대문자 |
| `Lcfirst(str)` | `lcfirst(str)` | 첫 글자 소문자 |
| `Substr(input, start, length)` | `substr(input, start, length)` | 유니코드 안전 서브스트링 |
| `Emoji(emoji, str)` | `emoji_prefix(emoji, str)` | 이모지 + 문자열 (접두어 제거 포함) |

이미 있는 것:
- `norm_new_lines()` ✅
- `first_word()` ✅
- `similar()` / `levenshtein()` ✅ (Go `similarity.go`)

#### 예상 줄 수: ~80줄
#### KnowledgeApi 노출: 불필요 (라이브러리 내부에서만 사용)

---

### Track 4: Chat 보완 (🟡)

**Go 원본**: `server/chat.go` 의 `appendToChatMsg()`
**Rust 대상**: `crates/oxios-markdown/src/chat.rs`

#### 누락 함수

| Go 함수 | Rust 이름 | 설명 |
|---------|-----------|------|
| `appendToChatMsg(content, msgHash, newText)` | `append_to_chat_msg(content, msg_hash, new_text)` | 기존 메시지에 텍스트 추가 |
| `moveFromChat()` | `move_from_chat()` | Chat → 다른 파일로 항목 이동 |

`moveFromChat`은 `Bot`에 의존하므로, 핵심 로직만 추출:
- Chat.md에서 항목을 해시로 찾아서 제거
- 대상 파일(Later.md 등)에 체크리스트 항목으로 추가
- → 체크리스트 엔진(Track 1) 완료 후 구현 가능

#### 예상 줄 수: ~60줄
#### KnowledgeApi 노출:
```rust
pub fn chat_move_to(&self, msg_hash: &str, target_path: &str) -> Result<bool>
```

---

### Track 5: Habits 보완 (🟡)

**Go 원본**: `server/habits/habits.go` 의 `LastWeekHabits()`, `Write()`
**Rust 대상**: `crates/oxios-markdown/src/habits.rs`

#### 누락 함수

| Go 함수 | Rust 이름 | 설명 |
|---------|-----------|------|
| `LastWeekHabits(fs, tz)` | `last_week_habits(fs, tz)` | 최근 주 습관 데이터 조회 |
| `Write(fs, year, habits)` | `write_habits(fs, year, habits)` | 습관 데이터를 insights/ 파일에 쓰기 |

`habits_render.go`는 HTML 템플릿으로 PNG 이미지를 생성하는데,
Oxios에서는 웹 UI가 직접 렌더링하므로 포팅 불필요.
대신 JSON API로 습관 데이터를 제공하면 충분.

#### 예상 줄 수: ~120줄
#### KnowledgeApi 노출:
```rust
pub fn habits_last_week(&self) -> Result<Habits>
pub fn habits_write(&self, year: i32, habits: &Habits) -> Result<()>
```

---

### Track 6: Worker — 매일 자정 작업 (🔴 필수)

**Go 원본**: `server/worker.go` 319줄
**Rust 대상**: 신규 `crates/oxios-markdown/src/worker.rs` 또는 `crates/oxios-kernel`에 배치

files.md의 worker는 매일 밤 23:50에 실행되어:

1. **완료 항목 정리** (`RemoveCompletedChecklistItems`):
   - Chat.md, Later.md에서 `- [x]` 항목을 제거
   - Done.md에 아카이브
   - 저널에 ✅ 기록 추가

2. **스케줄 실행** (`MoveDueTasks`):
   - config.json에서 `schedules` 읽기
   - `scheduled_at`이 지난 항목을 Chat.md에 추가
   - cron 표현식이 있으면 다음 실행 시간 계산
   - 1회성이면 스케줄에서 삭제

#### 포팅 전략
- 핵심 로직은 `oxios-markdown/src/worker.rs`에 **순수 함수**로 포팅
- Oxios의 `CronScheduler`가 호출하는 형태로 통합
- Bot/Telegram 의존성은 제거, "Chat에 추가"는 `KnowledgeApi::chat_append()`로 대체

#### 포팅할 함수

| Go 함수 | Rust 이름 | 설명 |
|---------|-----------|------|
| `RemoveCompletedChecklistItems(...)` | `remove_completed_items(fs, config)` | 완료 항목 정리 + 아카이브 |
| `removeCompletedInboxEntries(md)` | `remove_completed_inbox_entries(md)` | Chat 블록 단위 정리 |
| `MoveDueTasks(...)` | `move_due_tasks(fs, config)` | 스케줄된 작업을 Chat에 추가 |
| `BeginningOfTheDay(t)` | 이미 `schedule.rs`에 `beginning_of_day()` | ✅ |
| `Tomorrow()` | 이미 `schedule.rs`에 `tomorrow_timestamp()` | ✅ |
| `NextExcludeToday(cron)` | `next_exclude_today(cron_expr)` | cron 다음 실행 시간 |
| `ScheduleReport(tasks)` | `schedule_report(tasks)` | 스케줄 리포트 포맷팅 |
| `formatTaskDate(ts)` | 이미 `schedule.rs`에 `format_schedule_date()` | ✅ |

#### 의존성
- 체크리스트 엔진 (Track 1) — 완료 항목 제거/아카이브
- `chat.rs` — Chat 메시지 추가
- `journal.rs` — 저널 기록
- `schedule.rs` — 스케줄 조회/수정
- cron 파싱 — `cron` 또는 `safer_ffi` 크레이트

#### 예상 줄 수: ~200줄
#### KnowledgeApi 노출:
```rust
pub fn run_nightly_cleanup(&self) -> Result<NightlyReport>
pub fn run_scheduled_tasks(&self) -> Result<Vec<String>>
```

---

### Track 7: 통계 리포트 (🟡)

**Go 원본**: `server/stats/stats.go` 82줄
**Rust 대상**: `crates/oxios-markdown/src/stats.rs` (신규)

오늘 완료한 작업 목록 조회 + 전체 완료 수.
Go 원본은 DB에 의존하지만 실제로는 `userFS.FilesAndDirs(DirArchive)` 만 사용.

#### 포팅할 함수

| Go 함수 | Rust 이름 | 설명 |
|---------|-----------|------|
| `TodayReport(fs)` | `today_report(fs)` | 오늘 완료 작업 리포트 |
| `DoneToday(fs)` | `done_today(fs)` | 오늘 완료한 파일 목록 |

#### 예상 줄 수: ~60줄
#### KnowledgeApi 노출:
```rust
pub fn today_report(&self) -> Result<TodayReport>
```

---

### Track 8: i18n — 이모지 자동 매핑 (🟡)

**Go 원본**: `server/i18n/emoji.go` 63줄 + `emojis.json` 임베드
**Rust 대상**: `crates/oxios-markdown/src/i18n.rs` (신규)

파일명에서 키워드를 추출해서 이모지를 자동 매핑.
예: "Exercise" → 💪, "Reading" → 📚

#### 포팅할 함수

| Go 함수 | Rust 이름 | 설명 |
|---------|-----------|------|
| `LoadEmojiFile()` | `load_emojis()` | emojis.json 로드 |
| `Emoji(str)` | `emoji_for(str)` | 키워드 → 이모지 매핑 |
| `AddEmoji(str)` | `add_emoji(str)` | 문자열 앞에 이모지 추가 |

#### 설계
- `emojis.json`을 `include_str!()`로 임베드
- `HashMap<String, String>` 빌드
- 복수형(s), 단수형 자동 매칭

#### 예상 줄 수: ~70줄 + JSON 데이터
#### KnowledgeApi 노출:
```rust
pub fn auto_emoji(&self, text: &str) -> String
```

---

### Track 9: 미디어 동기화 (🟢)

**Go 원본**: `server/sync/sync_media.go` 152줄
**Rust 대상**: `crates/oxios-markdown/src/sync.rs`에 추가

이미지/미디어 파일의 동기화. base64 인코딩으로 전송.
현재 REST API에 `POST /api/knowledge/media` 엔드포인트가 없으므로
라우트도 함께 추가 필요.

#### 포팅할 함수

| Go 함수 | Rust 이름 | 설명 |
|---------|-----------|------|
| `SyncMediaFilenames(...)` | `sync_media_filenames(fs, timestamp)` | 변경된 미디어 목록 조회 |
| `SyncMediaFile(...)` | `sync_media_file(fs, filename, data)` | 미디어 업로드/다운로드 |

#### 예상 줄 수: ~100줄
#### KnowledgeApi 노출:
```rust
pub fn media_list(&self, since_timestamp: i64) -> Result<Vec<MediaEntry>>
pub fn media_upload(&self, filename: &str, data: &[u8]) -> Result<()>
pub fn media_read(&self, filename: &str) -> Result<Vec<u8>>
```

---

### Track 10: tgtxt — Telegram 텍스트 처리 (🟢)

**Go 원본**: `pkg/txt/tgtxt.go` 186줄
**Rust 대상**: `crates/oxios-markdown/src/tgtxt.rs` (신규) 또는 `parser.rs`에 추가

Telegram 엔티티를 마크다운으로 변환, 텍스트에서 이미지/링크 추출.
Oxios Telegram 채널에서만 필요.

#### 포팅할 함수

| Go 함수 | Rust 이름 | 설명 |
|---------|-----------|------|
| `TelegramEntitiesToMarkdown(text, entities)` | `tg_entities_to_md(text, entities)` | TG 엔티티 → 마크다운 |
| `ExtractTextImgsLinks(text)` | `extract_text_imgs_links(text)` | 텍스트에서 이미지/링크 분리 |
| `HasImage(msg)` | `has_image(msg)` | 이미지 포함 여부 (이미 parser.rs에 있음 ✅) |

Telegram 엔티티 타입 정의가 필요.
Go는 `tgbotapi.MessageEntity`를 사용.
Rust에서는 Oxios의 Telegram 채널 타입에 맞춤.

#### 예상 줄 수: ~150줄

---

### Track 11: 세계 시계 플러그인 (🟢)

**Go 원본**: `server/plugins/world_clock.go` 148줄
**Rust 대상**: `crates/oxios-markdown/src/plugins.rs` (신규)

사용자가 등록한 시간대의 현재 시간을 리포트.
Oxios에서는 에이전트 도구로 구현하는 것이 적합.

#### 포팅할 함수

| Go 함수 | Rust 이름 | 설명 |
|---------|-----------|------|
| `HandleTimezone(...)` | `world_clock_report(timezones)` | 시간대별 현재 시간 |

#### 예상 준수: ~80줄
#### KnowledgeApi 노출: 불필요 (에이전트 도구로 직접)

---

## 실행 순서

```
Phase 1 (핵심 — 체크리스트 없으면 아무것도 안 됨)
├── Track 1: 체크리스트 엔진        ~200줄  parser.rs 또는 checklist.rs
├── Track 3: 문자열 유틸             ~80줄  parser.rs
└── KnowledgeApi: checklist_* 메서드  ~40줄

Phase 2 (메시지 + 변환)
├── Track 2: MarkdownToHTML          ~180줄  parser.rs 또는 html.rs
├── Track 4: Chat 보완                ~60줄  chat.rs
└── KnowledgeApi: chat_move_to       ~20줄

Phase 3 (습관 + 스케줄 작업자)
├── Track 5: Habits 보완             ~120줄  habits.rs
├── Track 6: Worker                  ~200줄  worker.rs (신규)
└── KnowledgeApi: run_nightly 등      ~40줄

Phase 4 (나머지)
├── Track 7: Stats                    ~60줄  stats.rs (신규)
├── Track 8: i18n                     ~70줄  i18n.rs (신규)
├── Track 9: 미디어 동기화           ~100줄  sync.rs에 추가
├── Track 10: tgtxt                  ~150줄  tgtxt.rs (신규)
└── Track 11: 세계 시계               ~80줄  plugins.rs (신규)
```

---

## 포팅 제외 (Oxios가 이미 대체)

| 모듈 | Go 줄 수 | Oxios 대체 |
|------|----------|-----------|
| `bot.go` + `bot_forwards.go` + `bot_settings.go` | 3,322 | `oxios-telegram` 채널 + 에이전트 |
| `pkg/tg/` | 930 | `oxios-telegram` 채널 |
| `db/` | 234 | `StateStore` |
| `sync/webserver.go` | 302 | `knowledge_routes.rs` (Axum) |
| `sync/autocert.go` | 39 | Oxios에서 불필요 |
| `config/config.go` | 61 | `oxios` config.toml |
| **제외 합계** | **4,888** | |

---

## 최종 목표

```
files.md server/ 전체:                  20,622줄 (Go)
제외 (Oxios 대체):                      -4,888줄
포팅 대상:                              15,734줄 (Go)

목표 oxios-markdown:
  현재:                                  2,922줄 (Rust)
  Track 1~11 추가 예상:                 +1,280줄 (Rust)
  최종:                                  ~4,200줄 (Rust)

실질 커버리지 목표: 90%+ (핵심 로직 기준)
```

---

## 추정 공수

| Phase | 트랙 | 예상 시간 | 난이도 |
|-------|-------|-----------|--------|
| Phase 1 | Track 1, 3 | 1~2시간 | 중 — Go→Rust 직역 |
| Phase 2 | Track 2, 4 | 1~2시간 | 중 — 파서 콤비네이터 포팅 |
| Phase 3 | Track 5, 6 | 2~3시간 | 높음 — worker는 여러 모듈 연동 |
| Phase 4 | Track 7~11 | 2~3시간 | 낮음~중 |
| **합계** | | **6~10시간** | |
