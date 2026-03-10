//! Shell preprocessing — variable expansion, redirection, and piping.
//!
//! This module sits between line reading and command execution. It handles:
//!   - Shell variables (`set VAR value`, `$VAR` expansion)
//!   - Output redirection (`> file`, `>> file`)
//!   - Input redirection (`< file`)
//!   - Piping (`cmd1 | cmd2`)

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::Write;

/// Shell state persisted across lines in a session.
pub struct ShellState {
    /// Shell-local variables (not exported to child processes).
    vars: HashMap<String, String>,
}

/// Parsed redirection from a command line.
#[derive(Debug, PartialEq)]
pub struct Redirect {
    /// The command text with redirection operators stripped.
    pub command: String,
    /// Output file (overwrite mode), if `> file` was present.
    pub stdout_overwrite: Option<String>,
    /// Output file (append mode), if `>> file` was present.
    pub stdout_append: Option<String>,
    /// Input file, if `< file` was present.
    pub stdin_file: Option<String>,
}

/// A pipeline segment: one or more commands connected by `|`.
#[derive(Debug, PartialEq)]
pub struct Pipeline {
    pub segments: Vec<Redirect>,
}

impl ShellState {
    pub fn new() -> Self {
        Self {
            vars: HashMap::new(),
        }
    }

    /// Set a shell variable. Returns true if this was a `set` command.
    pub fn try_set_var(&mut self, line: &str) -> Option<String> {
        let trimmed = line.trim();
        let parts: Vec<&str> = trimmed.splitn(3, char::is_whitespace).collect();

        match parts.first().copied() {
            Some("set") => {
                if parts.len() < 2 {
                    return Some("set: usage: set VAR [value]".to_string());
                }
                let name = parts[1].to_string();
                let value = if parts.len() >= 3 {
                    parts[2].to_string()
                } else {
                    String::new()
                };
                self.vars.insert(name, value);
                Some(String::new())
            }
            Some("unset") => {
                if parts.len() < 2 {
                    return Some("unset: usage: unset VAR".to_string());
                }
                self.vars.remove(parts[1]);
                Some(String::new())
            }
            Some("export") => {
                if parts.len() < 2 {
                    return Some("export: usage: export VAR [value]".to_string());
                }
                let name = parts[1];
                let value = if parts.len() >= 3 {
                    parts[2].to_string()
                } else {
                    // Export existing shell var to env
                    self.vars.get(name).cloned().unwrap_or_default()
                };
                // SAFETY: jsh is single-threaded; no concurrent env reads.
                unsafe { std::env::set_var(name, &value); }
                self.vars.insert(name.to_string(), value);
                Some(String::new())
            }
            _ => None,
        }
    }

    /// Expand `$VAR` references in a string.
    ///
    /// Checks shell variables first, then falls back to process environment.
    /// `$$` escapes to a literal `$`.
    pub fn expand_vars(&self, input: &str) -> String {
        let mut result = String::with_capacity(input.len());
        let mut chars = input.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '$' {
                match chars.peek() {
                    Some('$') => {
                        // $$ → literal $
                        chars.next();
                        result.push('$');
                    }
                    Some(&c) if c.is_ascii_alphabetic() || c == '_' => {
                        // Collect variable name: [A-Za-z_][A-Za-z0-9_]*
                        let mut name = String::new();
                        while let Some(&c) = chars.peek() {
                            if c.is_ascii_alphanumeric() || c == '_' {
                                name.push(c);
                                chars.next();
                            } else {
                                break;
                            }
                        }
                        // Look up: shell vars first, then env
                        let value = self.vars.get(&name)
                            .cloned()
                            .or_else(|| std::env::var(&name).ok())
                            .unwrap_or_default();
                        result.push_str(&value);
                    }
                    _ => {
                        result.push('$');
                    }
                }
            } else {
                result.push(ch);
            }
        }

        result
    }
}

/// Parse a command line into a pipeline of redirected commands.
///
/// Splits on `|` first, then parses `>`, `>>`, `<` within each segment.
pub fn parse_pipeline(input: &str) -> Pipeline {
    let segments: Vec<Redirect> = input.split('|')
        .map(|seg| parse_redirect(seg.trim()))
        .collect();
    Pipeline { segments }
}

