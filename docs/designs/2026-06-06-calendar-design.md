# Calendar — Oxios 자체 시간 관리

> **날짜:** 2026-06-06
> **상태:** Draft
> **이전 문서:** `2026-06-06-calendar-design.md` (40KB — 과도하게 복잡했음. 본 문서로 대체)
> **핵심:** 일정 = .ics 파일. 에이전트 도구는 구조화된 파라미터. journal 자동 연동. cron 시각화. 단일 캘린더.

---

## 0. 한 줄

```
Oxios가 자체 캘린더를 가진다.
에이전트가 일정을 만들고, journal에 자동으로 표시되고, cron 작업이 보이고, 나중에 iCloud/Google과 동기화된다.
```

---

## 1. 목표

| # | 목표 |
|---|------|
| G1 | 에이전트가 일정을 **생성/수정/삭제** (구조화된 파라미터 → 우리가 .ics 생성) |
| G2 | 사용자가 Web/CLI에서 **수동 일정 등록** |
| G3 | 모든 일정이 **.ics 파일**로 저장 (RFC 5545) |
| G4 | **Journal 자동 연동** — 매일 journal에 "오늘의 일정" 섹션 주입 |
| G5 | **CronScheduler 작업을 시스템 캘린더로 시각화** |
| G6 | **풀 캘린더 UI** (월/주/일/agenda + cron 오버레이) |
| G7 | **충돌 경고** (겹치면 경고, 생성은 가능) |
| G8 | **알림** (.ics VALARM → Web/Telegram/푸시 채널) |
| G9 | **단일 캘린더** (v1). 다중 캘린더, iCloud/Google 동기화는 후속 |
| G10 | 과거 1년 + 미래 1년 롤링 보존 |

---

## 2. 저장

```
~/.oxios/workspace/calendar/
├── events/
│   ├── 2026-06-07_1on1-jane.ics
│   ├── 2026-06-07_dentist.ics
│   ├── 2026-06-09_team-sync.ics       # 반복: FREQ=WEEKLY;BYDAY=TU
│   └── 2026-06-10_lunch-mike.ics
├── index.json                          # 빠른 조회용 인덱스 (uid → filename, dtstart, dtend, summary)
└── archive/                            # 1년 넘은 .ics 파일 자동 이동
```

### 2.1 .ics 파일 예시

```ics
BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//Oxios//Calendar 1.0//EN
BEGIN:VEVENT
UID:f7c5a0b2-e8d1-4b3a-9c4d-123456789012@oxios
DTSTAMP:20260606T140000Z
DTSTART;TZID=Asia/Seoul:20260607T100000
DTEND;TZID=Asia/Seoul:20260607T110000
SUMMARY:1on1 with Jane
DESCRIPTION:Weekly check-in. Project status and blockers.
LOCATION:Blue Bottle Gangnam
STATUS:CONFIRMED
TRANSP:OPAQUE
BEGIN:VALARM
TRIGGER:-PT15M
ACTION:DISPLAY
DESCRIPTION:회의 15분 전: 1on1 with Jane
END:VALARM
END:VEVENT
END:VCALENDAR
```

### 2.2 index.json

```json
{
  "f7c5a0b2-...": {
    "file": "2026-06-07_1on1-jane.ics",
    "summary": "1on1 with Jane",
    "dtstart": "2026-06-07T10:00:00+09:00",
    "dtend": "2026-06-07T11:00:00+09:00",
    "rrule": null,
    "status": "confirmed",
    "source": "agent"
  },
  "a1b2c3d4-...": {
    "file": "2026-06-09_team-sync.ics",
    "summary": "Team sync",
    "dtstart": "2026-06-09T14:00:00+09:00",
    "dtend": "2026-06-09T14:30:00+09:00",
    "rrule": "FREQ=WEEKLY;BYDAY=TU",
    "status": "confirmed",
    "source": "agent"
  }
}
```

