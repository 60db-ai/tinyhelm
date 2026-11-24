//! Compose validation helpers from `bootstrap/helpers/parsers.php` and `shared.php`. Error texts are
//! the PHP exception messages (the golden snapshots compare them).

use std::collections::HashMap;
use std::sync::OnceLock;

use regex::Regex;

use super::php::{is_numeric_str, Value};

/// A parser failure; `message` is the PHP `Exception` message.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError(pub String);

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

const DANGEROUS_CHARS: &[(&str, &str)] = &[
    ("`", "backtick (command substitution)"),
    ("$(", "command substitution"),
    (
        "${",
        "variable substitution with potential command injection",
    ),
    ("|", "pipe operator"),
    ("&", "background/AND operator"),
    (";", "command separator"),
    ("\n", "newline (command separator)"),
    ("\r", "carriage return"),
    ("\t", "tab (token separator)"),
    (">", "output redirection"),
    ("<", "input redirection"),
];

/// `validateShellSafePath`.
pub fn validate_shell_safe_path(input: &str, context: &str) -> Result<(), ParseError> {
    for (ch, description) in DANGEROUS_CHARS {
        if input.contains(ch) {
            return Err(ParseError(format!(
                "Invalid {context}: contains forbidden character '{ch}' ({description}). Shell metacharacters are not allowed for security reasons."
            )));
        }
    }
    Ok(())
}

/// The service-name check `serviceParser` / `validateDockerComposeForInjection` run first.
pub fn validate_service_name_shell_safe(name: &str) -> Result<(), ParseError> {
    validate_shell_safe_path(name, "service name").map_err(|e| {
        ParseError(format!(
            "Invalid Docker Compose service name: {} Service names must not contain shell metacharacters.",
            e.0
        ))
    })
}

/// `validateComposeServiceName`.
pub fn validate_compose_service_name(name: &str) -> Result<(), ParseError> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"^[a-zA-Z0-9][a-zA-Z0-9_.-]*$").unwrap());
    if re.is_match(name) {
        return Ok(());
    }
    Err(ParseError(format!(
        "Service name \"{name}\" must start with a letter or a digit and contain only letters, digits, \".\", \"-\", and \"_\". Docker uses the same rule for container names, and TinyHelm uses the service name at the start of the container name."
    )))
}

/// `validateComposeServiceScale`.
pub fn validate_compose_service_scale(name: &str, service: &Value) -> Result<(), ParseError> {
    for key in ["scale", "deploy.replicas"] {
        let Some(value) = service.data_get(key) else {
            continue;
        };
        let (numeric, as_int, text) = match value {
            Value::Int(i) => (true, *i, i.to_string()),
            Value::Float(f) => (true, *f as i64, value.to_php_string()),
            Value::Str(s) if is_numeric_str(s) => {
                let n = s.trim().parse::<f64>().unwrap_or(0.0) as i64;
                (true, n, s.clone())
            }
            _ => (false, 0, String::new()),
        };
        if numeric && as_int > 1 {
            return Err(ParseError(format!(
                "Service \"{name}\" sets {key} to {text}. TinyHelm sets a container_name for each service, and Docker Compose cannot run more than one container with the same container_name. Remove {key}, or define each instance as a separate service."
            )));
        }
    }
    Ok(())
}

/// `rejectComposeVolumeTargetVariable`.
pub fn reject_compose_volume_target_variable(target: &str, prefix: &str) -> Result<(), ParseError> {
    if target.contains("${") {
        return Err(ParseError(format!(
            "{prefix}: the container path {target} contains a variable. TinyHelm does not support variables in the container path of a volume. Write the path without variables, for example /app/data."
        )));
    }
    Ok(())
}

/// `sourceIsLocal`.
pub fn source_is_local(source: &str) -> bool {
    ["./", "/", "~", ".."].iter().any(|p| source.starts_with(p))
}

