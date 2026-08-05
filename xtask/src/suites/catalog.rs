use crate::process::Step;

pub static STD_CATALOG: &[Step] = &[
    Step::new(
        "std-catalog-test",
        "Test conduit-std-catalog",
        "cargo",
        &["test", "-p", "conduit-std-catalog"],
    ),
    Step::new(
        "std-catalog-thumb-check",
        "Check conduit-std-catalog for thumbv6m-none-eabi (no default features)",
        "cargo",
        &["check", "-p", "conduit-std-catalog", "--no-default-features", "--target", "thumbv6m-none-eabi"],
    ),
];
