# OXIOS-VOICE: 음성 채널 설계

> **Date:** 2026-05-16  
> **Status:** Final Design — Future Milestone  
> **Target:** M4 Mac Mini (Apple Silicon)  
> **Position:** Gateway Channel Plugin (Telegram과 동등)

---

## 1. 핵심 개념

**음성은 채널이다.**

텔레그램이 텍스트를 받아 텍스트를 돌려주듯, 음성 채널은 소리를 받아 소리를 돌려준다.
LLM, 에이전트, 오로보로스 — 모든 지능은 커널에 있다. 음성 채널은 **I/O 변환**만 한다.

```
Telegram:  [텍스트] ──→ Gateway → Kernel → Gateway ──→ [텍스트]
Web:       [JSON]  ──→ Gateway → Kernel → Gateway ──→ [JSON]
Voice:     [음성]  ──→ STT ──→ Gateway → Kernel → Gateway ──→ TTS ──→ [음성]
```

STT는 입력 파서, TTS는 출력 포매터. 그 이상도 이하도 아니다.

---

## 2. 아키텍처

### 2.1 채널로서의 위치

```
channels/
├── oxios-web/       # HTTP/SSE 채널 (기존)
└── oxios-voice/     # 음성 채널 (신규)
    ├── Cargo.toml
    └── src/
        ├── lib.rs           # VoicePlugin (ChannelPlugin 구현)
        ├── channel.rs       # VoiceChannel (Channel 트레이트 구현)
        ├── config.rs        # VoiceChannelConfig
        ├── formatter.rs     # VoiceFormatter — 긴 응답을 요약
        ├── stt.rs           # STT 엔진 (Whisper → 텍스트)
        ├── tts.rs           # TTS 엔진 (Supertonic 3 → 음성)
        ├── audio.rs         # 마이크 입력 + 스피커 출력
        ├── wake.rs          # 웨이크 워드 / 핫키 감지
        └── event_loop.rs    # 메인 이벤트 루프
```

### 2.2 Channel 트레이트 구현

```rust
#[async_trait]
impl Channel for VoiceChannel {
    fn name(&self) -> &str { "voice" }

    async fn receive(&self) -> Result<Option<IncomingMessage>> {
        // 1. Wake word / 핫키 대기
        // 2. 마이크로 녹음
        // 3. STT: 음성 → 텍스트
        // 4. IncomingMessage { channel: "voice", content: 텍스트 }
    }

    async fn send(&self, msg: OutgoingMessage) -> Result<()> {
        // 1. msg.content (텍스트)를 받음
        // 2. VoiceFormatter로 음성에 맞게 포맷팅 (요약 또는 그대로)
        // 3. TTS: 텍스트 → 음성 (Supertonic 3)
        // 4. 스피커로 재생
    }
}
```

### 2.3 ChannelPlugin 구현

```rust
pub struct VoicePlugin;

#[async_trait]
impl ChannelPlugin for VoicePlugin {
    fn name(&self) -> &str { "voice" }

    async fn setup(&self, ctx: ChannelContext) -> Result<ChannelBundle> {
        let config: VoiceChannelConfig = ctx.config.read().voice.clone();
        let (voice_channel, event_loop) = VoiceChannel::new(config).await?;
        let task = tokio::spawn(async move { event_loop.run().await; });

        Ok(ChannelBundle {
            channel: Box::new(voice_channel),
            tasks: vec![task],
        })
    }
}
```

---

## 3. VoiceFormatter — 음성 출력 포매터

### 3.1 문제

커널이 생성하는 응답은 종종 길다:
- 3페이지 분량의 마크다운 보고서
- 코드 블록이 포함된 리팩토링 결과
- 다중 단계 작업 로그

이걸 TTS로 그대로 읽어주면 사용자 경험이 처참하다.

### 3.2 해결

**음성은 요약 채널이다.** 긴 내용은 간략히 말하고, 풀 텍스트는 다른 채널로 안내한다.

```
Kernel이 OutgoingMessage 생산
    │
    ├─▶ Web 채널:     [풀 마크다운] 그대로 렌더
    ├─▶ Telegram 채널: [풀 텍스트] 그대로 전송
    └─▶ Voice 채널:
            │
            ▼ VoiceFormatter
            │
            ├─ 짧은 응답 (<100자): 그대로 TTS
            │   "네, 알겠습니다."
            │
            └─ 긴 응답 (≥100자): 요약 + 다른 채널 안내
                "리팩토링 완료됐어. 3개 파일 변경됐고,
                 자세한 건 웹 대시보드에서 확인해."
```

### 3.3 구현

