/// SHA-256 of `scripts/validate.sh`, computed at build time by `build.rs`.
/// Substituted into `templates/init/.githooks/pre-commit` by `forge init`.
pub const VALIDATE_SH_SHA: &str = env!("VALIDATE_SH_SHA");

pub mod error;
pub mod manifest;
pub mod module;
pub mod ontology;
pub mod parse;
pub mod provider;
pub mod result;
pub mod services;
pub mod target;
pub mod view;
pub mod yaml;

#[cfg(feature = "assemble")]
pub mod assemble;

#[cfg(feature = "assemble")]
pub mod transform;

#[cfg(feature = "validate")]
pub mod validate;
