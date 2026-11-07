//! Actual owner worker and durable publication, using checked timer IMAGE custody.
use super::*;
use conduit_core::OfferGeneration;
use conduit_host_make::*;
use conduit_std_host::StdHostConfig;

const SOURCE: &str = "plot ticks (\n ticks: Tick...| >>\n) {\n clock: time/tick(count = 1, period-ms = 0)\n clock.tick >> ticks\n}\n";
pub(super) fn host(heap: u64) -> StdHost {
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
    configuration.limits.heap_arena_bytes = heap;
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let checked = check_host_configuration(configuration, &catalog, &packages).unwrap();
    let (image, _) = build_default_host_image(
        checked.profile().clone(),
        &catalog,
        &packages,
        &BuildInputs {
            source_identity: "fixture/owner-image-timer".into(),
            toolchain_available: true,
        },
    )
    .unwrap();
    StdHost::from_selected_timer_image(
        StdHostConfig {
            host_id: "host/owner-image-service".into(),
            boot_id: "boot/owner-image-service".into(),
            offer_generation: OfferGeneration(1),
        },
        &image,
        &catalog,
    )
    .unwrap()
}

#[test]
fn ordinary_owner_worker_uses_image_receipt_or_refuses_before_start() {
    use crate::durable_host::{fresh_identity, write_json_atomic, Installation, INSTALL_SCHEMA};
    for (heap, accepts, prepared) in [
        (64 * 1024 * 1024, true, false),
        (1, false, false),
        (64 * 1024 * 1024, true, true),
        (1, false, true),
    ] {
        let root = std::env::temp_dir().join(fresh_identity("owner-image-worker", "receipt"));
        std::fs::create_dir_all(&root).unwrap();
        let installation = Installation {
            schema: INSTALL_SCHEMA.into(),
            host_id: "host/owner-image-service".into(),
            release_source_identity: "fixture/owner-image-service".into(),
            release_bundle_sha256: crate::durable_host::digest(b"fixture/image-service"),
            product_executable: "fixture-unused".into(),
            body_state: None,
            joined_body_state: None,
            selected_speech: None,
            selected_model: None,
            selected_host_configuration: None,
            selected_todo_checkpoint: None,
        };
        write_json_atomic(&root.join("installation.json"), &installation).unwrap();
        let plot = crate::plot_source::parse(SOURCE)
            .unwrap()
            .expand_entry_for_authoring()
            .unwrap();
        let resident = conduit_body::ResidentPlot::new(
            plot.expanded.source_document_id.clone(),
            plot.expanded.checked_plot_id.clone(),
        );
        let mut owner = Owner::open(host(heap), resident, None, "Image timer").unwrap();
        let advertisement = owner.host.advertisement().clone();
        owner.set_resident_plot_name(&plot).unwrap();
        owner.persist(&root).unwrap();
        std::fs::write(root.join("body/source.conduit"), SOURCE).unwrap();
        let expected_plan = if prepared {
            std::fs::write(
                root.join("body/source.conduit"),
                SOURCE.replace("count = 1", "count = 2"),
            )
            .unwrap();
            assert!(owner.plan_retained(&root).is_err());
            assert!(owner.session.realization().is_none());
            std::fs::write(root.join("body/source.conduit"), SOURCE).unwrap();
            owner.plan_retained(&root).unwrap();
            let plan = owner.session.realization().unwrap().plan.plan_id.clone();
            std::fs::write(
                root.join("body/source.conduit"),
                SOURCE.replace("count = 1", "count = 2"),
            )
            .unwrap();
            assert!(owner.execute_prepared(&root, 5_000).is_err());
            assert!(!owner.host.is_playing());
            assert!(owner.current_play_id().is_none());
            assert_eq!(owner.session.realization().unwrap().plan.plan_id, plan);
            std::fs::write(root.join("body/source.conduit"), SOURCE).unwrap();
            let result = owner.execute_prepared(&root, 5_000);
            if accepts {
                result.unwrap();
            } else {
                assert!(result
                    .unwrap_err()
                    .contains("managed heap backing capacity"));
            }
            Some(plan)
        } else {
            let mut worker = owner.start_service_run(&root, 5_000).unwrap();
            let deadline = Instant::now() + Duration::from_secs(5);
            while !worker.progress(&mut owner, &root).unwrap() {
                assert!(Instant::now() < deadline, "owner worker failed to retire");
                std::thread::sleep(Duration::from_millis(2));
            }
            None
        };
        assert!(!owner.host.is_playing());
        assert_eq!(owner.host.advertisement(), &advertisement);
        let receipt = owner.last_execution.as_ref().unwrap();
        if let Some(plan) = expected_plan {
            assert_eq!(receipt["plan_id"], serde_json::json!(plan));
        }
        if accepts {
            assert!(!receipt["play"].is_null(), "{receipt}");
            assert_eq!(
                receipt["terminal"],
                serde_json::json!(conduit_core::TerminalDisposition::Completed),
                "{receipt}"
            );
            assert!(owner.session.realization().is_none());
            let values = receipt["fore_outputs"].as_array().unwrap();
            assert_eq!(values.len(), 1);
            assert_eq!(values[0]["front_port_id"], "ticks");
            assert_eq!(values[0]["bytes"].as_array().unwrap().len(), 8);
        } else {
            assert!(receipt["play"].is_null(), "{receipt}");
            assert!(
                receipt["pre_play_refusal"]
                    .as_str()
                    .unwrap()
                    .contains("managed heap backing capacity"),
                "{receipt}"
            );
            assert!(owner.current_play_id().is_none());
        }
        assert_eq!(state::execution(&root).unwrap().as_ref(), Some(receipt));
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn ingress_verification_refuses_while_worker_owns_host_without_panicking() {
    let mut host = super::super::OwnerHost::new(host(64 * 1024 * 1024));
    let input = host
        .retain_selected_image_fore_input(conduit_std_host::ExternalForeInput {
            front_port_id: "request".into(),
            track: conduit_core::ConnectionTrack::Payload,
            bytes: vec![1],
        })
        .unwrap();
    host.verify_selected_image_fore_input(&input).unwrap();
    let moved = host.take_for_play().unwrap();
    assert!(host
        .verify_selected_image_fore_input(&input)
        .unwrap_err()
        .contains("idle owner Host"));
    host.restore_after_play(moved).unwrap();
    host.verify_selected_image_fore_input(&input).unwrap();
}
