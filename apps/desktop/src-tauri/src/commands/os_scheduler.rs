//! OS-level scheduler integration so cron-driven automations fire even when
//! the Senda app is closed.
//!
//! macOS — LaunchAgent plist in `~/Library/LaunchAgents/`.
//! Linux — systemd user service + timer in `~/.config/systemd/user/`.
//! Windows — schtasks invocation. Phase 7-deferred (no test target locally).
//!
//! These commands install / uninstall a single "wake the scheduler" entry
//! that runs `senda --headless-tick` at a high frequency. The headless tick
//! re-uses the same scheduler crate to fire any due cron automation, then
//! exits. The user opts in from Settings; without explicit opt-in the cron
//! still works while the app is open as before.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OsSchedulerStatus {
    pub installed: bool,
    pub platform: &'static str,
    pub path: Option<String>,
}

const LABEL: &str = "app.senda.scheduler";

#[tauri::command]
pub async fn os_scheduler_status() -> Result<OsSchedulerStatus, String> {
    #[cfg(target_os = "macos")]
    return macos::status();

    #[cfg(target_os = "linux")]
    return linux::status();

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    Ok(OsSchedulerStatus {
        installed: false,
        platform: std::env::consts::OS,
        path: None,
    })
}

#[tauri::command]
pub async fn os_scheduler_install() -> Result<OsSchedulerStatus, String> {
    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;

    #[cfg(target_os = "macos")]
    return macos::install(&exe);

    #[cfg(target_os = "linux")]
    return linux::install(&exe);

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = exe;
        Err("OS-level scheduler not implemented for this platform yet".into())
    }
}

#[tauri::command]
pub async fn os_scheduler_uninstall() -> Result<OsSchedulerStatus, String> {
    #[cfg(target_os = "macos")]
    return macos::uninstall();

    #[cfg(target_os = "linux")]
    return linux::uninstall();

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    Err("OS-level scheduler not implemented for this platform yet".into())
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use std::path::PathBuf;
    use std::process::Command;

    fn plist_path() -> Result<PathBuf, String> {
        let home = dirs::home_dir().ok_or_else(|| "no home".to_string())?;
        Ok(home
            .join("Library")
            .join("LaunchAgents")
            .join(format!("{LABEL}.plist")))
    }

    pub fn status() -> Result<OsSchedulerStatus, String> {
        let path = plist_path()?;
        Ok(OsSchedulerStatus {
            installed: path.exists(),
            platform: "macos",
            path: Some(path.to_string_lossy().to_string()),
        })
    }

    pub fn install(exe: &std::path::Path) -> Result<OsSchedulerStatus, String> {
        let path = plist_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
        }

        // StartInterval = 60 — every minute the helper wakes the scheduler.
        let body = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{label}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{exe}</string>
    <string>--headless-tick</string>
  </array>
  <key>StartInterval</key>
  <integer>60</integer>
  <key>RunAtLoad</key>
  <false/>
  <key>StandardOutPath</key>
  <string>/tmp/senda-scheduler.log</string>
  <key>StandardErrorPath</key>
  <string>/tmp/senda-scheduler.log</string>
</dict>
</plist>
"#,
            label = LABEL,
            exe = exe.to_string_lossy(),
        );
        std::fs::write(&path, body).map_err(|e| format!("write plist: {e}"))?;

        // launchctl load is best-effort — the user can re-login if it fails.
        let _ = Command::new("launchctl")
            .args(["load", "-w"])
            .arg(&path)
            .status();

        Ok(OsSchedulerStatus {
            installed: true,
            platform: "macos",
            path: Some(path.to_string_lossy().to_string()),
        })
    }

    pub fn uninstall() -> Result<OsSchedulerStatus, String> {
        let path = plist_path()?;
        let _ = Command::new("launchctl")
            .args(["unload"])
            .arg(&path)
            .status();
        let _ = std::fs::remove_file(&path);
        Ok(OsSchedulerStatus {
            installed: false,
            platform: "macos",
            path: Some(path.to_string_lossy().to_string()),
        })
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use std::path::PathBuf;
    use std::process::Command;

    fn unit_dir() -> Result<PathBuf, String> {
        let home = dirs::home_dir().ok_or_else(|| "no home".to_string())?;
        Ok(home.join(".config").join("systemd").join("user"))
    }

    fn service_path() -> Result<PathBuf, String> {
        Ok(unit_dir()?.join(format!("{LABEL}.service")))
    }

    fn timer_path() -> Result<PathBuf, String> {
        Ok(unit_dir()?.join(format!("{LABEL}.timer")))
    }

    pub fn status() -> Result<OsSchedulerStatus, String> {
        let path = timer_path()?;
        Ok(OsSchedulerStatus {
            installed: path.exists(),
            platform: "linux",
            path: Some(path.to_string_lossy().to_string()),
        })
    }

    pub fn install(exe: &std::path::Path) -> Result<OsSchedulerStatus, String> {
        let dir = unit_dir()?;
        std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir: {e}"))?;

        let svc = format!(
            r#"[Unit]
Description=Senda headless scheduler tick

[Service]
Type=oneshot
ExecStart={exe} --headless-tick
"#,
            exe = exe.to_string_lossy(),
        );
        let timer = format!(
            r#"[Unit]
Description=Trigger Senda scheduler every minute

[Timer]
OnBootSec=1min
OnUnitActiveSec=1min
Unit={label}.service

[Install]
WantedBy=timers.target
"#,
            label = LABEL,
        );

        std::fs::write(service_path()?, svc).map_err(|e| format!("write service: {e}"))?;
        std::fs::write(timer_path()?, timer).map_err(|e| format!("write timer: {e}"))?;

        for args in [
            vec!["--user", "daemon-reload"],
            vec!["--user", "enable", "--now", &format!("{LABEL}.timer")],
        ] {
            let _ = Command::new("systemctl").args(&args).status();
        }

        status()
    }

    pub fn uninstall() -> Result<OsSchedulerStatus, String> {
        let _ = Command::new("systemctl")
            .args(["--user", "disable", "--now", &format!("{LABEL}.timer")])
            .status();
        let _ = std::fs::remove_file(timer_path()?);
        let _ = std::fs::remove_file(service_path()?);
        let _ = Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .status();
        status()
    }
}
