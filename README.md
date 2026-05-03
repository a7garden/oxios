# Oxios

> *"Do one thing well. Specify before you build. Evolve, don't repeat."*

Agent Operating System built in Rust. Unix philosophy meets Ouroboros spec-first methodology.

## What

Oxios는 인간의 허접한 의도를 명확한 명세로 변환하고, 그 명세에 따라 에이전트를 자동으로 생성/실행/검증/종료하는 Agent OS다.

## Architecture

```
Gateway (channel-agnostic) → Kernel (supervisor + ouroboros + oxi-agent) → Container Garden
```

- **Gateway** — Web, CLI, Telegram 등 어떤 채널이든 연결 가능한 메시지 허브
- **Kernel** — 에이전트 생명주기 관리 + Ouroboros spec-first protocol
- **Engine** — oxi-ai + oxi-agent (pi2oxi 의존성, 재구현 없음)
- **Container** — Apple Container로 격리된 실행 환경

## Status

🚧 설계 완료, 구현 진행 중

## Docs

- [DESIGN.md](DESIGN.md) — 전체 설계 문서

## License

MIT
