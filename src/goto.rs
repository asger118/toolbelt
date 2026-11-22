//! `tb goto <name>`: find a folder under your repo roots by name, forgiving small typos.
//! The shell function from `tb init` cd's into the printed path.

use std::fs;
use std::path::{Path, PathBuf};

/// How many folders deep to look below a root (`repos/bachelor/bachelor-docs` is 2).
const MAX_DEPTH: usize = 3;
/// Never listed or searched: build output and package caches.
const SKIP: &[&str] = &["node_modules", "target", "bin", "obj", "dist", "build", "out", "vendor"];

#[derive(Debug, Clone, PartialEq)]
pub struct Dir {
    /// Path below the root with `/` separators, e.g. `bachelor/bachelor-docs`.
    pub rel: String,
    pub path: PathBuf,
}

impl Dir {
    fn name(&self) -> &str {
        self.rel.rsplit('/').next().unwrap_or(&self.rel)
    }
}

/// `TOOLBELT_GOTO_ROOTS` (a PATH-style list) if set, else `~/repos`, plus `C:\repos` on Windows.
pub fn roots() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = match std::env::var_os("TOOLBELT_GOTO_ROOTS") {
        Some(v) => std::env::split_paths(&v).collect(),
        None => {
            let mut r: Vec<PathBuf> = dirs::home_dir().map(|h| h.join("repos")).into_iter().collect();
            if cfg!(windows) {
                let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
                r.push(PathBuf::from(format!("{drive}\\repos")));
            }
            r
        }
    };
    roots.retain(|r| r.is_dir());
    roots.dedup();
    roots
}

/// Every folder up to `MAX_DEPTH` below the roots, sorted by path. Hidden folders and build
/// output are skipped, and git repos aren't descended into, so their `src/` etc. stay out.
pub fn scan(roots: &[PathBuf]) -> Vec<Dir> {
    let mut out = Vec::new();
    for root in roots {
        walk(root, "", 1, &mut out);
    }
    out.sort_by(|a, b| a.rel.to_lowercase().cmp(&b.rel.to_lowercase()));
    out
}

fn walk(dir: &Path, prefix: &str, depth: usize, out: &mut Vec<Dir>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        if !e.file_type().is_ok_and(|t| t.is_dir()) {
            continue; // also skips symlinks/junctions, so no loops
        }
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || name.starts_with('$') || SKIP.contains(&name.to_lowercase().as_str()) {
            continue;
        }
        let rel = if prefix.is_empty() { name } else { format!("{prefix}/{name}") };
        let path = e.path();
        if depth < MAX_DEPTH && !path.join(".git").exists() {
            walk(&path, &rel, depth + 1, out);
        }
        out.push(Dir { rel, path });
    }
}

/// Indices of the best matches for `query`, best first. Several results means it's ambiguous.
///
/// Tiers, best first: exact folder name, name starts with it, name (or with several words,
/// the whole path) contains it, then name within a few typos of it. Only the best tier
/// that has any match is returned.
pub fn find(dirs: &[Dir], query: &str) -> Vec<usize> {
    let q = query.trim().to_lowercase().replace('\\', "/");
    if q.is_empty() {
        return (0..dirs.len()).collect();
    }
    let words: Vec<&str> = q.split(|c: char| c.is_whitespace() || c == '/').filter(|w| !w.is_empty()).collect();
    let max_typos = match q.chars().count() {
        0..=4 => 1,
        5..=8 => 2,
        _ => 3,
    };

    let mut best: Option<(usize, usize)> = None;
    let mut hits: Vec<(usize, (usize, usize))> = Vec::new();
    for (i, d) in dirs.iter().enumerate() {
        let name = d.name().to_lowercase();
        let rel = d.rel.to_lowercase();
        let score = if words.len() > 1 {
            words.iter().all(|w| rel.contains(w)).then_some((2, 0))
        } else if name == q {
            Some((0, 0))
        } else if name.starts_with(&q) {
            Some((1, 0))
        } else if name.contains(&q) {
            Some((2, 0))
        } else {
            let d = typos(&q, &name);
            (d <= max_typos).then_some((3, d))
        };
        if let Some(s) = score {
            best = Some(best.map_or(s, |b| b.min(s)));
            hits.push((i, s));
        }
    }
    let Some(best) = best else { return Vec::new() };
    let hits: Vec<usize> = hits.into_iter().filter(|&(_, s)| s == best).map(|(i, _)| i).collect();
    // A folder inside another match only matched because of its parent's name: drop it.
    hits.iter()
        .copied()
        .filter(|&i| !hits.iter().any(|&j| j != i && dirs[i].path.starts_with(&dirs[j].path)))
        .collect()
}

