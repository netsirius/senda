//! Cross-platform "reveal in file manager" Tauri command. Used by AgentDetail,
//! RepoDetail, MCPs page, and the History view to give the user a one-click
//! jump from a path string to the actual file in Finder/Explorer/Files.

use std::path::PathBuf;
use std::process::Command;

#[tauri::command]
pub async fn reveal_in_finder(path: String) -> Result<(), String> {
    let path = PathBuf::from(&path);
    if !path.exists() {
        return Err(format!("path not found: {}", path.display()));
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg("-R")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("open -R: {e}"))?;
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg("/select,")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("explorer: {e}"))?;
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        // xdg-open opens the *containing directory* — best we can do without
        // assuming a specific file manager.
        let dir = if path.is_dir() {
            path.clone()
        } else {
            path.parent()
                .map(|p| p.to_path_buf())
                .unwrap_or(path.clone())
        };
        Command::new("xdg-open")
            .arg(&dir)
            .spawn()
            .map_err(|e| format!("xdg-open: {e}"))?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err("unsupported platform".into())
}
