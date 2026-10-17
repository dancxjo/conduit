/// Production browser packaging must optimize the entire dependency graph.
pub const BROWSER_RELEASE_ENVIRONMENT: [(&str, &str); 3] = [
    ("CARGO_PROFILE_RELEASE_OPT_LEVEL", "z"),
    ("CARGO_PROFILE_RELEASE_LTO", "fat"),
    ("CARGO_PROFILE_RELEASE_CODEGEN_UNITS", "1"),
];
