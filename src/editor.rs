//! Opening the snippet file in an editor for `tb edit`.

use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;

/// `$VISUAL`, `$EDITOR`, VS Code, then notepad (Windows) or nano/vi (Linux).
pub fn open(file: &Path) -> Result<()> {
    let editor = ["VISUAL", "EDITOR"]
        .iter()
        .filter_map(|v| std::env::var(v).ok())
        .find(|v| !v.trim().is_empty());

    let status = match editor {
        Some(e) => shell_command(&e, file).status(),
        None if find_in_path("code").is_some() => shell_command("code --wait", file).status(),
        None if cfg!(windows) => Command::new("notepad").arg(file).status(),
        None => {
            let fallback = if find_in_path("nano").is_some() { "nano" } else { "vi" };
            Command::new(fallback).arg(file).status()
        }
    }
    .context("couldn't start an editor (set $EDITOR to choose one)")?;

    if !status.success() {
        bail!("the editor exited with {status}");
    }
    Ok(())
}

/// Run `editor <file>` through the shell, so `$EDITOR` may contain arguments.
#[cfg(windows)]
fn shell_command(editor: &str, file: &Path) -> Command {
    use std::os::windows::process::CommandExt;
    let mut c = Command::new("cmd");
    // /S strips exactly the outer quotes, so quoted editor paths survive.
    c.raw_arg(format!("/S /C \"{editor} \"{}\"\"", file.display()));
    c
}

#[cfg(unix)]
fn shell_command(editor: &str, file: &Path) -> Command {
    let mut c = Command::new("sh");
    c.arg("-c").arg(format!("{editor} \"$1\"")).arg("sh").arg(file);
    c
}

fn find_in_path(name: &str) -> Option<PathBuf> {
    let exts: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".EXE;.CMD;.BAT".into())
            .split(';')
            .map(str::to_string)
            .collect()
    } else {
        vec![String::new()]
    };
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).find_map(|dir| {
        exts.iter().map(|e| dir.join(format!("{name}{e}"))).find(|p| p.is_file())
    })
}
