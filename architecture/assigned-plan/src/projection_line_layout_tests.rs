fn line_pin(f: AssignedForeRequirement) -> AssignedLineForeRequirement {
    AssignedLineForeRequirement::new(
        f,
        AssignedRemoteBinding {
            line: id("line"),
            local_host: id("host"),
            local_boot: id("new-boot"),
            peer_host: id("peer"),
            peer_boot: id("peer-boot"),
        },
        id("session"),
        id("session-contract"),
        [9; 32],
    )
}
fn static_fore() -> AssignedForeRequirement {
    AssignedForeRequirement::new(2, 0, AssignedForeDirection::Input, id("front"), [8; 32])
}
#[test]
fn lazy_line_recipe_keeps_exact_cord_identity_and_session() {
    let bytes = fixture(true);
    let digest = assigned_local_projection_template_digest(&bytes, maxima()).unwrap();
    let template = AssignedLocalProjectionTemplate::check(&bytes, &digest, maxima()).unwrap();
    let fresh = [fore("actual-canonical-placement")];
    let line = [line_pin(static_fore())];
    let recipe = template
        .line_layout(
            &fresh,
            AssignedLineForeProfile {
                local: &[],
                lines: &line,
            },
        )
        .unwrap();
    let expected: Vec<_> = recipe.records().collect::<Result<_, _>>().unwrap();
    let cord = expected.iter().find(|r| r.tag == ASSIGNED_CORD).unwrap();
    let raw = AssignedPlanRecords::new(&bytes)
        .unwrap()
        .map(Result::unwrap)
        .find(|r| r.tag == ASSIGNED_CORD)
        .unwrap();
    let mut payload = [0; 42];
    payload.copy_from_slice(raw.payload);
    payload[2..18].copy_from_slice(
        &assigned_local_fore_connection(
            fresh[0].direction,
            fresh[0].front_port,
            fresh[0].placement,
            fresh[0].gear_port,
        )
        .0,
    );
    assert_eq!(
        *cord,
        AssignedRecordRequirement::from_record(AssignedRecord {
            tag: ASSIGNED_CORD,
            payload: &payload
        })
    );
    payload[2] ^= 1;
    assert_ne!(
        *cord,
        AssignedRecordRequirement::from_record(AssignedRecord {
            tag: ASSIGNED_CORD,
            payload: &payload
        })
    );
    assert!(
        expected.contains(&AssignedRecordRequirement::from_record(AssignedRecord {
            tag: ASSIGNED_REMOTE_ENDPOINT,
            payload: line[0].payload()
        }))
    );
    assert!(core::mem::size_of_val(&recipe) < 128);
}
#[test]
fn recipe_refuses_missing_duplicate_foreign_or_wrong_typed_slots() {
    let bytes = fixture(true);
    let digest = assigned_local_projection_template_digest(&bytes, maxima()).unwrap();
    let template = AssignedLocalProjectionTemplate::check(&bytes, &digest, maxima()).unwrap();
    let fresh = fore("actual");
    let line = line_pin(static_fore());
    for changed in [
        FreshAssignedFore {
            local_node: 1,
            ..fresh
        },
        FreshAssignedFore {
            local_port: 4,
            ..fresh
        },
        FreshAssignedFore {
            endpoint: 3,
            ..fresh
        },
        FreshAssignedFore {
            front_port: "foreign",
            ..fresh
        },
        FreshAssignedFore { cord: 1, ..fresh },
    ] {
        assert!(template
            .line_layout(
                &[changed],
                AssignedLineForeProfile {
                    local: &[],
                    lines: &[line]
                }
            )
            .is_err());
    }
    assert!(template
        .line_layout(
            &[fresh],
            AssignedLineForeProfile {
                local: &[],
                lines: &[]
            }
        )
        .is_err());
    assert!(template
        .line_layout(
            &[fresh, fresh],
            AssignedLineForeProfile {
                local: &[],
                lines: &[line, line]
            }
        )
        .is_err());
    for bad in [
        AssignedForeRequirement::new(3, 0, AssignedForeDirection::Input, id("front"), [8; 32]),
        AssignedForeRequirement::new(2, 0, AssignedForeDirection::Input, id("front"), [7; 32]),
        AssignedForeRequirement::new(2, 0, AssignedForeDirection::Output, id("front"), [8; 32]),
    ] {
        assert!(template
            .line_layout(
                &[fresh],
                AssignedLineForeProfile {
                    local: &[],
                    lines: &[line_pin(bad)]
                }
            )
            .is_err());
    }
    let zero_session = AssignedLineForeRequirement::new(
        static_fore(),
        AssignedRemoteBinding {
            line: id("line"),
            local_host: id("host"),
            local_boot: id("boot"),
            peer_host: id("peer"),
            peer_boot: id("peer-boot"),
        },
        AssignedIdentity([0; 16]),
        id("contract"),
        [9; 32],
    );
    assert!(template
        .line_layout(
            &[fresh],
            AssignedLineForeProfile {
                local: &[],
                lines: &[zero_session]
            }
        )
        .is_err());
}

