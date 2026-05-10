# Channel Registry

## Problem

`main.rs` directly imported and constructed each channel (WebChannel, WebServer).
Adding a new channel required modifying main.rs — violating Open/Closed.
Web was a mandatory compile-time dependency even for headless deployments.

## Solution

### ChannelPlugin Trait

Each channel implements `ChannelPlugin` (defined in `oxios-gateway`):

- `name()` — returns the channel identifier
- `setup(ChannelContext)` → `ChannelBundle` — creates the channel and optional background tasks

### ChannelContext

Shared context provided to all plugins:
- `kernel: Arc<KernelHandle>` — access to all kernel subsystems
- `config: Arc<RwLock<OxiosConfig>>` — hot-reloadable config
- `config_path: PathBuf` — for persistence operations

### ChannelBundle

Result of plugin setup:
- `channel: Box<dyn Channel>` — registered with the gateway
- `tasks: Vec<JoinHandle<()>>` — background tasks (servers, loops)

### Feature Flags

Channels are optional at compile time:
- `cargo build` — web + cli (default)
- `cargo build --no-default-features --features web` — web only
- `cargo build --features telegram` — web + cli + telegram

### Configuration

```toml
[channels]
enabled = ["web"]

[channels.telegram]
bot_token_env = "TELEGRAM_BOT_TOKEN"
allowed_users = []
```

## Architecture

```
main.rs
  └─ build_channel_plugins() → [WebPlugin, CliPlugin, TelegramPlugin]
  └─ for name in config.channels.enabled:
       plugin.setup(ctx) → ChannelBundle
       gateway.register(bundle.channel)
       spawn(bundle.tasks)
  └─ gateway.run()
```

## Migration

| Before | After |
|--------|-------|
| main.rs imports WebChannel, WebServer | main.rs uses ChannelPlugin trait |
| Web always compiled in | Feature flag: `web` (default on) |
| Telegram in workspace but not linked | Feature flag: `telegram` (default off) |
| Port check in main.rs | Plugin handles bind errors |
| Static files path hardcoded in main.rs | Plugin uses its own CARGO_MANIFEST_DIR |
| axum/tower-http deps in root Cargo.toml | Only in oxios-web Cargo.toml |
