//! The snippet file: `~/.config/toolbelt/toolbelt.json`.

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const SCHEMA_URL: &str =
    "https://raw.githubusercontent.com/asger118/toolbelt/main/schema.json";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snippet {
    pub description: String,
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os: Option<Os>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Os {
    Windows,
    Linux,
}

impl Os {
    pub fn current() -> Option<Os> {
        match std::env::consts::OS {
            "windows" => Some(Os::Windows),
            "linux" => Some(Os::Linux),
            _ => None,
        }
    }
}

impl Snippet {
    /// True when the snippet has no `os`, or its `os` is the one we're running on.
    pub fn runs_here(&self) -> bool {
        self.os.is_none() || self.os == Os::current()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct File {
    #[serde(rename = "$schema", default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    #[serde(default)]
    pub snippets: Vec<Snippet>,
}

impl Default for File {
    fn default() -> Self {
        File { schema: Some(SCHEMA_URL.to_string()), snippets: Vec::new() }
    }
}

/// `~/.config/toolbelt/toolbelt.json` on every OS. `TOOLBELT_FILE` overrides it.
pub fn path() -> Result<PathBuf> {
    if let Some(p) = std::env::var_os("TOOLBELT_FILE") {
        return Ok(PathBuf::from(p));
    }
    let home = dirs::home_dir().ok_or_else(|| anyhow!("can't find your home folder"))?;
    Ok(home.join(".config").join("toolbelt").join("toolbelt.json"))
}

/// Parse snippet JSON. serde_json's errors already end with "at line X column Y".
pub fn parse(text: &str) -> Result<File> {
    serde_json::from_str(text).map_err(|e| anyhow!("{e}"))
}

/// Load the file, creating an empty one if it doesn't exist yet.
pub fn load_or_create(path: &Path) -> Result<File> {
    if !path.exists() {
        let file = File::default();
        save(path, &file)?;
        return Ok(file);
    }
    let text = fs::read_to_string(path).with_context(|| format!("can't read {}", path.display()))?;
    parse(&text).with_context(|| format!("{} is not valid", path.display()))
}

pub fn save(path: &Path, file: &File) -> Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("can't create {}", dir.display()))?;
    }
    let mut text = serde_json::to_string_pretty(file)?;
    text.push('\n');
    fs::write(path, text).with_context(|| format!("can't write {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_snippet() {
        let f = parse(r#"{"snippets":[{"description":"d","command":"c"}]}"#).unwrap();
        assert_eq!(f.snippets[0].help, None);
        assert_eq!(f.snippets[0].os, None);
    }

    #[test]
    fn rejects_unknown_fields_with_line_number() {
        let err = parse("{\"snippets\": [\n {\"description\":\"d\",\"command\":\"c\",\"tags\":[]}\n]}")
            .unwrap_err()
            .to_string();
        assert!(err.contains("line 2"), "{err}");
    }

    #[test]
    fn rejects_unknown_os() {
        assert!(parse(r#"{"snippets":[{"description":"d","command":"c","os":"mac"}]}"#).is_err());
    }

    #[test]
    fn round_trips_without_empty_fields() {
        let f = File {
            schema: None,
            snippets: vec![Snippet { description: "d".into(), command: "c".into(), help: None, os: None }],
        };
        let text = serde_json::to_string(&f).unwrap();
        assert_eq!(text, r#"{"snippets":[{"description":"d","command":"c"}]}"#);
    }

    #[test]
    fn os_filter() {
        let mut s = Snippet { description: "d".into(), command: "c".into(), help: None, os: None };
        assert!(s.runs_here());
        s.os = Os::current();
        assert!(s.runs_here());
        s.os = Some(if Os::current() == Some(Os::Windows) { Os::Linux } else { Os::Windows });
        assert!(!s.runs_here());
    }
}
