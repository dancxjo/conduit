//! Browser realization of the tutorial Face through one ordinary Mask Form.

use conduit_body::BodyId;
use conduit_core::{
    bind_active_play, kind_id, port_id, ActivePlayIdentity, ArtifactId, Back, BackOfferBuilder,
    BootId, CapabilityId, CapabilityLimits, ExecutionProfileId, HostAdvertisement, HostId,
    HostProfileId, ImplementationId, Kind, KindIdentity, OfferGeneration, PortDescriptor,
    PortDirection, PortTemporal, SignId, PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    KindSignature, ProfileCatalog, StartupCatalog,
};
use conduit_presentation::{
    install_mask_form_value_aliases, AdmittedMaskFormRoutes, BodyMaskWardrobe,
    ManifestationLifecycle, MaskForm, MaskShow, MaskWardrobe, MaskWardrobeAction,
    MaskWardrobeControl, MaskWardrobeControlEvidence, MaskWardrobeLifetime, PlannedMaskForm,
    Presentation, SealedMaskFormRoute, PRESENTATION_INTERACTION_VALUE_KIND,
    PRESENTATION_VALUE_KIND, SHOW_VALUE_KIND,
};
use serde::{Deserialize, Serialize};

const MASK_SOURCE: &str = "form browser-graphical (\n >> presentation: Presentation\n interaction: FaceInteraction...| >>\n show: Show >>\n) {\n mask: presentation/browser-dom-mask\n presentation >> mask.presentation\n mask.interaction >> interaction\n mask.show >> show\n}\n";

#[derive(Debug, Clone, Serialize)]
pub struct BrowserMaskEffect {
    pub schema: &'static str,
    pub mask_form: conduit_core::FormIdentity,
    pub mask_plan_id: conduit_core::PlanId,
    pub mask_play: ActivePlayIdentity,
    pub show_id: String,
    pub manifestation_id: String,
    pub placement_id: conduit_core::PlacementId,
    pub presentation_id: String,
    pub presentation_revision: u64,
    pub text: Vec<conduit_presentation::PresentationText>,
    pub actions: Vec<conduit_presentation::PresentationAction>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserMaskAcknowledgement {
    pub show_id: String,
    pub manifestation_id: String,
    pub mask_plan_id: conduit_core::PlanId,
    pub active_play_id: conduit_core::ActivePlayId,
    pub placement_id: conduit_core::PlacementId,
    pub presentation_id: String,
    pub presentation_revision: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct BrowserMaskObservation {
    pub schema: &'static str,
    pub wardrobe_action: MaskWardrobeControlEvidence,
    pub planned_mask: PlannedMaskForm,
    pub mask_play: ActivePlayIdentity,
    pub presentation: Presentation,
    pub mask_show: MaskShow,
}

pub struct BrowserMaskRuntime {
    routes: AdmittedMaskFormRoutes,
    wardrobe_action: MaskWardrobeControlEvidence,
    planned: PlannedMaskForm,
    play: ActivePlayIdentity,
    presentation: Presentation,
    show: MaskShow,
}

impl BrowserMaskRuntime {
    pub fn prepare(
        body_id: BodyId,
        host_id: HostId,
        boot_id: BootId,
        presentation: Presentation,
    ) -> Result<(Self, BrowserMaskEffect), String> {
        let (mask, planned, routes) = planned_mask(host_id, boot_id)?;
        let wardrobe = MaskWardrobe::new(MaskWardrobeLifetime::Body, vec![], vec![])
            .map_err(|error| format!("{error:?}"))?;
        let scoped =
            BodyMaskWardrobe::new(body_id, None, wardrobe).map_err(|error| format!("{error:?}"))?;
        let scoped_body_id = scoped.body_id.clone();
        let mut control = MaskWardrobeControl::new(
            &scoped_body_id,
            scoped,
            planned.plan.plan_id.clone(),
            &routes,
            None,
        )
        .map_err(|error| format!("{error:?}"))?;
        let wardrobe_action = control
            .apply(
                0,
                MaskWardrobeAction::Wear(mask.form_identity.clone()),
                &routes,
            )
            .map_err(|error| format!("{error:?}"))?;
        let terminal = planned.show_placement();
        let play = bind_active_play(
            &planned.plan.plan_id,
            &terminal.host_id,
            &terminal.boot_id,
            1,
        );
        let face_subject = presentation
            .subjects
            .first()
            .ok_or_else(|| "browser Mask refuses a Presentation without a subject".to_string())?
            .identity
            .clone();
        let show = MaskShow::prepared(
            &planned,
            &presentation,
            play.clone(),
            face_subject,
            "browser/document".into(),
            SignId::from(format!(
                "sign/browser-mask-prepared/{}",
                presentation.revision
            )),
        )
        .map_err(|error| format!("{error:?}"))?;
        let effect = BrowserMaskEffect {
            schema: "conduit.browser/mask-effect@1",
            mask_form: mask.form_identity,
            mask_plan_id: planned.plan.plan_id.clone(),
            mask_play: play.clone(),
            show_id: show.show_id.as_str().into(),
            manifestation_id: show.show.manifestation_id.as_str().into(),
            placement_id: show.show.placement_id.clone(),
            presentation_id: presentation.identity.as_str().into(),
            presentation_revision: presentation.revision,
            text: presentation.text.clone(),
            actions: presentation.actions.clone(),
        };
        Ok((
            Self {
                routes,
                wardrobe_action,
                planned,
                play,
                presentation,
                show,
            },
            effect,
        ))
    }

    pub fn acknowledge(&mut self, ack: &BrowserMaskAcknowledgement) -> Result<(), String> {
        let exact = ack.show_id == self.show.show_id.as_str()
            && ack.manifestation_id == self.show.show.manifestation_id.as_str()
            && ack.mask_plan_id == self.planned.plan.plan_id
            && ack.active_play_id == self.play.active_play_id
            && ack.placement_id == self.show.show.placement_id
            && ack.presentation_id == self.presentation.identity.as_str()
            && ack.presentation_revision == self.presentation.revision;
        if !exact {
            return Err("browser Mask acknowledgement is stale or mismatched".into());
        }
        self.show = self
            .show
            .transition(
                ManifestationLifecycle::Available,
                SignId::from(format!(
                    "sign/browser-mask-available/{}",
                    self.presentation.revision
                )),
            )
            .map_err(|error| format!("{error:?}"))?;
        self.show
            .validate(&self.presentation)
            .map_err(|error| format!("{error:?}"))
    }

    pub fn observation(&self) -> BrowserMaskObservation {
        let _ = self.routes.plan_id();
        BrowserMaskObservation {
            schema: "conduit.browser/mask-observation@1",
            wardrobe_action: self.wardrobe_action.clone(),
            planned_mask: self.planned.clone(),
            mask_play: self.play.clone(),
            presentation: self.presentation.clone(),
            mask_show: self.show.clone(),
        }
    }
}

fn port(
    name: &str,
    value_kind: &str,
    direction: PortDirection,
    temporal: PortTemporal,
) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value_kind),
        direction,
        temporal,
        abnormal_kind: None,
    }
}

