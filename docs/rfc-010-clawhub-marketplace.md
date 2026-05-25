# RFC-010: ClawHub 마켓플레이스 — OpenClaw 스킬과 공유

> **날짜**: 2026-05-25  
> **상태**: 초안  
> **범위**: ClawHub API 클라이언트, 스킬 설치/업데이트, Web UI 마켓플레이스 탭  
> **관련 RFC**: RFC-009 (Skill/Unification)  
> **참고**: https://clawhub.ai

---

## 1. 배경

### 현재 상태

RFC-009에서 Oxios는 OpenClaw 스킬 모델과 100% 호환되는 단일 Skill 시스템을 만들었습니다:

```yaml
# OpenClaw 스킬과 동일한 포맷
---
name: code-review
description: Deep code review...
requires:
  bins: ["git"]
  env: ["GITHUB_TOKEN"]
install:
  - kind: brew
    formula: git
---

# Markdown instructions...
```

**SKILL.md 파일은 수정 없이 그대로 사용 가능합니다.**

### 문제

1. **스킬을 직접 복사해야 함** — GitHub에서 내려받아 복사해야 함
2. **마켓플레이스가 없음** — ClawHub에 공개된 수천 개의 스킬을 탐색/설치할 방법 없음
3. **업데이트 관리 없음** — 설치된 스킬의 버전을 추적/업데이트할 방법 없음

### 해결책

ClawHub API를 통해:
- 스킬 검색 및 설치
- 설치된 스킬 버전 관리
- 마켓플레이스 UI 통합

---

## 2. 참고: ClawHub API 분석

### 2.1 API 엔드포인트

| 메서드 | 경로 | 설명 |
|--------|------|------|
| `GET` | `/api/v1/search?q=<query>` | 스킬/패키지 검색 |
| `GET` | `/api/v1/skills` | 스킬 목록 (페이지네이션) |
| `GET` | `/api/v1/skills/{slug}` | 스킬 상세 (버전 정보 포함) |
| `GET` | `/api/v1/download?slug=<slug>&version=<ver>` | 스킬 아카이브 다운로드 |
| `GET` | `/api/v1/packages/{name}/versions/{ver}/artifact` | 패키지 아티팩트解析 |

### 2.2 스킬 상세 응답

```json
{
  "skill": {
    "slug": "code-review-helper",
    "displayName": "Code Review Helper",
    "summary": "Automated code review...",
    "tags": { "category": "development" },
    "createdAt": 1704067200000,
    "updatedAt": 1715000000000
  },
  "latestVersion": {
    "version": "1.2.0",
    "createdAt": 1715000000000,
    "changelog": "Bug fixes..."
  },
  "metadata": {
    "os": ["darwin", "linux"],
    "systems": ["openclaw"]
  },
  "owner": {
    "handle": "alice",
    "displayName": "Alice",
    "image": "https://..."
  }
}
```

### 2.3 검색 응답

```json
{
  "results": [
    {
      "score": 0.95,
      "slug": "security-triage",
      "displayName": "Security Triage",
      "summary": "Triage security advisories...",
      "version": "2.0.0",
      "updatedAt": 1715000000000
    }
  ]
}
```

### 2.4 아카이브 형식

- 스킬 폴더를 zip으로 압축 (`<slug>.zip`)
- 루트 마커: `SKILL.md`, `skill.md`, `skills.md`
- 설치 시 `workspace/skills/<slug>/`에 압축 해제

### 2.5 오리진 추적

```json
// .clawhub/origin.json (스킬 디렉토리 내)
{
  "version": 1,
  "registry": "https://clawhub.ai",
  "slug": "code-review-helper",
  "installedVersion": "1.2.0",
  "installedAt": 1715000000000
}

// .clawhub/lock.json (workspace 루트)
{
  "version": 1,
  "skills": {
    "code-review-helper": {
      "version": "1.2.0",
      "installedAt": 1715000000000
    }
  }
}
```

---

## 3. 제안: ClawHub 클라이언트

### 3.1 모듈 구조

```
crates/oxios-kernel/src/
├── clawhub/
│   ├── mod.rs              # 공개 API
│   ├── client.rs           # HTTP 클라이언트
│   ├── types.rs            # API 타입
│   └── installer.rs        # 설치 로직
```

### 3.2 API 타입

