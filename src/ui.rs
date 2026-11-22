//! Inline terminal UI: an arrow-key list and a one-line text input.
//!
//! Everything is drawn *below* the cursor and erased again on exit, with the cursor
//! put back exactly where it was. stdout stays free for the picked command, so a
//! shell binding can capture it with `$(tb)`.

use crate::store::Snippet;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor};
use crossterm::{cursor, queue, terminal};
use std::io::Write;
use unicode_width::UnicodeWidthChar;

const ACCENT: Color = Color::Cyan;
const MAX_LIST: usize = 12;
const MAX_HELP_LINES: usize = 8;

#[derive(Clone, Copy, PartialEq)]
pub enum Style {
    Normal,
    Bold,
    Dim,
    Accent,
    AccentBold,
    Error,
    Cursor,
}

pub struct Span {
    text: String,
    style: Style,
}

pub type Line = Vec<Span>;

pub fn span(text: impl Into<String>, style: Style) -> Span {
    Span { text: text.into(), style }
}

// ---------------------------------------------------------------- terminal

pub struct Term {
    out: Box<dyn Write>,
    drawn: usize,
    restore_col: Option<u16>,
    #[cfg(unix)]
    saved_stdout: libc::c_int,
    #[cfg(windows)]
    saved_codepage: u32,
}

impl Term {
    pub fn open() -> Result<Term> {
        // While the UI is open, point stdout at the terminal: crossterm writes its
        // cursor-position query to stdout, which a shell binding is capturing.
        #[cfg(unix)]
        let saved_stdout = unsafe {
            use std::os::fd::AsRawFd;
            let tty = std::fs::OpenOptions::new().read(true).write(true).open("/dev/tty")?;
            let saved = libc::dup(libc::STDOUT_FILENO);
            libc::dup2(tty.as_raw_fd(), libc::STDOUT_FILENO);
            saved
        };
        #[cfg(unix)]
        let out: Box<dyn Write> = Box::new(std::io::stdout());
        #[cfg(windows)]
        let out: Box<dyn Write> = Box::new(std::fs::OpenOptions::new().write(true).open("CONOUT$")?);
        // The console decodes our bytes with its code page (often 850 or 437), which
        // garbles ❯ ● ↑↓. Switch to UTF-8 while the UI is open; Drop switches back.
        #[cfg(windows)]
        let saved_codepage = unsafe {
            use windows_sys::Win32::System::Console::{GetConsoleOutputCP, SetConsoleOutputCP};
            let cp = GetConsoleOutputCP();
            SetConsoleOutputCP(65001);
            cp
        };

        let mut term = Term {
            out,
            drawn: 0,
            restore_col: None,
            #[cfg(unix)]
            saved_stdout,
            #[cfg(windows)]
            saved_codepage,
        };
        terminal::enable_raw_mode()?;
        let col = cursor::position().map(|(c, _)| c).unwrap_or(0);
        queue!(term.out, cursor::Hide)?;
        if col > 0 {
            // Called from a key binding: the prompt is on this line, so start below it.
            queue!(term.out, Print("\r\n"))?;
            term.restore_col = Some(col);
        }
        term.out.flush()?;
        Ok(term)
    }

    fn size(&self) -> (usize, usize) {
        terminal::size().map(|(w, h)| (w as usize, h as usize)).unwrap_or((80, 24))
    }

    /// Replace the drawn region with `lines`. The cursor ends at the region's first line.
    pub fn draw(&mut self, lines: &[Line]) -> Result<()> {
        let (width, height) = self.size();
        let lines = &lines[..lines.len().min(height.saturating_sub(1).max(1))];
        queue!(self.out, cursor::MoveToColumn(0), terminal::Clear(terminal::ClearType::FromCursorDown))?;
        for (i, line) in lines.iter().enumerate() {
            if i > 0 {
                queue!(self.out, Print("\r\n"))?;
            }
            self.print_line(line, width.saturating_sub(1))?;
        }
        if lines.len() > 1 {
            queue!(self.out, cursor::MoveUp((lines.len() - 1) as u16))?;
        }
        queue!(self.out, cursor::MoveToColumn(0))?;
        self.out.flush()?;
        self.drawn = lines.len();
        Ok(())
    }