```rust
/// 음성 출력을 사용자 친화적으로 포맷팅.
pub struct VoiceFormatter {
    /// 최대 음성 길이 (글자 수). 이 이상이면 요약.
    max_speak_length: usize,
    /// 다른 활성 채널 목록 (안내용)
    other_channels: Vec<String>,
}

/// 포맷팅 결과.
pub enum VoiceOutput {
    /// 짧은 응답 — 그대로 읽어줌.
    Speak(String),
    /// 긴 응답 — 요약만 읽고, 풀 텍스트는 다른 채널 안내.
    Summary {
        /// 음성으로 말할 요약문.
        speech: String,
        /// 확인 가능한 다른 채널.
        full_available_via: Vec<String>,
    },
}

impl VoiceFormatter {
    pub fn format(&self, content: &str) -> VoiceOutput {
        // 마크다운 제거 (코드 블록, 링크, 헤더 등)
        let plain = strip_markdown(content);

        if plain.chars().count() <= self.max_speak_length {
            VoiceOutput::Speak(plain)
        } else {
            let summary = summarize(&plain, self.max_speak_length);
            VoiceOutput::Summary {
                speech: summary,
                full_available_via: self.other_channels.clone(),
            }
        }
    }
}
```

### 3.4 요약 전략

초기에는 휴리스틱. 나중에 필요하면 tiny 모델로 교체.

```rust
/// 휴리스틱 요약: 첫 문장 + 핵심 정보 추출.
fn summarize(text: &str, max_len: usize) -> String {
    // 1. 마크다운 구조 파싱 (헤더, 리스트)
    // 2. 첫 번째 헤더 → 주제
    // 3. 첫 번째 문장 → 핵심
    // 4. "~ 완료됐어. 자세한 건 웹에서 확인해." 형식으로 생성
    //
    // 예시:
    //   입력: 3페이지 리팩토링 보고서
    //   출력: "리팩토링 완료됐어. 3개 파일 변경됐고,
    //          테스트 모두 통과했어. 자세한 건 웹에서 확인해."
}
```

### 3.5 향후 확장 (필요시)

```rust
pub enum FormatterStrategy {
    /// 규칙 기반 (초기). 휴리스틱으로 첫 문장 + 길이 절삭.
    Heuristic,
    /// Tiny 로컬 모델로 요약. Qwen3-1.7B 등.
    Model(String),
}
```

Model 전략은 Phase 4 이후에 고려. Heuristic으로 시작.

---

## 4. 데이터 흐름

### 4.1 사용자 발화 → 에이전트 응답

```
사용자: "oxi, 오늘 할 일 알려줘"
    │
    ▼ [wake.rs] 웨이크 감지
    ▼ [audio.rs] 마이크 캡처 (16kHz, mono, i16)
    ▼ [stt.rs] Whisper → "오늘 할 일 알려줘"
    │
    ▼ [channel.rs] receive()
    │   IncomingMessage { channel: "voice", content: "오늘 할 일 알려줘" }
    │
    ▼ [Gateway] → Kernel (oxi-agent가 처리)
    │   → "오늘 할 일은 다음과 같습니다:\n1. 10시 미팅\n2. PR 리뷰\n..."
    │
    ▼ [Gateway] OutgoingMessage { channel: "voice", content: "..." }
    │
    ▼ [formatter.rs] VoiceFormatter
    │   → 길이 판단 → 요약 생성
    │   → "오늘 할 일이 3개 있어. 10시 미팅, PR 리뷰, 배포.
    │      자세한 건 웹에서 확인해."
    │
    ▼ [tts.rs] Supertonic 3 → f32 PCM (44.1kHz)
    ▼ [audio.rs] 스피커 재생
```

### 4.2 짧은 응답 (그대로 읽어줌)

```
Kernel: "네, 알겠습니다."
    │
    ▼ VoiceFormatter → VoiceOutput::Speak("네, 알겠습니다.")
    ▼ TTS → 스피커
```

### 4.3 긴 응답 (요약 + 안내)

```
Kernel: "# 리팩토링 결과\n\n## 변경 사항\n- src/main.rs ...\n(3페이지)"
    │
    ▼ VoiceFormatter → VoiceOutput::Summary {
    │       speech: "리팩토링 완료됐어. 3개 파일 변경됐고,
    │                테스트 모두 통과했어. 자세한 건 웹에서 확인해.",
    │       full_available_via: ["web"],
    │   }
    ▼ TTS → 스피커
```

### 4.4 에이전트 작업 완료 → 음성 알림

