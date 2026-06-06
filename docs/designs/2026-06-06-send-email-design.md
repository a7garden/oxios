# Send Email — 에이전트 이메일 발송 도구

> **날짜:** 2026-06-06
> **상태:** Draft
> **이전 문서:** `2026-06-06-delivery-design.md` (폐기 — 과도하게 복잡했음)
> **핵심:** 에이전트에게 `send_email` 도구를 준다. HTML 작성, 템플릿, 콘텐츠 — 전부 에이전트가 결정. 우리는 SMTP 파이프만 제공.

---

## 1. 한 줄 요약

```
에이전트가 HTML 이메일을 작성해서 내 이메일 주소로 보낸다.
그게 다다.
```

## 2. 사용자가 설정하는 것 (1회)

```toml
# ~/.oxios/config.toml

[email]
my_email = "me@gmail.com"

[email.smtp]
provider = "gmail"                   # gmail | icloud | fastmail | custom
# provider = "gmail"이면 host/port/tls 자동 설정. 아래 생략 가능.
# host = "smtp.gmail.com"
# port = 465
# tls = "tls"
# user = "me@gmail.com"
# auth_kind = "app_password"         # app_password | oauth2
# secret_ref = "email_smtp"          # credential store 키
```

```bash
$ oxios email setup
# → provider 선택 (Gmail/iCloud/Fastmail/Custom)
# → 앱 비밀번호 입력 (1회, credential store에 암호화 저장)
# → 테스트 메일 발송 → "설정 완료"
```

**끝.** 도메인 불필요, 외부 서비스 가입 불필요, `lettre`로 Gmail SMTP 서버에 직접 연결.

## 3. 에이전트 도구

```json
{
  "name": "send_email",
  "description": "Compose and send an HTML email. You decide the format, layout, and content. For recurring sends, save as template and reuse. Templates are stored in ~/.oxios/workspace/email_templates/.",
  "parameters": {
    "subject": {
      "type": "string",
      "description": "Email subject line"
    },
    "body_html": {
      "type": "string",
      "description": "HTML body. Full <html> document or <body> fragment. Inline CSS only (email clients strip <style>)."
    },
    "body_text": {
      "type": "string",
      "description": "Plain text fallback. Optional but recommended for accessibility."
    },
    "save_template_as": {
      "type": "string",
      "description": "Save this email as a reusable template with this name. Stored in email_templates/<name>.html"
    },
    "use_template": {
      "type": "string",
      "description": "Name of a saved template to use. body_html is ignored; template_vars are substituted."
    },
    "template_vars": {
      "type": "object",
      "description": "Key-value pairs to substitute in template. {{key}} → value."
    },
    "list_templates": {
      "type": "boolean",
      "description": "If true, list available templates and return. All other params ignored."
    }
  }
}
```

### 3.1 사용 예시 (에이전트 관점)

**예시 1: 오늘의 다이제스트 (최초, 템플릿 없음)**
```
에이전트가 send_email 호출:
{
  "subject": "☀️ Morning Digest — 2026-06-06",
  "body_html": "<html><body style='font-family:...'>...(직접 작성한 HTML)...</body></html>",
  "body_text": "Morning Digest...\n1. 10:00 — 1on1 with Jane\n...",
  "save_template_as": "morning-digest"
}
```
→ 발송 + `~/.oxios/workspace/email_templates/morning-digest.html`에 템플릿 저장.

**예시 2: 다음 날 (템플릿 재사용)**
```
에이전트가 send_email 호출:
{
  "subject": "☀️ Morning Digest — 2026-06-07",
  "use_template": "morning-digest",
  "template_vars": {
    "date": "2026-06-07",
    "events_html": "<li>10:00 — 1on1 with Jane</li><li>14:00 — Product sync</li>",
    "action_items_html": "<li>Review PR #142</li>",
    "quote": "The only way to do great work is to love what you do."
  }
}
```
→ 템플릿 로드 → `{{date}}`, `{{events_html}}` 등 치환 → 발송.

**예시 3: 템플릿 목록 확인**
```
에이전트가 send_email 호출:
{ "list_templates": true }
→ 응답: ["morning-digest", "weekly-research", "build-failure-alert"]
```

### 3.2 에이전트가 템플릿을 관리하는 방식

```
~/.oxios/workspace/email_templates/
├── morning-digest.html          # 에이전트가 최초 발송 시 save_template_as로 생성
├── weekly-research.html         # 에이전트가 다른 발송 시 생성
└── build-failure-alert.html     # 에이전트가 필요하다고 판단하면 언제든 생성
```

에이전트는 일반 파일 도구(`read`, `write`)로 이 디렉토리에 접근 가능. 템플릿을 읽어 분석하고, 개선하고, 새로 만들고, 삭제. **우리는 도구만 제공, 관리는 에이전트가.**