/// Parse redirection operators from a single command segment.
fn parse_redirect(input: &str) -> Redirect {
    let mut command = String::new();
    let mut stdout_overwrite: Option<String> = None;
    let mut stdout_append: Option<String> = None;
    let mut stdin_file: Option<String> = None;

    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '>' => {
                if chars.peek() == Some(&'>') {
                    // >> append
                    chars.next();
                    let file = collect_filename(&mut chars);
                    stdout_append = Some(file);
                } else {
                    // > overwrite
                    let file = collect_filename(&mut chars);
                    stdout_overwrite = Some(file);
                }
            }
            '<' => {
                let file = collect_filename(&mut chars);
                stdin_file = Some(file);
            }
            _ => {
                command.push(ch);
            }
        }
    }

    Redirect {
        command: command.trim().to_string(),
        stdout_overwrite,
        stdout_append,
        stdin_file,
    }
}

/// Collect a filename after a redirection operator, skipping leading whitespace.
fn collect_filename(chars: &mut std::iter::Peekable<std::str::Chars>) -> String {
    // Skip whitespace
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else {
            break;
        }
    }
    // Collect until whitespace or another operator
    let mut name = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() || c == '>' || c == '<' || c == '|' {
            break;
        }
        name.push(c);
        chars.next();
    }
    name
}

/// Write output text to the appropriate destination based on redirection.
pub fn write_output(text: &str, redirect: &Redirect) -> std::io::Result<()> {
    if let Some(ref path) = redirect.stdout_overwrite {
        let mut f = File::create(path)?;
        f.write_all(text.as_bytes())?;
    } else if let Some(ref path) = redirect.stdout_append {
        let mut f = OpenOptions::new().create(true).append(true).open(path)?;
        f.write_all(text.as_bytes())?;
    } else {
        print!("{}", text);
    }
    Ok(())
}

