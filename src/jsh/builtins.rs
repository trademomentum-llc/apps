//! Built-in shell commands for jsh.
//!
//! These are commands available in both REPL and script modes.
//! They bypass the JStar compiler and execute directly.


/// A built-in command result.
#[derive(Debug)]
pub enum BuiltinResult {
    /// Command produced output text
    Output(String),
    /// Command completed silently
    Ok,
    /// Command requests shell exit
    Exit(i32),
    /// Not a built-in command
    NotBuiltin,
}

/// Try to execute a line as a built-in command.
/// Returns NotBuiltin if it should be passed to the JStar compiler.
pub fn try_builtin(line: &str) -> BuiltinResult {
    let trimmed = line.trim();
    let parts: Vec<&str> = trimmed.splitn(2, char::is_whitespace).collect();
    let cmd = parts[0];
    let args = parts.get(1).unwrap_or(&"");

    match cmd {
        "exit" | "quit" => {
            let code = args.parse::<i32>().unwrap_or(0);
            BuiltinResult::Exit(code)
        }

        "help" => BuiltinResult::Output(
            "jsh — Jasterish Shell\n\
             Built-in commands:\n\
             \x20 help           Show this help\n\
             \x20 exit [code]    Exit the shell\n\
             \x20 pwd            Print working directory\n\
             \x20 cd <dir>       Change directory\n\
             \x20 ls [dir]       List directory contents\n\
             \x20 cat <file>     Print file contents\n\
             \x20 echo <text>    Print text\n\
             \x20 env            Print environment variables\n\
             \x20 tokenize <text> Run morphlex tokenizer\n\
             \nAnything else is compiled and executed as JStar code."
                .to_string(),
        ),

        "pwd" => match std::env::current_dir() {
            Ok(path) => BuiltinResult::Output(path.display().to_string()),
            Err(e) => BuiltinResult::Output(format!("Error: {}", e)),
        },

        "cd" => {
            let dir = if args.is_empty() {
                std::env::var("HOME").unwrap_or_else(|_| "/".to_string())
            } else {
                args.to_string()
            };
            match std::env::set_current_dir(&dir) {
                Ok(()) => BuiltinResult::Ok,
                Err(e) => BuiltinResult::Output(format!("cd: {}: {}", dir, e)),
            }
        }

        "ls" => {
            let dir = if args.is_empty() { "." } else { args };
            match std::fs::read_dir(dir) {
                Ok(entries) => {
                    let mut names: Vec<String> = entries
                        .filter_map(|e| e.ok())
                        .map(|e| e.file_name().to_string_lossy().to_string())
                        .collect();
                    names.sort();
                    BuiltinResult::Output(names.join("\n"))
                }
                Err(e) => BuiltinResult::Output(format!("ls: {}: {}", dir, e)),
            }
        }

        "cat" => {
            if args.is_empty() {
                return BuiltinResult::Output("cat: missing file argument".to_string());
            }
            match std::fs::read_to_string(args.trim()) {
                Ok(contents) => BuiltinResult::Output(contents),
                Err(e) => BuiltinResult::Output(format!("cat: {}: {}", args, e)),
            }
        }

        "echo" => BuiltinResult::Output(args.to_string()),

        "env" => {
            let mut vars: Vec<String> = std::env::vars()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect();
            vars.sort();
            BuiltinResult::Output(vars.join("\n"))
        }

        "tokenize" => {
            if args.is_empty() {
                return BuiltinResult::Output("tokenize: missing text argument".to_string());
            }
            match crate::compile(args) {
                Ok((lemmas, vectors)) => {
                    let mut out = String::new();
                    for (lemma, tv) in lemmas.iter().zip(vectors.iter()) {
                        let id = tv.id;
                        let pos = tv.pos;
                        let role = tv.role;
                        let morph = tv.morph;
                        out.push_str(&format!(
                            "{}: id={} pos={} role={} morph=0b{:016b}\n",
                            lemma, id, pos, role, morph
                        ));
                    }
                    BuiltinResult::Output(out)
                }
                Err(e) => BuiltinResult::Output(format!("tokenize error: {}", e)),
            }
        }

        _ => BuiltinResult::NotBuiltin,
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exit_builtin() {
        match try_builtin("exit 0") {
            BuiltinResult::Exit(0) => {}
            other => panic!("Expected Exit(0), got {:?}", other),
        }
    }

    #[test]
    fn test_help_builtin() {
        match try_builtin("help") {
            BuiltinResult::Output(text) => {
                assert!(text.contains("jsh"));
            }
            other => panic!("Expected Output, got {:?}", other),
        }
    }

    #[test]
    fn test_echo_builtin() {
        match try_builtin("echo hello world") {
            BuiltinResult::Output(text) => {
                assert_eq!(text, "hello world");
            }
            other => panic!("Expected Output, got {:?}", other),
        }
    }

    #[test]
    fn test_pwd_builtin() {
        match try_builtin("pwd") {
            BuiltinResult::Output(path) => {
                assert!(!path.is_empty());
            }
            other => panic!("Expected Output, got {:?}", other),
        }
    }

    #[test]
    fn test_not_builtin() {
        match try_builtin("store the integer into buffer") {
            BuiltinResult::NotBuiltin => {}
            other => panic!("Expected NotBuiltin, got {:?}", other),
        }
    }
}
