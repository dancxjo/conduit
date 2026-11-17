use super::*;
fn args(command: &Command) -> Vec<String> {
    command
        .get_args()
        .map(|s| s.to_str().unwrap().to_owned())
        .collect()
}
#[test]
fn default_product_commands_retain_exact_existing_targets_and_features() {
    assert_eq!(
        args(&command_for(None, false, None)),
        ["build", "--locked", "--release", "-p", "conduit"]
    );
    let cross = command_for(Some("aarch64-unknown-linux-gnu"), false, None);
    assert_eq!(
        args(&cross),
        [
            "+1.98.1",
            "build",
            "--locked",
            "--release",
            "-p",
            "conduit",
            "--target",
            "aarch64-unknown-linux-gnu"
        ]
    );
    assert!(cross.get_envs().any(
        |(k, v)| k == "CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER"
            && v == Some(std::ffi::OsStr::new("aarch64-linux-gnu-gcc"))
    ));
}
#[test]
fn native_feature_uses_same_locked_producer_and_only_explicit_linker_search() {
    let search = std::ffi::OsStr::new("/fixture/target-sdk");
    let native = command_for(Some("x86_64-unknown-linux-gnu"), true, Some(search));
    assert_eq!(
        args(&native),
        [
            "build",
            "--locked",
            "--release",
            "-p",
            "conduit",
            "--target",
            "x86_64-unknown-linux-gnu",
            "--features",
            "native-display"
        ]
    );
    assert!(native
        .get_envs()
        .any(|(k, v)| k == "LIBRARY_PATH" && v == Some(search)));
    assert!(!command_for(None, false, None)
        .get_envs()
        .any(|(k, _)| k == "LIBRARY_PATH"));
}