/// Read input from a file if stdin redirection is present.
pub fn read_input(redirect: &Redirect) -> std::io::Result<Option<String>> {
    match redirect.stdin_file {
        Some(ref path) => {
            let content = std::fs::read_to_string(path)?;
            Ok(Some(content))
        }
        None => Ok(None),
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Variable expansion ──────────────────────────────────────────────

    #[test]
    fn test_expand_shell_var() {
        let mut state = ShellState::new();
        state.vars.insert("NAME".to_string(), "jsh".to_string());
        assert_eq!(state.expand_vars("hello $NAME"), "hello jsh");
    }

    #[test]
    fn test_expand_env_var() {
        let state = ShellState::new();
        unsafe { std::env::set_var("JSH_TEST_VAR", "works"); }
        assert_eq!(state.expand_vars("it $JSH_TEST_VAR"), "it works");
        unsafe { std::env::remove_var("JSH_TEST_VAR"); }
    }

    #[test]
    fn test_expand_undefined_var() {
        let state = ShellState::new();
        assert_eq!(state.expand_vars("$UNDEFINED_XYZ"), "");
    }

    #[test]
    fn test_expand_dollar_escape() {
        let state = ShellState::new();
        assert_eq!(state.expand_vars("price is $$5"), "price is $5");
    }

    #[test]
    fn test_expand_multiple_vars() {
        let mut state = ShellState::new();
        state.vars.insert("A".to_string(), "hello".to_string());
        state.vars.insert("B".to_string(), "world".to_string());
        assert_eq!(state.expand_vars("$A $B"), "hello world");
    }

    #[test]
    fn test_expand_no_vars() {
        let state = ShellState::new();
        assert_eq!(state.expand_vars("no variables here"), "no variables here");
    }

    #[test]
    fn test_expand_var_at_end() {
        let mut state = ShellState::new();
        state.vars.insert("X".to_string(), "42".to_string());
        assert_eq!(state.expand_vars("value is $X"), "value is 42");
    }

    // ── set / unset / export ────────────────────────────────────────────

    #[test]
    fn test_set_var() {
        let mut state = ShellState::new();
        let result = state.try_set_var("set NAME morphlex");
        assert_eq!(result, Some(String::new()));
        assert_eq!(state.vars.get("NAME").unwrap(), "morphlex");
    }

    #[test]
    fn test_set_empty_value() {
        let mut state = ShellState::new();
        state.try_set_var("set FLAG");
        assert_eq!(state.vars.get("FLAG").unwrap(), "");
    }

    #[test]
    fn test_unset_var() {
        let mut state = ShellState::new();
        state.vars.insert("X".to_string(), "123".to_string());
        state.try_set_var("unset X");
        assert!(state.vars.get("X").is_none());
    }

    #[test]
    fn test_export_var() {
        let mut state = ShellState::new();
        state.try_set_var("export JSH_EXPORT_TEST hello");
        assert_eq!(std::env::var("JSH_EXPORT_TEST").unwrap(), "hello");
        unsafe { std::env::remove_var("JSH_EXPORT_TEST"); }
    }

    #[test]
    fn test_not_a_var_command() {
        let mut state = ShellState::new();
        assert_eq!(state.try_set_var("echo hello"), None);
    }

    // ── Redirection parsing ─────────────────────────────────────────────

    #[test]
    fn test_parse_stdout_overwrite() {
        let r = parse_redirect("echo hello > out.txt");
        assert_eq!(r.command, "echo hello");
        assert_eq!(r.stdout_overwrite, Some("out.txt".to_string()));
    }

    #[test]
    fn test_parse_stdout_append() {
        let r = parse_redirect("echo hello >> log.txt");
        assert_eq!(r.command, "echo hello");
        assert_eq!(r.stdout_append, Some("log.txt".to_string()));
    }

    #[test]
    fn test_parse_stdin_redirect() {
        let r = parse_redirect("cat < input.txt");
        assert_eq!(r.command, "cat");
        assert_eq!(r.stdin_file, Some("input.txt".to_string()));
    }

    #[test]
    fn test_parse_no_redirect() {
        let r = parse_redirect("echo hello");
        assert_eq!(r.command, "echo hello");
        assert_eq!(r.stdout_overwrite, None);
        assert_eq!(r.stdout_append, None);
        assert_eq!(r.stdin_file, None);
    }

    // ── Pipeline parsing ────────────────────────────────────────────────

    #[test]
    fn test_parse_simple_pipeline() {
        let p = parse_pipeline("ls | cat");
        assert_eq!(p.segments.len(), 2);
        assert_eq!(p.segments[0].command, "ls");
        assert_eq!(p.segments[1].command, "cat");
    }

    #[test]
    fn test_parse_pipeline_with_redirect() {
        let p = parse_pipeline("ls | cat > out.txt");
        assert_eq!(p.segments.len(), 2);
        assert_eq!(p.segments[0].command, "ls");
        assert_eq!(p.segments[1].command, "cat");
        assert_eq!(p.segments[1].stdout_overwrite, Some("out.txt".to_string()));
    }

    #[test]
    fn test_parse_single_command() {
        let p = parse_pipeline("echo hello");
        assert_eq!(p.segments.len(), 1);
        assert_eq!(p.segments[0].command, "echo hello");
    }

    // ── Redirection I/O ─────────────────────────────────────────────────

    #[test]
    fn test_write_output_to_file() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_jsh_redirect_write.txt");
        let redirect = Redirect {
            command: String::new(),
            stdout_overwrite: Some(path.to_string_lossy().to_string()),
            stdout_append: None,
            stdin_file: None,
        };
        write_output("hello\n", &redirect).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "hello\n");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_write_output_append() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_jsh_redirect_append.txt");
        let _ = std::fs::remove_file(&path);

        let redirect = Redirect {
            command: String::new(),
            stdout_overwrite: None,
            stdout_append: Some(path.to_string_lossy().to_string()),
            stdin_file: None,
        };
        write_output("line1\n", &redirect).unwrap();
        write_output("line2\n", &redirect).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "line1\nline2\n");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_read_input_from_file() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_jsh_redirect_read.txt");
        std::fs::write(&path, "input data\n").unwrap();

        let redirect = Redirect {
            command: String::new(),
            stdout_overwrite: None,
            stdout_append: None,
            stdin_file: Some(path.to_string_lossy().to_string()),
        };
        let input = read_input(&redirect).unwrap();
        assert_eq!(input, Some("input data\n".to_string()));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_read_input_no_redirect() {
        let redirect = Redirect {
            command: String::new(),
            stdout_overwrite: None,
            stdout_append: None,
            stdin_file: None,
        };
        let input = read_input(&redirect).unwrap();
        assert_eq!(input, None);
    }
}