/// `resolveComposeVolumeSourceVariables`.
pub fn resolve_compose_volume_source_variables(
    source: &str,
    variables: Option<&HashMap<String, String>>,
) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re =
        RE.get_or_init(|| Regex::new(r"\$\{([A-Za-z_][A-Za-z0-9_]*)(?::?-([^}]*))?\}").unwrap());
    let resolve = |values: Option<&HashMap<String, String>>| {
        re.replace_all(source, |c: &regex::Captures| {
            let value = values
                .and_then(|v| v.get(&c[1]))
                .cloned()
                .unwrap_or_default();
            if !value.is_empty() {
                return value;
            }
            match c.get(2).map(|m| m.as_str()) {
                Some(default) if !default.is_empty() => default.to_string(),
                _ => format!("${{{}}}", &c[1]),
            }
        })
        .into_owned()
    };
    let without = resolve(None);
    let Some(variables) = variables else {
        return without;
    };
    let with = resolve(Some(variables));
    if source_is_local(&with) {
        with
    } else {
        without
    }
}

/// Result of `parseDockerVolumeString`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumeSpec {
    pub source: Option<String>,
    pub target: Option<String>,
    pub mode: Option<String>,
}

const VALID_MODES: &[&str] = &[
    "ro",
    "rw",
    "z",
    "Z",
    "rslave",
    "rprivate",
    "rshared",
    "slave",
    "private",
    "shared",
    "cached",
    "delegated",
    "consistent",
];

fn split_mode(remaining: &str) -> (String, Option<String>) {
    match remaining.rfind(':') {
        Some(last) if VALID_MODES.contains(&&remaining[last + 1..]) => (
            remaining[..last].to_string(),
            Some(remaining[last + 1..].to_string()),
        ),
        _ => (remaining.to_string(), None),
    }
}

fn starts_with_drive(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':'
}

