//! `<name>` and `<name=default>` placeholders, same syntax as pet.
//! A name is letters, digits, `_` or `-`, so `cmd > out.txt` is not a placeholder.

#[derive(Debug, Clone, PartialEq)]
pub struct Placeholder {
    pub name: String,
    pub default: String,
}

enum Piece<'a> {
    Text(&'a str),
    Slot { name: &'a str, default: &'a str },
}

fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

/// Split a command into plain text and placeholder slots.
fn pieces(cmd: &str) -> Vec<Piece<'_>> {
    let mut out = Vec::new();
    let mut text_start = 0;
    let mut i = 0;
    while let Some(off) = cmd[i..].find('<') {
        let open = i + off;
        if let Some((name, default, end)) = slot_at(cmd, open) {
            if text_start < open {
                out.push(Piece::Text(&cmd[text_start..open]));
            }
            out.push(Piece::Slot { name, default });
            text_start = end;
            i = end;
        } else {
            i = open + 1;
        }
    }
    if text_start < cmd.len() {
        out.push(Piece::Text(&cmd[text_start..]));
    }
    out
}

/// Try to read a slot starting at `cmd[open] == '<'`. Returns (name, default, index after '>').
fn slot_at(cmd: &str, open: usize) -> Option<(&str, &str, usize)> {
    let rest = &cmd[open + 1..];
    let name_len = rest.find(|c: char| !is_name_char(c)).unwrap_or(rest.len());
    if name_len == 0 {
        return None;
    }
    let name = &rest[..name_len];
    let after = &rest[name_len..];
    if after.starts_with('>') {
        return Some((name, "", open + 1 + name_len + 1));
    }
    if let Some(def_and_more) = after.strip_prefix('=') {
        let close = def_and_more.find(['>', '<'])?;
        if def_and_more.as_bytes()[close] != b'>' {
            return None;
        }
        let default = &def_and_more[..close];
        return Some((name, default, open + 1 + name_len + 1 + close + 1));
    }
    None
}

/// Unique placeholders in order of first appearance. The first default wins.
pub fn find(cmd: &str) -> Vec<Placeholder> {
    let mut out: Vec<Placeholder> = Vec::new();
    for p in pieces(cmd) {
        if let Piece::Slot { name, default } = p
            && !out.iter().any(|x| x.name == name)
        {
            out.push(Placeholder { name: name.to_string(), default: default.to_string() });
        }
    }
    out
}

/// Replace every placeholder with its value. `values` is looked up by name.
pub fn fill(cmd: &str, values: &[(String, String)]) -> String {
    let mut out = String::with_capacity(cmd.len());
    for p in pieces(cmd) {
        match p {
            Piece::Text(t) => out.push_str(t),
            Piece::Slot { name, default } => {
                let v = values.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str());
                out.push_str(v.unwrap_or(default));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(cmd: &str) -> Vec<(String, String)> {
        find(cmd).into_iter().map(|p| (p.name, p.default)).collect()
    }

    #[test]
    fn finds_name_and_default() {
        assert_eq!(names("tree -L <level=1> -I x"), vec![("level".into(), "1".into())]);
        assert_eq!(names("ssh <host>"), vec![("host".into(), "".into())]);
        assert_eq!(names("run <args=>"), vec![("args".into(), "".into())]);
    }

    #[test]
    fn default_may_contain_spaces_and_backslashes() {
        let cmd = r"x.ps1 <args=-OutputFile C:\Users\me\context.txt>";
        assert_eq!(names(cmd), vec![("args".into(), r"-OutputFile C:\Users\me\context.txt".into())]);
    }

    #[test]
    fn ignores_redirects_and_comparisons() {
        assert!(find("echo hi > out.txt").is_empty());
        assert!(find("sort < in.txt > out.txt").is_empty());
        assert!(find("a <b c>").is_empty());
        assert!(find("x <").is_empty());
        assert!(find("x <=1>").is_empty());
    }

    #[test]
    fn duplicate_names_asked_once() {
        assert_eq!(names("cp <f=a> <f=b>.bak"), vec![("f".into(), "a".into())]);
        assert_eq!(fill("cp <f=a> <f=b>.bak", &[("f".into(), "z".into())]), "cp z z.bak");
    }

    #[test]
    fn fills_values_and_falls_back_to_default() {
        let cmd = "tree -L <level=1> -I \"bin|obj\" <x>";
        assert_eq!(fill(cmd, &[("level".into(), "3".into())]), "tree -L 3 -I \"bin|obj\" ");
    }

    #[test]
    fn unicode_text_is_kept() {
        assert_eq!(fill("echo æøå <n=1> ✓", &[]), "echo æøå 1 ✓");
    }
}
