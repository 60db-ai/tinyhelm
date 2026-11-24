//! Port of `bootstrap/helpers/sudo.php`: rewrite remote commands for servers whose SSH user is not root.
//!
//! `tests/fixtures/sudo_golden.json` (generated from the PHP functions by `scripts/parity/sudo-golden.php`)
//! is the oracle: every input must produce the exact PHP output.

use std::sync::OnceLock;

use regex::Regex;

const BASH_KEYWORDS: &[&str] = &[
    "cd", "command", "declare", "echo", "export", "local", "readonly", "return", "true", "if",
    "fi", "for", "done", "while", "until", "case", "esac", "select", "then", "else", "elif",
    "break", "continue", "do",
];

const SYSTEM_PATHS: &[&str] = &[
    "/var", "/etc", "/usr", "/opt", "/sys", "/proc", "/dev", "/bin", "/sbin", "/lib", "/lib64",
    "/boot", "/root", "/home", "/media", "/mnt", "/srv", "/run",
];

/// PCRE `\s` without the `u` flag: ASCII whitespace including vertical tab.
const WS: &str = r"[ \t\n\x0B\x0C\r]";
const WS_CHARS: &str = r" \t\n\x0B\x0C\r";

fn re(cell: &'static OnceLock<Regex>, pattern: impl FnOnce() -> String) -> &'static Regex {
    cell.get_or_init(|| Regex::new(&pattern()).expect("valid regex"))
}

/// `parseCommandsByLineForSudo`: every line of a non-root command list.
pub fn parse_commands_by_line_for_sudo(commands: &[String], user: &str) -> Vec<String> {
    commands
        .iter()
        .map(|line| {
            let line = add_sudo_prefix(line);
            let line = add_ownership_after_mkdir(&line, user);
            elevate_inner_commands(&line)
        })
        .collect()
}

/// `parseLineForSudo`: the single-command variant.
pub fn parse_line_for_sudo(command: &str, user: &str) -> String {
    // PHP method names are case-insensitive: `startSwith` is `startsWith`, without a word boundary.
    let mut command = if !command.starts_with("cd") && !command.starts_with("command") {
        format!("sudo {command}")
    } else {
        command.to_string()
    };
    command = add_ownership_after_mkdir(&command, user);
    if is_single_sudo_shell_script(&command) {
        return command;
    }
    if command.contains("$(") || command.contains('`') {
        command = command.replace("$(", "$(sudo ").replace('`', "`sudo ");
    }
    if command.contains("||") {
        command = command.replace("||", "|| sudo ");
    }
    if command.contains("&&") {
        command = command.replace("&&", "&& sudo ");
    }
    add_sudo_after_unquoted_pipes(&command)
}

fn add_sudo_prefix(line: &str) -> String {
    static NEGATION: OnceLock<Regex> = OnceLock::new();
    static NEGATION_PREFIX: OnceLock<Regex> = OnceLock::new();
    static IF_PREFIX: OnceLock<Regex> = OnceLock::new();

    let trimmed = php_trim(line);
    if trimmed.starts_with('#') {
        return line.to_string();
    }
    if re(&NEGATION, || format!(r"^{WS}*!{WS}+")).is_match(line) {
        return re(&NEGATION_PREFIX, || format!(r"^({WS}*(?:!{WS}+)+)"))
            .replacen(line, 1, "${1}sudo ")
            .into_owned();
    }
    for keyword in BASH_KEYWORDS {
        if let Some(rest) = trimmed.strip_prefix(keyword) {
            let boundary =
                rest.is_empty() || rest.starts_with(';') || rest.starts_with(is_pcre_space);
            if boundary {
                if *keyword == "if" {
                    return re(&IF_PREFIX, || format!(r"^({WS}*if{WS}+(?:!{WS}+)*)"))
                        .replacen(line, 1, "${1}sudo ")
                        .into_owned();
                }
                return line.to_string();
            }
        }
    }
    format!("sudo {line}")
}

fn elevate_inner_commands(line: &str) -> String {
    if is_single_sudo_shell_script(line) {
        return line.to_string();
    }
    let is_complex_pipe = line.contains(" | sh")
        || line.contains(" | bash")
        || line.contains(" sh -c ")
        || (line.contains(" | ") && (line.contains("||") || line.contains("&&")));

    if is_complex_pipe {
        if let Some(without_sudo) = line.strip_prefix("sudo ") {
            return format!("sudo bash -c '{}'", without_sudo.replace('\'', "'\\''"));
        }
    }

    let mut line = line.to_string();
    if line.contains("$(") {
        line = line.replace("$(", "$(sudo ");
    }
    if !is_complex_pipe && line.contains("||") {
        line = line.replace("||", "|| sudo");
    }
    if !is_complex_pipe && line.contains("&&") {
        line = line.replace("&&", "&& sudo");
    }
    if !is_complex_pipe {
        line = add_sudo_after_unquoted_pipes(&line);
    }
    line
}

/// `shouldChangeOwnership`: only TinyHelm's own directories, never system paths.
fn should_change_ownership(path: &str) -> bool {
    let path = php_trim(path);
    if SYSTEM_PATHS
        .iter()
        .any(|sys| path == *sys || path.starts_with(&format!("{sys}/")))
    {
        return false;
    }
    path.starts_with("/data/tinyhelm") || path.starts_with("/tmp/tinyhelm")
}

/// `addOwnershipAfterMkdir`.
fn add_ownership_after_mkdir(line: &str, user: &str) -> String {
    static MKDIR: OnceLock<Regex> = OnceLock::new();
    let mkdir = re(&MKDIR, || {
        format!(r"(?s)^(sudo mkdir -p ([^{WS_CHARS};&|<>]+))({WS}*(?:(?:&&|;).*)?)$")
    });
    let Some(caps) = mkdir.captures(line) else {
        return line.to_string();
    };
    let path = &caps[2];
    if !should_change_ownership(path) {
        return line.to_string();
    }
    let rest = caps.get(3).map_or("", |m| m.as_str());
    format!(
        "{} && find {path} -user root -exec chown -h {user}:{user} {{}} + && chmod o-rwx {path}{rest}",
        &caps[1]
    )
}

/// `isSingleSudoShellScript`: one `sudo sh -c '...'` call that already runs completely as root.
fn is_single_sudo_shell_script(line: &str) -> bool {
    static SINGLE: OnceLock<Regex> = OnceLock::new();
    // PCRE `$` without /m also matches before a final newline.
    re(&SINGLE, || {
        format!(r"^{WS}*sudo (?:ba)?sh -c '(?:[^']|'\\'')*'{WS}*\n?\z")
    })
    .is_match(line)
}

/// `addSudoAfterUnquotedPipes`: sudo after each ` | ` the shell sees as a pipe (not inside quotes).
fn add_sudo_after_unquoted_pipes(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len() + 16);
    let mut quote: Option<u8> = None;
    let mut i = 0;
    while i < bytes.len() {
        let ch = bytes[i];
        if quote.is_none() && bytes[i..].starts_with(b" | ") {
            if bytes[i + 3..].starts_with(b"sudo ") {
                out.extend_from_slice(b" | ");
            } else {
                out.extend_from_slice(b" | sudo ");
            }
            i += 3;
            continue;
        }
        out.push(ch);
        if ch == b'\\' && quote != Some(b'\'') && i + 1 < bytes.len() {
            i += 1;
            out.push(bytes[i]);
        } else if quote.is_none() && (ch == b'\'' || ch == b'"') {
            quote = Some(ch);
        } else if Some(ch) == quote {
            quote = None;
        }
        i += 1;
    }
    // Only ASCII bytes were inserted between whole input bytes, so the result stays valid UTF-8.
    String::from_utf8(out).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
}