**왜 index.json이 필요한가:** .ics 파일을 전부 파싱하지 않고 빠른 조회. Web UI의 월 뷰에서 한 달치 이벤트를 1초 안에 로드. index.json은 파일 변경 시 자동 갱신.

### 2.3 롤링 보존

매일 자정 cron이 `archive/` 처리:
- `dtend < now - 365d`인 .ics 파일을 `archive/`로 이동
- `archive/`는 git에 커밋 (영구 보존), index.json에서 제거 (빠른 조회 대상 아님)
- 사용자가 과거 일정 검색 시 `archive/`도 함께 검색

---

## 3. 에이전트 도구

```json
{
  "name": "calendar",
  "description": "Manage calendar events. Events are stored as .ics files. Use op=create to add new events, op=update to modify, op=delete to remove, op=list to query. Reminders are set via VALARM in the .ics file.",
  "parameters": {
    "op": {
      "type": "string",
      "enum": ["create", "update", "delete", "list", "get", "freebusy", "search"],
      "description": "Operation to perform."
    },

    "title":        { "type": "string",  "description": "Event title. Required for create." },
    "start":        { "type": "string",  "description": "Start time. ISO 8601 with timezone. e.g. '2026-06-07T10:00:00+09:00'" },
    "end":          { "type": "string",  "description": "End time. ISO 8601 with timezone." },
    "all_day":      { "type": "boolean", "description": "All-day event. start/end are dates only." },
    "description":  { "type": "string",  "description": "Event description. Markdown." },
    "location":     { "type": "string",  "description": "Location text or URL." },

    "repeat": {
      "type": "object",
      "description": "Recurrence rule. If set, event repeats.",
      "properties": {
        "frequency": { "type": "string", "enum": ["daily", "weekly", "monthly", "yearly"] },
        "days":      { "type": "array",  "items": { "type": "string" }, "description": "For weekly: ['mon','wed','fri']" },
        "interval":  { "type": "integer", "description": "Every N freq. Default 1." },
        "until":     { "type": "string",  "description": "End date. ISO date. e.g. '2026-12-31'" },
        "count":     { "type": "integer", "description": "Number of occurrences." }
      }
    },

    "reminder_minutes": {
      "type": "array",
      "items": { "type": "integer" },
      "description": "Minutes before event to trigger reminder. e.g. [15, 60]. Empty = no reminder."
    },

    "uid":          { "type": "string",  "description": "Event UID. Required for update/delete/get." },
    "from":         { "type": "string",  "description": "Query start. ISO 8601. For list/freebusy." },
    "to":           { "type": "string",  "description": "Query end. ISO 8601. For list/freebusy." },
    "query":        { "type": "string",  "description": "Search term. For search." }
  }
}
```

### 3.1 사용 예시

**생성:**
```json
{
  "op": "create",
  "title": "1on1 with Jane",
  "start": "2026-06-07T10:00:00+09:00",
  "end": "2026-06-07T11:00:00+09:00",
  "description": "Weekly check-in. Project status and blockers.",
  "location": "Blue Bottle Gangnam",
  "repeat": { "frequency": "weekly", "days": ["tue"], "until": "2026-12-31" },
  "reminder_minutes": [15]
}
```
→ 응답:
```json
{
  "uid": "f7c5a0b2-...",
  "status": "created",
  "conflicts": [],
  "file": "2026-06-07_1on1-jane.ics"
}
```

**충돌 있는 생성:**
```json
{
  "op": "create",
  "title": "Dentist",
  "start": "2026-06-07T10:30:00+09:00",
  "end": "2026-06-07T11:30:00+09:00"
}
```
→ 응답:
```json
{
  "uid": "a3b4c5d6-...",
  "status": "created",
  "conflicts": [
    { "uid": "f7c5a0b2-...", "title": "1on1 with Jane", "overlap_minutes": 30 }
  ],
  "file": "2026-06-07_dentist.ics"
}
```