```
Kernel: AgentTaskComplete 이벤트
    │
    ▼ Gateway → OutgoingMessage { channel: "voice", content: "..." }
    ▼ VoiceFormatter → 요약
    ▼ TTS → "파일 정리 작업 완료됐어."
```

### 4.5 비교: 채널별 동일 메시지 처리

| 커널 응답 | Web | Telegram | Voice |
|-----------|-----|----------|-------|
| 짧은 텍스트 | 그대로 렌더 | 그대로 전송 | 그대로 TTS |
| 긴 마크다운 | 풀 렌더 | 풀 전송 | 요약 TTS + "웹에서 확인해" |
| 코드 블록 | 문법 하이라이트 | 마크다운 | "코드 생성됐어. 웹에서 확인해" |
| 작업 완료 | 상태 업데이트 | 알림 메시지 | 음성 알림 |

---

## 5. 컴포넌트 설계

### 5.1 STT 엔진 (stt.rs)

입력 파서. JSON 파서와 같은 역할.

```rust
/// Speech-to-text engine.
pub struct SttEngine {
    // whisper.cpp Rust bindings (whisper-rs)
}

impl SttEngine {
    /// Transcribe audio samples to text.
    /// - Input: 16kHz mono i16 PCM
    /// - Output: text string
    pub async fn transcribe(&self, audio: &[i16]) -> Result<String>;
}
```

**옵션:**

| Provider | 방식 | 메모리 | 비고 |
|----------|------|--------|------|
| whisper.cpp | Rust bindgen (whisper-rs) | ~75MB (tiny) | 가장 경량, 선택 |
| WhisperKit | Swift FFI | ~1.5GB | ANE 활용 |
| MLX Audio | Python 워커 | ~1GB | Python 의존 |

### 5.2 TTS 엔진 (tts.rs)

출력 포매터. JSON serializer와 같은 역할.

```rust
/// Text-to-speech engine using Supertonic 3 (99M params, 44.1kHz).
pub struct TtsEngine {
    model: TextToSpeech,  // supertonic Rust SDK (ONNX Runtime)
    voice_style: VoiceStyle,
}

impl TtsEngine {
    pub fn new(model_dir: &Path, voice: &str) -> Result<Self>;

    /// Synthesize text to raw audio samples (44.1kHz f32).
    pub fn synthesize(&mut self, text: &str, lang: &str) -> Result<Vec<f32>>;
}
```

**Supertonic 3:** 공식 Rust SDK, ONNX Runtime 기반, 한국어 포함 31개 언어,
감정 태그(`<laugh>`, `<breath>`) 지원.

### 5.3 오디오 I/O (audio.rs)

```rust
/// Microphone input using cpal.
pub struct Microphone { /* ... */ }

impl Microphone {
    /// Record until silence (VAD). Returns 16kHz mono i16 PCM.
    pub async fn record_until_silence(&self) -> Result<Vec<i16>>;
}

/// Speaker output using rodio.
pub struct Speaker { /* ... */ }

impl Speaker {
    /// Play raw audio samples.
    pub async fn play(&self, samples: Vec<f32>, sample_rate: u32) -> Result<()>;
}
```

### 5.4 웨이크 워드 (wake.rs)

```rust
/// Wake trigger — TCP 리스너처럼 연결 대기.
pub enum WakeTrigger {
    /// Global hotkey (e.g. "cmd+shift+v").
    Hotkey(String),
    /// Voice wake word (e.g. "oxi").
    WakeWord(String),
    /// Push-to-talk.
    PushToTalk,
}

/// Wake detector.
pub struct WakeDetector { /* ... */ }

impl WakeDetector {
    /// Block until wake trigger is detected.
    pub async fn wait(&mut self) -> Result<()>;
}
```

### 5.5 이벤트 루프 (event_loop.rs)

```rust
pub struct VoiceEventLoop {
    wake: WakeDetector,
    mic: Microphone,
    speaker: Speaker,
    stt: SttEngine,
    tts: TtsEngine,
    formatter: VoiceFormatter,
}

impl VoiceEventLoop {
    pub async fn run(mut self) -> Result<()> {
        loop {
            self.wake.wait().await?;
            let audio = self.mic.record_until_silence().await?;
            let text = self.stt.transcribe(&audio).await?;
            if text.trim().is_empty() { continue; }

            let incoming = IncomingMessage::new("voice", "local-user", &text);
            self.send_to_gateway(incoming).await?;
            // 응답은 VoiceChannel::send()로 콜백
        }
    }
}
```

---

## 6. 설정