fn is_pcre_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\x0B' | '\x0C' | '\r')
}

/// PHP `trim()` default characters: space, \t, \n, \r, \0, \x0B.
fn php_trim(s: &str) -> &str {
    s.trim_matches(|c| matches!(c, ' ' | '\t' | '\n' | '\r' | '\0' | '\x0B'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(serde::Deserialize)]
    struct Golden {
        input: String,
        output: String,
        line_output: String,
    }

    #[test]
    fn matches_php_golden_outputs() {
        let raw = include_str!("../../tests/fixtures/sudo_golden.json");
        let cases: Vec<Golden> = serde_json::from_str(raw).expect("golden json");
        assert!(cases.len() > 1000, "golden file looks truncated");
        let mut failures = Vec::new();
        for c in &cases {
            let got =
                parse_commands_by_line_for_sudo(std::slice::from_ref(&c.input), "ubuntu").remove(0);
            if got != c.output {
                failures.push(format!(
                    "by-line\n  in:   {}\n  php:  {}\n  rust: {}",
                    c.input, c.output, got
                ));
            }
            let got_line = parse_line_for_sudo(&c.input, "ubuntu");
            if got_line != c.line_output {
                failures.push(format!(
                    "line\n  in:   {}\n  php:  {}\n  rust: {}",
                    c.input, c.line_output, got_line
                ));
            }
        }
        assert!(
            failures.is_empty(),
            "{} of {} differ:\n{}",
            failures.len(),
            cases.len() * 2,
            failures
                .iter()
                .take(8)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
