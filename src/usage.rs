//! Remembers when each snippet was last picked, keyed by description.
//! Lives outside the config folder so it doesn't show up as chezmoi noise.

use crate::store::Snippet;
use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub type Usage = HashMap<String, u64>;

/// `%LOCALAPPDATA%\toolbelt\usage.json` on Windows, `~/.local/state/toolbelt/usage.json` on Linux.
pub fn path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("TOOLBELT_USAGE_FILE") {
        return Some(PathBuf::from(p));
    }
    let base = dirs::state_dir().or_else(dirs::data_local_dir)?;
    Some(base.join("toolbelt").join("usage.json"))
}

/// Missing or broken usage data is never fatal; it just means no recent-first ordering.
pub fn load() -> Usage {
    path()
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

/// Record a pick and drop entries for snippets that no longer exist.
pub fn record(picked: &str, snippets: &[Snippet]) -> Result<()> {
    let Some(path) = path() else { return Ok(()) };
    let mut usage = load();
    usage.retain(|d, _| snippets.iter().any(|s| &s.description == d));
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
    usage.insert(picked.to_string(), now);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, serde_json::to_string(&usage)?)?;
    Ok(())
}

/// Indices of `snippets` in display order: recently used first, then file order.
pub fn order(snippets: &[Snippet], usage: &Usage) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..snippets.len()).collect();
    // Stable sort keeps file order among unused (and equally timed) snippets.
    idx.sort_by_key(|&i| std::cmp::Reverse(usage.get(&snippets[i].description).copied().unwrap_or(0)));
    idx
}

/// Every whitespace-separated word of `query` must appear in the description or command.
pub fn matches(s: &Snippet, query: &str) -> bool {
    let hay = format!("{}\n{}", s.description, s.command).to_lowercase();
    query.to_lowercase().split_whitespace().all(|w| hay.contains(w))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(d: &str, c: &str) -> Snippet {
        Snippet { description: d.into(), command: c.into(), help: None, os: None }
    }

    #[test]
    fn recent_first_then_file_order() {
        let list = vec![s("a", ""), s("b", ""), s("c", ""), s("d", "")];
        let usage = Usage::from([("c".to_string(), 5), ("b".to_string(), 9)]);
        assert_eq!(order(&list, &usage), vec![1, 2, 0, 3]);
    }

    #[test]
    fn match_words_anywhere_case_insensitive() {
        let x = s("Show folder sizes in GB", "powershell -File show-folder-sizes.ps1");
        assert!(matches(&x, ""));
        assert!(matches(&x, "fold siz"));
        assert!(matches(&x, "GB powershell"));
        assert!(!matches(&x, "shfs"));
        assert!(!matches(&x, "folder kubectl"));
    }
}