## 4. 아키텍처 (최소)

```
crates/oxios-kernel/src/
├── email.rs                     # SmtpClient (lettre 래퍼)
├── tools/builtin/email_tool.rs  # send_email AgentTool
└── config.rs                    # + [email] 섹션 파싱
```

**새 파일 2개.** 외부 크레이트 1개 (`lettre`). 

```
~/.oxios/
├── config.toml                  # [email] 섹션
├── workspace/
│   ├── email_templates/         # 템플릿 (에이전트가 read/write)
│   └── email_sent/              # 발송 이력 (StateStore)
└── secrets/
    └── email.enc                # 앱 비밀번호 (credential store)
```

### 4.1 SmtpClient

```rust
// crates/oxios-kernel/src/email.rs
pub struct SmtpClient {
    transport: lettre::AsyncSmtpTransport<lettre::Tokio1Executor>,
    from: Mailbox,
    default_to: Mailbox,
}

impl SmtpClient {
    pub fn from_config(config: &EmailConfig, creds: &CredentialStore) -> Result<Self>;
    
    pub async fn send(&self, to: &str, subject: &str, html: &str, text: Option<&str>) -> Result<SendReceipt> {
        let email = Message::builder()
            .from(self.from.clone())
            .to(to.parse()?)
            .subject(subject)
            .multipart(
                MultiPart::alternative()
                    .singlepart(SinglePart::builder()
                        .header(header::ContentType::TEXT_PLAIN)
                        .body(text.unwrap_or_default()))
                    .singlepart(SinglePart::builder()
                        .header(header::ContentType::TEXT_HTML)
                        .body(html.to_string()))
            )?;
        
        let message_id = self.transport.send(email).await?;
        Ok(SendReceipt { message_id, sent_at: Utc::now() })
    }
}
```

### 4.2 EmailTool

```rust
// crates/oxios-kernel/src/tools/builtin/email_tool.rs
pub struct EmailTool {
    smtp: Arc<SmtpClient>,
    template_dir: PathBuf,
    sent_store: Arc<StateStore>,
    event_bus: Option<EventBus>,
}

impl AgentTool for EmailTool {
    fn name(&self) -> &str { "send_email" }
    
    async fn execute(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        let args: EmailArgs = serde_json::from_value(args)?;
        
        // list_templates 모드
        if args.list_templates {
            let templates = self.list_templates()?;
            return Ok(json!({ "templates": templates }));
        }
        
        // 본문 결정: 템플릿 사용 vs 직접 HTML
        let html = if let Some(name) = &args.use_template {
            let template = self.load_template(name)?;
            self.render_template(&template, &args.template_vars.unwrap_or_default())?
        } else {
            args.body_html.clone()
                .ok_or(anyhow!("body_html or use_template required"))?
        };
        
        let subject = args.subject.clone()
            .ok_or(anyhow!("subject required"))?;
        
        // 발송
        let receipt = self.smtp.send("me", &subject, &html, args.body_text.as_deref()).await?;
        
        // 템플릿 저장 (요청 시)
        if let Some(name) = &args.save_template_as {
            self.save_template(name, &html)?;
        }
        
        // 이력 기록
        self.save_sent(&receipt, &subject, &html)?;
        
        // EventBus 알림 (Telegram 등이 "이메일 발송됨" 알림을 보내도록)
        if let Some(bus) = &self.event_bus {
            bus.publish(KernelEvent::EmailSent {
                subject,
                message_id: receipt.message_id,
                template_name: args.save_template_as,
            });
        }
        
        Ok(json!({
            "status": "sent",
            "message_id": receipt.message_id,
            "template_saved": args.save_template_as.is_some(),
        }))
    }
}
```

## 5. Telegram 알림 (별도)

`send_email`이 발송 성공 → `KernelEvent::EmailSent` 발행 → 기존 EventBus 구독자가 처리.

Telegram 채널이 이미 EventBus를 구독 중이므로, 새 이벤트 타입만 추가하면 Telegram에 자동으로:

```
📬 이메일 발송 완료
제목: ☀️ Morning Digest — 2026-06-06
수신: me@gmail.com
[이력 보기]
```

**본문은 안 보냄.** "발송됨" 보고만. 이것이 이전 설계와의 핵심 차이.

## 6. CronScheduler 연동

정기 발송 = 기존 `CronScheduler` + `send_email` 도구의 조합. **새 메커니즘 불필요.**

```toml
# 이미 config.toml에 있는 cron 섹션 활용
[[cron.jobs]]
name = "morning-digest"
schedule = "0 8 * * *"
goal = """
오늘의 다이제스트 이메일을 발송.
1. 오늘 캘린더 일정 확인
2. 어제 memory에서 액션 아이템 추출
3. morning-digest 템플릿 사용해 HTML 이메일 작성
4. send_email로 발송 (save_template_as는 최초 1회만)
"""
enabled = true
```