    fn print_line(&mut self, line: &Line, max: usize) -> Result<()> {
        let mut used = 0;
        for s in line {
            let mut text = String::new();
            for c in s.text.chars() {
                let c = if c == '\t' { ' ' } else { c };
                let w = c.width().unwrap_or(0);
                if used + w > max {
                    break;
                }
                used += w;
                text.push(c);
            }
            if text.is_empty() {
                continue;
            }
            match s.style {
                Style::Normal => {}
                Style::Bold => queue!(self.out, SetAttribute(Attribute::Bold))?,
                Style::Dim => queue!(self.out, SetAttribute(Attribute::Dim))?,
                Style::Accent => queue!(self.out, SetForegroundColor(ACCENT))?,
                Style::AccentBold => {
                    queue!(self.out, SetForegroundColor(ACCENT), SetAttribute(Attribute::Bold))?
                }
                Style::Error => queue!(self.out, SetForegroundColor(Color::Red))?,
                Style::Cursor => queue!(self.out, SetAttribute(Attribute::Reverse))?,
            }
            queue!(self.out, Print(text), SetAttribute(Attribute::Reset), ResetColor)?;
        }
        Ok(())
    }
}

impl Drop for Term {
    fn drop(&mut self) {
        let _ = queue!(self.out, cursor::MoveToColumn(0), terminal::Clear(terminal::ClearType::FromCursorDown));
        if let Some(col) = self.restore_col {
            let _ = queue!(self.out, cursor::MoveUp(1), cursor::MoveToColumn(col));
        }
        let _ = queue!(self.out, cursor::Show);
        let _ = self.out.flush();
        let _ = terminal::disable_raw_mode();
        #[cfg(windows)]
        if self.saved_codepage != 0 {
            unsafe { windows_sys::Win32::System::Console::SetConsoleOutputCP(self.saved_codepage) };
        }
        #[cfg(unix)]
        unsafe {
            libc::dup2(self.saved_stdout, libc::STDOUT_FILENO);
            libc::close(self.saved_stdout);
        }
    }
}

/// Next key press. Windows also reports key releases; those are skipped.
fn next_key() -> Result<KeyEvent> {
    loop {
        if let Event::Key(k) = event::read()?
            && k.kind != KeyEventKind::Release
        {
            return Ok(k);
        }
    }
}

fn is_cancel(k: &KeyEvent) -> bool {
    k.code == KeyCode::Esc || (k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c'))
}

fn typed_char(k: &KeyEvent) -> Option<char> {
    match k.code {
        KeyCode::Char(c) if !k.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) => Some(c),
        // AltGr on Windows shows up as Ctrl+Alt; it's how `\` `|` `@` `{` get typed on Nordic layouts.
        KeyCode::Char(c) if k.modifiers.contains(KeyModifiers::CONTROL | KeyModifiers::ALT) => Some(c),
        _ => None,
    }
}

fn footer(text: &str) -> Line {
    vec![span(format!("  {text}"), Style::Dim)]
}

// ---------------------------------------------------------------- picker

/// Pick one of `snippets[order[..]]`. Returns the index into `snippets`, or None if cancelled.
pub fn pick(term: &mut Term, snippets: &[Snippet], order: &[usize]) -> Result<Option<usize>> {
    let mut query = String::new();
    let mut sel = 0usize;
    let mut top = 0usize;
    loop {
        let shown: Vec<usize> =
            order.iter().copied().filter(|&i| crate::usage::matches(&snippets[i], &query)).collect();
        sel = sel.min(shown.len().saturating_sub(1));

        let (_, height) = term.size();
        let details = shown.get(sel).map(|&i| detail_lines(&snippets[i]).len()).unwrap_or(0);
        let room = height.saturating_sub(4 + details).clamp(3, MAX_LIST);
        if sel < top {
            top = sel;
        } else if sel >= top + room {
            top = sel + 1 - room;
        }
        top = top.min(shown.len().saturating_sub(room));

        term.draw(&picker_lines(snippets, &shown, sel, top, room, &query))?;

        let k = next_key()?;
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        match k.code {
            _ if is_cancel(&k) => return Ok(None),
            KeyCode::Enter => {
                if let Some(&i) = shown.get(sel) {
                    return Ok(Some(i));
                }
            }
            KeyCode::Up => sel = if sel == 0 { shown.len().saturating_sub(1) } else { sel - 1 },
            KeyCode::Down => sel = if sel + 1 >= shown.len() { 0 } else { sel + 1 },
            KeyCode::Char('p') if ctrl => sel = sel.saturating_sub(1),
            KeyCode::Char('n') if ctrl => sel = (sel + 1).min(shown.len().saturating_sub(1)),
            KeyCode::PageUp => sel = sel.saturating_sub(room),
            KeyCode::PageDown => sel = (sel + room).min(shown.len().saturating_sub(1)),
            KeyCode::Home => sel = 0,
            KeyCode::End => sel = shown.len().saturating_sub(1),
            KeyCode::Backspace => {
                query.pop();
                sel = 0;
            }
            KeyCode::Char('u') if ctrl => {
                query.clear();
                sel = 0;
            }
            _ => {
                if let Some(c) = typed_char(&k) {
                    query.push(c);
                    sel = 0;
                }
            }
        }
    }
}

