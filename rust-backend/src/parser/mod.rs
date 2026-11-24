//! Docker Compose parsers (port of `bootstrap/helpers/parsers.php`: `serviceParser`, `updateCompose`,
//! `applicationParser`) and the Symfony-compatible YAML layer they need.
//!
//! The oracle is the PHP golden test (`tests/Feature/ServiceParserReferenceTest.php`) and its expected
//! files under `tests/Fixtures/parser/expected/`; see `golden.rs`.

#![allow(dead_code)]

pub mod php;
pub mod service;
pub mod validate;
pub mod yaml;

#[cfg(test)]
mod golden;
