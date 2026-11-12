//! Same original complete-workset admission and Start with a carrier loan.
use super::*;
fn narrow_line() -> c::AdmittedLine {
    let mut line = actual_line();
    line.binding.limits.maximum_payload_bytes = 32;
    line.binding.limits.maximum_frame_bytes = 48;
    line.binding.limits.maximum_buffered_bytes = 48;
    line
}
#[test]
fn original_parts_carrier_is_funded_and_binds_only_through_original_admission_start() {
    let f = fixture();
    let seed = seed(&f);
    let pins = [pin(&narrow_line())];
    let local = [docs::local()];
    let bindings = [binding()];
    let document = document(&pins[0]);
    let mut root = root(&f, &seed, &document);
    let io = Cell::new(0);
    let bound = Cell::new(0);
    let custody = Cell::new([0; 32]);
    let actual_caps = narrow_line().binding.limits;
    assert!(actual_caps.maximum_payload_bytes < bounds().maximum_payload_bytes);
    let mut link = Link::new(Mock(&io));
    let mut backing = [0; 64];
    let parts = root.execution_parts().unwrap();
    let context = b::NativePreparationContext {
        bootstrap: parts.bootstrap,
        member: parts.member,
        actual: &f.host,
        image_id: "image/1",
        configuration_id: "config/1",
        now_millis: 11,
    };
    let policy = crate::execution::Policy {
        preparation: Some(&PREP),
        execution: Some(&EXEC),
        active_worksets: 0,
        maximum_retained_bytes: 16384,
    };
    let prepared = {
        // Cold owned installation evidence exists only inside this preparation
        // block; retaining any of these borrows would fail this compile proof.
        let lines = [narrow_line()];
        let peer = f.issuer.clone();
        let image = std::string::String::from("image/1");
        let configuration = std::string::String::from("config/1");
        let session = std::string::String::from("session");
        let session_contract = std::string::String::from("serial-profile");
        // SAFETY: this fixture explicitly installs its original exclusive Mock
        // serial provider/backing and immutable checked peer/Line selection.
        let selection = unsafe {
            NativeForeCarrierSelection::from_native_custody(
                &mut link,
                &mut backing,
                &image,
                &configuration,
                &peer,
                c::AssignedLineForeProfile {
                    local: &local,
                    lines: &pins,
                },
                &lines,
                &session,
                &session_contract,
                bounds(),
            )
        }
        .unwrap();
        let mut selection = Some(selection);
        let prepared =
            PreparedLocalWorkset::<_, 1>::prepare_selected_profiles::<4096, 2, ForeCarrierRefusal>(
                parts.workspace,
                context,
                policy,
                |_, _| {
                    Ok(AssignedPreparationProfile::LineFore(docs::profile(
                        &local, &pins, &bindings,
                    )))
                },
                |fragments, fore| {
                    let ticket = parts.prepare_fore_carrier::<Link<Mock<'_>>, 1, 2>(
                        11,
                        selection.take(),
                        &fragments[0],
                        &fore[0],
                        docs::profile(&local, &pins, &bindings),
                    )?;
                    custody.set(*ticket.custody_digest());
                    assert_eq!(ticket.selected_line_limits(1, 6), Some(actual_caps));
                    assert_eq!(ticket.selected_line_limits(2, 7), None);
                    let wrapped = ticket.attach(Ticket(&bound));
                    assert!(
                        wrapped.retained_backing_bytes().unwrap()
                            >= 64 + core::mem::size_of_val(&wrapped)
                                - core::mem::size_of::<Ticket<'_>>()
                    );
                    Ok(wrapped)
                },
            )
            .unwrap();
        drop(selection);
        drop((lines, peer, image, configuration, session, session_contract));
        prepared
    };
    assert_eq!(io.get(), 0);
    assert_eq!(bound.get(), 0);
    let mut committed = prepared
        .admit::<ForeCarrierRefusal>(parts.resources, &[], 0)
        .unwrap();
    assert_eq!(io.get(), 0);
    assert_eq!(bound.get(), 0);
    let mut started = committed
        .start::<4096, ForeCarrierRefusal>(parts.workspace, context, policy, parts.resources)
        .unwrap();
    let evidence = started.evidence();
    assert_eq!(evidence.custody_identity(), &custody.get());
    assert_eq!(
        started.kernels_mut().selected_line_limits(1, 6),
        Some(actual_caps)
    );
    assert_eq!(started.kernels_mut().selected_line_limits(2, 7), None);

    assert!(started.kernels_mut().matches_started(&evidence));
    let child_sha: [u8; 32] = Sha256::digest(&document).into();
    let expected = external_fore_scope_commitment(
        &fore_carrier_scope_commitment(&child_sha, "image/1", "config/1", bounds()),
        evidence.custody_identity(),
        &pins[0],
    );
    assert_eq!(
        started.kernels_mut().fore_scope_digest(1, 6),
        Some(expected)
    );
    assert_eq!(started.kernels_mut().fore_scope_digest(2, 7), None);
    let other = c::AssignedLineForeRequirement::new(
        c::AssignedForeRequirement::new(
            3,
            8,
            c::AssignedForeDirection::Output,
            docs::id("read_result"),
            [9; 32],
        ),
        binding(),
        docs::id("session"),
        docs::id("serial-profile"),
        c::admitted_line_commitment(&narrow_line()),
    );
    assert_ne!(
        external_fore_scope_commitment(
            &fore_carrier_scope_commitment(&child_sha, "image/1", "config/1", bounds()),
            evidence.custody_identity(),
            &other
        ),
        expected
    );

    assert_eq!(bound.get(), 1);
    assert_eq!(io.get(), 0);
}