```rust
// crates/oxios-kernel/src/clawhub/types.rs

/// ClawHub 검색 결과.
#[derive(Debug, Clone, Deserialize)]
pub struct ClawHubSearchResult {
    pub score: f64,
    pub slug: String,
    pub display_name: String,
    pub summary: Option<String>,
    pub version: Option<String>,
    pub updated_at: Option<i64>,
}

/// ClawHub 스킬 상세.
#[derive(Debug, Clone, Deserialize)]
pub struct ClawHubSkillDetail {
    pub skill: Option<ClawHubSkillMeta>,
    #[serde(default)]
    pub latest_version: Option<ClawHubVersion>,
    pub metadata: Option<ClawHubMetadata>,
    pub owner: Option<ClawHubOwner>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClawHubSkillMeta {
    pub slug: String,
    pub display_name: String,
    pub summary: Option<String>,
    pub tags: Option<HashMap<String, String>>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClawHubVersion {
    pub version: String,
    pub created_at: i64,
    pub changelog: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClawHubMetadata {
    pub os: Option<Vec<String>>,
    pub systems: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClawHubOwner {
    pub handle: Option<String>,
    pub display_name: Option<String>,
    pub image: Option<String>,
}
```

### 3.3 ClawHub 클라이언트

```rust
// crates/oxios-kernel/src/clawhub/client.rs

const DEFAULT_BASE_URL: &str = "https://clawhub.ai";

pub struct ClawHubClient {
    base_url: Url,
    client: reqwest::Client,
}

impl ClawHubClient {
    pub fn new(base_url: Option<String>) -> Self {
        let base = Url::parse(&base_url.unwrap_or(DEFAULT_BASE_URL.to_string()))
            .unwrap_or_else(|_| Url::parse(DEFAULT_BASE_URL).unwrap());
        Self {
            base_url: base,
            client: reqwest::Client::new(),
        }
    }

    /// Search skills by query.
    pub async fn search_skills(&self, query: &str, limit: Option<usize>) -> anyhow::Result<Vec<ClawHubSearchResult>> {
        let mut url = self.base_url.join("/api/v1/search")?;
        url.query_pairs_mut()
            .append_pair("q", query)
            .append_pair("limit", &limit.unwrap_or(20).to_string());

        let resp = self.client.get(url).send().await?;
        let body: SearchResponse = resp.json().await?;
        Ok(body.results)
    }

    /// Get skill detail.
    pub async fn get_skill(&self, slug: &str) -> anyhow::Result<ClawHubSkillDetail> {
        let url = self.base_url.join(&format!("/api/v1/skills/{}", slug))?;
        let resp = self.client.get(url).send().await?;
        let detail: ClawHubSkillDetail = resp.json().await?;
        Ok(detail)
    }

    /// Download skill archive.
    pub async fn download_skill(&self, slug: &str, version: Option<&str>) -> anyhow::Result<DownloadedArchive> {
        let mut url = self.base_url.join("/api/v1/download")?;
        url.query_pairs_mut()
            .append_pair("slug", slug);
        if let Some(v) = version {
            url.query_pairs_mut().append_pair("version", v);
        }

        let resp = self.client.get(url).send().await?;
        let bytes = resp.bytes().await?;

        let tmp = tempfile::tempfile()?;
        std::io::Write::write_all(&mut tmp, &bytes)?;
        Ok(DownloadedArchive { path: tmp })
    }
}
```

### 3.4 설치 관리자