에이전트가 cron job의 `goal`을 읽고 실행. `send_email` 도구를 자연스럽게 호출. **우리가 delivery engine을 만들 필요 없음.**

## 7. 발송 이력

```
~/.oxios/workspace/email_sent/
├── 2026-06-06_080012_morning-digest.json
├── 2026-06-06_141530_report.json
└── 2026-06-07_080005_morning-digest.json
```

각 파일:
```json
{
  "id": "uuid",
  "sent_at": "2026-06-06T08:00:12+09:00",
  "subject": "☀️ Morning Digest — 2026-06-06",
  "to": "me@gmail.com",
  "template_used": "morning-digest",
  "message_id": "<abc123@gmail.com>",
  "html_preview": "<!-- 첫 500자 -->",
  "cron_job": "morning-digest"
}
```

StateStore에 저장. Web UI `/email/history`에서 열람. 검색 가능.

## 8. Web UI

| Route | 용도 |
|---|---|
| `/email/setup` | SMTP 설정 마법사 (1회) |
| `/email/history` | 발송 이력 (최근 100건) |
| `/email/history/:id` | 발송 상세 (HTML 미리보기 + 원문) |
| `/email/templates` | 템플릿 목록 (읽기 전용 — 편집은 에이전트가) |

**컴포넌트 4개.** `surface/oxios-web/web/src/components/email/` 에 약 400 LOC.

## 9. CLI

```bash
oxios email setup                    # SMTP 설정 마법사
oxios email test                     # 테스트 메일 발송
oxios email history [--limit 20]     # 발송 이력
oxios email templates                # 템플릿 목록
```

**100 LOC.** 나머지는 에이전트가 `send_email` 도구로 처리.

## 10. HITL (인간 승인)

v1은 **수신자 = 나 자신**이므로 기본 HITL 불필요. 다만:

- **에이전트가 외부 수신자에게 보내려 하면** → 거부 (v1에서는 `to` 파라미터 무시, 항상 `my_email`로 발송)
- **발송 빈도 제한** → 1시간에 최대 10통 (설정 가능). 초과 시 "rate limit, try later"
- **subject/body 길이 제한** → subject 200자, body 1MB

v2에서 외부 수신자 허용 시 → 그때 HITL 승인 추가.

## 11. 보안

- 앱 비밀번호 → `credential` 모듈에 암호화 저장 (OS 키체인)
- 비밀번호 절대 로그/JSON/API 응답에 노출 안 됨
- 발송 이력에 HTML 원문 저장 (audit 목적) — GitLayer로 버전 관리
- `KernelEvent::EmailSent` → AuditTrail (Merkle 체인)

## 12. 의존성

| 크레이트 | 용도 | 크기 |
|---|---|---|
| `lettre` | SMTP 클라이언트 (tokio) | 중간 |
| (기존) `credential` | 비밀번호 저장 | — |
| (기존) `state_store` | 발송 이력 | — |
| (기존) `event_bus` | 알림 | — |

**새 외부 의존성 1개.** `async-imap`, `mailparse`, `caldav-rs` 등은 포함 안 함.

## 13. 구현 규모

| 파일 | LOC |
|---|---|
| `email.rs` | ~200 |
| `tools/builtin/email_tool.rs` | ~250 |
| `config.rs` 변경 | ~30 |
| Web UI (4 컴포넌트) | ~400 |
| CLI | ~100 |
| 테스트 | ~400 |
| **총계** | **~1,400 LOC** |

**1~1.5주, 1명.**

## 14. 테스트

| 테스트 | 방법 |
|---|---|
| SMTP 발송 | `mailhog` (로컬 SMTP 테스트 서버) |
| 템플릿 로드/치환 | `{{var}}` → 값, 누락 변수, HTML 이스케이프 |
| rate limit | 11회 연속 발송 → 11번째 거부 |
| MIME 생성 | text/plain + text/html 멀티파트 검증 |
| credential 저장 | 암호화 저장 → 복호화 → 원본 일치 |
| EventBus | 발송 성공 → `EmailSent` 이벤트 발행 확인 |
| 길이 제한 | 1MB HTML → 발송 성공, 1.1MB → 거부 |

## 15. 향후 확장 (v2+)

- 외부 수신자 (HITL 승인 추가)
- 첨부파일 (PDF 리포트 등)
- 커스텀 SMTP (Resend, SES — 도메인 있는 사용자)
- IMAP 수신 (메일 읽기)
- 수신 이메일 트리아지

**지금은 안 만든다.** v1은 발신 전용, 나에게만, 에이전트가 전부 결정.
