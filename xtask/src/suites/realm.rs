use crate::process::Step;

pub static REALM: &[Step] = &[
    Step::new(
        "realm-test",
        "Test conduit-realm",
        "cargo",
        &["test", "-p", "conduit-realm"],
    ),
    Step::new(
        "realm-thumb-check",
        "Check conduit-realm for thumbv6m-none-eabi",
        "cargo",
        &["check", "-p", "conduit-realm", "--target", "thumbv6m-none-eabi"],
    ),
];
