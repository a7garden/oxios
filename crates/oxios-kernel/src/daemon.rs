//! Daemon lifecycle management — PID file, start/stop, system service install.
//!
//! On macOS: launchd (`~/Library/LaunchAgents/com.a7garden.oxios.plist`)
//! On Linux: systemd (`/etc/systemd/system/oxiosd.service`)

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Daemon status.
#[derive(Debug, Clone)]
pub enum DaemonStatus {
    /// Daemon is running.
    Running { /// Process ID.
    pid: u32 },
    /// PID file exists but process is dead (stale).
    Stale { /// Process ID of the dead process.
    pid: u32 },
    /// Daemon is not running.
    Stopped,
}

impl std::fmt::Display for DaemonStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DaemonStatus::Running { pid } => write!(f, "running (PID {})", pid),
            DaemonStatus::Stale { pid } => write!(f, "stale (PID {} dead)", pid),
            DaemonStatus::Stopped => write!(f, "stopped"),
        }
    }
}

/// Manages the oxios background daemon.
pub struct DaemonManager {
    pid_file: PathBuf,
    log_dir: PathBuf,
}

impl DaemonManager {
    /// Create a daemon manager from config paths.
    pub fn new(pid_file: &str, log_dir: &str) -> Self {
        Self {
            pid_file: crate::config::expand_home(pid_file),
            log_dir: crate::config::expand_home(log_dir),
        }
    }

    /// Check daemon status by reading the PID file.
    pub fn status(&self) -> DaemonStatus {
        match self.read_pid() {
            Some(pid) => {
                if self.is_alive(pid) {
                    DaemonStatus::Running { pid }
                } else {
                    DaemonStatus::Stale { pid }
                }
            }
            None => DaemonStatus::Stopped,
        }
    }

    /// Start the daemon in the background.
    pub fn start(&self, config_path: &Path) -> Result<()> {
        match self.status() {
            DaemonStatus::Running { pid } => {
                anyhow::bail!("oxios is already running (PID {})", pid);
            }
            DaemonStatus::Stale { .. } => {
                self.cleanup()?;
            }
            DaemonStatus::Stopped => {}
        }

        // Ensure log directory exists
        std::fs::create_dir_all(&self.log_dir).context("failed to create log directory")?;

        let log_file = self.log_dir.join("oxios.log");
        let exe = std::env::current_exe().context("failed to locate oxios binary")?;

        let child = std::process::Command::new(&exe)
            .arg("--foreground")
            .arg("--config")
            .arg(config_path)
            .stdout(std::fs::File::create(&log_file)?)
            .stderr(std::fs::File::create(&log_file)?)
            .spawn()
            .context("failed to spawn oxios daemon")?;

        let pid = child.id();
        self.write_pid(pid)?;

        println!("⬡ oxios started (PID {})", pid);
        println!("  Logs: {}", log_file.display());
        println!("  Dashboard: http://127.0.0.1:4200");
        Ok(())
    }

    /// Stop the daemon by sending SIGTERM.
    pub fn stop(&self) -> Result<()> {
        match self.status() {
            DaemonStatus::Running { pid } => {
                #[cfg(unix)]
                {
                    let ret = unsafe { libc::kill(pid as i32, libc::SIGTERM) };
                    if ret != 0 {
                        anyhow::bail!("failed to send SIGTERM to PID {}", pid);
                    }
                }
                #[cfg(not(unix))]
                {
                    // On non-Unix, just kill the process
                    let _ = std::process::Command::new("taskkill")
                        .args(["/PID", &pid.to_string(), "/F"])
                        .output();
                }

                // Wait briefly for process to die
                for _ in 0..10 {
                    std::thread::sleep(std::time::Duration::from_millis(200));
                    if !self.is_alive(pid) {
                        break;
                    }
                }

                self.cleanup()?;
                println!("⬡ oxios stopped");
                Ok(())
            }
            DaemonStatus::Stale { .. } => {
                self.cleanup()?;
                println!("⬡ cleaned up stale PID file");
                Ok(())
            }
            DaemonStatus::Stopped => {
                println!("⬡ oxios is not running");
                Ok(())
            }
        }
    }