```rust
// crates/oxios-kernel/src/clawhub/installer.rs

use std::fs::File;
use std::io::Read;
use zip::ZipArchive;

/// 설치 출처 메타데이터.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClawHubOrigin {
    pub version: u32,
    pub registry: String,
    pub slug: String,
    pub installed_version: String,
    pub installed_at: String,
}

/// lock.json 내용.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClawHubLockfile {
    pub version: u32,
    pub skills: HashMap<String, LockEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockEntry {
    pub version: String,
    pub installed_at: String,
}

impl ClawHubInstaller {
    /// ClawHub에서 스킬 설치.
    pub async fn install(&self, slug: &str, version: Option<&str>) -> anyhow::Result<InstallResult> {
        let client = self.client.as_ref();

        // 상세 정보 가져오기
        let detail = client.get_skill(slug).await?;
        let resolved_version = version.unwrap_or(
            detail.latest_version.as_ref()
                .map(|v| v.version.as_str())
                .unwrap_or("latest")
        );

        // 다운로드
        let archive = client.download_skill(slug, Some(resolved_version)).await?;

        // 압축 해제
        let target_dir = self.skills_dir.join(slug);
        if target_dir.exists() {
            anyhow::bail!("스킬이 이미 설치되어 있습니다: {}", slug);
        }

        std::fs::create_dir_all(&target_dir)?;
        self.extract_archive(&archive, &target_dir)?;

        // origin.json 작성
        let origin = ClawHubOrigin {
            version: 1,
            registry: client.base_url.to_string(),
            slug: slug.to_string(),
            installed_version: resolved_version.to_string(),
            installed_at: chrono::Utc::now().to_rfc3339(),
        };
        let origin_path = target_dir.join(".clawhub").join("origin.json");
        std::fs::create_dir_all(origin_path.parent().unwrap())?;
        std::fs::write(&origin_path, serde_json::to_string_pretty(&origin)?)?;

        // lock.json 업데이트
        self.update_lockfile(slug, resolved_version)?;

        Ok(InstallResult {
            ok: true,
            slug: slug.to_string(),
            version: resolved_version.to_string(),
            target_dir,
        })
    }

    /// 설치된 스킬 업데이트.
    pub async fn update(&self, slug: &str) -> anyhow::Result<UpdateResult> {
        let client = self.client.as_ref();

        // 현재 버전 확인
        let current = self.get_installed_version(slug)?;
        let detail = client.get_skill(slug).await?;
        let latest = detail.latest_version.as_ref()
            .map(|v| v.version.as_str())
            .unwrap_or("latest");

        if current.as_deref() == Some(latest) {
            return Ok(UpdateResult {
                ok: true,
                slug: slug.to_string(),
                previous_version: current.clone(),
                version: latest.to_string(),
                changed: false,
            });
        }

        // 재설치 (force)
        let archive = client.download_skill(slug, Some(latest)).await?;
        let target_dir = self.skills_dir.join(slug);

        if target_dir.exists() {
            std::fs::remove_dir_all(&target_dir)?;
        }
        std::fs::create_dir_all(&target_dir)?;
        self.extract_archive(&archive, &target_dir)?;

        // origin.json 업데이트
        let origin = ClawHubOrigin {
            version: 1,
            registry: client.base_url.to_string(),
            slug: slug.to_string(),
            installed_version: latest.to_string(),
            installed_at: chrono::Utc::now().to_rfc3339(),
        };
        let origin_path = target_dir.join(".clawhub").join("origin.json");
        std::fs::write(&origin_path, serde_json::to_string_pretty(&origin)?)?;
        self.update_lockfile(slug, latest)?;

        Ok(UpdateResult {
            ok: true,
            slug: slug.to_string(),
            previous_version: current,
            version: latest.to_string(),
            changed: true,
        })
    }

    /// 모든 설치된 ClawHub 스킬 업데이트.
    pub async fn update_all(&self) -> anyhow::Result<Vec<UpdateResult>> {
        let lock = self.read_lockfile()?;
        let mut results = Vec::new();

        for (slug, entry) in lock.skills {
            let result = self.update(&slug).await
                .unwrap_or_else(|e| UpdateResult {
                    ok: false,
                    slug,
                    previous_version: Some(entry.version),
                    version: String::new(),
                    changed: false,
                    error: Some(e.to_string()),
                });
            results.push(result);
        }

        Ok(results)
    }

    /// 검색.
    pub async fn search(&self, query: &str) -> anyhow::Result<Vec<ClawHubSearchResult>> {
        self.client.search_skills(query, None).await
    }
}
```

---

## 4. API 엔드포인트

### 4.1 `/api/marketplace/search`

```json
// GET /api/marketplace/search?q=code+review&limit=20
{
  "results": [
    {
      "slug": "code-review-helper",
      "displayName": "Code Review Helper",
      "summary": "Automated code review...",
      "version": "1.2.0",
      "owner": {
        "handle": "alice",
        "displayName": "Alice"
      }
    }
  ],
  "total": 1
}
```

### 4.2 `/api/marketplace/skills`