```toml
# ~/.oxios/config.toml

[channels]
enabled = ["web", "voice"]

[channels.voice]
# 웨이크 트리거: "hotkey:cmd+shift+v" | "wake_word:oxi" | "push_toTalk"
trigger = "wake_word:oxi"

# STT
stt_provider = "whisper-cpp"
stt_model = "tiny"                # tiny | base | small | medium | large

# TTS
tts_voice = "M1"                  # Supertonic 프리셋
tts_speed = 1.05
tts_lang = "ko"

# Formatter
max_speak_length = 100            # 이 길이(자) 이상이면 요약

# 알림
notify_on_task_complete = true
notify_on_agent_error = true

# 오디오
volume = 0.8
silence_threshold = 0.01
silence_duration_ms = 1500
```

---

## 7. Cargo 설정

### channels/oxios-voice/Cargo.toml

```toml
[package]
name = "oxios-voice"
version = "0.1.0"
edition = "2021"

[dependencies]
oxios-gateway = { path = "../../crates/oxios-gateway" }
oxios-kernel = { path = "../../crates/oxios-kernel" }

tokio = { version = "1", features = ["full"] }
async-trait = "0.1"
anyhow = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Audio I/O
cpal = "0.15"
rodio = "0.19"

# TTS (Supertonic 3 via ONNX Runtime)
ort = "2.0.0-rc.7"
ndarray = { version = "0.17", features = ["rayon"] }
hound = "3.5"

# STT (whisper.cpp)
whisper-rs = "0.12"

# Wake word / hotkey
rdev = "0.5"

# Markdown stripping (formatter)
pulldown-cmark = "0.11"

tracing = "0.1"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4"] }
ringbuf = "0.4"
```

### 메인 바이너리

```toml
# crates/oxios/Cargo.toml

[features]
default = ["web"]
web = ["oxios-web"]
voice = ["oxios-voice"]

[dependencies]
oxios-voice = { path = "../../channels/oxios-voice", optional = true }
```

---

## 8. 구현 마일스톤

### Phase 1: TTS + 스피커 출력 (1-2일)
- [ ] `channels/oxios-voice/` 스캐폴딩
- [ ] `tts.rs` — Supertonic 3 통합
- [ ] `audio.rs` — rodio 스피커 출력
- [ ] `channel.rs` — VoiceChannel (send만)
- [ ] 커널 이벤트 → 음성 알림

### Phase 2: STT + 마이크 입력 (2일)
- [ ] `stt.rs` — whisper.cpp 통합
- [ ] `audio.rs` — cpal 마이크 캡처
- [ ] `channel.rs` — VoiceChannel (receive)

### Phase 3: 웨이크 워드 + 이벤트 루프 (1-2일)
- [ ] `wake.rs` — 핫키 + 웨이크 워드
- [ ] `event_loop.rs` — 전체 루프
- [ ] 설정 통합

### Phase 4: VoiceFormatter + 폴리싱 (1-2일)
- [ ] `formatter.rs` — 휴리스틱 요약
- [ ] 크로스채널 안내 ("자세한 건 웹에서")
- [ ] 감정 태그 활용
- [ ] 에러 복구

---

## 9. 예상 성능 (M4 Mac Mini)

| 컴포넌트 | 지연 | 메모리 |
|----------|------|--------|
| Wake Word (VAD) | ~50ms | <5MB |
| STT (whisper.cpp tiny) | ~100ms | ~75MB |
| TTS (Supertonic 3) | ~100ms | ~200MB |
| VoiceFormatter (heuristic) | <1ms | — |
| Audio I/O | ~20ms | — |
| **채널 총 오버헤드** | **~270ms** | **~280MB** |

커널의 LLM 추론 시간은 별개. 채널은 I/O 변환만 담당.

---

## 10. 핵심 원칙

1. **음성은 채널이다.** Telegram, Web과 동등한 게이트웨이 플러그인.
2. **STT는 입력 파서.** 마이크 → 텍스트.
3. **TTS는 출력 포매터.** 텍스트 → 스피커.
4. **VoiceFormatter는 음성 변환기.** 긴 텍스트를 요약해서 말로 할 수 있게.
5. **LLM은 커널에 있다.** 채널은 지능이 없다.
6. **모든 것이 내장.** 외부 API 없이 로컬 완결.
7. **Feature flag.** `cargo build --features voice`.

---

## 11. 내장 모델 관리 (커널 수준)

### 11.1 원칙

바이너리에 모델을 포함하지 않는다. 사이즈가 너무 크다:
- Supertonic 3 ONNX: ~200MB
- Whisper tiny: ~75MB, base: ~150MB
- 향후 추가될 수 있는 로컬 LLM: 1~16GB

대신 **최초 실행 시 다운로드 + 로컬 캐시** 방식을 사용한다.
Ollama, Whispree, Supertonic 자체도 같은 방식이다.