**조회:**
```json
{ "op": "list", "from": "2026-06-07", "to": "2026-06-07" }
```
→ 응답:
```json
{
  "events": [
    { "uid": "f7c5a0b2-...", "title": "1on1 with Jane", "start": "10:00", "end": "11:00", "status": "confirmed" },
    { "uid": "a3b4c5d6-...", "title": "Dentist", "start": "10:30", "end": "11:30", "status": "confirmed" }
  ]
}
```

---

## 4. 모듈 구조

```
crates/oxios-calendar/                  # 신규 크레이트
├── src/
│   ├── lib.rs
│   ├── engine.rs            # CalendarEngine — CRUD, 조회, 충돌 검사
│   ├── ical.rs              # .ics 생성/파싱 (RFC 5545)
│   ├── rrule.rs             # 반복 규칙 확장 (simple → RRULE)
│   ├── index.rs             # index.json 관리
│   ├── alarm.rs             # VALARM 처리 + 알림 디스패치
│   ├── conflict.rs          # 충돌 감지
│   ├── journal_bridge.rs    # journal 자동 주입
│   ├── cron_bridge.rs       # CronScheduler → 가상 이벤트
│   ├── archive.rs           # 롤링 보존 (1년)
│   └── types.rs             # Event, EventDraft, Repeat, 등
│
crates/oxios-kernel/src/
├── tools/builtin/calendar_tool.rs      # 에이전트 도구
├── kernel_handle/calendar_api.rs       # 15번째 KernelHandle API
└── config.rs                           # + [calendar] 섹션
```

### 4.1 CalendarEngine

```rust
pub struct CalendarEngine {
    dir: PathBuf,                           // ~/.oxios/workspace/calendar/
    index: RwLock<CalendarIndex>,           // uid → 메타
    journal: Arc<JournalBridge>,
    cron: Arc<CronBridge>,
    alarm_tx: mpsc::Sender<AlarmEvent>,     // 알림 디스패처로
    event_bus: Option<EventBus>,
}

impl CalendarEngine {
    pub async fn create(&self, draft: EventDraft) -> Result<CreateResult>;
    pub async fn update(&self, uid: &str, patch: EventPatch) -> Result<UpdateResult>;
    pub async fn delete(&self, uid: &str) -> Result<()>;
    pub async fn get(&self, uid: &str) -> Result<Event>;
    pub async fn list(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Vec<Event>>;
    pub async fn search(&self, query: &str) -> Result<Vec<Event>>;
    pub async fn freebusy(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Vec<FreeBusySlot>>;
}
```

### 4.2 .ics 생성 (구조화 파라미터 → .ics)

```rust
// crates/oxios-calendar/src/ical.rs

pub fn build_ics(draft: &EventDraft) -> Result<String> {
    let mut vevent = VEvent::new();
    vevent.uid(&draft.uid);
    vevent.dtstamp(Utc::now());
    vevent.dtstart(draft.dtstart);
    vevent.dtend(draft.dtend);
    vevent.summary(&draft.title);
    if let Some(desc) = &draft.description { vevent.description(desc); }
    if let Some(loc) = &draft.location { vevent.location(loc); }
    if let Some(repeat) = &draft.repeat {
        let rrule = simple_repeat_to_rrule(repeat);
        vevent.rrule(&rrule);
    }
    for mins in &draft.reminder_minutes {
        vevent.add_alarm(Alarm::display(
            -chrono::Duration::minutes(*mins as i64),
            &format!("{}분 전: {}", mins, draft.title),
        ));
    }
    Ok(VCalendar::from(vevent).to_string())
}
```

### 4.3 반복 규칙 변환