```json
// GET /api/marketplace/skills
{
  "skills": [
    {
      "slug": "code-review-helper",
      "displayName": "Code Review Helper",
      "summary": "Automated code review...",
      "version": "1.2.0",
      "updatedAt": 1715000000000,
      "tags": { "category": "development" },
      "os": ["darwin", "linux"]
    }
  ],
  "nextCursor": "..."
}
```

### 4.3 `/api/marketplace/skills/{slug}`

```json
// GET /api/marketplace/skills/code-review-helper
{
  "slug": "code-review-helper",
  "displayName": "Code Review Helper",
  "summary": "Automated code review...",
  "version": "1.2.0",
  "changelog": "Bug fixes...",
  "os": ["darwin", "linux"],
  "owner": {
    "handle": "alice",
    "displayName": "Alice"
  }
}
```

### 4.4 `/api/marketplace/skills/{slug}/install`

```json
// POST /api/marketplace/skills/code-review-helper/install
// Body: { "version": "1.2.0" } (선택, 없으면 latest)

{
  "ok": true,
  "slug": "code-review-helper",
  "version": "1.2.0",
  "skill": { ... full skill entry ... }
}
```

### 4.5 `/api/marketplace/updates`

```json
// GET /api/marketplace/updates
{
  "updates": [
    {
      "slug": "code-review-helper",
      "currentVersion": "1.1.0",
      "latestVersion": "1.2.0",
      "changelog": "Bug fixes..."
    }
  ]
}
```

---

## 5. Web UI — 마켓플레이스 탭

### 5.1 레이아웃

```
Marketplace
┌─────────────────────────────────────────────────────────────┐
│ [🔍 검색...]                              [Filters ▼]       │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│ ┌───────────────────────────────────────────────────────┐  │
│ │ 🔍 code-review-helper                    v1.2.0      │  │
│ │    Automated code review with AI     by alice · 2d ago  │  │
│ │    OS: darwin, linux                                  │  │
│ │    Tags: development, code-review                      │  │
│ │                                                       │  │
│ │    [Install]                                          │  │
│ └───────────────────────────────────────────────────────┘  │
│                                                             │
│ ┌───────────────────────────────────────────────────────┐  │
│ │ 🛡️ security-triage                     v2.0.0      │  │
│ │    Triage security advisories     by openclaw · 5d    │  │
│ │    OS: darwin, linux                                  │  │
│ │    Tags: security, automation                         │  │
│ │                                                       │  │
│ │    [Install]                                          │  │
│ └───────────────────────────────────────────────────────┘  │
│                                                             │
│ ┌───────────────────────────────────────────────────────┐  │
│ │ 📊 metrics-dashboard                   v1.5.0      │  │
│ │    Monitor agent metrics...        by datadog · 1w    │  │
│ │    OS: linux                                        │  │
│ │                                                       │  │
│ │    [Install]                                          │  │
│ └───────────────────────────────────────────────────────┘  │
│                                                             │
│ ─────────────────────────  Load More  ────────────────────  │
└─────────────────────────────────────────────────────────────┘
```

### 5.2 설치된 스킬과 연동

```
Skills 탭에 [🔍 마켓플레이스] 버튼

설치된 스킬 카드에:
  [↑ Update available: 1.1.0 → 1.2.0] (업데이트 있음)
  [↗ ClawHub: @alice/code-review-helper] (출처 링크)
```

### 5.3 설치 대화상자

```
┌─ Install Code Review Helper ────────────────────────┐
│                                                     │
│  v1.2.0 (latest)                            by alice │
│                                                     │
│  Automated code review with AI analysis.          │
│                                                     │
│  Requirements check:                               │
│    bins     git ✅  gh ✅                          │
│    env      GITHUB_TOKEN ⚠️ (설정 필요)            │
│                                                     │
│  ─────────────────────────────────────              │
│                                                     │
│  [Cancel]                        [Install → SKILL] │
│                                                     │
│  ⚠️ GITHUB_TOKEN is required.                      │
│     Set it in ~/.oxios/config.toml:                │
│     [skills.entries.code-review-helper]             │
│     env.GITHUB_TOKEN = "ghp_xxx"                    │
└─────────────────────────────────────────────────────┘
```

---

## 6. CLI 명령어

