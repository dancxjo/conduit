//! CPU delegated preparation and one real typed kernel transaction; no native IO.
use conduit_body::*;
use conduit_core as c;
use conduit_embedded_build as b;
use conduit_plan_lowering::{
    assigned_play_scope::AssignedPlayScope,
    selected_assigned_expression::ResidentExpressionBindingRefusal as E,
};
use ed25519_dalek::{Signer, SigningKey};
use std::cell::Cell;

struct Receiver {
    member: MembershipCredential,
    actual: c::HostAdvertisement,
    bootstrap: MemberBootstrap<'static>,
    key: SigningKey,
    nonce: Cell<u8>,
}
impl Receiver {
    fn new(scope: PreparedBodyFragment<'_>) -> Self {
        let f = scope.fragment();
        let key = SigningKey::from_bytes(&[9; 32]);
        let member: MembershipCredential=serde_json::from_value(serde_json::json!({
            "credential_id":"fixture/credential", "body_id":scope.plan().body_id,"part_id":"fixture/part",
            "host_id":f.host_id,"boot_id":f.boot_id,"issued_at_millis":1,
        })).unwrap();
        let owner = FederationPeer {
            host_id: "fixture/issuer".into(),
            boot_id: "fixture/issuer/boot".into(),
            offer_generation: c::OfferGeneration(1),
            verifying_key: key.verifying_key().to_bytes(),
        };
        let bootstrap = MemberBootstrap {
            schema: MEMBER_BOOTSTRAP_SCHEMA.into(),
            body_id: member.body_id.clone(),
            host_id: f.host_id.as_str().into(),
            image_id: "fixture/image".into(),
            configuration_id: "fixture/config".into(),
            owner,
            admission: MemberBootstrapMaterial::Retained {
                credential: member.clone(),
                secret: [3; 32],
            },
        };
        Self {
            member,
            bootstrap,
            key,
            nonce: Cell::new(1),
            actual: c::HostAdvertisement {
                declared_name: None,
                protocol_version: c::PROTOCOL_VERSION,
                host_id: f.host_id.clone(),
                boot_id: f.boot_id.clone(),
                offer_generation: f.offer_generation,
                profile: "fixture/profile".into(),
                bases: vec![],
                resources: vec![],
                capabilities: vec![],
                planner_capabilities: vec![],
            },
        }
    }
    fn context(&self) -> NativePreparationContext<'_> {
        NativePreparationContext {
            bootstrap: &self.bootstrap,
            member: &self.member,
            actual: &self.actual,
            image_id: "fixture/image",
            configuration_id: "fixture/config",
            now_millis: 11,
        }
    }
    fn command<'a>(&'a self, bytes: &'a [u8]) -> AttributedHostCommand<'a, 'a> {
        self.command_with_contract(bytes, HOST_PREPARED_FRAGMENT_CONTRACT)
    }
    fn command_with_contract<'a>(
        &'a self,
        bytes: &'a [u8],
        contract: &'a str,
    ) -> AttributedHostCommand<'a, 'a> {
        let nonce = self.nonce.get();
        self.nonce.set(nonce.checked_add(1).unwrap());
        let slot = IssuedHostCommandChallenge::issue(
            &self.member,
            &self.actual,
            &self.bootstrap.owner,
            HostCommandReceiveBounds {
                payload_contract: contract,
                maximum_payload_bytes: 8192,
                lifetime_millis: 10,
            },
            [nonce; 32],
            10,
        )
        .unwrap();
        let signature = self
            .key
            .sign(&host_command_attribution_transcript(slot.challenge(), bytes).unwrap())
            .to_bytes();
        slot.authenticate(bytes, &signature, 11).unwrap()
    }
}
fn policy() -> HostPreparationDelegation {
    HostPreparationDelegation {
        authorizer: HostPreparationAuthorizer::SeedOwner,
        maximum_assignment_bytes: 8192,
        maximum_runtime_state_bytes: 8192,
    }
}
fn profile(a: &crate::support::Assignment) -> AssignedBodyPreparationProfile<'_> {
    let f = &a.model.fragments[0];
    AssignedBodyPreparationProfile {
        maxima: a.maxima,
        requirements: c::AssignedPlanRequirements {
            host: c::AssignedIdentity::from_text(f.host_id.as_str()),
            boot: c::AssignedIdentity::from_text(f.boot_id.as_str()),
            host_calls: &[],
            resources: &[0],
            remote_bindings: &[],
        },
        computations: c::AssignedComputationProfile {
            maximum_text_bytes: a.pins[0].text_bytes,
            maximum_total_text_bytes: a.pins[0].text_bytes,
            requirements: &a.pins,
        },
        fore: Some(&a.fore),
    }
}

