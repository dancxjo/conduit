use super::{host_release_artifact_options, host_release_options};

fn arguments(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

#[test]
fn host_release_dispatch_accepts_supported_make_entrance() {
    let arguments = arguments(&[
        "make",
        "host",
        "release",
        "--platform",
        "windows",
        "--output",
        "target/releases",
    ]);

    assert_eq!(host_release_options(&arguments), Some(&arguments[3..]));
}

#[test]
fn host_release_dispatch_retains_internal_direct_entrance() {
    let arguments = arguments(&[
        "host",
        "release",
        "--platform",
        "macos",
        "--output",
        "target/releases",
    ]);

    assert_eq!(host_release_options(&arguments), Some(&arguments[2..]));
}

#[test]
fn unrelated_make_commands_stay_on_the_full_dispatcher() {
    let arguments = arguments(&["make", "conduitos", "release"]);
    assert_eq!(host_release_options(&arguments), None);
}

#[test]
fn artifact_review_routes_both_supported_entrances_without_full_xtask_fallback() {
    for prefix in [
        &["make", "host", "release-artifact"][..],
        &["host", "release-artifact"][..],
    ] {
        let mut args = arguments(prefix);
        args.extend(arguments(&["--configuration", "host.conduit"]));
        assert_eq!(
            host_release_artifact_options(&args),
            Some(&args[prefix.len()..])
        );
        assert!(host_release_options(&args).is_none());
    }
    assert!(
        host_release_artifact_options(&arguments(&["make", "host", "release-artifact-other"]))
            .is_none()
    );
}