### 11.2 모델 저장소

```
~/.oxios/
├── config.toml
└── models/
    ├── stt/
    │   └── whisper-tiny.bin          # ~75MB
    ├── tts/
    │   └── supertonic-3/             # ~200MB
    │       ├── onnx/
    │       └── voice_styles/
    └── (향후) llm/
        └── qwen3-4b-4bit/           # ~2.1GB
```

### 11.3 모델 관리 모듈 (커널)

모델 관리는 voice 채널 전용이 아니다. 향후 다른 로컬 모델도 생길 수 있으므로
커널 수준에서 관리한다.

```
oxios-kernel/src/
└── model/
    ├── mod.rs           # ModelRegistry, ModelManager
    ├── manifest.rs      # ModelManifest (메타데이터)
    └── download.rs      # 다운로드 + 캐시 관리
```

```rust
/// 커널 수준 모델 관리자.
///
/// 모든 내장 모델의 라이프사이클을 관리한다.
/// 채널과 프로그램은 ModelManager를 통해 모델 경로를 얻는다.
pub struct ModelManager {
    models_dir: PathBuf,  // ~/.oxios/models/
    registry: HashMap<String, ModelManifest>,
}

/// 모델 메타데이터.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelManifest {
    /// 모델 식별자 (e.g. "voice/stt", "voice/tts").
    pub id: String,
    /// 모델 이름.
    pub name: String,
    /// 다운로드 소스 (Hugging Face repo).
    pub source: ModelSource,
    /// 모델 크기 (bytes).
    pub size_bytes: u64,
    /// 필요한 기능 (e.g. "voice 채널").
    pub required_for: Vec<String>,
    /// 체크섬 (무결성 검증).
    pub sha256: String,
    /// 버전.
    pub version: String,
}

/// 다운로드 소스.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelSource {
    /// Hugging Face 리포지토리.
    HuggingFace {
        repo: String,      // e.g. "Supertone/supertonic-3"
        revision: String,  // e.g. "main"
    },
    /// 직접 URL.
    Url(String),
}

impl ModelManager {
    /// 모델이 로컬에 있는지 확인.
    pub fn is_downloaded(&self, model_id: &str) -> bool;

    /// 모델 로컬 경로 반환. 없으면 None.
    pub fn model_path(&self, model_id: &str) -> Option<PathBuf>;

    /// 모델 다운로드.
    pub async fn download(&self, model_id: &str) -> Result<()>;

    /// 특정 기능에 필요한 모든 모델 반환.
    pub fn required_models(&self, feature: &str) -> Vec<&ModelManifest>;

    /// 설치된 모델 목록 + 디스크 사용량.
    pub fn list_installed(&self) -> Vec<ModelStatus>;

    /// 모델 삭제.
    pub async fn remove(&self, model_id: &str) -> Result<()>;
}
```

### 11.4 CLI 인터페이스

```bash
# 모델 관리
oxios model list                        # 설치된 모델 + 디스크 사용량
oxios model download voice              # 음성 채널 모델 일괄 다운
oxios model download voice/stt          # STT만
oxios model download voice/tts          # TTS만
oxios model remove voice/stt            # 개별 삭제
oxios model verify                      # 체크섬 무결성 검증
```

### 11.5 다운로드 동작 흐름

```
1. oxios 실행 → voice 채널 활성화
2. VoicePlugin::setup()
   → ModelManager::is_downloaded("voice/stt")?
   → ModelManager::is_downloaded("voice/tts")?
3. 없으면:
   ├─ 대화형: "음성 모델이 설치되지 않았습니다.
   │          다운로드하시겠습니까? [Y/n]"
   │          → ModelManager::download("voice/stt")
   │          → ModelManager::download("voice/tts")
   └─ 비대화형: 경고 로그 + 채널 비활성화
4. 이후부터는 로컬에서 즉시 로드
```

### 11.6 설정 (config.toml)

```toml
[models]
# 모델 저장 경로 (기본: ~/.oxios/models/)
dir = "~/.oxios/models"

# 다운로드 설정
download_on_demand = true    # 필요할 때 자동 다운
verify_checksum = true       # 다운로드 후 체크섬 검증
```

---

## 12. 참고 자료

- Supertonic 3: https://github.com/supertone-inc/supertonic (Rust SDK 포함)
- Whispree: https://github.com/Arsture/whispree (STT/LLM 교정 참고)
- whisper.cpp: https://github.com/ggerganov/whisper.cpp (whisper-rs)
- cpal: https://github.com/rustaudio/cpal
- rodio: https://github.com/RustAudio/rodio
