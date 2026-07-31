use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use conduit_compile::{InstalledProfile, compile_source};
use conduit_core::NodeContract;
use conduit_media::{
    DEMUX_CONTRACT, ENCODE_CONTRACT, MUX_CONTRACT, PROBE_CONTRACT, WAVE_LITERAL_CONTRACT,
    register_deterministic_codec_providers, register_deterministic_media_providers,
    register_media_codec_contracts, register_media_contracts,
};
use conduit_runtime::{
    AvailabilityState, CompiledInHostService, Handler, Registry, RegistryError, RunIo,
    RuntimeError, Value as RuntimeValue,
};

fn workspace_file(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

fn register_codec_fixture_provider(
    registry: &mut Registry,
    providers: &[(
        &'static NodeContract<'static>,
        &'static str,
        &'static str,
        &'static str,
        &'static [u8],
    )],
) -> Result<(), RegistryError> {
    struct MediaCodecFixtureProvider;

    impl Handler for MediaCodecFixtureProvider {
        fn run(
            &mut self,
            _node: &conduit_panel::Node,
            _inputs: &[RuntimeValue],
            _io: &mut RunIo<'_>,
        ) -> Result<Vec<RuntimeValue>, RuntimeError> {
            Ok(Vec::new())
        }
    }

    static NO_AUTHORITIES: [conduit_core::SemanticHash; 0] = [];
    for (contract, implementation_id, artifact_id, entrypoint, source_bytes) in providers {
        registry.register_compiled_in_host_service(CompiledInHostService {
            contract,
            implementation_id,
            artifact_id,
            entrypoint,
            source_bytes,
            required_authorities: &NO_AUTHORITIES,
            factory: || Box::new(MediaCodecFixtureProvider),
            validate_config: |_node: &conduit_panel::Node| Ok(()),
        })?;
    }
    Ok(())
}

fn register_cross_host_fixture_providers(registry: &mut Registry) -> Result<(), RegistryError> {
    register_codec_fixture_provider(
        registry,
        &[
            (
                &WAVE_LITERAL_CONTRACT,
                "conduit.media/wave-literal-cross-host-fixture-alpha",
                "conduit.media/wave-literal-cross-host-fixture-alpha-artifact",
                "media-wave-literal-cross-host-fixture-alpha",
                b"conduit.media/wave-literal-cross-host-fixture-alpha.source",
            ),
            (
                &PROBE_CONTRACT,
                "conduit.media/wave-probe-cross-host-fixture-alpha",
                "conduit.media/wave-probe-cross-host-fixture-alpha-artifact",
                "media-wave-probe-cross-host-fixture-alpha",
                b"conduit.media/wave-probe-cross-host-fixture-alpha.source",
            ),
            (
                &DEMUX_CONTRACT,
                "conduit.media/wave-demux-cross-host-fixture-alpha",
                "conduit.media/wave-demux-cross-host-fixture-alpha-artifact",
                "media-wave-demux-cross-host-fixture-alpha",
                b"conduit.media/wave-demux-cross-host-fixture-alpha.source",
            ),
            (
                &MUX_CONTRACT,
                "conduit.media/wave-mux-cross-host-fixture-alpha",
                "conduit.media/wave-mux-cross-host-fixture-alpha-artifact",
                "media-wave-mux-cross-host-fixture-alpha",
                b"conduit.media/wave-mux-cross-host-fixture-alpha.source",
            ),
            (
                &conduit_media::DECODE_CONTRACT,
                "conduit.media/pcm-decode-cross-host-fixture-alpha",
                "conduit.media/pcm-decode-cross-host-fixture-alpha-artifact",
                "media-pcm-decode-cross-host-fixture-alpha",
                b"conduit.media/pcm-decode-cross-host-fixture-alpha.source",
            ),
            (
                &ENCODE_CONTRACT,
                "conduit.media/pcm-encode-cross-host-fixture-alpha",
                "conduit.media/pcm-encode-cross-host-fixture-alpha-artifact",
                "media-pcm-encode-cross-host-fixture-alpha",
                b"conduit.media/pcm-encode-cross-host-fixture-alpha.source",
            ),
        ],
    )?;
    register_codec_fixture_provider(
        registry,
        &[
            (
                &WAVE_LITERAL_CONTRACT,
                "conduit.media/wave-literal-cross-host-fixture-beta",
                "conduit.media/wave-literal-cross-host-fixture-beta-artifact",
                "media-wave-literal-cross-host-fixture-beta",
                b"conduit.media/wave-literal-cross-host-fixture-beta.source",
            ),
            (
                &PROBE_CONTRACT,
                "conduit.media/wave-probe-cross-host-fixture-beta",
                "conduit.media/wave-probe-cross-host-fixture-beta-artifact",
                "media-wave-probe-cross-host-fixture-beta",
                b"conduit.media/wave-probe-cross-host-fixture-beta.source",
            ),
            (
                &DEMUX_CONTRACT,
                "conduit.media/wave-demux-cross-host-fixture-beta",
                "conduit.media/wave-demux-cross-host-fixture-beta-artifact",
                "media-wave-demux-cross-host-fixture-beta",
                b"conduit.media/wave-demux-cross-host-fixture-beta.source",
            ),
            (
                &MUX_CONTRACT,
                "conduit.media/wave-mux-cross-host-fixture-beta",
                "conduit.media/wave-mux-cross-host-fixture-beta-artifact",
                "media-wave-mux-cross-host-fixture-beta",
                b"conduit.media/wave-mux-cross-host-fixture-beta.source",
            ),
            (
                &conduit_media::DECODE_CONTRACT,
                "conduit.media/pcm-decode-cross-host-fixture-beta",
                "conduit.media/pcm-decode-cross-host-fixture-beta-artifact",
                "media-pcm-decode-cross-host-fixture-beta",
                b"conduit.media/pcm-decode-cross-host-fixture-beta.source",
            ),
            (
                &ENCODE_CONTRACT,
                "conduit.media/pcm-encode-cross-host-fixture-beta",
                "conduit.media/pcm-encode-cross-host-fixture-beta-artifact",
                "media-pcm-encode-cross-host-fixture-beta",
                b"conduit.media/pcm-encode-cross-host-fixture-beta.source",
            ),
        ],
    )?;
    Ok(())
}

#[test]
fn typed_media_contracts_are_sealed_into_the_exact_compile_catalog() {
    let source = include_str!("../../../examples/media-audio-frame.panel");
    let mut registry = Registry::hosted_primitives();
    register_deterministic_media_providers(&mut registry).unwrap();
    register_deterministic_codec_providers(&mut registry).unwrap();
    let installed = InstalledProfile::observe_registry(source, &registry).unwrap();

    let literal = installed
        .input
        .catalog
        .external_leaf_contracts
        .iter()
        .find(|contract| contract.id == "conduit.media/audio-frame/literal")
        .expect("the exact input seals the domain contract");
    assert_eq!(literal.config.len(), 1);
    assert_eq!(literal.config[0].key, "fixture");
    assert_eq!(literal.outputs.len(), 1);
    assert_eq!(
        literal.outputs[0].value_type.id,
        "conduit.media/audio-frame"
    );
    assert!(installed.input.catalog.types.iter().any(|pin| {
        pin.id == "conduit.media/audio-frame"
            && pin.semantic_hash == literal.outputs[0].value_type.semantic_hash
    }));

    let document = compile_source(source, &installed.input).unwrap();
    assert!(document.nodes.iter().any(|node| {
        node.contract.id == "conduit.media/audio-frame/literal"
            && node.implementation.id == "conduit.media/audio-literal-deterministic"
    }));
}

#[test]
fn standalone_and_composed_media_panels_run_through_the_canonical_cli() {
    for (path, expected) in [
        (
            "examples/media-audio-frame.panel",
            "audio:s16le:48000:stereo:192",
        ),
        (
            "examples/media-frame-compose.panel",
            "audio:s16le:48000:stereo:192video:rgb24:2x2",
        ),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_conduct"))
            .arg(workspace_file(path))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
    }
}

#[test]
fn media_value_overflow_is_an_explicit_pressure_failure() {
    let source = include_str!("../../../examples/media-audio-frame.panel")
        .replacen("max_value_bytes = 64", "max_value_bytes = 4", 1)
        .replacen("max_queued_bytes = 64", "max_queued_bytes = 4", 1);
    let mut child = Command::new(env!("CARGO_BIN_EXE_conduct"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(source.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");
    assert!(String::from_utf8_lossy(&output.stderr).contains("CND-RUN-004"));
}

#[test]
fn exact_pcm_wave_operations_are_sealed_and_run_through_the_canonical_cli() {
    let source = include_str!("../../../examples/media-wave-roundtrip.panel");
    let mut registry = Registry::hosted_primitives();
    register_deterministic_media_providers(&mut registry).unwrap();
    register_deterministic_codec_providers(&mut registry).unwrap();
    let installed = InstalledProfile::observe_registry(source, &registry).unwrap();
    let document = compile_source(source, &installed.input).unwrap();
    for (contract, implementation) in [
        (
            "conduit.media/audio/encode",
            "conduit.media/pcm-encode-deterministic",
        ),
        (
            "conduit.media/container/mux",
            "conduit.media/wave-mux-deterministic",
        ),
        (
            "conduit.media/container/probe",
            "conduit.media/wave-probe-deterministic",
        ),
        (
            "conduit.media/container/demux",
            "conduit.media/wave-demux-deterministic",
        ),
        (
            "conduit.media/audio/decode",
            "conduit.media/pcm-decode-deterministic",
        ),
    ] {
        assert!(document.nodes.iter().any(|node| {
            node.contract.id == contract && node.implementation.id == implementation
        }));
    }

    for (path, expected) in [
        (
            "examples/media-wave-probe.panel",
            "wave:pcm-s16le:48000:2:1-track:192-frames:812-bytes",
        ),
        (
            "examples/media-wave-roundtrip.panel",
            "wave:pcm-s16le:48000:2:1-track:192-frames:812-bytesaudio:s16le:48000:stereo:192",
        ),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_conduct"))
            .arg(workspace_file(path))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
    }
}

#[test]
fn pcm_wave_binding_and_cord_bounds_fail_closed() {
    let wrong_profile = include_str!("../../../examples/media-wave-probe.panel")
        .replace("stereo-48000-192", "stereo-44100-192");
    let mut child = Command::new(env!("CARGO_BIN_EXE_conduct"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(wrong_profile.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("CND-CODEC-002"));

    let undersized = include_str!("../../../examples/media-wave-probe.panel")
        .replace("max_value_bytes = 1024", "max_value_bytes = 800");
    let mut child = Command::new(env!("CARGO_BIN_EXE_conduct"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(undersized.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("CND-RUN-004"));
}

#[test]
fn known_codec_contracts_report_missing_and_stale_providers_separately() {
    let source = include_str!("../../../examples/media-wave-probe.panel");
    let mut contract_only = Registry::default();
    register_media_contracts(&mut contract_only);
    register_media_codec_contracts(&mut contract_only);
    assert_eq!(
        contract_only
            .node_availability("conduit.media/container/probe")
            .state,
        AvailabilityState::ContractOnly
    );
    let panel = conduit_panel::parse(source).unwrap();
    assert_eq!(
        contract_only.resolve(&panel).unwrap_err().code,
        "CND-IMP-001"
    );

    let mut registry = Registry::hosted_primitives();
    register_deterministic_media_providers(&mut registry).unwrap();
    register_deterministic_codec_providers(&mut registry).unwrap();
    let installed = InstalledProfile::observe_registry(source, &registry).unwrap();
    let mut stale = installed.input.clone();
    let candidate = stale
        .candidates
        .iter_mut()
        .find(|candidate| {
            candidate.implementation.semantic_contract.id == "conduit.media/container/probe"
        })
        .unwrap();
    candidate.host_report.valid_until_tick = stale.current_tick - 1;
    stale.seal().unwrap();
    assert_eq!(
        compile_source(source, &stale).unwrap_err().code(),
        "CND-CMP-006"
    );
}

#[test]
fn overlapping_media_codec_contracts_are_observable_and_expose_multiple_implementations() {
    let source = include_str!("../../../examples/media-wave-roundtrip.panel");
    let mut registry = Registry::hosted_primitives();
    register_deterministic_media_providers(&mut registry).unwrap();
    register_deterministic_codec_providers(&mut registry).unwrap();
    register_cross_host_fixture_providers(&mut registry).unwrap();
    let installed = InstalledProfile::observe_registry(source, &registry).unwrap();

    for (contract, expected_implementations) in [
        (
            "conduit.media/container/probe",
            &[
                "conduit.media/wave-probe-deterministic",
                "conduit.media/wave-probe-cross-host-fixture-alpha",
                "conduit.media/wave-probe-cross-host-fixture-beta",
            ][..],
        ),
        (
            "conduit.media/container/mux",
            &[
                "conduit.media/wave-mux-deterministic",
                "conduit.media/wave-mux-cross-host-fixture-alpha",
                "conduit.media/wave-mux-cross-host-fixture-beta",
            ][..],
        ),
        (
            "conduit.media/container/demux",
            &[
                "conduit.media/wave-demux-deterministic",
                "conduit.media/wave-demux-cross-host-fixture-alpha",
                "conduit.media/wave-demux-cross-host-fixture-beta",
            ][..],
        ),
        (
            "conduit.media/audio/decode",
            &[
                "conduit.media/pcm-decode-deterministic",
                "conduit.media/pcm-decode-cross-host-fixture-alpha",
                "conduit.media/pcm-decode-cross-host-fixture-beta",
            ][..],
        ),
        (
            "conduit.media/audio/encode",
            &[
                "conduit.media/pcm-encode-deterministic",
                "conduit.media/pcm-encode-cross-host-fixture-alpha",
                "conduit.media/pcm-encode-cross-host-fixture-beta",
            ][..],
        ),
    ] {
        let mut implementations = installed
            .input
            .candidates
            .iter()
            .filter(|candidate| candidate.implementation.semantic_contract.id == contract)
            .map(|candidate| candidate.implementation.id.as_str())
            .collect::<Vec<_>>();
        implementations.sort_unstable();
        let mut expected = expected_implementations.to_vec();
        expected.sort_unstable();
        assert_eq!(implementations, expected, "contract {contract}");
    }
}

#[test]
fn implementation_preference_selects_overlapping_media_providers() {
    let source = include_str!("../../../examples/media-wave-roundtrip.panel");
    let mut registry = Registry::hosted_primitives();
    register_deterministic_media_providers(&mut registry).unwrap();
    register_deterministic_codec_providers(&mut registry).unwrap();
    register_cross_host_fixture_providers(&mut registry).unwrap();
    let installed = InstalledProfile::observe_registry(source, &registry).unwrap();

    for (preference, expected_decode, expected_encode) in [
        (
            [
                "conduit.media/pcm-decode-deterministic",
                "conduit.media/pcm-encode-deterministic",
            ],
            "conduit.media/pcm-decode-deterministic",
            "conduit.media/pcm-encode-deterministic",
        ),
        (
            [
                "conduit.media/pcm-decode-cross-host-fixture-alpha",
                "conduit.media/pcm-encode-cross-host-fixture-alpha",
            ],
            "conduit.media/pcm-decode-cross-host-fixture-alpha",
            "conduit.media/pcm-encode-cross-host-fixture-alpha",
        ),
        (
            [
                "conduit.media/pcm-decode-cross-host-fixture-beta",
                "conduit.media/pcm-encode-cross-host-fixture-beta",
            ],
            "conduit.media/pcm-decode-cross-host-fixture-beta",
            "conduit.media/pcm-encode-cross-host-fixture-beta",
        ),
    ] {
        let mut input = installed.input.clone();
        input.implementation_preference = preference
            .iter()
            .map(|implementation| implementation.to_string())
            .collect();
        input.seal().unwrap();

        let document = compile_source(source, &input).unwrap();
        for (contract, expected) in [
            ("conduit.media/audio/decode", expected_decode),
            ("conduit.media/audio/encode", expected_encode),
        ] {
            assert!(document.nodes.iter().any(|node| {
                node.contract.id == contract && node.implementation.id == expected
            }));
        }
    }
}