#[test]
fn mixed_recipe_keeps_append_status_local_and_line_pins_exact() {
    let mut bytes = fixture(true);
    let mut cord = [0; 42];
    cord[0..2].copy_from_slice(&1u16.to_le_bytes());
    cord[2..18].copy_from_slice(&id("old-status-connection").0);
    cord[19..21].copy_from_slice(&0u16.to_le_bytes());
    cord[21..23].copy_from_slice(&4u16.to_le_bytes());
    cord[23] = 1;
    cord[24..26].copy_from_slice(&3u16.to_le_bytes());
    record(&mut bytes, ASSIGNED_CORD, &cord);
    let local = AssignedForeRequirement::new(
        3,
        1,
        AssignedForeDirection::Output,
        id("append_status"),
        [7; 32],
    );
    record(&mut bytes, ASSIGNED_LOCAL_FORE, local.payload());
    bytes[83] += 1;
    bytes[89] += 1;
    seal(&mut bytes);
    let digest = assigned_local_projection_template_digest(&bytes, maxima()).unwrap();
    let template = AssignedLocalProjectionTemplate::check(&bytes, &digest, maxima()).unwrap();
    let fresh = [
        fore("actual"),
        FreshAssignedFore {
            cord: 1,
            endpoint: 3,
            direction: AssignedForeDirection::Output,
            local_node: 0,
            local_port: 4,
            front_port: "append_status",
            placement: "actual",
            gear_port: "status",
        },
    ];
    let lines = [line_pin(static_fore())];
    let locals = [local];
    let recipe = template
        .line_layout(
            &fresh,
            AssignedLineForeProfile {
                local: &locals,
                lines: &lines,
            },
        )
        .unwrap();
    let expected: Vec<_> = recipe.records().collect::<Result<_, _>>().unwrap();
    assert!(
        expected.contains(&AssignedRecordRequirement::from_record(AssignedRecord {
            tag: ASSIGNED_LOCAL_FORE,
            payload: &assigned_line_fore_local_payload(&local)
        }))
    );
    cord[2..18].fill(0);
    assert!(
        expected.contains(&AssignedRecordRequirement::from_record(AssignedRecord {
            tag: ASSIGNED_CORD,
            payload: &cord
        }))
    );
    let changed = AssignedLineForeRequirement::new(
        static_fore(),
        AssignedRemoteBinding {
            line: id("line"),
            local_host: id("host"),
            local_boot: id("new-boot"),
            peer_host: id("peer"),
            peer_boot: id("peer-boot"),
        },
        id("other-session"),
        id("session-contract"),
        [9; 32],
    );
    let changed_lines = [changed];
    let changed_recipe = template
        .line_layout(
            &fresh,
            AssignedLineForeProfile {
                local: &locals,
                lines: &changed_lines,
            },
        )
        .unwrap();
    let changed_records: Vec<_> = changed_recipe.records().collect::<Result<_, _>>().unwrap();
    assert_ne!(expected, changed_records);
}
