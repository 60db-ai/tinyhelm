//! Golden checks against the PHP parser reference outputs (`tests/Fixtures/parser/expected/`).

use std::path::PathBuf;

use super::php::{Array, Value};
use super::service::{service_parser, Outcome, ServiceInput};
use super::yaml;

pub(super) fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/Fixtures/parser")
}

pub(super) fn first_difference(name: &str, php: &str, rust: &str) -> String {
    let line = rust
        .lines()
        .zip(php.lines())
        .position(|(a, b)| a != b)
        .unwrap_or_else(|| php.lines().count().min(rust.lines().count()));
    format!(
        "{name}: first difference at line {}\n  php:  {:?}\n  rust: {:?}",
        line + 1,
        php.lines().nth(line).unwrap_or("<eof>"),
        rust.lines().nth(line).unwrap_or("<eof>")
    )
}

/// The Symfony dumper port must reproduce every expected file byte for byte from its parsed value
/// (the files were written by `Yaml::dump($snapshot, 12, 2, Yaml::DUMP_MULTI_LINE_LITERAL_BLOCK)`).
#[test]
fn dumper_round_trips_every_expected_file() {
    let mut failures = Vec::new();
    let mut count = 0;
    for entry in std::fs::read_dir(fixtures().join("expected")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
            continue;
        }
        count += 1;
        let text = std::fs::read_to_string(&path).unwrap();
        let value = yaml::parse(&text).unwrap();
        let dumped = yaml::dump(&value, 12);
        if dumped != text {
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            failures.push(first_difference(&name, &text, &dumped));
        }
    }
    assert!(count >= 12, "expected files missing");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Service cases of `serviceParserReferenceCases()` (fixture, expected file). The setting variations
/// (proxy, labels, application settings, migrated parts) only matter once the main loop is ported.
const SERVICE_CASES: &[(&str, &str)] = &[
    ("service-reference.yaml", "reference"),
    (
        "service-reference.yaml",
        "reference-with-exact-traefik-labels",
    ),
    (
        "service-reference.yaml",
        "reference-with-exact-caddy-labels",
    ),
    (
        "service-reference.yaml",
        "reference-with-application-settings",
    ),
    ("service-reference.yaml", "reference-with-migrated-services"),
    ("service-unsafe.yaml", "unsafe-volume-path"),
    ("service-unsafe-name.yaml", "unsafe-service-name"),
    ("service-invalid-yaml.yaml", "invalid-yaml"),
];

const APPLICATION_CASES: &[&str] = &[
    "application",
    "application-with-exact-traefik-labels",
    "application-with-exact-caddy-labels",
    "application-with-settings",
];

enum CaseResult {
    Dumped(String),
    NotPorted(&'static str),
}

fn str_map(entries: Vec<(&str, Value)>) -> Value {
    Value::Array(entries.into_iter().map(|(k, v)| (k.into(), v)).collect())
}

/// `serviceParserReferenceSnapshot` for the rows the port manages so far.
fn service_snapshot(compose: Value) -> Value {
    str_map(vec![
        ("applications", Value::Array(Array::new())),
        ("databases", Value::Array(Array::new())),
        ("environment_variables", Value::Array(Array::new())),
        ("compose", compose),
    ])
}

/// One step: `parses` parser runs (deploy runs two: Create + StartService); the snapshot takes the
/// last result, or the error.
fn run_step(input: &ServiceInput, parses: usize) -> Result<Result<Value, String>, &'static str> {
    let mut result = None;
    for _ in 0..parses {
        match service_parser(input) {
            Ok(Outcome::Compose(c)) => result = Some(Ok(c)),
            Ok(Outcome::NotPorted(what)) => return Err(what),
            Err(e) => {
                result = Some(Err(format!("Exception: {e}")));
                break;
            }
        }
    }
    Ok(result.expect("at least one parse"))
}

/// The three steps of `it('parses the compose exactly as recorded')`.
fn run_service_case(fixture: &str) -> CaseResult {
    let raw = std::fs::read_to_string(fixtures().join(fixture)).unwrap();
    let input = ServiceInput {
        uuid: "referenceservice00000001".into(),
        docker_compose_raw: Some(raw),
    };
    let mut snapshot = Array::new();
    let steps: &[(&str, usize)] = &[
        ("1_deploy", 2),
        // Steps 2 and 3 save domains on applications; with no application rows they only parse again.
        ("2_custom_domains", 1),
        ("3_domain_on_applications_without_one", 1),
    ];
    for (label, parses) in steps {
        match run_step(&input, *parses) {
            Err(what) => return CaseResult::NotPorted(what),
            Ok(Ok(compose)) => snapshot.set(*label, service_snapshot(compose)),
            Ok(Err(error)) => {
                snapshot.set(*label, str_map(vec![("error", Value::str(error))]));
                // The test stops after a failed deploy; a later failed step does not stop it.
                if *label == "1_deploy" {
                    break;
                }
            }
        }
    }
    CaseResult::Dumped(yaml::dump(&Value::Array(snapshot), 12))
}

/// Every reference case: the ones the port can produce must match the PHP output exactly; the others
/// are listed as not ported. The count of matching cases may only go up.
#[test]
fn reference_cases_match_php() {
    let mut matched = Vec::new();
    let mut not_ported = Vec::new();
    let mut failures = Vec::new();
    for (fixture, expected) in SERVICE_CASES {
        let php =
            std::fs::read_to_string(fixtures().join(format!("expected/{expected}.yaml"))).unwrap();
        match run_service_case(fixture) {
            CaseResult::Dumped(rust) if rust == php => matched.push(*expected),
            CaseResult::Dumped(rust) => failures.push(first_difference(expected, &php, &rust)),
            CaseResult::NotPorted(what) => not_ported.push(format!("{expected} ({what})")),
        }
    }
    for expected in APPLICATION_CASES {
        not_ported.push(format!("{expected} (applicationParser)"));
    }
    eprintln!(
        "parser golden: {}/{} cases match; not ported: {}",
        matched.len(),
        SERVICE_CASES.len() + APPLICATION_CASES.len(),
        not_ported.join(", ")
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert!(matched.len() >= 3, "regression: {matched:?}");
}
