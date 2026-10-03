#[path = "build_support/graph.rs"]
mod graph;
#[path = "build_support/lower.rs"]
mod lower;
use conduit_core::StructuredInfoTypeShape;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, ProfileCatalog, StartupCatalog,
};
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=types.conduit");
    println!("cargo:rerun-if-changed=listening.conduit");
    println!("cargo:rerun-if-changed=translation.conduit");
    println!("cargo:rerun-if-changed=timing.conduit");
    println!("cargo:rerun-if-changed=intent.conduit");
    println!("cargo:rerun-if-changed=timing_projection.conduit");
    println!("cargo:rerun-if-changed=control_projection.conduit");
    println!("cargo:rerun-if-changed=duration_projection.conduit");
    println!("cargo:rerun-if-changed=duration_render.conduit");
