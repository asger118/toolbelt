//! `tb init <shell>`: key binding that inserts a picked snippet at the cursor.

pub const POWERSHELL: &str = r#"# toolbelt: Ctrl+S picks a tool and inserts it at the cursor
Set-PSReadLineKeyHandler -Chord 'Ctrl+s' -BriefDescription 'toolbelt' -Description 'Pick a toolbelt snippet' -ScriptBlock {
    $top = [Console]::CursorTop
    $out = (& tb) -join "`n"
    $code = $LASTEXITCODE
    # If the picker scrolled the window, tell PSReadLine where its line moved to.
    $scrolled = $top - [Console]::CursorTop
    if ($scrolled -gt 0) {
        try {
            $t = [Microsoft.PowerShell.PSConsoleReadLine]
            $rl = $t.GetField('_singleton', 'NonPublic,Static').GetValue($null)
            $y = $t.GetField('_initialY', 'NonPublic,Instance')
            $y.SetValue($rl, $y.GetValue($rl) - $scrolled)
        } catch {
            [Microsoft.PowerShell.PSConsoleReadLine]::InvokePrompt()
        }
    }
    if ($code -eq 0 -and $out) {
        [Microsoft.PowerShell.PSConsoleReadLine]::Insert($out)
    }
}

# toolbelt: `goto <name>` jumps to a folder under ~/repos, typos allowed
function goto {
    $dir = & tb goto @args
    if ($LASTEXITCODE -eq 0 -and $dir) { Set-Location -LiteralPath $dir }
}
"#;

pub const BASH: &str = r#"# toolbelt: Ctrl+S picks a tool and inserts it at the cursor
__toolbelt_insert() {
    local out
    out=$(tb) || return
    READLINE_LINE="${READLINE_LINE:0:READLINE_POINT}${out}${READLINE_LINE:READLINE_POINT}"
    READLINE_POINT=$((READLINE_POINT + ${#out}))
}
[[ $- == *i* ]] && stty -ixon 2>/dev/null   # free Ctrl+S from terminal flow control
bind -x '"\C-s": __toolbelt_insert'

# toolbelt: `goto <name>` jumps to a folder under ~/repos, typos allowed
goto() { local dir; dir=$(command tb goto "$@") && [ -n "$dir" ] && cd -- "$dir"; }
"#;

pub const ZSH: &str = r#"# toolbelt: Ctrl+S picks a tool and inserts it at the cursor
__toolbelt_insert() {
    local out
    out=$(tb) && LBUFFER+="$out"
    zle reset-prompt
}
zle -N __toolbelt_insert
[[ -o interactive ]] && stty -ixon 2>/dev/null   # free Ctrl+S from terminal flow control
bindkey '^S' __toolbelt_insert

# toolbelt: `goto <name>` jumps to a folder under ~/repos, typos allowed
goto() { local dir; dir=$(command tb goto "$@") && [ -n "$dir" ] && cd -- "$dir"; }
"#;

pub fn script(shell: &str) -> Option<&'static str> {
    match shell {
        "powershell" | "pwsh" => Some(POWERSHELL),
        "bash" => Some(BASH),
        "zsh" => Some(ZSH),
        _ => None,
    }
}