```rust
// crates/oxios-calendar/src/rrule.rs

pub fn simple_repeat_to_rrule(r: &Repeat) -> String {
    let mut parts = vec![format!("FREQ={}", r.frequency.to_uppercase())];
    if r.interval > 1 { parts.push(format!("INTERVAL={}", r.interval)); }
    if !r.days.is_empty() {
        let days: Vec<&str> = r.days.iter().map(|d| d.to_uppercase()).collect();
        parts.push(format!("BYDAY={}", days.join(",")));
    }
    if let Some(until) = &r.until { parts.push(format!("UNTIL={}", until.format("%Y%m%d"))); }
    else if let Some(count) = r.count { parts.push(format!("COUNT={}", count)); }
    parts.join(";")
}
```

---

## 5. Journal 연동

```rust
// crates/oxios-calendar/src/journal_bridge.rs

impl JournalBridge {
    /// journal 파일이 열리거나 생성될 때 호출.
    /// 또는 매일 자정 cron.
    pub async fn inject_events(&self, date: NaiveDate) -> Result<()> {
        let events = self.calendar.list(
            date.and_hms(0,0,0).and_local_timezone(seoul).unwrap(),
            date.and_hms(23,59,59).and_local_timezone(seoul).unwrap(),
        ).await?;

        if events.is_empty() { return Ok(()); }

        let filename = format!("{}.md", date.format("%Y-%m-%d"));
        let mut content = match self.fs.read(DIR_JOURNAL, &filename) {
            Ok(c) => c,
            Err(_) => return Ok(()),  // journal 없으면 안 만듦
        };

        // "## Today's events" 섹션 찾아서 교체, 없으면 추가
        let section = self.build_event_section(date, &events);
        content = replace_or_append_section(&mut content, "## Today's events", &section);

        self.fs.write(DIR_JOURNAL, &filename, &content)?;
        Ok(())
    }

    fn build_event_section(&self, date: NaiveDate, events: &[Event]) -> String {
        let mut lines = vec!["## Today's events".to_string()];
        for e in events.sorted_by_key(|e| e.dtstart) {
            let time = e.dtstart.format("%H:%M");
            let end = e.dtend.format("%H:%M");
            let ics_file = format!("calendar/events/{}.ics", e.filename);
            lines.push(format!("- **{}–{}** [{}]({}) {}", time, end, e.title, ics_file, e.description.unwrap_or_default()));
        }
        lines.join("\n")
    }
}
```

journal 예시:
```markdown
---
type: journal
date: 2026-06-07
---

# Sunday, June 7

## Today's events
- **10:00–11:00** [1on1 with Jane](calendar/events/2026-06-07_1on1-jane.ics) Weekly check-in
- **14:00–14:30** [Team sync](calendar/events/2026-06-09_team-sync.ics)
- **10:30–11:30** [Dentist](calendar/events/2026-06-07_dentist.ics)

## Notes
`15:04` Had a good chat with Jane about the refactor plan.
```

---

## 6. Cron 시각화

```rust
// crates/oxios-calendar/src/cron_bridge.rs

pub fn cron_events(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Vec<SyntheticEvent> {
    let jobs = self.cron.list_jobs();
    jobs.iter()
        .filter(|j| j.enabled)
        .flat_map(|job| {
            let schedule = Schedule::from_str(&job.schedule).ok()?;
            let fires: Vec<DateTime<Utc>> = schedule
                .after(&from)
                .take_while(|t| *t < to)
                .collect();
            Some(fires.into_iter().map(|fire| SyntheticEvent {
                title: format!("⚙️ {}", job.name),
                start: fire,
                end: fire + chrono::Duration::minutes(5),  // 추정
                kind: SyntheticKind::Cron { job_id: job.id, goal: job.goal.clone() },
            }))
        })
        .flatten()
        .collect()
}
```

Web UI에서 `⚙️ Oxios System` 토글 ON → cron fire 시각이 캘린더에 반투명 칩으로 표시. 클릭 → cron job 상세 팝업.

---

## 7. 알림 (VALARM → 채널)