```bash
oxios marketplace search <query>     # 검색
oxios marketplace install <slug>    # 설치
oxios marketplace update [<slug>]   # 업데이트 (전체 또는 단일)
oxios marketplace list               # ClawHub 스킬 목록
oxios marketplace info <slug>       # 스킬 상세

oxios skill search <query>          # alias
oxios skill install <slug>          # alias
oxios skill update [<slug>]         # alias
```

---

## 7. 구현 단계

### Phase 1: ClawHub 클라이언트 (2일)

- [ ] `crates/oxios-kernel/src/clawhub/mod.rs` — 모듈 선언
- [ ] `crates/oxios-kernel/src/clawhub/types.rs` — API 타입
- [ ] `crates/oxios-kernel/src/clawhub/client.rs` — HTTP 클라이언트
- [ ] `crates/oxios-kernel/src/clawhub/installer.rs` — 설치/업데이트
- [ ] 단위 테스트

### Phase 2: Kernel 통합 (1일)

- [ ] `KernelHandle`에 `MarketplaceApi` 추가
- [ ] `kernel_bridge.rs`에 `MarketplaceTool` 등록
- [ ] CLI 명령어 (`src/main.rs`)

### Phase 3: Backend API (1일)

- [ ] `/api/marketplace/search`
- [ ] `/api/marketplace/skills`
- [ ] `/api/marketplace/skills/{slug}`
- [ ] `/api/marketplace/skills/{slug}/install`
- [ ] `/api/marketplace/updates`

### Phase 4: Web UI (2일)

- [ ] 마켓플레이스 탭 (routes/marketplace/)
- [ ] 검색 + 필터 UI
- [ ] 설치 대화상자
- [ ] 사이드바에 마켓플레이스 링크 추가

### Phase 5: 퍼블리시 (선택, 나중에)

- [ ] `oxios publish` 명령 (스킬 폴더 → ClawHub)
- [ ] API 키 관리

---

## 8. 체크리스트

### Backend

- [ ] ClawHub 타입 정의
- [ ] ClawHub 클라이언트 구현
- [ ] ClawHubInstaller 구현 (install, update, update_all)
- [ ] KernelHandle::marketplace_api()
- [ ] `/api/marketplace/*` 엔드포인트
- [ ] `cargo check -p oxios-kernel` 통과

### Frontend

- [ ] Marketplace 탭
- [ ] 검색 UI
- [ ] 설치 대화상자
- [ ] 사이드바 통합
- [ ] `bun run build` 통과

### CLI

- [ ] `oxios marketplace search`
- [ ] `oxios marketplace install`
- [ ] `oxios marketplace update`

### Docs

- [ ] AGENTS.md에 ClawHub 클라이언트 추가
- [ ] `docs/marketplace.md` 작성

---

## 9. 리스크

| 리스크 | 대응 |
|--------|------|
| ClawHub API Rate Limit | 토큰 인증 시 더 높은 제한, 캐싱 |
| 네트워크 오류 | 설치 실패 시 graceful error, retry |
| 비순차 압축 해제 | zip 마커로 루트 디렉토리 자동 감지 |
| 스킬 충돌 (동일 이름) | workspace가 managed보다 우선 — workspace 설치 시 경고 |
| 악성 스킬 | ClawHub는 보안 검사를 통과한 것만 제공, 추가 검증 불필요 |

---

## 10. 관계도

```
┌─────────────────────────────────────────────────────────────┐
│  User                                                       │
│    ├── Web UI: /marketplace                                   │
│    │     └── 마켓플레이스 탭 (검색, 설치, 업데이트)           │
│    │                                                          │
│    └── CLI: oxios marketplace install <slug>                  │
│          └── MarketplaceApi                                   │
│                                                             │
│  ClawHub (clawhub.ai)                                        │
│    ├── /api/v1/search?q=...                                  │
│    ├── /api/v1/skills/{slug}                                 │
│    ├── /api/v1/download?slug=...&version=...                │
│    └── 1000+ 스킬                                             │
│                                                             │
│  Oxios Kernel                                                │
│    └── ClawHubClient → ClawHubInstaller → ~/.oxios/skills/  │
│          └── SkillManager (로컬 스킬과 동일 취급)            │
└─────────────────────────────────────────────────────────────┘
```

**핵심: ClawHub에서 설치된 스킬은 SkillManager가 로컬 스킬과 동일하게 취급합니다.** 프롬프트 주입, requirements 평가, eligibility 체크 — 전부 동일하게 작동합니다.