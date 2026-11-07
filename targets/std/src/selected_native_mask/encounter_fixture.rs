//! Explicit membership/Line contract fixture; no live admission or TLS claim.
use super::*;
use conduit_body::{
    AuthenticatedHostObservation, Body, BodyBiographyEvidence, BodyLifecycleSession,
    BodyMembership, MembershipProofId, PartId,
};
use conduit_face::{FaceBasis, FaceRole, FaceSubject};

pub(super) struct FixtureEncounter {
    session: BodyLifecycleSession,
    owner: HostAdvertisement,
    input: LineOffer,
    output: LineOffer,
    pub calls: std::cell::Cell<usize>,
}
impl NativeMaskEncounter for FixtureEncounter {
    fn validate_current(
        &self,
        face: &Face,
        route: &RemoteOwnerMaskRouteSeal,
        local: &HostAdvertisement,
    ) -> Result<(), String> {
        self.calls.set(self.calls.get() + 1);
        route
            .validate_current(
                &self.session,
                face,
                &self.owner,
                local,
                &self.input,
                &self.output,
            )
            .map_err(|error| format!("fixture encounter: {error:?}"))
    }
}
pub(super) fn encounter(
    host: &HostAdvertisement,
    revision: u64,
) -> (FixtureEncounter, RemoteOwnerMaskRouteSeal, Face) {
    encounter_with_binding(host, revision, "fixture/binding")
}
pub(super) fn encounter_with_binding(
    host: &HostAdvertisement,
    revision: u64,
    binding: &str,
) -> (FixtureEncounter, RemoteOwnerMaskRouteSeal, Face) {
    let planned = conduit_conduitos_mask_offer::prepare_native_stage_with_profile(
        host,
        NativeMaskStorageProfile::Standard,
    )
    .unwrap()
    .planned_mask;
    let body = Body::born(
        planned.mask.plot_identity.source_document_id.clone(),
        planned.mask.plot_identity.checked_plot_id.clone(),
        1,
        "fixture/born".into(),
    )
    .unwrap();
    let id = body.body_id.clone();
    let mut membership = BodyMembership::new(id.clone()).unwrap();
    let mut evidence =
        BodyBiographyEvidence::born(body, membership.clone(), "Fixture".into()).unwrap();
    let part = PartId::bind(&id, host.host_id.as_str(), 1).unwrap();
    let proof = MembershipProofId::bind("fixture/native-mask").unwrap();
    let admitted = membership
        .admit(
            &id,
            membership.revision,
            part.clone(),
            proof.clone(),
            "fixture/admit".into(),
        )
        .unwrap();
    let present = membership
        .observe_present(
            &id,
            membership.revision,
            &part,
            AuthenticatedHostObservation {
                host_id: host.host_id.clone(),
                boot_id: host.boot_id.clone(),
                offer_generation: host.offer_generation,
                proof_id: proof,
                sequence: 1,
            },
            "fixture/present".into(),
        )
        .unwrap();
    evidence
        .append_membership_events(membership, &[(admitted, 2), (present, 3)])
        .unwrap();
    let session = BodyLifecycleSession::open(evidence).unwrap();
    let face = Face::new(
        revision,
        FaceBasis {
            body_id: Some(id),
            wake_id: None,
            source_document_id: None,
            checked_plot_id: None,
            expanded_plot_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![],
        },
        vec![FaceSubject {
            identity: "front/native-mask".into(),
            role: FaceRole::Document,
            name: "Real semantic CPU diagnostic".into(),
        }],
        vec![],
        vec![],
        vec![],
    )
    .unwrap();
    let owner = StdHost::new_with_config(crate::StdHostConfig {
        host_id: "fixture/owner".into(),
        boot_id: "fixture/owner-boot".into(),
        offer_generation: OfferGeneration(1),
    })
    .advertisement;
    let line = |label: &str, source: &HostAdvertisement, sink: &HostAdvertisement| {
        let mut line = process_owned_line_offer_with_limits(
            &format!("fixture/line/{label}"),
            &format!("{binding}/{label}"),
            "conduit.base/websocket-rfc6455@1".into(),
            "fixture/carrier",
            source,
            sink,
            LinkLimits {
                maximum_in_flight_items: 1,
                maximum_payload_bytes: 64 * 1024,
                maximum_buffered_bytes: 128 * 1024,
                maximum_frame_bytes: 64 * 1024,
            },
        );
        line.binding.credential = LinkCredentialReference::Opaque("fixture/credential".into());
        line.binding.authority = LinkAuthorityReference::Grant("fixture/grant".into());
        line.contract.scope = LineScope::RoutedNetwork;
        line.contract.security = LineSecurity::PlaintextNetwork;
        line
    };
    let input = line("face", &owner, host);
    let output = line("show", host, &owner);
    let choices = [
        (
            PortDirection::Input,
            &planned.mask.face_input.front_port_id,
            &input,
        ),
        (
            PortDirection::Output,
            &planned.mask.show_output.front_port_id,
            &output,
        ),
    ]
    .map(
        |(direction, port, line)| conduit_planner::ExternalForeLineChoice {
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            direction,
            front_port_id: port.clone(),
            track: ConnectionTrack::Payload,
            peer_host_id: owner.host_id.clone(),
            peer_boot_id: owner.boot_id.clone(),
            line_id: line.line_id.clone(),
        },
    );
    let plan = conduit_planner::bind_external_fore_lines(
        &planned.plan,
        &choices,
        &[input.clone(), output.clone()],
        &["conduit.base/websocket-rfc6455@1".into()],
    )
    .unwrap();
    let planned = PlannedMaskPlot::admit(&planned.mask, &plan).unwrap();
    let route = RemoteOwnerMaskRouteSeal::seal_current(
        &session, &face, &owner, host, &planned, &input, &output,
    )
    .unwrap();
    (
        FixtureEncounter {
            session,
            owner,
            input,
            output,
            calls: std::cell::Cell::new(0),
        },
        route,
        face,
    )
}
