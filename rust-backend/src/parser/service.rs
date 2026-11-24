//! `serviceParser` (+ `updateCompose`) port.
//!
//! Ported so far: the paths before the per-service main loop, exactly as PHP runs them — empty compose,
//! invalid YAML and a non-array `services` key give an empty result; service names and scale are
//! validated; volume definitions are validated with `parseDockerVolumeString`. A compose that needs the
//! main loop (magic variables, domains, labels, storages) returns [`Outcome::NotPorted`] instead of a
//! wrong result, so callers and the golden test can tell "not ported" from "different".

use super::php::{Array, Value};
use super::validate::{
    parse_docker_volume_string, validate_compose_service_name, validate_compose_service_scale,
    validate_service_name_shell_safe, ParseError,
};
use super::yaml::parse_compose;

/// What the parser reads from the `services` row.
#[derive(Debug, Clone)]
pub struct ServiceInput {
    pub uuid: String,
    pub docker_compose_raw: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// The generated compose (PHP returns it as a Collection; `toArray()` of it).
    Compose(Value),
    /// The compose reaches code that is not ported yet.
    NotPorted(&'static str),
}

/// `serviceParser($service)`.
pub fn service_parser(input: &ServiceInput) -> Result<Outcome, ParseError> {
    let empty = || Ok(Outcome::Compose(Value::Array(Array::new())));
    let Some(compose) = input
        .docker_compose_raw
        .as_deref()
        .filter(|c| !c.is_empty())
    else {
        return empty();
    };
    let Ok(yaml) = parse_compose(compose) else {
        return empty();
    };
    let services = match yaml.data_get("services") {
        None => Value::Array(Array::new()),
        Some(v) => v.clone(),
    };
    let Value::Array(services) = services else {
        return empty();
    };

    // Presave loop: names and scale.
    for (name, service) in services.iter() {
        let name = name.as_string();
        validate_service_name_shell_safe(&name)?;
        validate_compose_service_name(&name)?;
        validate_compose_service_scale(&name, service)?;
    }
    if services.is_empty() {
        // ponytail: an empty `services` map still runs the top-level volume/network handling in PHP;
        // nothing in it is ported yet.
        return Ok(Outcome::NotPorted("top-level compose sections"));
    }

    // Volume definitions are validated in the main loop; check them so invalid composes fail with the
    // PHP message even though the rest of the loop is not ported.
    for (_, service) in services.iter() {
        let Some(Value::Array(volumes)) = service.data_get("volumes") else {
            continue;
        };
        for (_, volume) in volumes.iter() {
            if let Value::Str(v) = volume {
                // ponytail: PHP passes the resource's variables when the volume has `${`; without the
                // environment rows only the injection checks can run, not the "variable has no value" one.
                parse_docker_volume_string(v, None)?;
            }
        }
    }
    Ok(Outcome::NotPorted("serviceParser main loop"))
}
