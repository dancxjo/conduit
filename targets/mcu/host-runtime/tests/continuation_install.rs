#![cfg(feature = "execution")]
//! CPU media proof: actual owner-attested exchange, original Root signing,
//! physical-shaped NOR cuts and retained-identity restore. No device/Start proof.
use conduit_body as b;
use conduit_core as c;
use conduit_mcu_host_runtime::{
    continuation_artifact as a, root::*, seed::Seed, transport::MessageTransport,
};
use conduit_resource_snapshot::nor::{NorRefusal, NorStorage};
use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};
use std::{
    collections::VecDeque,
    future::Future,
    task::{Context, Poll, Waker},
};
#[path = "../../../../architecture/body/tests/continuation_scope/reference.rs"]
mod reference;
static PREP: b::HostPreparationDelegation = b::HostPreparationDelegation {
    authorizer: b::HostPreparationAuthorizer::SeedOwner,
    maximum_assignment_bytes: 24576,
    maximum_runtime_state_bytes: 24576,
};
static EXEC: b::HostExecutionDelegation = b::HostExecutionDelegation {
    authorizer: b::HostExecutionAuthorizer::SeedOwner,
    maximum_active_worksets: 1,
    continuation: Some(b::ContinuationCeilings {
        assignment_bytes: 24576,
        runtime_bytes: 24576,
        durable_bytes: 4096,
        operation_slots: 8,
        active_worksets: 1,
    }),
};
fn config() -> Configuration {
    Configuration {
        image_id: "image",
        configuration_id: "config",
        friendly_label: "CPU",
        preparation: Some(&PREP),
        execution: Some(&EXEC),
    }
}
fn ready<T: Future>(f: T) -> T::Output {
    let mut f = core::pin::pin!(f);
    match f.as_mut().poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(v) => v,
        Poll::Pending => panic!("bounded CPU transport"),
    }
}
struct Messages(VecDeque<Vec<u8>>);
impl MessageTransport for Messages {
    type Error = ();
    async fn receive<'a>(&mut self, out: &'a mut [u8]) -> Result<&'a [u8], ()> {
        let next = self.0.pop_front().ok_or(())?;
        let n = next.len();
        out.get_mut(..n).ok_or(())?.copy_from_slice(&next);
        Ok(&out[..n])
    }
    async fn send(&mut self, _: &[u8]) -> Result<(), ()> {
        Ok(())
    }
}
struct Media {
    bytes: Vec<u8>,
    programs: usize,
    fail: Option<usize>,
}
impl Media {
    fn erased() -> Self {
        Self {
            bytes: vec![255; a::PREFIX_BYTES + 256],
            programs: 0,
            fail: None,
        }
    }
}
impl NorStorage for Media {
    fn capacity(&self) -> u32 {
        self.bytes.len() as u32
    }
    fn program_unit(&self) -> u32 {
        256
    }
    fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), NorRefusal> {
        out.copy_from_slice(
            self.bytes
                .get(at as usize..at as usize + out.len())
                .ok_or(NorRefusal::Geometry)?,
        );
        Ok(())
    }
    fn program(&mut self, at: u32, input: &[u8]) -> Result<(), NorRefusal> {
        assert_eq!(input.len(), 256);
        assert!((at as usize + 256) <= a::PREFIX_BYTES);
        self.programs += 1;
        let cut = self.fail == Some(self.programs);
        let n = if cut { 128 } else { 256 };
        for (old, new) in self.bytes[at as usize..at as usize + n]
            .iter_mut()
            .zip(input)
        {
            assert_eq!(*old & *new, *new);
            *old &= *new;
        }
        if cut {
            Err(NorRefusal::Media)
        } else {
            Ok(())
        }
    }
}
#[test]
fn original_root_selects_commits_and_restores_only_retained_acceptance() {
    reference::with_full_reference(|fresh_scope, verified, actual, full, pins, source| {
        let i = verified.intent();
        let owner = SigningKey::from_bytes(&[7; 32]);
        let member = SigningKey::from_bytes(&[3; 32]);
        let bootstrap = b::MemberBootstrap {
            schema: b::MEMBER_BOOTSTRAP_SCHEMA.into(),
            body_id: serde_json::from_value(serde_json::json!(i.body_id)).unwrap(),
            host_id: i.host_id.into(),
            image_id: i.image_id.into(),
            configuration_id: i.configuration_id.into(),
            owner: b::FederationPeer {
                host_id: i.issuer_host.into(),
                boot_id: "owner/boot".into(),
                offer_generation: c::OfferGeneration(1),
                verifying_key: owner.verifying_key().to_bytes(),
            },
            admission: b::MemberBootstrapMaterial::Spawn {
                provision: b::PicoSpawnProvision {
                    protocol: b::PICO_SPAWN_PROTOCOL,
                    spore_id: "spore",
                    image_id: i.image_id,
                    invitation_id: "invitation",
                    body_id: i.body_id,
                    nonce: [2; 32],
                    expires_at_millis: 1000,
                    secret: [3; 32],
                },
            },
        };
        let json = serde_json::to_vec(&bootstrap).unwrap();
        let mut carrier = b"CONDUIT_SEED@2\0\0".to_vec();
        carrier.extend_from_slice(&(json.len() as u32).to_le_bytes());
        carrier.extend_from_slice(&json);
        let seed = || Seed::parse(&carrier, actual.host_id.as_str(), "image", "config").unwrap();
        let mut original_actual = actual.clone();
        original_actual.boot_id = "local/previous".into();
        let mut root = Root::<24576>::new(seed(), original_actual.clone(), config()).unwrap();
        let receipt = b::PortableAdmissionReceipt {
            schema: b::SPAWN_ADMISSION_RECEIPT_SCHEMA.into(),
            credential: b::MembershipCredential {
                credential_id: serde_json::from_value(serde_json::json!(i.credential_id)).unwrap(),
                body_id: bootstrap.body_id.clone(),
                part_id: serde_json::from_value(serde_json::json!(i.part_id)).unwrap(),
                host_id: actual.host_id.clone(),
                boot_id: original_actual.boot_id.clone(),
                issued_at_millis: 10,
            },
            host_advertisement: original_actual,
            membership_admitted: true,
            current_offers_available: false,
            plan_created: false,
            play_created: false,
        };
        let signed = b::attest_portable_admission_receipt(
            &receipt,
            bootstrap.owner.clone(),
            member.verifying_key().to_bytes(),
            |t| owner.sign(t).to_bytes(),
        )
        .unwrap();
        let mut messages = Messages(
            [
                serde_json::to_vec(&signed.transport_metadata().unwrap()).unwrap(),
                signed.receipt_json.into_bytes(),
            ]
            .into(),
        );
        let mut output = [0; 4096];
        ready(root.spawn(&mut messages, &mut output)).unwrap();
        assert!(root.has_current_member());
        assert!(root.receipt_digest().is_some());
        let mut slots = [None; b::MAX_BODY_PLOTS];
        for (out, p) in slots.iter_mut().zip(full.plots) {
            *out = Some(a::PlotPin {
                source_document: p.source_document,
                checked_plot: p.checked_plot,
                plan_id: p.plan_id,
                local: p.local,
            });
        }
        let owner_signature = owner.sign(&i.signing_transcript()).to_bytes();
        let mut artifact = a::Artifact {
            schema: a::SCHEMA,
            intent: *i,
            signature: [
                owner_signature[..32].try_into().unwrap(),
                owner_signature[32..].try_into().unwrap(),
            ],
            wake_id: full.wake_id,
            wake_sequence: full.wake_sequence,
            workload_revision: full.workload_revision,
            base_body_plan: full.base_body_plan,
            plot_count: full.plots.len() as u8,
            plots: slots,
        };
        let mut candidate = [0; a::PREFIX_BYTES];
        a::encode(&artifact, &mut candidate).unwrap();
        let mut scratch = [0; a::PREFIX_BYTES];
        let prepared = root
            .prepare_continuation_install(&candidate, &mut scratch, source, pins, None)
            .unwrap();
        let mut selected = [0; a::PREFIX_BYTES];
        prepared
            .encode_selected_prefix(&candidate, &mut selected)
            .unwrap();
        let mut media = Media::erased();
        let mut readback = [0; a::PREFIX_BYTES];
        let mut work = [0; a::PREFIX_BYTES];
        // SAFETY: this CPU owner is exclusive, no actors exist, and all fixed
        // buffers/pages/work are funded by this fixture; not physical permission.
        let accepted = unsafe {
            root.commit_continuation_cold(
                prepared,
                &mut media,
                &candidate,
                &mut readback,
                &mut work,
            )
        }
        .unwrap();
        assert_eq!(&media.bytes[..a::PREFIX_BYTES], selected);
        assert!(media.bytes[a::PREFIX_BYTES..].iter().all(|b| *b == 255));
        assert_eq!(accepted.credential(), &receipt.credential);
        assert_eq!(accepted.membership_generation(), 1);
        assert_eq!(accepted.intent_generation(), 1);
        let writes = media.programs;
        assert!(writes > 0 && writes <= 32);
        for cut in 1..=writes {
            let prepared = root
                .prepare_continuation_install(&candidate, &mut scratch, source, pins, None)
                .unwrap();
            let mut interrupted = Media::erased();
            interrupted.fail = Some(cut);
            // SAFETY: same isolated, fully funded CPU media custody as above.
            assert!(matches!(
                unsafe {
                    root.commit_continuation_cold(
                        prepared,
                        &mut interrupted,
                        &candidate,
                        &mut readback,
                        &mut work,
                    )
                },
                Err(ContinuationAcceptanceRefusal::Uncertain)
            ));
            let actual: &[u8; a::PREFIX_BYTES] =
                interrupted.bytes[..a::PREFIX_BYTES].try_into().unwrap();
            assert!(root
                .read_continuation_acceptance(actual, &mut scratch)
                .is_err());
            assert!(interrupted.bytes[a::PREFIX_BYTES..]
                .iter()
                .all(|b| *b == 255));
        }
        assert!(root
            .prepare_continuation_install(&candidate, &mut scratch, b"foreign Source", pins, None)
            .is_err());
        artifact.signature[0][0] ^= 1;
        a::encode(&artifact, &mut candidate).unwrap();
        assert!(matches!(
            root.prepare_continuation_install(&candidate, &mut scratch, source, pins, None),
            Err(ContinuationAcceptanceRefusal::Signature)
        ));
        artifact.intent.intent_generation = 2;
        let signature = owner.sign(&artifact.intent.signing_transcript()).to_bytes();
        artifact.signature = [
            signature[..32].try_into().unwrap(),
            signature[32..].try_into().unwrap(),
        ];
        a::encode(&artifact, &mut candidate).unwrap();
        assert!(matches!(
            root.prepare_continuation_install(&candidate, &mut scratch, source, pins, None),
            Err(ContinuationAcceptanceRefusal::Generation)
        ));
        let replacement = root
            .prepare_continuation_install(&candidate, &mut scratch, source, pins, Some(&accepted))
            .unwrap();
        let before = media.bytes.clone();
        let count = media.programs;
        // SAFETY: isolated original CPU media, still no live effects/actors.
        assert!(matches!(
            unsafe {
                root.commit_continuation_cold(
                    replacement,
                    &mut media,
                    &candidate,
                    &mut readback,
                    &mut work,
                )
            },
            Err(ContinuationAcceptanceRefusal::RequiresErase)
        ));
        assert_eq!(media.bytes, before);
        assert_eq!(media.programs, count, "no erase or partial replacement");
        // Move the fresh fixture inventory into its original Root; no second
        // inventory is retained for permission or later ledger admission.
        let mut next = Root::<24576>::new(seed(), actual.clone(), config()).unwrap();
        let next_boot = next.actual().boot_id.as_str().to_owned();
        let inventory_address = core::ptr::from_ref(next.actual());
        let acceptance = next
            .read_continuation_acceptance(&selected, &mut scratch)
            .unwrap();
        assert!(!next.has_current_member());
        next.restore_continuation_acceptance(&acceptance).unwrap();
        assert!(!next.has_current_member());
        let retained = next.return_context(1).unwrap();
        assert_eq!(retained.member, &receipt.credential);
        assert_ne!(retained.member.boot_id.as_str(), next_boot);
        assert!(
            matches!(next.execution_parts(), Err(Refusal::NotMember)),
            "no online current-Boot receipt manufactured"
        );
        let source_digest: [u8; 32] = Sha256::digest(source).into();
        assert!(matches!(
            next.verify_installed_continuation_boot_digest(
                &acceptance,
                &selected,
                &mut scratch,
                "foreign/boot",
                source_digest,
                pins,
            ),
            Err(ContinuationAcceptanceRefusal::Scope)
        ));
        let permission = next
            .verify_installed_continuation_boot_digest(
                &acceptance,
                &selected,
                &mut scratch,
                &next_boot,
                source_digest,
                pins,
            )
            .unwrap();
        assert_eq!(permission.actual_boot(), next_boot);
        fresh_scope
            .check_actual(&permission, next.actual())
            .unwrap();
        {
            let parts = next.take_continuation_parts(&acceptance).unwrap();
            assert_eq!(
                parts.bootstrap.admission.credential(),
                Some(&receipt.credential)
            );
            let context = conduit_mcu_host_runtime::continuation_native::ContinuationNativeContext::check_installation(
                parts.bootstrap, parts.resources.host(), &permission, "image", "config",
                conduit_mcu_host_runtime::execution::Policy {
                    preparation: parts.configuration.preparation,
                    execution: parts.configuration.execution,
                    active_worksets: 0, maximum_retained_bytes: 24576,
                },
            ).unwrap();
            assert_eq!(context.verified_intent().actual_boot(), next_boot.as_str());
            fresh_scope
                .check_actual(&permission, parts.resources.host())
                .unwrap();
            assert_eq!(parts.resources.host().boot_id.as_str(), next_boot);
            assert!(
                core::ptr::eq(parts.resources.host(), inventory_address),
                "original inventory behind the mutable ledger"
            );
        }
        assert!(
            matches!(
                next.take_continuation_parts(&acceptance),
                Err(ContinuationAcceptanceRefusal::Scope)
            ),
            "one Root attempt per Boot"
        );
        let mut tampered = selected;
        tampered[20] ^= 1;
        assert!(matches!(
            next.read_continuation_acceptance(&tampered, &mut scratch),
            Err(ContinuationAcceptanceRefusal::Signature)
        ));
    });
}