fn detail_lines(s: &Snippet) -> Vec<Line> {
    let mut out = vec![vec![span(format!("       {}", s.command), Style::Dim)]];
    if let Some(help) = s.help.as_deref().map(str::trim_end).filter(|h| !h.is_empty()) {
        let lines: Vec<&str> = help.lines().collect();
        for l in lines.iter().take(MAX_HELP_LINES) {
            out.push(vec![span(format!("       {l}"), Style::Dim)]);
        }
        if lines.len() > MAX_HELP_LINES {
            out.push(vec![span(format!("       … {} more lines", lines.len() - MAX_HELP_LINES), Style::Dim)]);
        }
    }
    out
}

fn picker_lines(
    snippets: &[Snippet],
    shown: &[usize],
    sel: usize,
    top: usize,
    room: usize,
    query: &str,
) -> Vec<Line> {
    let mut lines = Vec::new();
    if query.is_empty() {
        lines.push(vec![span(" Pick a tool", Style::Bold), span("  type to filter", Style::Dim)]);
    } else {
        lines.push(vec![
            span(" Pick a tool", Style::Bold),
            span("  filter: ", Style::Dim),
            span(query, Style::Accent),
            span(" ", Style::Cursor),
        ]);
    }

    if shown.is_empty() {
        lines.push(vec![span("   No matches", Style::Dim)]);
    }
    let end = (top + room).min(shown.len());
    for (row, &i) in shown[top..end].iter().enumerate() {
        let s = &snippets[i];
        if top + row == sel {
            lines.push(vec![span(" ❯ ● ", Style::AccentBold), span(&s.description, Style::AccentBold)]);
            lines.extend(detail_lines(s));
        } else {
            lines.push(vec![span("   ○ ", Style::Dim), span(&s.description, Style::Normal)]);
        }
    }

    let more = shown.len() - end + top;
    let count = if more > 0 { format!(" · {} of {}", end - top, shown.len()) } else { String::new() };
    lines.push(footer(&format!("↑↓ navigate · enter select · esc cancel{count}")));
    lines
}

// ---------------------------------------------------------------- text input

/// One-line text input with `initial` pre-filled and editable.
/// `title` is drawn above, `error` (if any) below. Returns None if cancelled.
pub fn input(term: &mut Term, title: &[Line], label: &str, initial: &str, error: Option<&str>) -> Result<Option<String>> {
    let mut buf: Vec<char> = initial.chars().collect();
    let mut pos = buf.len();
    loop {
        let (width, _) = term.size();
        let mut lines: Vec<Line> = title.iter().map(|l| l.iter().map(|s| span(s.text.clone(), s.style)).collect()).collect();
        lines.push(input_line(label, &buf, pos, width));
        if let Some(e) = error {
            lines.push(vec![span(format!("   {e}"), Style::Error)]);
        }
        lines.push(footer("enter confirm · esc cancel"));
        term.draw(&lines)?;

        let k = next_key()?;
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        match k.code {
            _ if is_cancel(&k) => return Ok(None),
            KeyCode::Enter => return Ok(Some(buf.into_iter().collect())),
            KeyCode::Left => pos = pos.saturating_sub(1),
            KeyCode::Right => pos = (pos + 1).min(buf.len()),
            KeyCode::Home => pos = 0,
            KeyCode::Char('a') if ctrl => pos = 0,
            KeyCode::End => pos = buf.len(),
            KeyCode::Char('e') if ctrl => pos = buf.len(),
            KeyCode::Backspace if pos > 0 => {
                pos -= 1;
                buf.remove(pos);
            }
            KeyCode::Delete if pos < buf.len() => {
                buf.remove(pos);
            }
            KeyCode::Char('u') if ctrl => {
                buf.drain(..pos);
                pos = 0;
            }
            _ => {
                if let Some(c) = typed_char(&k) {
                    buf.insert(pos, c);
                    pos += 1;
                }
            }
        }
    }
}

fn input_line(label: &str, buf: &[char], pos: usize, width: usize) -> Line {
    let prefix = format!(" ❯ {label}: ");
    let avail = width.saturating_sub(prefix.chars().count() + 2).max(10);
    // Scroll horizontally so the cursor stays visible.
    let start = (pos + 1).saturating_sub(avail);
    let end = (start + avail).min(buf.len());
    let before: String = buf[start..pos.min(end)].iter().collect();
    let at: String = buf.get(pos).map(|c| c.to_string()).unwrap_or_else(|| " ".into());
    let after: String = if pos < end { buf[pos + 1..end].iter().collect() } else { String::new() };
    vec![
        span(" ❯ ", Style::AccentBold),
        span(format!("{label}: "), Style::Bold),
        span(before, Style::Normal),
        span(at, Style::Cursor),
        span(after, Style::Normal),
    ]
}