```rust
// crates/oxios-calendar/src/alarm.rs

pub struct AlarmDispatcher {
    event_bus: Option<EventBus>,
    channels: Vec<AlarmChannel>,            // Web, Telegram, Push
}

pub enum AlarmChannel {
    Web { sse: Arc<SseBridge> },
    Telegram { bot: Arc<TelegramAdapter> },
    Push { ntfy_url: String },
}

impl AlarmDispatcher {
    /// 매 분마다 wake → 다가오는 VALARM 트리거 확인 → 채널 디스패치
    pub async fn tick(&self) {
        let now = Utc::now();
        let soon = now + chrono::Duration::minutes(1);
        // index에서 soon 전에 trigger될 VALARM 조회
        let alarms = self.find_pending_alarms(now, soon).await;
        for alarm in alarms {
            self.dispatch(&alarm).await;
        }
    }

    async fn dispatch(&self, alarm: &PendingAlarm) {
        let msg = format!("🔔 {} — {}", alarm.event_title, alarm.human_until());
        for ch in &self.channels {
            ch.send(&msg, &alarm).await;
        }
        // EventBus에도 발행
        if let Some(bus) = &self.event_bus {
            bus.publish(KernelEvent::CalendarAlarm { uid: alarm.event_uid.clone() });
        }
    }
}
```

**알림은 Web/Telegram/푸시로.** 이메일 아님. VALARM 트리거 시간은 .ics 파일에 저장되어 있으므로, alarm ticker가 매 분마다 확인.

---

## 8. Web UI

### 8.1 라우트

| Path | 용도 |
|------|------|
| `/calendar` | 메인 캘린더 뷰 (월/주/일/agenda 토글) |
| `/calendar/new` | 새 이벤트 모달 |
| `/calendar/event/:uid` | 이벤트 상세 + 편집 |
| `/calendar/alarms` | 다가오는 알림 목록 |

### 8.2 컴포넌트

```
surface/oxios-web/web/src/components/calendar/
├── CalendarView.tsx          # 메인. react-big-calendar 또는 @schedule-x
├── EventChip.tsx             # 이벤트 칩 (색상 + 시간 + 제목)
├── EventEditor.tsx           # 생성/수정 모달 (title, start, end, repeat, reminder)
├── EventDetail.tsx           # 상세 + [편집] [삭제]
├── RepeatEditor.tsx          # 반복 규칙 빌더 (frequency, days, until)
├── ReminderEditor.tsx        # 알림 시간 편집
├── CronOverlay.tsx           # cron 이벤트 토글 + 칩
├── CronPopover.tsx           # cron 칩 hover: 직전 run + [지금 실행]
├── ConflictWarning.tsx       # 충돌 경고 배너
├── AlarmList.tsx             # 다가오는 알림
├── AgendaList.tsx            # agenda 뷰
└── FreeBusyBar.tsx           # free/busy 바 (일 단위)
```

**캘린더 라이브러리:** `@schedule-x/react` (가벼움, iCal 호환) 또는 `react-big-calendar`. 10KB~. 커스텀 이벤트 렌더러로 cron 칩과 충돌 표시.

### 8.3 UX

```
┌─────────────────────────────────────────────────────────────┐
│ ◀  2026년 6월  ▶      [월] [주] [일] [Agenda]   [+ 새 일정] │
├───┬─────┬─────┬─────┬─────┬─────┬─────┬─────────────────────┤
│일 │ 월  │ 화  │ 수  │ 목  │ 금  │ 토  │                     │
│   │  1  │  2  │  3  │  4  │  5  │  6  │                     │
│   │     │     │     │     │     │ ⚙️8 │  ← cron: morning    │
│   │     │     │     │     │     │     │     digest          │
│ 7 │  8  │  9  │ 10  │ 11  │ 12  │ 13  │                     │
│☕ │     │📞  │     │     │     │ ⚙️8 │                     │
│10 │     │14  │     │     │     │     │                     │
│🦷 │     │     │     │     │     │     │                     │
│10:│     │     │     │     │     │     │                     │
│30 │     │     │     │     │     │     │                     │
├───┴─────┴─────┴─────┴─────┴─────┴─────┴─────────────────────┤
│ ⚙️ Oxios System ☑                                           │
└─────────────────────────────────────────────────────────────┘
```