/// Edit distance where swapping two neighbouring letters counts as one typo.
fn typos(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut d = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i;
    }
    for j in 0..=b.len() {
        d[0][j] = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            d[i][j] = (d[i - 1][j] + 1).min(d[i][j - 1] + 1).min(d[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                d[i][j] = d[i][j].min(d[i - 2][j - 2] + 1);
            }
        }
    }
    d[a.len()][b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dirs(rels: &[&str]) -> Vec<Dir> {
        rels.iter().map(|r| Dir { rel: r.to_string(), path: PathBuf::from(r) }).collect()
    }

    fn names(ds: &[Dir], q: &str) -> Vec<String> {
        find(ds, q).into_iter().map(|i| ds[i].rel.clone()).collect()
    }

    const REPOS: &[&str] = &[
        "bachelor",
        "bachelor/bachelor-docs",
        "bachelor/bachelor-rapport",
        "dotnetsandbox/docs",
        "SWD",
        "SWT",
        "toolbelt",
    ];

    #[test]
    fn exact_name_wins() {
        let ds = dirs(REPOS);
        assert_eq!(names(&ds, "bachelor-docs"), ["bachelor/bachelor-docs"]);
        assert_eq!(names(&ds, "BACHELOR"), ["bachelor"]);
        assert_eq!(names(&ds, "swt"), ["SWT"]);
    }

    #[test]
    fn forgives_typos() {
        let ds = dirs(REPOS);
        assert_eq!(names(&ds, "bachlor-docs"), ["bachelor/bachelor-docs"]);
        assert_eq!(names(&ds, "bachelor-dcos"), ["bachelor/bachelor-docs"]);
        assert_eq!(names(&ds, "tolbelt"), ["toolbelt"]);
        assert!(names(&ds, "xyzzy").is_empty());
    }

    #[test]
    fn prefix_and_substring() {
        let ds = dirs(REPOS);
        assert_eq!(names(&ds, "tool"), ["toolbelt"]);
        assert_eq!(names(&ds, "rapport"), ["bachelor/bachelor-rapport"]);
        assert_eq!(names(&ds, "docs"), ["dotnetsandbox/docs"]); // exact name beats contains
    }

    #[test]
    fn ambiguous_gives_several() {
        let ds = dirs(REPOS);
        assert_eq!(names(&ds, "bachelor-"), ["bachelor/bachelor-docs", "bachelor/bachelor-rapport"]);
    }

    #[test]
    fn several_words_match_the_path() {
        let ds = dirs(REPOS);
        assert_eq!(names(&ds, "dotnet docs"), ["dotnetsandbox/docs"]);
        assert_eq!(names(&ds, "bachelor/docs"), ["bachelor/bachelor-docs"]);
    }

    #[test]
    fn subfolders_of_a_match_are_dropped() {
        let ds = dirs(&["SWT/SWT-Eksamen", "SWT/SWT-Eksamen/src", "SWT/SWT-Eksamen/tests", "SWD/swd-eksamen"]);
        assert_eq!(names(&ds, "swt eksamen"), ["SWT/SWT-Eksamen"]);
    }

    #[test]
    fn typo_distance() {
        assert_eq!(typos("abc", "abc"), 0);
        assert_eq!(typos("acb", "abc"), 1);
        assert_eq!(typos("ab", "abc"), 1);
        assert_eq!(typos("kitten", "sitting"), 3);
    }
}
