mod editor;
mod placeholder;
mod shell;
mod store;
mod ui;
mod usage;

use anyhow::{Result, bail};
use clap::{Parser, Subcommand};
use std::io::{BufRead, Write};
use std::process::ExitCode;
use store::Snippet;
use ui::{Style, Term, span};

/// Pick a saved command and paste it onto your prompt.
///
/// Run `tb` to open the picker. The chosen command is printed; with the shell
/// binding from `tb init`, Ctrl+S inserts it at the cursor instead.
#[derive(Parser)]
#[command(name = "tb", version)]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Add a new tool
    New,
    /// Open the snippet file in your editor
    Edit,
    /// Print the Ctrl+S key binding for a shell (powershell, bash, zsh)
    Init { shell: String },
    /// Print the path of the snippet file
    Path,
}

/// Exit code when the user presses Esc, like other shell pickers.
const CANCELLED: u8 = 130;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.cmd {
        None => pick(),
        Some(Cmd::New) => new(),
        Some(Cmd::Edit) => edit(),
        Some(Cmd::Init { shell }) => init(&shell),
        Some(Cmd::Path) => store::path().map(|p| {
            println!("{}", p.display());
            ExitCode::SUCCESS
        }),
    };
    result.unwrap_or_else(|e| {
        eprintln!("tb: {e:#}");
        ExitCode::FAILURE
    })
}

fn pick() -> Result<ExitCode> {
    let path = store::path()?;
    let file = match store::load_or_create(&path) {
        Ok(f) => f,
        Err(e) => bail!("{e:#}\n    fix it with `tb edit`"),
    };
    let visible: Vec<Snippet> = file.snippets.iter().filter(|s| s.runs_here()).cloned().collect();
    if visible.is_empty() {
        eprintln!("No tools yet. Add one with `tb new`.");
        return Ok(ExitCode::FAILURE);
    }
    let order = usage::order(&visible, &usage::load());

    let mut term = Term::open()?;
    let Some(i) = ui::pick(&mut term, &visible, &order)? else {
        return Ok(ExitCode::from(CANCELLED));
    };
    let snippet = &visible[i];
    let Some(values) = fill_placeholders(&mut term, snippet)? else {
        return Ok(ExitCode::from(CANCELLED));
    };
    drop(term);

    let _ = usage::record(&snippet.description, &file.snippets);
    println!("{}", placeholder::fill(&snippet.command, &values));
    Ok(ExitCode::SUCCESS)
}

/// Ask for each placeholder. None if the user cancelled.
fn fill_placeholders(term: &mut Term, s: &Snippet) -> Result<Option<Vec<(String, String)>>> {
    let mut title = vec![
        vec![span(" ", Style::Normal), span(&s.description, Style::Bold)],
        vec![span(format!("   {}", s.command), Style::Dim)],
    ];
    if let Some(help) = s.help.as_deref().map(str::trim_end).filter(|h| !h.is_empty()) {
        title.push(vec![]);
        title.extend(help.lines().map(|l| vec![span(format!("   {l}"), Style::Dim)]));
    }
    title.push(vec![]);

    let mut values = Vec::new();
    for p in placeholder::find(&s.command) {
        match ui::input(term, &title, &p.name, &p.default, None)? {
            Some(v) => values.push((p.name, v)),
            None => return Ok(None),
        }
    }
    Ok(Some(values))
}

fn new() -> Result<ExitCode> {
    let path = store::path()?;
    let mut file = store::load_or_create(&path)?;

    let mut term = Term::open()?;
    let header = |hint: &str| {
        vec![
            vec![span(" New tool", Style::Bold)],
            vec![span(format!("   {hint}"), Style::Dim)],
            vec![],
        ]
    };

    let mut error = None;
    let description = loop {
        let hint = "What does it do? Shown in the list.";
        let Some(d) = ui::input(&mut term, &header(hint), "Description", "", error)? else {
            return Ok(ExitCode::from(CANCELLED));
        };
        let d = d.trim().to_string();
        error = if d.is_empty() {
            Some("A description is required")
        } else if file.snippets.iter().any(|s| s.description == d) {
            Some("A tool with this description already exists")
        } else {
            break d;
        };
    };

    error = None;
    let command = loop {
        let hint = "Use <name> or <name=default> for values to fill in when picked.";
        let Some(c) = ui::input(&mut term, &header(hint), "Command", "", error)? else {
            return Ok(ExitCode::from(CANCELLED));
        };
        let c = c.trim().to_string();
        if !c.is_empty() {
            break c;
        }
        error = Some("A command is required");
    };

    let hint = "Optional. Shown under the tool. Press enter to skip.";
    let Some(help) = ui::input(&mut term, &header(hint), "Help", "", None)? else {
        return Ok(ExitCode::from(CANCELLED));
    };
    drop(term);

    let help = Some(help.trim().to_string()).filter(|h| !h.is_empty());
    file.snippets.push(Snippet { description: description.clone(), command, help, os: None });
    store::save(&path, &file)?;
    eprintln!("Added \"{description}\" to {}", path.display());
    Ok(ExitCode::SUCCESS)
}

fn edit() -> Result<ExitCode> {
    let path = store::path()?;
    if !path.exists() {
        store::save(&path, &store::File::default())?;
    }
    loop {
        editor::open(&path)?;
        let text = std::fs::read_to_string(&path)?;
        match store::parse(&text) {
            Ok(f) => {
                eprintln!("Saved {} tools.", f.snippets.len());
                return Ok(ExitCode::SUCCESS);
            }
            Err(e) => {
                eprintln!("The file has an error: {e}");
                eprint!("Open the editor again? [Y/n] ");
                std::io::stderr().flush()?;
                let mut answer = String::new();
                std::io::stdin().lock().read_line(&mut answer)?;
                if answer.trim().eq_ignore_ascii_case("n") {
                    return Ok(ExitCode::FAILURE);
                }
            }
        }
    }
}

fn init(shell: &str) -> Result<ExitCode> {
    match shell::script(shell) {
        Some(s) => {
            print!("{s}");
            Ok(ExitCode::SUCCESS)
        }
        None => bail!("unknown shell `{shell}`. Use powershell, bash or zsh"),
    }
}
