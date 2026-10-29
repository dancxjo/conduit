//! Every allocation of the actual valid borrowed planner, isolated per process.
use super::*;
type Pair = n::ReadyNetworkPair<Inert, Inert>;
type Trial = (
    Pair,
    Result<(), conduit_planner::borrowed_local::BorrowedLocalPlanningRefusal>,
    allocation_probe::Observation,
    bool,
);
fn pair() -> Pair {
    let (source, join) = ready();
    Pair::new(source.unwrap(), join.unwrap())
}
fn trial(pair: Pair, failure: Option<usize>) -> Trial {
    let image = source_image();
    let issuer = issuer();
    let bases = [c::BaseImplementationId::from(
        c::LOCAL_BASE_IMPLEMENTATION_ID,
    )];
    let attachment =
        conduit_net::PreparedCanonicalNetworkOffer::attachment_local(CAP, IMPLEMENTATION, ARTIFACT);
    let mut observed = None;
    let mut consumed = false;
    let result = pair.prepare::<(), ()>(|owners| {
        let grants = [
            issuer
                .grant_ref(owners.credential_identity(), Operation::Credentials)
                .unwrap(),
            issuer
                .grant_ref(owners.join_identity(), Operation::Join)
                .unwrap(),
        ];
        let header = c::HostAdvertisementRef {
            protocol_version: c::PROTOCOL_VERSION,
            host_id: owners.join_identity().host.as_str(),
            boot_id: owners.join_identity().boot.as_str(),
            offer_generation: c::OfferGeneration(u64::MAX),
            profile: "environment/esp32-native-root@1",
            bases: c::BaseProviderAdvertisementRefs::Borrowed(&[]),
            resources: c::ResourceOfferRefs::Borrowed(&[]),
            capabilities: c::CapabilityOfferRefs::Borrowed(&[]),
            planner_capabilities: &[],
        };
        owners
            .with_advertisement(header, &attachment, |_, host| {
                observed = Some(allocation_probe::observe(|| {
                    let _refusal = failure.map(allocation_probe::refuse_nth_allocation);
                    conduit_planner::borrowed_local::with_linked_local_plan(
                        &image,
                        host,
                        &bases,
                        c::AuthorityGrantRefs::Borrowed(&grants),
                        1,
                        conduit_net::MAXIMUM_JOIN_INPUT_BYTES,
                        |_| {
                            consumed = true;
                        },
                    )
                }));
            })
            .unwrap();
        Err(()) // Return the same actual Ready owners; no operation is selected.
    });
    let returned = match result {
        Err((pair, ())) => pair,
        Ok(_) => panic!("no binding was issued"),
    };
    let (result, memory) = observed.unwrap();
    assert_eq!(issuer.next_play_sequence(), Some(1));
    (returned, result, memory, consumed)
}
#[test]
fn each_valid_planner_allocation_refuses_without_consuming_the_plan() {
    let (_, result, baseline, consumed) = trial(pair(), None);
    result.unwrap();
    assert!(consumed);
    let attempts = baseline.allocations + baseline.reallocations;
    assert!(attempts > 0 && attempts < 256);
    let name = format!(
        "{}::allocation_failure_child",
        module_path!().split_once("::").unwrap().1
    );
    for index in 0..attempts {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &name, "--ignored", "--nocapture"])
            .env("CONDUIT_PLANNER_FAILURE_INDEX", index.to_string())
            .env("RUST_BACKTRACE", "0")
            .output()
            .unwrap();
        assert!(
            output.status.success()
                && String::from_utf8_lossy(&output.stdout)
                    .contains("planner-allocation-refusal-verified"),
            "allocation {index}/{attempts} failed: {} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    println!("all {attempts} valid borrowed-planner allocation attempts refuse before consume; original owners retry");
}
#[test]
#[ignore = "finite failure-index parent launches one isolated child per allocation"]
fn allocation_failure_child() {
    let index = std::env::var("CONDUIT_PLANNER_FAILURE_INDEX")
        .unwrap()
        .parse()
        .unwrap();
    let (pair, result, memory, consumed) = trial(pair(), Some(index));
    assert!(matches!(
        result,
        Err(conduit_planner::borrowed_local::BorrowedLocalPlanningRefusal::WorkspaceCapacity)
    ));
    assert!(!consumed);
    assert_eq!(memory.retained_bytes(), 0);
    let (_, result, _, consumed) = trial(pair, None);
    result.unwrap();
    assert!(consumed);
    println!("planner-allocation-refusal-verified");
}
