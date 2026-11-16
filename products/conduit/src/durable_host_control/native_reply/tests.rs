//! Real selected IMAGE counter custody, CPU response/writer fixtures only.
use super::*;
use conduit_std_host::{StdHost, StdHostConfig};
const HEAP: usize = 1024 * 1024;
const CODE: usize = 4096;
fn host() -> StdHost {
    host_with_heap(HEAP)
}
fn host_with_heap(heap: usize) -> StdHost {
    use conduit_host_make::*;
    let source = include_str!("../../../../../targets/std/profiles/linux-computer.host.conduit")
        .replace("\"clock/monotonic\"", "\"timer/monotonic\"")
        .replace(
            "  limits =",
            r#"
  capability = {kind: "time/tick", pinned_implementation: "std/kernel-time-tick@2"}
  need = {id: "timer/main", class: "conduit.resource/timer-slot@1", slots: 8, bytes: 512}
  limits ="#,
        );
    let mut configuration = parse_host_configuration_conduit(&source).unwrap();
    configuration.limits.heap_arena_bytes = heap as u64;
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let checked = check_host_configuration(configuration, &catalog, &packages).unwrap();
    let (image, _) = build_default_host_image(
        checked.profile().clone(),
        &catalog,
        &packages,
        &BuildInputs {
            source_identity: "fixture/native-response-custody".into(),
            toolchain_available: true,
        },
    )
    .unwrap();
    StdHost::from_selected_timer_image(
        StdHostConfig {
            host_id: "fixture/response-owner".into(),
            boot_id: "fixture/response-owner/boot".into(),
            offer_generation: conduit_core::OfferGeneration(1),
        },
        &image,
        &catalog,
    )
    .unwrap()
}
fn funded(host: &mut StdHost, character: char, code_bytes: usize) -> GuardedResponse {
    // Header has no heap payload. Counting the fixed header allocates nothing.
    let empty = Response::Refused {
        protocol: super::super::PROTOCOL,
        code: String::new(),
    };
    let mut header = FrameCount(0);
    serde_json::to_writer(&mut header, &empty).unwrap();
    assert!(character.is_ascii_alphanumeric());
    let encoded = header.0 + code_bytes;
    let envelope = std::mem::size_of::<GuardedResponse>() + code_bytes + encoded;
    let mut response = None;
    // SAFETY: one fixed String plus inline reply owner and exactly sized JSON
    // output slot. The alphanumeric code needs no escaping. Factory returns no
    // worker/effect; escaped response retains the SAME original witness below.
    let prepared = unsafe {
        host.prepare_selected_image_value(envelope, envelope, || {
            let mut code = String::with_capacity(code_bytes);
            code.extend(std::iter::repeat_n(character, code_bytes));
            response = Some(Response::Refused {
                protocol: super::super::PROTOCOL,
                code,
            });
            Ok(())
        })
    }
    .unwrap();
    let backing = prepared.retain_memory();
    drop(prepared);
    GuardedResponse::new(response.unwrap(), Some(backing))
}
fn available(host: &mut StdHost) -> usize {
    available_to(host, HEAP)
}
fn available_to(host: &mut StdHost, maximum: usize) -> usize {
    // Pure pressure probe: no invented provider/grant; the callback allocates
    // nothing. Temporary workspace admission/drop measures the original ledger.
    let (mut lo, mut hi) = (0, maximum);
    while lo < hi {
        let mid = lo + (hi - lo).div_ceil(2);
        // SAFETY: empty synchronous callback; conservative unused workspace is
        // immediately refunded. No value, mutation, worker or resource escapes.
        let value = unsafe { host.prepare_selected_image_value(mid, 0, || Ok(())) };
        if let Ok(value) = value {
            drop(value);
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    lo
}
#[test]
fn guarded_response_preserves_wire_and_refunds_after_real_write_error() {
    let mut host = host();
    let before = available(&mut host);
    let response = funded(&mut host, 'a', CODE);
    let expected = serde_json::to_vec(&response.response).unwrap();
    let mut bytes = Vec::new();
    response.write(&mut bytes).unwrap();
    assert_eq!(bytes, expected);
    assert_eq!(available(&mut host), before);
    let response = funded(&mut host, 'b', CODE);
    struct Broken<'a> {
        host: &'a mut StdHost,
        unheld: usize,
    }
    impl Write for Broken<'_> {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            assert!(available(self.host) < self.unheld);
            Err(io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let error = response
        .write(&mut Broken {
            host: &mut host,
            unheld: before,
        })
        .unwrap_err();
    assert!(error.contains("write local control frame"));
    assert_eq!(available(&mut host), before);
}
#[test]
fn held_original_response_remains_charged_when_new_response_is_replaced() {
    let mut host = host();
    let before = available(&mut host);
    let old = funded(&mut host, 'o', CODE);
    let old_charge = available(&mut host);
    assert!(old_charge < before);
    let mut newest = Some(funded(&mut host, 'n', CODE));
    assert!(available(&mut host) < old_charge);
    drop(newest.take());
    assert_eq!(
        available(&mut host),
        old_charge,
        "replacement must not release an escaped older response"
    );
    drop(host);
    // The response is still serializable after its original Host owner is gone;
    // its witness, rather than the newest slot, owns the old counter lifetime.
    let mut output = Vec::new();
    old.write(&mut output).unwrap();
    assert!(!output.is_empty());
}
#[test]
fn exact_output_slot_and_counting_ceiling_refuse_before_publication() {
    let mut count = FrameCount(MAXIMUM_LOCAL_CONTROL_FRAME_BYTES);
    assert!(count.write_all(b"x").is_err());
    let mut bytes = Vec::with_capacity(1);
    let mut output = ExactOutput {
        bytes: &mut bytes,
        maximum: 1,
    };
    assert!(serde_json::to_writer(&mut output, &"too long").is_err());
    assert!(bytes.len() <= 1);
    assert_eq!(bytes.capacity(), 1);
}

#[test]
fn guarded_oversize_reply_refuses_without_writing_any_frame() {
    let mut host = host_with_heap(8 * 1024 * 1024);
    let reply = funded(&mut host, 'x', MAXIMUM_LOCAL_CONTROL_FRAME_BYTES + 1);
    struct Writes(usize);
    impl Write for Writes {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0 += 1;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut writes = Writes(0);
    let error = reply.write(&mut writes).unwrap_err();
    assert!(error.contains("finite encoding bound"));
    assert_eq!(writes.0, 0);
}

fn initial_stage(host: &mut StdHost) -> ImageMemoryRetention {
    let mut temporary = funded(host, 's', CODE);
    let guard = temporary.backing.take().unwrap();
    drop(temporary); // DTO first; original already-funded stage remains held.
    guard
}
fn retired() -> Response {
    Response::BodyMemberCarrierRetired {
        protocol: super::super::PROTOCOL,
    }
}

#[test]
fn outgoing_reply_has_independent_lease_across_new_epoch_and_new_reply() {
    let heap = 8 * 1024 * 1024;
    let mut host = host_with_heap(heap);
    let first_owner = host.image_root_preparation_owner().unwrap();
    let old = transfer(retired(), initial_stage(&mut host), Some(&first_owner)).unwrap();
    let held_old = available_to(&mut host, heap);
    first_owner.invalidate();
    let next_owner = host.image_root_preparation_owner().unwrap();
    let before_new = available_to(&mut host, heap);
    let new = transfer(retired(), initial_stage(&mut host), Some(&next_owner)).unwrap();
    assert!(available_to(&mut host, heap) < before_new);
    drop(new);
    assert_eq!(
        available_to(&mut host, heap),
        before_new,
        "new epoch reply retirement cannot release the old reply lease"
    );
    let mut bytes = Vec::new();
    old.write(&mut bytes).unwrap();
    assert!(!bytes.is_empty());
    assert!(available_to(&mut host, heap) > held_old);
}
#[test]
fn outgoing_pressure_drops_candidate_guard_and_preserves_existing_reply() {
    let mut host = host();
    let owner = host.image_root_preparation_owner().unwrap();
    let first = funded(&mut host, 'f', CODE);
    let baseline = available(&mut host);
    let result = transfer(retired(), initial_stage(&mut host), Some(&owner));
    assert!(
        result.is_err(),
        "finite IMAGE smaller than outgoing output slot must refuse before serialization"
    );
    assert_eq!(
        available(&mut host),
        baseline,
        "failed transfer must refund its temporary stage"
    );
    let mut bytes = Vec::new();
    first.write(&mut bytes).unwrap();
    assert!(!bytes.is_empty());
    assert!(available(&mut host) > baseline);
}
#[test]
fn outgoing_bridge_refuses_foreign_counter_and_unsupported_json_without_leak() {
    let mut host = host();
    let owner = host.image_root_preparation_owner().unwrap();
    let mut other = host_with_heap(HEAP);
    let foreign = other.image_root_preparation_owner().unwrap();
    let before = available(&mut host);
    let other_before = available(&mut other);
    assert!(transfer(retired(), initial_stage(&mut host), Some(&foreign)).is_err());
    assert_eq!(available(&mut host), before);
    assert_eq!(available(&mut other), other_before);
    let stage = initial_stage(&mut host);
    let result = Response::BodyInteraction {
        protocol: super::super::PROTOCOL,
        result: Box::new(serde_json::Value::Bool(true)),
    };
    assert!(transfer(result, stage, Some(&owner)).is_err());
    assert_eq!(available(&mut host), before);
}
