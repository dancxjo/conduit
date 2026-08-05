use crate::process::Step;

pub static OBSERVATORY: &[Step] = &[
    Step::new(
        "observatory-test",
        "Test conduit-observatory",
        "cargo",
        &["test", "-p", "conduit-observatory"],
    ),
    Step::new(
        "observatory-thumb-check",
        "Check conduit-observatory for thumbv6m-none-eabi",
        "cargo",
        &["check", "-p", "conduit-observatory", "--target", "thumbv6m-none-eabi"],
    ),
];
