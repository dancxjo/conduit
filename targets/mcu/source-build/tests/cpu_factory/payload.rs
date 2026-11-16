//! Fresh CPU command data instantiated from immutable checked Source templates.
use conduit_body as b;
use conduit_core as c;
use conduit_mcu_host_runtime::native_authority::Issuers;
use sha2::{Digest, Sha256};
fn id(s: &str) -> c::AssignedIdentity {
    c::AssignedIdentity::from_text(s)
}
fn placement(
    template: &[u8],
    checked: &str,
    gears: &[(u16, &str)],
    fore: &c::FreshAssignedFore<'_>,
    host: &c::HostAdvertisement,
) -> String {
    let mut selected = None;
    for record in c::AssignedPlanRecords::new(template).unwrap() {
        let record = record.unwrap();
        if record.tag != c::ASSIGNED_HOST_CALL
            || u16::from_le_bytes(record.payload[..2].try_into().unwrap()) != fore.local_node
        {
            continue;
        }
        let Some(binding) = c::assigned_native_binding(record.payload).unwrap() else {
            continue;
        };
        let mut bases = host
            .bases
            .iter()
            .filter(|base| id(base.base_id.as_str()) == binding.base);
        let base = bases.next().expect("actual native Fore owner");
        assert!(bases.next().is_none());
        let mut capabilities = base
            .capability_ids
            .iter()
            .filter(|capability| id(capability.as_str()) == binding.capability);
        let capability = capabilities.next().expect("actual Fore capability");
        assert!(capabilities.next().is_none());
        if let Some(prior) = selected {
            assert_eq!(prior, capability.as_str());
        } else {
            selected = Some(capability.as_str());
        }
    }
    let gear = gears
        .iter()
        .find(|(node, _)| *node == fore.local_node)
        .unwrap()
        .1;
    String::from_utf8(
        c::planned_placement_identity_bytes(
            checked,
            gear,
            host.host_id.as_str(),
            selected.expect("independent native capability"),
        )
        .to_vec(),
    )
    .unwrap()
}
fn projection(
    template: &[u8],
    digest: &[u8; 32],
    maxima: c::AssignedPlanMaxima,
    fore: &[c::FreshAssignedFore<'_>],
    host: &c::HostAdvertisement,
    issuers: &Issuers<{ crate::equipment::WAIT_SLOTS }>,
    index: usize,
) -> Vec<u8> {
    let mut native = Vec::new();
    for record in c::AssignedPlanRecords::new(template).unwrap() {
        let record = record.unwrap();
        if record.tag != c::ASSIGNED_HOST_CALL {
            continue;
        }
        let Some(mut binding) = c::assigned_native_binding(record.payload).unwrap() else {
            continue;
        };
        let base = host
            .bases
            .iter()
            .find(|base| id(base.base_id.as_str()) == binding.base)
            .expect("independent initialized base");
        let grant = issuers
            .grants()
            .find(|grant| {
                id(grant.contract_id.as_str()) == binding.authority
                    && id(grant.host_call_contract_id.as_str()) == binding.call
                    && id(grant.subject_kind.as_str()) == binding.subject
            })
            .expect("original authorized issuer");
        binding.instance = id(base.provider_instance_id.as_str());
        binding.generation = base.provider_generation;
        binding.grant = id(grant.grant_id.as_str());
        binding.host = id(host.host_id.as_str());
        binding.boot = id(host.boot_id.as_str());
        binding.pool = id(base.resource_pool_ids.first().unwrap().as_str());
        native.push(c::FreshAssignedNative {
            node: u16::from_le_bytes(record.payload[..2].try_into().unwrap()),
            call: u16::from_le_bytes(record.payload[2..4].try_into().unwrap()),
            contract: c::AssignedIdentity(record.payload[4..20].try_into().unwrap()),
            binding,
        });
    }
    let mut output = vec![0; template.len()];
    let checked = c::AssignedLocalProjectionTemplate::check(template, digest, maxima).unwrap();
    checked
        .instantiate(
            c::FreshAssignedHeader {
                plan: id(&format!("cpu-factory/plot-plan/{index}")),
                fragment: id(&format!("cpu-factory/fragment/{index}")),
                host: id(host.host_id.as_str()),
                boot: id(host.boot_id.as_str()),
            },
            &native,
            fore,
            &mut output,
        )
        .unwrap();
    output
}
fn line_projection(bytes: Vec<u8>, profile: c::AssignedLineForeProfile<'_>) -> Vec<u8> {
    let mut output = bytes[..c::ASSIGNED_PLAN_HEADER_BYTES].to_vec();
    output[8..10].copy_from_slice(&c::ASSIGNED_LINE_FORE_NATIVE_PLAN_SCHEMA.to_le_bytes());
    for record in c::AssignedPlanRecords::new(&bytes).unwrap() {
        let record = record.unwrap();
        let mut owned = None;
        if record.tag == c::ASSIGNED_REMOTE_ENDPOINT {
            owned = Some(
                if let Some(line) = profile
                    .lines
                    .iter()
                    .find(|line| line.fore().payload().as_slice() == record.payload)
                {
                    line.payload().to_vec()
                } else {
                    c::assigned_line_fore_local_payload(
                        profile
                            .local
                            .iter()
                            .find(|pin| pin.payload().as_slice() == record.payload)
                            .expect("independent local Fore"),
                    )
                    .to_vec()
                },
            );
        }
        let payload = owned.as_deref().unwrap_or(record.payload);
        output.push(record.tag);
        output.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        output.extend_from_slice(payload);
    }
    let length = output.len() as u16;
    output[10..12].copy_from_slice(&length.to_le_bytes());
    let digest = c::assigned_plan_payload_digest(&output[125..]);
    output[93..125].copy_from_slice(&digest);
    output
}
pub fn workset(
    context: b::NativePreparationContext<'_>,
    issuers: &Issuers<{ crate::equipment::WAIT_SLOTS }>,
    hot: &crate::play::fore::installation::Hot,
) -> Vec<u8> {
    use crate::workset::generated as s;
    let fragments = [
        "cpu-factory/fragment/0",
        "cpu-factory/fragment/1",
        "cpu-factory/fragment/2",
    ];
    let (count, commitment) = b::local_fragment_set_commitment(&fragments).unwrap();
    let play = format!(
        "body-play/sha256:{}",
        b::body_play_digest(
            context.member.body_id.as_str(),
            "cpu-factory/wake",
            "cpu-factory/plan",
            1
        )
        .iter()
        .map(|v| format!("{v:02x}"))
        .collect::<String>()
    );
    let mut documents = Vec::new();
    macro_rules! entry {
        ($m:ident,$index:expr,$line:expr) => {{
            let placements: Vec<String> = s::$m::FRESH_FORE
                .iter()
                .map(|fore| {
                    if $line {
                        placement(
                            s::$m::PROJECTION_TEMPLATE,
                            s::$m::CHECKED_PLOT_ID,
                            s::$m::GEAR_IDS,
                            fore,
                            context.actual,
                        )
                    } else {
                        fore.placement.to_owned()
                    }
                })
                .collect();
            let fresh: Vec<_> = s::$m::FRESH_FORE
                .iter()
                .zip(&placements)
                .map(|(fore, placement)| c::FreshAssignedFore {
                    placement: placement.as_str(),
                    ..*fore
                })
                .collect();
            let projection = projection(
                s::$m::PROJECTION_TEMPLATE,
                &s::$m::PROJECTION_TEMPLATE_DIGEST,
                s::$m::MAXIMA,
                &fresh,
                context.actual,
                issuers,
                $index,
            );
            let projection = if $line {
                line_projection(projection, hot.profile())
            } else {
                projection
            };
            let inventory: Vec<u8> = s::$m::FORE
                .iter()
                .flat_map(|pin| pin.payload().iter().copied())
                .collect();
            let plan = format!("cpu-factory/plot-plan/{}", $index);
            let metadata = b::BodyFragmentApprovalMetadata {
                schema: 3,
                body: context.member.body_id.as_str(),
                wake: "cpu-factory/wake",
                body_plan: "cpu-factory/plan",
                body_play: &play,
                play_sequence: 1,
                workload_revision: 1,
                source: s::$m::SOURCE_DOCUMENT_ID,
                checked_plot: s::$m::CHECKED_PLOT_ID,
                expanded_plot: s::$m::EXPANDED_PLOT_ID,
                plot_plan: &plan,
                fragment: fragments[$index],
                host: context.actual.host_id.as_str(),
                boot: context.actual.boot_id.as_str(),
                offer_generation: context.actual.offer_generation.0,
                local_fragment_count: count,
                local_fragment_digest: commitment,
                resource_bindings: s::$m::INSTALLED_PLAIN_RESOURCE_BINDINGS.unwrap(),
                fore_ports: &inventory,
                projection_length: projection.len() as u32,
                projection_digest: Sha256::digest(&projection).into(),
            };
            documents.push(
                b::encode_body_fragment_document(
                    &metadata,
                    &projection,
                    crate::config::MAX_ASSIGNMENT_BYTES as u32,
                )
                .unwrap(),
            );
        }};
    }
    entry!(sampling_layout, 0, true);
    entry!(mask_layout, 1, false);
    entry!(output_layout, 2, false);
    b::encode_body_workset_document(
        &documents.iter().map(Vec::as_slice).collect::<Vec<_>>(),
        crate::config::MAX_ASSIGNMENT_BYTES as u32,
    )
    .unwrap()
}