fn selection(
    scope: PreparedBodyFragment<'_>,
    a: &crate::support::Assignment,
) -> (
    Vec<b::BoundResidentExpression>,
    b::BodyFragmentProjectionLimits,
) {
    let (doc, program, temporal) =
        crate::support::context_for(scope.fragment().placements[0].inputs[0].temporal);
    let image = b::PreparedResidentExpression::prepare(
        &doc,
        &program,
        temporal,
        crate::support::bounds(1000),
    )
    .unwrap();
    let fragment = scope.fragment();
    let bound = b::bind_current_resident_expressions(
        &a.model,
        &fragment.fragment_id,
        &a.advertisement,
        &[&image],
    )
    .unwrap();
    (
        bound,
        b::BodyFragmentProjectionLimits {
            image: b::EmbeddedImageBounds::HOST_TOOLING,
            computations: b::EmbeddedComputationBounds {
                maximum_program_bytes: 512 * 1024,
                maximum_total_program_bytes: 1024 * 1024,
            },
            assigned: a.maxima,
            maximum_fore_ports: 2,
            maximum_document_bytes: 8192,
        },
    )
}
fn document(
    scope: PreparedBodyFragment<'_>,
    a: &crate::support::Assignment,
) -> b::GeneratedBodyFragmentDocument {
    let (resident, limits) = selection(scope, a);
    b::generate_prepared_body_fragment(scope, &resident, limits).unwrap()
}
pub fn mask(scope: PreparedBodyFragment<'_>, a: &crate::support::Assignment) {
    let doc = document(scope, a);
    let receiver = Receiver::new(scope);
    let prepared = receive_delegated_body_fragment(
        Some(&policy()),
        receiver.context(),
        receiver.command(doc.bytes()),
        profile(a),
    )
    .unwrap();
    assert_eq!(prepared.projection(), a.encoded);
    assert_eq!(prepared.metadata().body_plan, scope.plan().plan_id.as_str());
}
pub fn run(scope: PreparedBodyFragment<'_>, a: &crate::support::Assignment) {
    current_resolution::run(a);
    inventory_preparation::run(scope, a);
    resources::run(a);
    let document = document(scope, a);
    let (_, program, _) = crate::support::context();
    let receiver = Receiver::new(scope);
    let mut library = crate::library::Initialized::prepare().unwrap();
    let input = crate::support_kernel::frame(&program);
    let expected = program.evaluate(&input).unwrap();
    let ((), cost) = crate::probe::observe(|| {
        assert!(matches!(
            receive_delegated_body_fragment(
                None,
                receiver.context(),
                receiver.command(document.bytes()),
                profile(a)
            ),
            Err(DelegatedFragmentRefusal::Policy(
                PreparationDelegationRefusal::MissingDelegation
            ))
        ));
        let prepared = receive_delegated_body_fragment(
            Some(&policy()),
            receiver.context(),
            receiver.command(document.bytes()),
            profile(a),
        )
        .unwrap();
        assert_eq!(prepared.projection(), a.encoded);
        assert_eq!(prepared.metadata().body_plan, scope.plan().plan_id.as_str());
        assert_eq!(
            prepared.metadata().body_play,
            scope.play().active_play_id.as_str()
        );
        let projection = prepared.projection();
        assert!(projection.as_ptr() >= document.bytes().as_ptr());
        assert!(
            (projection.as_ptr() as usize) + projection.len()
                <= document.bytes().as_ptr() as usize + document.bytes().len()
        );
        let (_, back) = library
            .borrow_entry_0(a.selected_scope(AssignedPlayScope::Delegated(&prepared)))
            .unwrap();
        let back = crate::combined::execute_and_retire(back, a, &input, &expected);
        assert!(back.finish_retired().is_ok());
        assert!(library.offers().iter().all(Option::is_some));
    });
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
    let mut malformed = document.bytes().to_vec();
    malformed[0] = 0;
    assert!(matches!(
        receive_delegated_body_fragment(
            Some(&policy()),
            receiver.context(),
            receiver.command(&malformed),
            profile(a)
        ),
        Err(DelegatedFragmentRefusal::Document(
            BodyFragmentDocumentRefusal::Metadata
        ))
    ));
    let mut changed = document.bytes().to_vec();
    *changed.last_mut().unwrap() ^= 1;
    assert!(matches!(
        receive_delegated_body_fragment(
            Some(&policy()),
            receiver.context(),
            receiver.command(&changed),
            profile(a)
        ),
        Err(DelegatedFragmentRefusal::Document(
            BodyFragmentDocumentRefusal::Projection
        ))
    ));
    for policy_small in [true, false] {
        let mut p = policy();
        let mut actual = profile(a);
        if policy_small {
            p.maximum_runtime_state_bytes = 1;
        } else {
            actual.maxima.runtime_state_bytes = 1;
        }
        assert!(matches!(
            receive_delegated_body_fragment(
                Some(&p),
                receiver.context(),
                receiver.command(document.bytes()),
                actual
            ),
            Err(DelegatedFragmentRefusal::Assignment(
                c::AssignedPlanRefusal::RuntimeCapacityExceeded
            ))
        ));
    }
    let prepared = receive_delegated_body_fragment(
        Some(&policy()),
        receiver.context(),
        receiver.command(document.bytes()),
        profile(a),
    )
    .unwrap();
    let mut alternate = a.encoded.clone();
    let first = c::ASSIGNED_PLAN_HEADER_BYTES;
    assert_eq!(alternate[first], c::ASSIGNED_NODE);
    alternate[first + 3 + 4] ^= 1;
    let digest = c::assigned_plan_payload_digest(&alternate[c::ASSIGNED_PLAN_HEADER_BYTES..]);
    alternate[93..125].copy_from_slice(&digest);
    let p = profile(a);
    let view = c::decode_assigned_plan_with_fore(
        &alternate,
        p.maxima,
        p.requirements,
        p.computations,
        p.fore.unwrap(),
    )
    .unwrap();
    assert_eq!(view, prepared.view()); // IDs/counts alone would accept substitution.
    let mut selected = a.selected_scope(AssignedPlayScope::Delegated(&prepared));
    selected.input =
        conduit_plan_lowering::selected_assigned_operation::AssignedOperationInput::Contiguous(
            &alternate,
        );
    assert!(matches!(library.borrow_entry_0(selected), Err(E::Scope)));
    let records: Vec<_> = c::AssignedPlanRecords::new(&alternate)
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let mut selected = a.selected_scope(AssignedPlayScope::Delegated(&prepared));
    selected.input =
        conduit_plan_lowering::selected_assigned_operation::AssignedOperationInput::Records {
            header: &alternate[..c::ASSIGNED_PLAN_HEADER_BYTES],
            records: &records,
        };
    assert!(matches!(library.borrow_entry_0(selected), Err(E::Scope)));
    assert!(library.offers().iter().all(Option::is_some));
    println!("delegated exact Body projection: signed receive/bind/typed kernel output/retirement without allocation; mutated projection, absent policy, insufficient policy/profile memory and same-ID table substitution refused");
}

#[path = "delegated_resources.rs"]
mod resources;

pub fn batch(
    a: PreparedBodyFragment<'_>,
    b: PreparedBodyFragment<'_>,
    first: &crate::support::Assignment,
    second: &crate::support::Assignment,
) {
    preparation_workset::run(a, b, first, second);
    resources::batch(a, b, first, second);
}

#[path = "inventory_preparation.rs"]
mod inventory_preparation;

#[path = "workset.rs"]
mod preparation_workset;

#[path = "current_resolution.rs"]
mod current_resolution;