fn planned_mask(
    host_id: HostId,
    boot_id: BootId,
) -> Result<(MaskForm, PlannedMaskForm, AdmittedMaskFormRoutes), String> {
    let definition = Kind {
        startup_parameters: vec![],
        shorthand: None,
        kind_id: kind_id("presentation/browser-dom-mask"),
        kind_contract_revision: KindIdentity::from("conduit.browser/presentation-dom-mask@1"),
        inputs: vec![port(
            "presentation",
            PRESENTATION_VALUE_KIND,
            PortDirection::Input,
            PortTemporal::Value,
        )],
        outputs: vec![
            port(
                "interaction",
                PRESENTATION_INTERACTION_VALUE_KIND,
                PortDirection::Output,
                PortTemporal::Flow { closes: true },
            ),
            port(
                "show",
                SHOW_VALUE_KIND,
                PortDirection::Output,
                PortTemporal::Value,
            ),
        ],
        configuration: vec![],
        semantic_laws: Default::default(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 4,
            max_queue_bytes: 512 * 1024,
        },
    };
    let mut startup = StartupCatalog::new();
    install_mask_form_value_aliases(&mut startup).map_err(|error| format!("{error:?}"))?;
    startup
        .insert(KindSignature {
            kind: definition.kind_id.as_str().into(),
            startup_parameters: vec![],
        })
        .map_err(|error| format!("{error:?}"))?;
    let mut profiles = ProfileCatalog::new();
    profiles
        .insert_kind(definition.clone())
        .map_err(|error| format!("{error:?}"))?;
    let syntax = parse_syntax_document(MASK_SOURCE);
    if !syntax.diagnostics.is_empty() {
        return Err(format!("{:?}", syntax.diagnostics));
    }
    let checked = check_syntax_document(&syntax, &startup).map_err(|error| format!("{error:?}"))?;
    let authoring = expand_canonical_form_for_authoring(&checked, "browser-graphical", &profiles)
        .map_err(|error| format!("{error:?}"))?;
    let mask = MaskForm::admit(&authoring).map_err(|error| format!("{error:?}"))?;
    let offer = BackOfferBuilder::new(
        definition,
        Back {
            capability_id: CapabilityId::from("capability/browser-dom-mask"),
            execution_profile_id: ExecutionProfileId::from("browser/mask@1"),
            implementation_id: ImplementationId::from("implementation/browser-dom-mask"),
            artifact_id: ArtifactId::from("artifact/browser-runtime"),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build();
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id,
        boot_id,
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("browser/mask@1"),
        bases: vec![],
        resources: vec![],
        capabilities: vec![offer],
        planner_capabilities: vec![],
    };
    let placements = conduit_planner::default_expanded_placements(
        &authoring.expanded,
        core::slice::from_ref(&host),
    )
    .map_err(|error| format!("{error:?}"))?;
    let plan =
        conduit_planner::plan_expanded_canonical(&authoring.expanded, &[host], &placements, &[])
            .map_err(|error| format!("{error:?}"))?;
    let planned = PlannedMaskForm::admit(&mask, &plan).map_err(|error| format!("{error:?}"))?;
    let route = SealedMaskFormRoute {
        route_id: "route/browser-graphical".into(),
        mask_form: mask.form_identity.clone(),
        plan_id: plan.plan_id.clone(),
        placement_ids: plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .map(|placement| placement.placement_id.clone())
            .collect(),
        currently_available: true,
    };
    let routes = AdmittedMaskFormRoutes::new(&plan, core::slice::from_ref(&mask), vec![route])
        .map_err(|error| format!("{error:?}"))?;
    Ok((mask, planned, routes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{CheckedFormId, ExpandedFormId, PlanId, SourceDocumentId};
    use conduit_presentation::{
        PresentationBasis, PresentationRole, PresentationSubject, PresentationText,
    };

    fn body_id() -> BodyId {
        conduit_body::Body::born(
            SourceDocumentId::from("source/browser-mask-test"),
            CheckedFormId::from("checked/browser-mask-test"),
            1,
            SignId::from("sign/body-born"),
        )
        .unwrap()
        .body_id
    }

    fn presentation() -> Presentation {
        Presentation::new(
            7,
            PresentationBasis {
                body_id: Some(body_id()),
                wake_id: None,
                source_document_id: Some(SourceDocumentId::from("source/application")),
                checked_form_id: Some(CheckedFormId::from("checked/application")),
                expanded_form_id: Some(ExpandedFormId::from("expanded/application")),
                plan_id: Some(PlanId::from("plan/application")),
                active_play_id: None,
                sign_ids: vec![SignId::from("sign/presentation")],
            },
            vec![PresentationSubject {
                identity: "body/browser-mask-test".into(),
                role: PresentationRole::Body,
                label: "Test Body".into(),
                accessibility_name: "Test Body".into(),
            }],
            vec![],
            vec![],
            vec![PresentationText {
                subject: "body/browser-mask-test".into(),
                text: "One canonical journey".into(),
            }],
        )
        .unwrap()
    }

    fn acknowledgement(effect: &BrowserMaskEffect) -> BrowserMaskAcknowledgement {
        BrowserMaskAcknowledgement {
            show_id: effect.show_id.clone(),
            manifestation_id: effect.manifestation_id.clone(),
            mask_plan_id: effect.mask_plan_id.clone(),
            active_play_id: effect.mask_play.active_play_id.clone(),
            placement_id: effect.placement_id.clone(),
            presentation_id: effect.presentation_id.clone(),
            presentation_revision: effect.presentation_revision,
        }
    }

    #[test]
    fn show_becomes_available_only_after_exact_browser_acknowledgement() {
        let (mut runtime, effect) = BrowserMaskRuntime::prepare(
            body_id(),
            HostId::from("host/browser"),
            BootId::from("boot/browser"),
            presentation(),
        )
        .unwrap();
        assert_eq!(
            runtime.show.show.lifecycle,
            ManifestationLifecycle::Prepared
        );
        assert_ne!(
            runtime.presentation.basis.plan_id.as_ref(),
            Some(&runtime.planned.plan.plan_id)
        );

        let mut stale = acknowledgement(&effect);
        stale.presentation_revision += 1;
        assert!(runtime.acknowledge(&stale).is_err());
        assert_eq!(
            runtime.show.show.lifecycle,
            ManifestationLifecycle::Prepared
        );

        runtime.acknowledge(&acknowledgement(&effect)).unwrap();
        assert_eq!(
            runtime.show.show.lifecycle,
            ManifestationLifecycle::Available
        );
        assert_eq!(
            runtime.wardrobe_action.resulting_wardrobe.worn,
            vec![runtime.planned.mask.form_identity.clone()]
        );
    }
}