/// `parseDockerVolumeString`.
pub fn parse_docker_volume_string(
    volume: &str,
    variables: Option<&HashMap<String, String>>,
) -> Result<VolumeSpec, ParseError> {
    static VAR: OnceLock<Regex> = OnceLock::new();
    static PLACEHOLDER: OnceLock<Regex> = OnceLock::new();
    static SIMPLE_VAR: OnceLock<Regex> = OnceLock::new();
    static NAMED_VAR: OnceLock<Regex> = OnceLock::new();
    let var = VAR.get_or_init(|| Regex::new(r"\$\{[^}]*\}").unwrap());
    let placeholder = PLACEHOLDER.get_or_init(|| Regex::new(r"\x00(\d+)\x00").unwrap());

    let volume = volume.trim_matches(|c| matches!(c, ' ' | '\t' | '\n' | '\r' | '\0' | '\x0B'));
    // A Compose variable such as ${VAR:-/data} can contain a colon: hide variables while splitting.
    let mut texts: Vec<String> = Vec::new();
    let hidden = var
        .replace_all(volume, |c: &regex::Captures| {
            texts.push(c[0].to_string());
            format!("\x00{}\x00", texts.len() - 1)
        })
        .into_owned();
    let restore = |part: Option<String>| {
        part.map(|p| {
            placeholder
                .replace_all(&p, |c: &regex::Captures| {
                    c[1].parse::<usize>()
                        .ok()
                        .and_then(|i| texts.get(i).cloned())
                        .unwrap_or_else(|| c[0].to_string())
                })
                .into_owned()
        })
    };

    let colons = hidden.matches(':').count();
    let (source, target, mode): (String, String, Option<String>) = match colons {
        0 => (hidden.clone(), hidden.clone(), None),
        1 => {
            let (s, t) = hidden.split_once(':').unwrap_or((&hidden, ""));
            (s.to_string(), t.to_string(), None)
        }
        _ if starts_with_drive(&hidden) => match hidden[2..].find(':').map(|i| i + 2) {
            Some(second) => {
                let source = hidden[..second].to_string();
                let remaining = &hidden[second + 1..];
                if colons == 2 {
                    (source, remaining.to_string(), None)
                } else {
                    let (target, mode) = split_mode(remaining);
                    (source, target, mode)
                }
            }
            None => (hidden.clone(), hidden.clone(), None),
        },
        2 => {
            let last = hidden.rfind(':').unwrap_or(0);
            if VALID_MODES.contains(&&hidden[last + 1..]) {
                let without = &hidden[..last];
                match without.split_once(':') {
                    Some((s, t)) => (
                        s.to_string(),
                        t.to_string(),
                        Some(hidden[last + 1..].to_string()),
                    ),
                    None => (
                        without.to_string(),
                        without.to_string(),
                        Some(hidden[last + 1..].to_string()),
                    ),
                }
            } else {
                let (s, t) = hidden.split_once(':').unwrap_or((&hidden, ""));
                (s.to_string(), t.to_string(), None)
            }
        }
        _ => {
            let (s, remaining) = hidden.split_once(':').unwrap_or((&hidden, ""));
            let (target, mode) = split_mode(remaining);
            (s.to_string(), target, mode)
        }
    };

    let mut source = restore(Some(source));
    let target = restore(Some(target));
    let mode = restore(mode);

    if let Some(s) = source
        .as_mut()
        .filter(|s| !s.is_empty() && s.as_str() != "0")
    {
        *s = resolve_compose_volume_source_variables(s, variables);
    }

    if let Some(source) = &source {
        let simple =
            SIMPLE_VAR.get_or_init(|| Regex::new(r"\$\{[A-Za-z_][A-Za-z0-9_]*\}").unwrap());
        validate_shell_safe_path(&simple.replace_all(source, ""), "volume source").map_err(|e| {
            ParseError(format!(
                "Invalid Docker volume definition: {} Please use safe path names without shell metacharacters.",
                e.0
            ))
        })?;
        let named =
            NAMED_VAR.get_or_init(|| Regex::new(r"\$\{([A-Za-z_][A-Za-z0-9_]*)\}").unwrap());
        if variables.is_some() && source_is_local(source) {
            if let Some(c) = named.captures(source) {
                let name = &c[1];
                return Err(ParseError(format!(
                    "Invalid Docker volume definition: the host path {source} uses the variable {name}, which has no value. Set {name} in the environment variables of the resource, or add a default, for example ${{{name}:-/data}}."
                )));
            }
        }
    }
    if let Some(target) = &target {
        reject_compose_volume_target_variable(target, "Invalid Docker volume definition")?;
        validate_shell_safe_path(target, "volume target").map_err(|e| {
            ParseError(format!(
                "Invalid Docker volume definition: {} Please use safe path names without shell metacharacters.",
                e.0
            ))
        })?;
    }

    Ok(VolumeSpec {
        source,
        target,
        mode,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(v: &str) -> (Option<String>, Option<String>, Option<String>) {
        let p = parse_docker_volume_string(v, None).unwrap();
        (p.source, p.target, p.mode)
    }

    #[test]
    fn volume_forms() {
        let s = |x: &str| Some(x.to_string());
        assert_eq!(parsed("gitea:/data"), (s("gitea"), s("/data"), None));
        assert_eq!(parsed("./data:/app:ro"), (s("./data"), s("/app"), s("ro")));
        assert_eq!(
            parsed("${DATA:-./data}:/app"),
            (s("./data"), s("/app"), None)
        );
        assert_eq!(parsed("C:/x:/app"), (s("C:/x"), s("/app"), None));
        assert_eq!(parsed("ab:/b:c"), (s("ab"), s("/b:c"), None));
        // PHP's `^[A-Za-z]:` reads a one-letter source as a Windows drive.
        assert_eq!(parsed("a:/b:c"), (s("a:/b"), s("c"), None));
        assert_eq!(parsed("myvolume"), (s("myvolume"), s("myvolume"), None));
    }

    #[test]
    fn unsafe_source_message() {
        let e = parse_docker_volume_string("./data;touch /tmp/pwned:/data", None).unwrap_err();
        assert!(e.0.starts_with("Invalid Docker volume definition: Invalid volume source: contains forbidden character ';'"));
    }
}
