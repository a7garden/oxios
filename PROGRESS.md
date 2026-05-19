# Progress

## Status
Done

## Tasks
- [x] Track A: oxios-markdown POSIX API + 미사용 모듈 제거
  - [x] fs.rs: POSIX path API 추가 (read_path, write_path, delete_path, rename_path, exists_path, mtime_path + split_posix_path)
  - [x] lib.rs: 미사용 모듈 제거 (chat, journal, habits, schedule, tokens) + re-export 정리 + split_posix_path, today_chat_header, today_journal_path re-export
  - [x] 파일 삭제: chat.rs, journal.rs, habits.rs, schedule.rs, tokens.rs
  - [x] parser.rs: today_chat_header(), today_journal_path() 유틸리티 함수 추가
  - [x] sync, fslog 모듈 #[allow(dead_code)] 유지
  - [x] cargo check -p oxios-markdown 통과
  - [x] cargo test -p oxios-markdown 통과 (38 tests + 1 doc-test)
  - [x] cargo check --workspace 통과 (다운스트림 영향 없음)

## Files Changed
- `crates/oxios-markdown/src/fs.rs` — POSIX path API 6개 메서드 + split_posix_path() free function 추가
- `crates/oxios-markdown/src/lib.rs` — 모듈/Re-export 정리 (5개 모듈 삭제, 3개 Re-export 추가)
- `crates/oxios-markdown/src/parser.rs` — today_chat_header(), today_journal_path() 추가
- `crates/oxios-markdown/src/chat.rs` — 삭제
- `crates/oxios-markdown/src/journal.rs` — 삭제
- `crates/oxios-markdown/src/habits.rs` — 삭제
- `crates/oxios-markdown/src/schedule.rs` — 삭제
- `crates/oxios-markdown/src/tokens.rs` — 삭제

## Notes
- today_journal_path() format string 수정: task 원본의 `"journal/{}.{} {}.md"` → `"journal/{}.{}.md"` (3 positional args → 2)
- 삭제된 모듈을 참조하는 외부 crate 없음 확인
- types.rs의 Habits/Schedule 타입과 상수들은 다른 코드에서 사용될 수 있어 유지