---

## 9. CLI

```bash
oxios calendar today
oxios calendar tomorrow
oxios calendar week
oxios calendar month
oxios calendar list --from 2026-06-01 --to 2026-06-30
oxios calendar search <query>

oxios calendar create --title "Dentist" --start "2026-06-07T10:30:00+09:00" \
                      --end "2026-06-07T11:30:00+09:00" --reminder 15
oxios calendar update <uid> --title "Dentist (rescheduled)" --start "..."
oxios calendar delete <uid>

oxios calendar freebusy --from 2026-06-07 --to 2026-06-07
oxios calendar alarms                  # 다가오는 알림
```

---

## 10. Configuration

```toml
[calendar]
enabled = true
timezone = "Asia/Seoul"                 # 기본 타임존
default_reminder_minutes = [15]         # 새 이벤트 기본 알림
alarm_channels = ["web", "telegram"]    # 알림 채널
journal_sync = "on_open"                # on_open | midnight | both
system_calendar = true                  # cron 시각화
archive_after_days = 365                # 롤링 보존
```

---

## 11. 의존성

| 크레이트 | 용도 |
|---|---|
| `icalendar` (또는 `ical`) | RFC 5545 .ics 생성/파싱 |
| `rrule` | 반복 규칙 확장 |
| `chrono-tz` | 타임존 |
| `cron` (기존) | CronBridge |
| (기존) `oxios-markdown` | JournalBridge, VirtualFs |
| (기존) `lettre` | 알림 발송 시 사용 안 함 (Web/Telegram만) |

---

## 12. EventBus 이벤트

```rust
KernelEvent::CalendarEventCreated { uid, title, start, end }
KernelEvent::CalendarEventUpdated { uid, title }
KernelEvent::CalendarEventDeleted { uid, title }
KernelEvent::CalendarAlarm { uid, title, minutes_until }
```

---

## 13. 구현 규모

| 모듈 | LOC |
|---|---|
| `oxios-calendar` (engine, ical, rrule, index, alarm, conflict, journal_bridge, cron_bridge, archive) | ~2,500 |
| `calendar_tool.rs` (에이전트 도구) | ~300 |
| `calendar_api.rs` (KernelHandle) | ~200 |
| Web UI (11 컴포넌트) | ~1,500 |
| CLI | ~200 |
| 테스트 | ~1,000 |
| **총계** | **~5,700 LOC** |

**~3~4주, 1명.**

---

## 14. 테스트

| 테스트 | 방법 |
|---|---|
| .ics 생성/파싱 round-trip | fixture 30개 |
| rrule 변환 | simple_repeat → RRULE → 확장 결과 검증 |
| 충돌 감지 | 단일/반복/전일 경계값 |
| index.json 동기화 | 생성/수정/삭제 후 인덱스 일치 |
| journal 주입 | 이벤트 있는 날/없는 날/빈 journal/이미 섹션 있는 journal |
| cron 확장 | 5개 cron 표현식 → 정확한 fire 시각 |
| alarm tick | VALARM 트리거 시간 전후 디스패치 |
| archive | 366일+ 이벤트 archive/ 이동 |
| 구조화 파라미터 검증 | 누락 필드, 잘못된 타임존, 음수 duration |

---

## 15. 후속 (v2+)

| 기능 | 시점 |
|---|---|
| iCloud CalDAV 동기화 | v2 |
| Google Calendar CalDAV | v2 |
| 다중 캘린더 (personal/work) | v2 |
| iTip 초대/응답 | v2 |
| 외부 free/busy 공유 | v3 |
| 반복 인스턴스 개별 수정 (RECURRENCE-ID) | v2 |
