//! Actual acknowledged mailbox/profile custody around the recovery decision.
//! Real SDK cleanup, returned native owners and Root integration require HIL.
use super::*;
use recovery::{Action, Recovery, Refusal};

#[test]
fn ap_input_acknowledged_during_join_keeps_custody_until_returned() {
    for cleanup_succeeds in [false, true] {
        let mut owner = known::Owner::empty(3);
        let old = owner
            .upsert_after_acknowledgement(input(b"old", b"public-old-pass"), |_| Ok(1))
            .unwrap();
        let snapshot = owner.freeze(old).unwrap();
        let mut policy = Recovery::new();
        policy.configuration(1, 0).unwrap();
        policy.poll(0, true, false).unwrap();
        let mut mailbox = NetworkSetupMailbox::new(2);
        let token = mailbox
            .submit(
                NetworkSetupInput::from_form(
                    b"ssid=new&password=public-new-pass",
                    NetworkSetupLimits {
                        minimum_credential_bytes: 8,
                        maximum_credential_bytes: 63,
                    },
                )
                .unwrap(),
            )
            .unwrap();
        assert!(!mailbox.has_acknowledged());
        assert_eq!(policy.poll(1, true, true), Ok(Action::None));
        mailbox.acknowledge(token).unwrap();
        assert!(mailbox.has_acknowledged());
        // AP serving remains selected until its asynchronous withdrawal is
        // acknowledged. A later AP exchange may finish during this interval;
        // this fixture does not claim a credential route on the station.
        assert!(policy.is_joining());
        assert_eq!(policy.configuration_retirement(1), Err(Refusal::Busy));
        assert!(mailbox.acknowledged_submission() == Ok(token));
        policy.attached(1).unwrap();
        assert_eq!(
            policy.configuration_retirement(1),
            Ok(Action::RestoreAccessPoint { generation: 1 })
        );
        for now in [1, 2, 1_000, 90_000] {
            assert_eq!(policy.configuration_retirement(now), Ok(Action::None));
            assert_eq!(policy.poll(now, true, true), Ok(Action::None));
            assert_eq!(policy.configuration(2, now), Err(Refusal::Busy));
            assert!(mailbox.acknowledged_submission() == Ok(token));
            assert_eq!(owner.selection(), Some(snapshot));
            assert_eq!(owner.store_revision(), 1);
            assert_eq!(owner.accepted_updates(), 1);
            owner
                .with_selected_request(|request| {
                    assert_eq!(request.ssid, b"old");
                    assert_eq!(request.credential, b"public-old-pass");
                })
                .unwrap();
            // Even a premature preparation callback cannot replace the frozen
            // old profile; refusal retains the same acknowledged submission.
            let prepared = mailbox
                .prepare_acknowledged(|request| {
                    owner.upsert_after_acknowledgement(
                        input(request.ssid, request.credential),
                        |_| panic!("pending owner must refuse before commit"),
                    )
                })
                .unwrap();
            assert!(matches!(prepared, Err(known::Refusal::Pending)));
            assert!(mailbox.acknowledged_submission() == Ok(token));
        }
        policy.restored(1, 90_001, cleanup_succeeds).unwrap();
        if !cleanup_succeeds {
            assert_eq!(policy.configuration_retirement(90_002), Err(Refusal::Busy));
            assert_eq!(policy.configuration(2, 90_002), Err(Refusal::Busy));
            assert_eq!(owner.selection(), Some(snapshot));
            assert_eq!(owner.store_revision(), 1);
            assert!(mailbox.acknowledged_submission() == Ok(token));
            continue;
        }
        // Models only the existing actual-return report. The policy cannot
        // synthesize that report, retire a snapshot or write private storage.
        let mut origin = None;
        selection::retire_superseded(&mut owner, &mut origin, Some(token)).unwrap();
        assert!(mailbox
            .prepare_acknowledged(|request| {
                owner.upsert_after_acknowledgement(input(request.ssid, request.credential), |_| {
                    Err(())
                })
            })
            .unwrap()
            .is_err());
        assert_eq!(owner.store_revision(), 1);
        assert_eq!(owner.accepted_updates(), 1);
        assert!(mailbox.acknowledged_submission() == Ok(token));
        let key = mailbox
            .prepare_acknowledged(|request| {
                owner.upsert_after_acknowledgement(input(request.ssid, request.credential), |_| {
                    Ok(2)
                })
            })
            .unwrap()
            .unwrap();
        owner.freeze(key).unwrap();
        policy
            .configuration(owner.store_revision(), 90_002)
            .unwrap();
        assert_eq!(policy.revision(), 2);
        assert_eq!(policy.generation(), 1);
        assert_eq!(owner.accepted_updates(), 2);
        assert!(mailbox.acknowledged_submission() == Ok(token));
        owner
            .with_selected_request(|request| assert_eq!(request.ssid, b"new"))
            .unwrap();
    }
}
