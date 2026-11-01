mod offline_preparation {
    use super::*;
    fn input(f: &Fixture) -> PathBuf {
        let path = f.directory.join("captured-offer.json");
        std::fs::write(&path, serde_json::to_vec(&f.offer).unwrap()).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        path
    }
    #[test]
    fn offline_ack_is_exact_private_durable_and_never_overwrites() {
        let f = fixture();
        let offer = input(&f);
        let output = f.directory.join("ack.json");
        prepare::run(&f.binding, &f.private, &f.history, &offer, &output).unwrap();
        let bytes = std::fs::read(&output).unwrap();
        let request: wire::Request<'_> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(request.part, "03".repeat(32));
        assert_eq!(request.host, "04".repeat(32));
        assert_eq!(request.boot, "05".repeat(32));
        assert_eq!(
            request
                .offer
                .0
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>(),
            f.reply["offer"].as_str().unwrap()
        );
        f.continuity
            .verifying_key()
            .verify_strict(
                &request.transcript(),
                &ed25519_dalek::Signature::from_bytes(&request.signature.0),
            )
            .unwrap();
        assert_eq!(
            std::fs::metadata(&output).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(prepare::run(&f.binding, &f.private, &f.history, &offer, &output).is_err());
        assert_eq!(std::fs::read(&output).unwrap(), bytes);
        assert!(f.batch.exists());
    }
    #[test]
    fn offline_refusals_leave_no_ack() {
        for mutation in 0..10 {
            let mut f = fixture();
            match mutation {
                0 => f.offer["offer"]["signature"] = "00".repeat(64).into(),
                1 => std::fs::remove_file(&f.batch).unwrap(),
                2 => std::fs::write(&f.batch, b"corrupt").unwrap(),
                3 | 4 => {
                    let mut binding: serde_json::Value =
                        serde_json::from_slice(&std::fs::read(&f.binding).unwrap()).unwrap();
                    if mutation == 3 {
                        binding["receipt"]["credential"]["body_id"] = "ff".repeat(32).into();
                    } else {
                        binding["local_boot"] = "ff".repeat(32).into();
                    }
                    std::fs::write(&f.binding, serde_json::to_vec(&binding).unwrap()).unwrap();
                }
                5 => {
                    let mut private: serde_json::Value =
                        serde_json::from_slice(&std::fs::read(&f.private).unwrap()).unwrap();
                    private["invitation"]["claim"]["invitation_id"] = "ff".repeat(32).into();
                    std::fs::write(&f.private, serde_json::to_vec(&private).unwrap()).unwrap();
                }
                6 => f.offer["event"] = "different".into(),
                7 => f.offer["slot"] = 6.into(),
                8 => f.offer["unknown"] = true.into(),
                9 => (),
                _ => unreachable!(),
            }
            let offer = input(&f);
            if mutation == 9 {
                std::fs::write(&offer, vec![b' '; 4097]).unwrap();
            }
            let output = f.directory.join("ack.json");
            assert!(
                prepare::run(&f.binding, &f.private, &f.history, &offer, &output).is_err(),
                "mutation {mutation}"
            );
            assert!(!output.exists());
        }
    }
    #[test]
    fn offline_private_input_and_symlink_outputs_are_refused() {
        let f = fixture();
        let offer = input(&f);
        let output = f.directory.join("ack.json");
        std::fs::set_permissions(&offer, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(prepare::run(&f.binding, &f.private, &f.history, &offer, &output).is_err());
        assert!(!output.exists());
        std::fs::set_permissions(&offer, std::fs::Permissions::from_mode(0o600)).unwrap();
        let linked = f.directory.join("offer-link");
        std::os::unix::fs::symlink(&offer, &linked).unwrap();
        assert!(prepare::run(&f.binding, &f.private, &f.history, &linked, &output).is_err());
        let sentinel = f.directory.join("sentinel");
        std::fs::write(&sentinel, b"unchanged").unwrap();
        std::os::unix::fs::symlink(&sentinel, &output).unwrap();
        assert!(prepare::run(&f.binding, &f.private, &f.history, &offer, &output).is_err());
        assert_eq!(std::fs::read(&sentinel).unwrap(), b"unchanged");
    }
}
