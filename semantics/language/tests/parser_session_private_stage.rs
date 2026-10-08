//! Private extraction checkpoints. These tests do not assert public Session acceptance.
#![cfg(feature = "parser-model-selection")]
extern crate alloc;
#[path = "../src/lib.rs"]
mod language;
pub use language::revision;
pub use language::*;
#[path = "../src/parser_session_fixed_ingress.rs"]
mod parser_session_fixed_ingress;
#[path = "../src/parser_session_historical_base.rs"]
mod parser_session_historical_base;
#[path = "../src/parser_session_mixed_custody.rs"]
mod parser_session_mixed_custody;
#[path = "../src/parser_session_numeric_custody.rs"]
mod parser_session_numeric_custody;
#[path = "../src/parser_session_numeric_plan_storage.rs"]
mod parser_session_numeric_plan_storage;
#[path = "../src/parser_session_profile.rs"]
mod parser_session_profile;
#[path = "../src/parser_session_revision_custody.rs"]
mod parser_session_revision_custody;

#[path = "../src/parser_session_stage.rs"]
mod parser_session_stage;

#[path = "../src/parser_session_target_registry.rs"]
mod parser_session_target_registry;

#[path = "../src/parser_session_candidate_admission.rs"]
mod parser_session_candidate_admission;

#[path = "../src/parser_canonical_schema.rs"]
mod parser_canonical_schema;

#[path = "../src/parser_canonical_composition.rs"]
mod parser_canonical_composition;

#[path = "../src/parser_canonical_refinement.rs"]
mod parser_canonical_refinement;

#[path = "../src/parser_session_revision_stage.rs"]
mod parser_session_revision_stage;
