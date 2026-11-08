use super::*;
use std::{vec, vec::Vec};
fn id(text: &str) -> AssignedIdentity {
    AssignedIdentity::from_text(text)
}
fn binding() -> AssignedNativeBinding {
    AssignedNativeBinding {
        capability: id("cap"),
        kind_revision: id("kind"),
        execution_profile: id("execution"),
        maximum_instances: 1,
        queue_items: 1,
        queue_bytes: 64,
        base: id("base"),
        instance: id("old-instance"),
        generation: 1,
        base_implementation: id("implementation"),
        mechanism: id("mechanism"),
        enforcement: 0,
        grant: id("old-grant"),
        authority: id("authority"),
        call: id("call"),
        subject: id("subject"),
        host: id("host"),
        boot: id("old-boot"),
        authority_capability: id("authority-cap"),
        resource: 0,
        pool: id("old-pool"),
        resource_class: id("class"),
        units: 1,
    }
}
fn record(out: &mut Vec<u8>, tag: u8, payload: &[u8]) {
    out.push(tag);
    out.extend_from_slice(&(payload.len() as u16).to_le_bytes());
    out.extend_from_slice(payload);
}
fn fixture(fore: bool) -> Vec<u8> {
    let mut out = vec![0; 125];
    out[..8].copy_from_slice(&assigned_plan_magic());
    out[8..10].copy_from_slice(&(if fore { 7u16 } else { 6 }).to_le_bytes());
    out[12..14].copy_from_slice(&128u16.to_le_bytes());
    for (offset, text) in [
        (16, "old-plan"),
        (32, "old-fragment"),
        (48, "host"),
        (64, "old-boot"),
    ] {
        out[offset..offset + 16].copy_from_slice(&id(text).0);
    }
    let mut prefix = [0; 46];
    prefix[2..4].copy_from_slice(&1u16.to_le_bytes());
    prefix[4..20].copy_from_slice(&id("call").0);
    let mut native = [0; 326];
    encode_assigned_native_binding(&prefix, Some(&binding()), &mut native).unwrap();
    record(&mut out, ASSIGNED_HOST_CALL, &native);
    out[86] = 1;
    let config = [0, 0, 7, 8, 9];
    record(&mut out, ASSIGNED_CONFIGURATION, &config);
    out[82] = 1;
    if fore {
        let mut cord = [0; 42];
        cord[2..18].copy_from_slice(&id("old-fore").0);
        cord[18] = 1;
        cord[19..21].copy_from_slice(&2u16.to_le_bytes());
        cord[26..28].copy_from_slice(&3u16.to_le_bytes());
        record(&mut out, ASSIGNED_CORD, &cord);
        out[83] = 1;
        let requirement =
            AssignedForeRequirement::new(2, 0, AssignedForeDirection::Input, id("front"), [8; 32]);
        record(&mut out, ASSIGNED_LOCAL_FORE, requirement.payload());
        out[89] = 1;
    }
    seal(&mut out);
    out
}
fn seal(out: &mut [u8]) {
    let len = out.len() as u16;
    out[10..12].copy_from_slice(&len.to_le_bytes());
    let digest = assigned_plan_payload_digest(&out[125..]);
    out[93..125].copy_from_slice(&digest);
}
fn maxima() -> AssignedPlanMaxima {
    AssignedPlanMaxima {
        encoded_bytes: 4096,
        runtime_state_bytes: 4096,
        counts: [255; 13],
    }
}
fn header(boot: &str) -> FreshAssignedHeader {
    FreshAssignedHeader {
        plan: id(&std::format!("plan/{boot}")),
        fragment: id(&std::format!("fragment/{boot}")),
        host: id("host"),
        boot: id(boot),
    }
}
fn native(boot: &str) -> FreshAssignedNative {
    let mut b = binding();
    b.instance = id(&std::format!("instance/{boot}"));
    b.generation = 9;
    b.grant = id(&std::format!("grant/{boot}"));
    b.boot = id(boot);
    b.pool = id(&std::format!("pool/{boot}"));
    FreshAssignedNative {
        node: 0,
        call: 1,
        contract: id("call"),
        binding: b,
    }
}
fn fore(placement: &str) -> FreshAssignedFore<'_> {
    FreshAssignedFore {
        cord: 0,
        endpoint: 2,
        direction: AssignedForeDirection::Input,
        local_node: 0,
        local_port: 3,
        front_port: "front",
        placement,
        gear_port: "input",
    }
}
#[test]
fn two_boots_rebuild_native_and_fore_preserving_all_static_records() {
    let bytes = fixture(true);
    let digest = assigned_local_projection_template_digest(&bytes, maxima()).unwrap();
    let template = AssignedLocalProjectionTemplate::check(&bytes, &digest, maxima()).unwrap();
    for boot in ["boot/one", "boot/two"] {
        let mut output = vec![0; bytes.len()];
        let n = native(boot);
        let f = fore(boot);
        template
            .instantiate(header(boot), &[n], &[f], &mut output)
            .unwrap();
        assert_eq!(read_identity(&output, 64).unwrap(), id(boot));
        assert_eq!(
            assigned_local_projection_template_digest(&output, maxima()).unwrap(),
            digest
        );
        for (before, after) in AssignedPlanRecords::new(&bytes)
            .unwrap()
            .zip(AssignedPlanRecords::new(&output).unwrap())
        {
            let before = before.unwrap();
            let after = after.unwrap();
            assert_eq!(before.tag, after.tag);
            match before.tag {
                ASSIGNED_HOST_CALL => {
                    assert_eq!(&before.payload[..47], &after.payload[..47]);
                    assert_eq!(
                        assigned_native_binding(after.payload).unwrap(),
                        Some(n.binding)
                    );
                }
                ASSIGNED_CORD => {
                    assert_eq!(&before.payload[..2], &after.payload[..2]);
                    assert_eq!(&before.payload[18..], &after.payload[18..]);
                    assert_eq!(
                        read_identity(after.payload, 2).unwrap(),
                        assigned_local_fore_connection(
                            f.direction,
                            f.front_port,
                            f.placement,
                            f.gear_port
                        )
                    );
                }
                _ => assert_eq!(before.payload, after.payload),
            }
        }
        assert_eq!(
            &output[93..125],
            &assigned_plan_payload_digest(&output[125..])
        );
    }
}
#[test]
fn absent_duplicate_extra_wrong_scope_and_short_output_refuse_atomically() {
    let bytes = fixture(true);
    let digest = assigned_local_projection_template_digest(&bytes, maxima()).unwrap();
    let template = AssignedLocalProjectionTemplate::check(&bytes, &digest, maxima()).unwrap();
    let n = native("boot/new");
    let f = fore("placement/new");
    for (ns, fs) in [
        (vec![], vec![f]),
        (vec![n, n], vec![f]),
        (vec![n], vec![]),
        (vec![n], vec![f, f]),
    ] {
        let mut output = vec![0x5a; bytes.len()];
        assert!(template
            .instantiate(header("boot/new"), &ns, &fs, &mut output)
            .is_err());
        assert!(output.iter().all(|b| *b == 0x5a));
    }
    for length in 0..bytes.len() {
        let mut output = vec![0x5a; bytes.len()];
        assert!(template
            .instantiate(header("boot/new"), &[n], &[f], &mut output[..length])
            .is_err());
        assert!(output.iter().all(|b| *b == 0x5a));
    }
    let mut stale = header("boot/new");
    stale.boot = id("old-boot");
    let mut output = vec![0x5a; bytes.len()];
    assert!(template
        .instantiate(stale, &[n], &[f], &mut output)
        .is_err());
    assert!(output.iter().all(|b| *b == 0x5a));
}
#[test]
fn all_static_native_definitions_and_fore_endpoints_remain_pinned() {
    let bytes = fixture(true);
    let digest = assigned_local_projection_template_digest(&bytes, maxima()).unwrap();
    let template = AssignedLocalProjectionTemplate::check(&bytes, &digest, maxima()).unwrap();
    let original = native("boot/new");
    let f = fore("placement/new");
    macro_rules! refuse {
        ($field:ident,$value:expr) => {{
            let mut n = original;
            n.binding.$field = $value;
            let mut output = vec![0x5a; bytes.len()];
            assert!(
                template
                    .instantiate(header("boot/new"), &[n], &[f], &mut output)
                    .is_err(),
                stringify!($field)
            );
            assert!(output.iter().all(|b| *b == 0x5a));
        }};
    }
    for field in [
        "capability",
        "kind_revision",
        "execution_profile",
        "base",
        "base_implementation",
        "mechanism",
        "authority",
        "call",
        "subject",
        "authority_capability",
        "resource_class",
    ] {
        let mut n = original;
        let b = &mut n.binding;
        match field {
            "capability" => b.capability = id("wrong"),
            "kind_revision" => b.kind_revision = id("wrong"),
            "execution_profile" => b.execution_profile = id("wrong"),
            "base" => b.base = id("wrong"),
            "base_implementation" => b.base_implementation = id("wrong"),
            "mechanism" => b.mechanism = id("wrong"),
            "authority" => b.authority = id("wrong"),
            "call" => b.call = id("wrong"),
            "subject" => b.subject = id("wrong"),
            "authority_capability" => b.authority_capability = id("wrong"),
            _ => b.resource_class = id("wrong"),
        };
        let mut output = vec![0x5a; bytes.len()];
        assert!(template
            .instantiate(header("boot/new"), &[n], &[f], &mut output)
            .is_err());
        assert!(output.iter().all(|b| *b == 0x5a));
    }
    refuse!(maximum_instances, 2);
    refuse!(queue_items, 2);
    refuse!(queue_bytes, 65);
    refuse!(resource, 1);
    refuse!(units, 2);
    refuse!(enforcement, 1);
    refuse!(generation, 0);
    refuse!(host, id("foreign"));
    refuse!(boot, id("old-boot"));
    for field in 0..5 {
        let mut wrong = f;
        match field {
            0 => wrong.endpoint += 1,
            1 => wrong.direction = AssignedForeDirection::Output,
            2 => wrong.local_node += 1,
            3 => wrong.local_port += 1,
            _ => wrong.front_port = "wrong",
        };
        let mut output = vec![0x5a; bytes.len()];
        assert!(template
            .instantiate(header("boot/new"), &[original], &[wrong], &mut output)
            .is_err());
        assert!(output.iter().all(|b| *b == 0x5a));
    }
}
#[test]
fn digest_and_network_refusal_and_canonical_fore_derivation() {
    let mut bytes = fixture(false);
    let digest = assigned_local_projection_template_digest(&bytes, maxima()).unwrap();
    bytes[125 + 3 + 48] ^= 1;
    assert!(AssignedLocalProjectionTemplate::check(&bytes, &digest, maxima()).is_err());
    seal(&mut bytes);
    assert!(AssignedLocalProjectionTemplate::check(&bytes, &digest, maxima()).is_err());
    let mut network = fixture(false);
    record(&mut network, ASSIGNED_REMOTE_ENDPOINT, &[0; 250]);
    network[89] = 1;
    seal(&mut network);
    assert_eq!(
        assigned_local_projection_template_digest(&network, maxima()),
        Err(ProjectionTemplateRefusal::UnsupportedNetwork)
    );
    for direction in [AssignedForeDirection::Input, AssignedForeDirection::Output] {
        assert_eq!(
            assigned_local_fore_connection(direction, "front", "placement", "gear"),
            id(&std::format!(
                "front/{}/{}/{}/{}",
                direction as u8,
                "front",
                "placement",
                "gear"
            ))
        );
    }
}

#[test]
fn duplicate_template_native_record_cannot_hide_an_unmatched_extra_slot() {
    let mut bytes = fixture(false);
    let payload = AssignedPlanRecords::new(&bytes)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .payload
        .to_vec();
    record(&mut bytes, ASSIGNED_HOST_CALL, &payload);
    bytes[86] = 2;
    seal(&mut bytes);
    let digest = assigned_local_projection_template_digest(&bytes, maxima()).unwrap();
    let template = AssignedLocalProjectionTemplate::check(&bytes, &digest, maxima()).unwrap();
    let n = native("boot/new");
    let mut extra = n;
    extra.node = 1;
    let mut output = vec![0x5a; bytes.len()];
    assert!(template
        .instantiate(header("boot/new"), &[n, extra], &[], &mut output)
        .is_err());
    assert!(output.iter().all(|b| *b == 0x5a));
}
