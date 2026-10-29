//! Actual NativePlay cold installer, not native Root/SDK setup or radio proof.
use super::*;
type Trial = (
    NativePlay,
    RootIssuer,
    Option<ReadyCredentialBase<Source>>,
    Option<ReadyNetworkJoinBase<Provider>>,
    Result<(), ()>,
    allocation_probe::Observation,
    Option<allocation_probe::Observation>,
);
fn trial(failure: Option<usize>) -> Trial {
    // Source facts, actual Ready owner descriptions and fixture Root policy are
    // deliberately established before this installer's allocation boundary.
    let image = source_image();
    let pins = image.identity().unwrap();
    let mut issuer = issuer();
    let (mut source, mut join) = ready();
    let mut play = NativePlay::new();
    // Instrumentation uses fixed TLS Cells; warm/reset it before injection.
    let _ = flat::phase_measurements();
    let _ = flat::phased_refusal();
    let mut admitted = None;
    let (result, memory) = allocation_probe::observe(|| {
        let _refusal = failure.map(allocation_probe::refuse_nth_allocation);
        play.install_linked_phased_held(
            &image,
            &pins,
            c::OfferGeneration(u64::MAX),
            &mut source,
            &mut join,
            &mut issuer,
            || {
                admitted = allocation_probe::current_observation();
                Ok(())
            },
        )
    });
    (play, issuer, source, join, result, memory, admitted)
}
#[test]
fn each_actual_installer_allocation_refuses_before_admission_and_returns_ready_owners() {
    let (_, _, _, _, result, memory, admitted) = trial(None);
    result.unwrap();
    let admitted = admitted.unwrap();
    assert_eq!(
        (admitted.allocations, admitted.reallocations),
        (memory.allocations, memory.reallocations)
    );
    let attempts = memory.allocations + memory.reallocations;
    assert!(attempts > 26 && attempts < 256);
    let name = format!(
        "{}::installer_failure_child",
        module_path!().split_once("::").unwrap().1
    );
    for index in 0..attempts {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &name, "--ignored", "--nocapture"])
            .env("CONDUIT_INSTALLER_FAILURE_INDEX", index.to_string())
            .env("RUST_BACKTRACE", "0")
            .output()
            .unwrap();
        assert!(
            output.status.success()
                && String::from_utf8_lossy(&output.stdout)
                    .contains("installer-allocation-refusal-verified"),
            "allocation {index}/{attempts} failed: {} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    println!("all {attempts} actual NativePlay installer allocation attempts refuse before admission; same Ready owners/issuer retry");
}
#[test]
#[ignore = "finite parent launches one isolated child per actual installer allocation"]
fn installer_failure_child() {
    let index = std::env::var("CONDUIT_INSTALLER_FAILURE_INDEX")
        .unwrap()
        .parse()
        .unwrap();
    let (mut play, mut issuer, mut source, mut join, result, memory, admitted) = trial(Some(index));
    assert!(result.is_err());
    assert!(admitted.is_none());
    assert!(!play.attempted && play.active.is_none() && play.flat_refusal.is_empty());
    assert!(play.prepared.is_none() && play.source.is_none() && play.join.is_none());
    assert!(source.is_some() && join.is_some());
    assert_eq!(issuer.next_play_sequence(), Some(1));
    assert_eq!(memory.retained_bytes(), 0);
    let image = source_image();
    let pins = image.identity().unwrap();
    play.install_linked_phased_held(
        &image,
        &pins,
        c::OfferGeneration(u64::MAX),
        &mut source,
        &mut join,
        &mut issuer,
        || Ok(()),
    )
    .unwrap();
    assert_eq!(play.active.as_ref().unwrap().play_sequence, 1);
    println!("installer-allocation-refusal-verified");
}
#[test]
fn temporary_local_base_id_refusal_returns_before_any_ready_owner_moves() {
    let image = source_image();
    let pins = image.identity().unwrap();
    let mut issuer = issuer();
    let (mut source, mut join) = ready();
    let mut play = NativePlay::new();
    let (result, memory) = allocation_probe::observe(|| {
        let _refusal =
            allocation_probe::refuse_next_allocation_size(c::LOCAL_BASE_IMPLEMENTATION_ID.len());
        play.install_linked_phased_held(
            &image,
            &pins,
            c::OfferGeneration(u64::MAX),
            &mut source,
            &mut join,
            &mut issuer,
            || panic!("refused allocation cannot admit"),
        )
    });
    assert!(result.is_err());
    assert_eq!(flat::phased_refusal(), Some((116, false, false)));
    assert!(source.is_some() && join.is_some() && !play.attempted);
    assert_eq!(memory.retained_bytes(), 0);
}