    /// Restart the daemon.
    pub fn restart(&self, config_path: &Path) -> Result<()> {
        if matches!(self.status(), DaemonStatus::Running { .. }) {
            self.stop()?;
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        self.start(config_path)
    }

    /// Install as a system service (launchd on macOS, systemd on Linux).
    pub fn install_service(&self) -> Result<()> {
        let exe = std::env::current_exe().context("failed to locate oxios binary")?;

        #[cfg(target_os = "macos")]
        {
            let plist_dir = dirs::home_dir()
                .map(|h| h.join("Library/LaunchAgents"))
                .context("failed to locate LaunchAgents directory")?;
            std::fs::create_dir_all(&plist_dir)?;
            let plist_path = plist_dir.join("com.a7garden.oxios.plist");

            let home = dirs::home_dir().context("failed to get HOME")?;
            let log_path = self.log_dir.join("oxiosd.log");

            let plist = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.a7garden.oxios</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe}</string>
        <string>--foreground</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>{log}</string>
    <key>StandardErrorPath</key>
    <string>{log}</string>
    <key>WorkingDirectory</key>
    <string>{home}</string>
</dict>
</plist>
"#,
                exe = exe.display(),
                log = log_path.display(),
                home = home.display(),
            );

            std::fs::write(&plist_path, &plist)?;
            println!("✓ Installed launchd service");
            println!("  {}", plist_path.display());
            println!();
            println!("  Start with:   launchctl load {}", plist_path.display());
            println!("  Stop with:    launchctl unload {}", plist_path.display());
            println!("  Or simply:    oxios start / oxios stop");
        }

        #[cfg(target_os = "linux")]
        {
            let unit_dir = PathBuf::from("/etc/systemd/system");
            let unit_path = unit_dir.join("oxiosd.service");

            let unit = format!(
                r#"[Unit]
Description=Oxios Agent Operating System
After=network.target

[Service]
Type=simple
ExecStart={exe} --foreground
Restart=on-failure
RestartSec=5s

[Install]
WantedBy=multi-user.target
"#,
                exe = exe.display(),
            );

            // Try to write — may fail without sudo
            if let Err(e) = std::fs::write(&unit_path, &unit) {
                anyhow::bail!(
                    "Failed to write {} — run with sudo: {}",
                    unit_path.display(),
                    e
                );
            }

            println!("✓ Installed systemd service");
            println!("  {}", unit_path.display());
            println!();
            println!("  Reload:  sudo systemctl daemon-reload");
            println!("  Start:   sudo systemctl start oxiosd");
            println!("  Enable:  sudo systemctl enable oxiosd");
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            anyhow::bail!("daemon install only supported on macOS and Linux");
        }

        Ok(())
    }

    /// Uninstall the system service.
    pub fn uninstall_service(&self) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            let plist_path = dirs::home_dir()
                .map(|h| h.join("Library/LaunchAgents/com.a7garden.oxios.plist"))
                .context("failed to locate plist")?;

            if plist_path.exists() {
                std::fs::remove_file(&plist_path)?;
                println!("✓ Removed launchd service");
            } else {
                println!("  Service not installed");
            }
        }

        #[cfg(target_os = "linux")]
        {
            let unit_path = PathBuf::from("/etc/systemd/system/oxiosd.service");
            if unit_path.exists() {
                if let Err(e) = std::fs::remove_file(&unit_path) {
                    anyhow::bail!(
                        "Failed to remove {} — run with sudo: {}",
                        unit_path.display(),
                        e
                    );
                }
                println!("✓ Removed systemd service");
            } else {
                println!("  Service not installed");
            }
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            anyhow::bail!("daemon uninstall only supported on macOS and Linux");
        }

        Ok(())
    }

    // ── Internal helpers ──

    fn read_pid(&self) -> Option<u32> {
        let content = std::fs::read_to_string(&self.pid_file).ok()?;
        content.trim().parse().ok()
    }

    fn write_pid(&self, pid: u32) -> Result<()> {
        if let Some(parent) = self.pid_file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.pid_file, pid.to_string())?;
        Ok(())
    }

    fn cleanup(&self) -> Result<()> {
        if self.pid_file.exists() {
            std::fs::remove_file(&self.pid_file)?;
        }
        Ok(())
    }

    fn is_alive(&self, pid: u32) -> bool {
        #[cfg(unix)]
        {
            // Signal 0 = check if process exists
            unsafe { libc::kill(pid as i32, 0) == 0 }
        }
        #[cfg(not(unix))]
        {
            // On non-Unix, always return false (conservative)
            let _ = pid;
            false
        }
    }
}
