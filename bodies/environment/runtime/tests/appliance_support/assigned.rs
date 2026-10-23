//! Make-side CPU recipe construction. Firmware must retain this independently
//! of received assignment bytes; this is not a native provider or an issuer.
use super::*;
use conduit_environment_runtime::appliance::assigned::{ResidentAppliance, ResidentRoles};
pub struct Recipe {
    records: Vec<c::AssignedRecordRequirement>,
    counts: [u8; c::ASSIGNED_PLAN_COUNT_KINDS],
    roles: ResidentRoles,
}
impl Recipe {
    pub fn prepare(
        plan: &c::Plan,
        lowered: &conduit_plan_lowering::lowering::LoweredPlanFragment,
        image: &conduit_embedded_build::GeneratedEmbeddedPlan,
        projection: &assignment::NativeProjection,
    ) -> Self {
        let find = |kind: &str| {
            let gear = plan.fragments[0]
                .placements
                .iter()
                .find(|g| g.kind_id.as_str() == kind)
                .unwrap();
            lowered
                .identity
                .node_for_placement(&gear.placement_id)
                .unwrap()
        };
        let roles = ResidentRoles {
            pulse: find(conduit_pulse_capture::PULSE_READ_KIND),
            tick: find(conduit_time::TICK_KIND),
            trigger: find(conduit_time::TICK_TRIGGER_KIND),
            observation: find(conduit_environment_runtime::observation::KIND),
            publications: Channel::ALL.map(|channel| find(channel.kind_id())),
        };
        let mut records = conduit_embedded_build::assigned_native_layout_requirements(
            image,
            assignment::NativeProjection::maxima(),
        )
        .unwrap();
        records.extend(
            c::AssignedPlanRecords::new(&projection.bytes)
                .unwrap()
                .map(Result::unwrap)
                .filter(|r| r.tag == c::ASSIGNED_CONFIGURATION)
                .map(c::AssignedRecordRequirement::from_record),
        );
        let mut counts = [0; c::ASSIGNED_PLAN_COUNT_KINDS];
        counts.copy_from_slice(&projection.bytes[80..80 + c::ASSIGNED_PLAN_COUNT_KINDS]);
        Self {
            records,
            counts,
            roles,
        }
    }
    pub fn resident<'a>(
        &'a self,
        projection: &'a assignment::NativeProjection,
    ) -> ResidentAppliance<'a> {
        ResidentAppliance {
            layout: conduit_environment_runtime::assigned::ResidentLayout {
                records: &self.records,
                counts: self.counts,
            },
            computations: projection.profile(),
            roles: self.roles,
        }
    }
}

#[test]
fn assigned_preparation_refuses_wrong_root_scope_roles_and_storage_before_effects() {
    use conduit_environment_runtime::appliance::assigned::{
        NativePreparationRefusal as E, PreparedAssignedAppliance,
    };
    let source = compile(1, 0).unwrap();
    let (ready, effects) = pulse::ready(0);
    let mut software = software(&source);
    let host = appliance_support::advertisement(&source, &mut software, &ready);
    let plan = source
        .plan_local(&host, &["conduit.base/local@1".into()], &[pulse::grant()])
        .unwrap();
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(&plan.fragments[0]).unwrap();
    let image = appliance_support::native_image(&plan, &lowered);
    let projection = assignment::NativeProjection::prepare(&image);
    let recipe = Recipe::prepare(&plan, &lowered, &image, &projection);
    for mode in 0..6 {
        let mut resident = recipe.resident(&projection);
        let mut required = projection.required();
        match mode {
            0 => resident.roles.pulse = resident.roles.tick,
            1 => resident.roles.publications.swap(0, 1),
            2 => resident.roles.observation = k::NodeId(15),
            3 => required.host = c::AssignedIdentity::from_text("foreign/host"),
            4 => required.boot = c::AssignedIdentity::from_text("stale/boot"),
            _ => {}
        }
        // Deliberately inadequate wait storage in mode 5. Other refusals precede
        // the first software allocation and leave native Ready ownership intact.
        let values = k::FixedValueStore::<1, 1>::new(1).unwrap();
        let (result, memory) = allocation_probe::observe(|| {
            PreparedAssignedAppliance::prepare(
                &projection.bytes,
                assignment::NativeProjection::maxima(),
                required,
                &resident,
                values,
            )
        });
        match mode {
            0..=2 => assert!(matches!(result, Err(E::Roles))),
            3..=4 => assert!(matches!(result, Err(E::Tables(_)))),
            _ => assert!(matches!(result, Err(E::Storage(_)))),
        }
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
        assert_eq!(effects.get(), 0);
    }
    // A changed program remains an untrusted assignment, not a new resident
    // inventory. Independent pinned hashes must refuse it before initialization.
    let mut altered = projection.bytes.clone();
    let text = projection.computations[0].text_bytes as usize;
    let mut cursor = c::ASSIGNED_PLAN_HEADER_BYTES;
    while cursor < altered.len() {
        let length = usize::from(u16::from_le_bytes([
            altered[cursor + 1],
            altered[cursor + 2],
        ]));
        if altered[cursor] == c::ASSIGNED_CONFIGURATION
            && length == 19 + text
            && altered[cursor + 3 + 18] == 3
        {
            altered[cursor + 3 + 19] = if altered[cursor + 3 + 19] == b'0' {
                b'1'
            } else {
                b'0'
            };
            break;
        }
        cursor += 3 + length;
    }
    assert!(cursor < altered.len());
    let (result, memory) = allocation_probe::observe(|| {
        PreparedAssignedAppliance::prepare(
            &altered,
            assignment::NativeProjection::maxima(),
            projection.required(),
            &recipe.resident(&projection),
            k::FixedValueStore::<1, 8>::new(8).unwrap(),
        )
    });
    assert!(matches!(result, Err(E::Tables(_))));
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    assert_eq!(effects.get(), 0);
}
