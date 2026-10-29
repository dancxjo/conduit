use super::*;

#[test]
fn changed_provider_refuses_before_replacement_execution_and_clears_old_receipt() {
    let _process = microphone_process();
    let script = format!("#!/bin/sh\nif [ \"$2\" = -l ]; then printf '{}'; exit 0; fi\ndd if=/dev/zero bs=32 count=1 2>/dev/null\n",listing().replace('\n',"\\n"));
    let (root, executable) = fixture(&script, "changed-provider");
    let discovery = AlsaMicrophoneDiscovery::inspect(&executable).unwrap();
    let selected = discovery.observations[0].clone();
    let mut adapter = discovery
        .initialize(
            &selected,
            MicrophoneLimits {
                capture_milliseconds: 1,
                timeout: Duration::from_secs(2),
            },
        )
        .unwrap();
    adapter.capture(|| false).unwrap();
    assert!(adapter.last_receipt.is_some());
    fs::write(
        &executable,
        "#!/bin/sh\ntouch \"$0.replacement-ran\"\ndd if=/dev/zero bs=32 count=1 2>/dev/null\n",
    )
    .unwrap();
    let result = adapter.capture(|| false);
    let replacement_ran = root.join("arecord.replacement-ran").exists();
    fs::remove_dir_all(root).unwrap();
    assert_eq!(result, Err(MicrophoneFailure::SelectionDrift));
    assert!(!replacement_ran);
    assert!(adapter.take_receipt().is_none());
}

#[test]
fn missing_nonexecutable_and_oversized_provider_refusals_do_not_launch_capture() {
    for kind in ["missing", "nonexecutable", "oversized"] {
        let (root, discovery) = parsed_fixture("#!/bin/sh\ntouch \"$0.ran\"\n", kind);
        let path = discovery.executable.clone();
        let selected = discovery.observations[0].clone();
        let mut adapter = discovery
            .initialize(
                &selected,
                MicrophoneLimits {
                    capture_milliseconds: 1,
                    timeout: Duration::from_secs(2),
                },
            )
            .unwrap();
        let expected = match kind {
            "missing" => {
                fs::remove_file(&path).unwrap();
                MicrophoneFailure::MissingProvider
            }
            "nonexecutable" => {
                fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
                MicrophoneFailure::InvalidProvider
            }
            _ => {
                fs::OpenOptions::new()
                    .write(true)
                    .open(&path)
                    .unwrap()
                    .set_len(MAXIMUM_EXECUTABLE_BYTES + 1)
                    .unwrap();
                MicrophoneFailure::InvalidProvider
            }
        };
        assert_eq!(adapter.capture(|| false), Err(expected));
        assert!(!root.join("arecord.ran").exists());
        assert!(adapter.take_receipt().is_none());
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn cancellation_before_capture_does_not_invoke_or_revalidate_a_missing_provider() {
    let (root, discovery) = parsed_fixture("#!/bin/sh\ntouch \"$0.ran\"\n", "cancel-before-launch");
    let selected = discovery.observations[0].clone();
    let path = discovery.executable.clone();
    let mut adapter = discovery
        .initialize(
            &selected,
            MicrophoneLimits {
                capture_milliseconds: 1,
                timeout: Duration::from_secs(2),
            },
        )
        .unwrap();
    fs::remove_file(path).unwrap();
    assert_eq!(adapter.capture(|| true), Err(MicrophoneFailure::Cancelled));
    assert!(adapter.take_receipt().is_none());
    fs::remove_dir_all(root).unwrap();
}
