//! Thin presentation Backs over the existing canonical Patchbay graph.

use crate::{FaceControl, PatchbayCord, PatchbayGear, PatchbayPort};
use conduit_core::{KindId, LineId, PlanId, PortDirection, PortTemporal};
use conduit_presentation::{
    GraphicsCommand, GraphicsError, GraphicsPaintRole, GraphicsScene, GraphicsShapeStyle,
    LayoutRect, PresentationComposition, PresentationIconKey,
};

pub use conduit_std_catalog::{PATCHBAY_CORD_KIND, PATCHBAY_GEAR_FACE_KIND, PATCHBAY_PORT_KIND};
pub const MAX_PATCHBAY_PRESENTATION_TEXT_BYTES: usize = 256;
pub const PATCHBAY_BACK_KINDS: [&str; 3] = [
    PATCHBAY_GEAR_FACE_KIND,
    PATCHBAY_PORT_KIND,
    PATCHBAY_CORD_KIND,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackInspection {
    Hidden,
    Explicit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GearFacePresentation {
    pub subject_identity: String,
    pub accessibility_name: String,
    pub kind_id: KindId,
    pub port_subjects: Vec<String>,
    /// Existing authoritative descriptors from the checked Kind contract.
    /// Their variants are value intents, never widget types.
    pub controls: Vec<FaceControl>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortPresentation {
    pub subject_identity: String,
    pub accessibility_name: String,
    pub direction: PortDirection,
    pub value_kind: KindId,
    pub temporal: PortTemporal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CordLineAnnotation {
    pub line_id: LineId,
    pub plan_id: PlanId,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CordPresentation {
    pub subject_identity: String,
    pub accessibility_name: String,
    pub source_port_subject: String,
    pub sink_port_subject: String,
    pub value_kind: KindId,
    pub temporal: PortTemporal,
    /// Optional active-lens annotation. It never replaces Cord identity.
    pub line: Option<CordLineAnnotation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchbaySubjectPresentation {
    GearFace(GearFacePresentation),
    Port(PortPresentation),
    Cord(CordPresentation),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchbayRealization {
    pub subject: PatchbaySubjectPresentation,
    /// Back details stay hidden on the default canvas. Exact expansion and
    /// implementation facts live only in the canonical expanded Form and Plan.
    pub back_inspected: bool,
    pub graphics: Option<GraphicsScene>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchbayBackError {
    Graphics(GraphicsError),
    Composition(conduit_presentation::CompositionError),
    EmptySubject,
    TextTooLong,
}

impl From<GraphicsError> for PatchbayBackError {
    fn from(value: GraphicsError) -> Self {
        Self::Graphics(value)
    }
}
impl From<conduit_presentation::CompositionError> for PatchbayBackError {
    fn from(value: conduit_presentation::CompositionError) -> Self {
        Self::Composition(value)
    }
}

pub fn gear_face_presentation(
    gear: &PatchbayGear,
) -> Result<GearFacePresentation, PatchbayBackError> {
    if gear.identity.is_empty() {
        return Err(PatchbayBackError::EmptySubject);
    }
    let accessibility_name = format!("{} Gear, {}", gear.gear_id.as_str(), gear.kind_id.as_str());
    validate_text(&accessibility_name)?;
    Ok(GearFacePresentation {
        subject_identity: gear.identity.clone(),
        accessibility_name,
        kind_id: gear.kind_id.clone(),
        port_subjects: gear
            .inputs
            .iter()
            .chain(&gear.outputs)
            .map(|port| port.identity.clone())
            .collect(),
        controls: gear.controls.clone(),
    })
}

pub fn port_presentation(port: &PatchbayPort) -> Result<PortPresentation, PatchbayBackError> {
    if port.identity.is_empty() {
        return Err(PatchbayBackError::EmptySubject);
    }
    let accessibility_name = format!(
        "{} {:?} Port carrying {}",
        port.descriptor.port_id.as_str(),
        port.descriptor.direction,
        port.descriptor.value_kind.as_str()
    );
    validate_text(&accessibility_name)?;
    Ok(PortPresentation {
        subject_identity: port.identity.clone(),
        accessibility_name,
        direction: port.descriptor.direction,
        value_kind: port.descriptor.value_kind.clone(),
        temporal: port.descriptor.temporal,
    })
}

pub fn cord_presentation(
    cord: &PatchbayCord,
    line: Option<CordLineAnnotation>,
) -> Result<CordPresentation, PatchbayBackError> {
    if cord.identity.is_empty() {
        return Err(PatchbayBackError::EmptySubject);
    }
    if line.as_ref().is_some_and(|annotation| {
        annotation.label.is_empty() || annotation.label.len() > MAX_PATCHBAY_PRESENTATION_TEXT_BYTES
    }) {
        return Err(PatchbayBackError::TextTooLong);
    }
    let accessibility_name = format!(
        "Cord from {} to {} carrying {}",
        cord.source_port,
        cord.sink_port,
        cord.value_kind.as_str()
    );
    validate_text(&accessibility_name)?;
    Ok(CordPresentation {
        subject_identity: cord.identity.clone(),
        accessibility_name,
        source_port_subject: cord.source_port.clone(),
        sink_port_subject: cord.sink_port.clone(),
        value_kind: cord.value_kind.clone(),
        temporal: cord.temporal,
        line,
    })
}

pub fn realize_direct(
    subject: PatchbaySubjectPresentation,
    _inspection: BackInspection,
) -> PatchbayRealization {
    PatchbayRealization {
        subject,
        back_inspected: false,
        graphics: None,
    }
}

pub fn realize_recursive(
    subject: PatchbaySubjectPresentation,
    inspection: BackInspection,
) -> Result<PatchbayRealization, PatchbayBackError> {
    let graphics = match &subject {
        PatchbaySubjectPresentation::GearFace(face) => gear_graphics(face)?,
        PatchbaySubjectPresentation::Port(port) => {
            label_graphics(&port.accessibility_name, GraphicsPaintRole::Foreground)?
        }
        PatchbaySubjectPresentation::Cord(cord) => {
            label_graphics(&cord.accessibility_name, GraphicsPaintRole::Status)?
        }
    };
    Ok(PatchbayRealization {
        subject,
        back_inspected: inspection == BackInspection::Explicit,
        graphics: Some(graphics),
    })
}

pub fn normalized_subject(realization: &PatchbayRealization) -> (&str, &str) {
    match &realization.subject {
        PatchbaySubjectPresentation::GearFace(value) => {
            (&value.subject_identity, &value.accessibility_name)
        }
        PatchbaySubjectPresentation::Port(value) => {
            (&value.subject_identity, &value.accessibility_name)
        }
        PatchbaySubjectPresentation::Cord(value) => {
            (&value.subject_identity, &value.accessibility_name)
        }
    }
}

fn gear_graphics(face: &GearFacePresentation) -> Result<GraphicsScene, PatchbayBackError> {
    let icon = conduit_std_catalog::palette_metadata(&face.kind_id)
        .map(|metadata| metadata.icon)
        .unwrap_or(PresentationIconKey::GenericGear);
    let composition = PresentationComposition::icon(icon.as_str(), &face.accessibility_name)?
        .frame("gear-face", &face.accessibility_name)?
        .badge("ready", "Gear ready")?;
    let mut scene = crate::constrained_graphics_scene(&composition, 160, 96)?;
    scene.push(GraphicsCommand::text(
        LayoutRect {
            x: 8,
            y: 60,
            width: 144,
            height: 20,
        },
        LayoutRect {
            x: 4,
            y: 4,
            width: 152,
            height: 88,
        },
        GraphicsPaintRole::Foreground,
        face.kind_id.as_str(),
    )?)?;
    Ok(scene)
}

fn label_graphics(label: &str, paint: GraphicsPaintRole) -> Result<GraphicsScene, GraphicsError> {
    let bounds = LayoutRect {
        x: 4,
        y: 4,
        width: 152,
        height: 24,
    };
    let mut scene = GraphicsScene::empty();
    scene.push(GraphicsCommand::rect(
        bounds,
        bounds,
        GraphicsPaintRole::Background,
        GraphicsShapeStyle::Stroke,
    )?)?;
    scene.push(GraphicsCommand::text(
        bounds,
        bounds,
        paint,
        truncate_label(label),
    )?)?;
    Ok(scene)
}

fn truncate_label(label: &str) -> &str {
    if label.len() <= conduit_presentation::MAX_GRAPHICS_TEXT_BYTES {
        return label;
    }
    let mut end = conduit_presentation::MAX_GRAPHICS_TEXT_BYTES;
    while !label.is_char_boundary(end) {
        end -= 1;
    }
    &label[..end]
}

fn validate_text(value: &str) -> Result<(), PatchbayBackError> {
    if value.is_empty() {
        Err(PatchbayBackError::EmptySubject)
    } else if value.len() > MAX_PATCHBAY_PRESENTATION_TEXT_BYTES {
        Err(PatchbayBackError::TextTooLong)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FaceControlKind;
    use conduit_core::{
        kind_id, port_id, ConfigurationValue, GearId, PortDescriptor, PortTemporal,
    };

    fn port(direction: PortDirection) -> PatchbayPort {
        PatchbayPort {
            identity: format!("port/gear/demo/{direction:?}/value"),
            gear_id: GearId::from("demo"),
            descriptor: PortDescriptor {
                port_id: port_id("value"),
                value_kind: kind_id("value/text@1"),
                direction,
                temporal: PortTemporal::Value,
            },
        }
    }

    #[test]
    fn direct_and_recursive_preserve_existing_subjects_and_hide_default_back() {
        let input = port(PortDirection::Input);
        let output = port(PortDirection::Output);
        let gear = PatchbayGear {
            identity: "gear/demo".into(),
            gear_id: GearId::from("demo"),
            kind_id: kind_id("presentation/text"),
            kind_contract_revision: conduit_core::KindContractRevision::from(
                "conduit.std/presentation-text@1",
            ),
            source_form: "demo".into(),
            form_path: vec!["demo".into()],
            inputs: vec![input.clone()],
            outputs: vec![output.clone()],
            controls: vec![FaceControl {
                key: "enabled".into(),
                value: ConfigurationValue::Bool(true),
                kind: FaceControlKind::BooleanChoice {
                    choices: ["false", "true"],
                },
                interaction: None,
            }],
        };
        let subject = PatchbaySubjectPresentation::GearFace(gear_face_presentation(&gear).unwrap());
        let direct = realize_direct(subject.clone(), BackInspection::Hidden);
        let recursive = realize_recursive(subject, BackInspection::Explicit).unwrap();
        assert_eq!(normalized_subject(&direct), normalized_subject(&recursive));
        assert!(!direct.back_inspected);
        assert!(recursive.back_inspected);
        assert!(recursive.graphics.unwrap().commands().len() >= 4);
        let PatchbaySubjectPresentation::GearFace(face) = &direct.subject else {
            panic!("Gear Face subject");
        };
        assert_eq!(face.controls[0].key, "enabled");
        assert!(matches!(
            face.controls[0].kind,
            FaceControlKind::BooleanChoice { .. }
        ));
    }

    #[test]
    fn ports_and_cords_keep_identity_and_line_as_annotation_only() {
        let source = port(PortDirection::Output);
        let sink = port(PortDirection::Input);
        let port_view = port_presentation(&source).unwrap();
        assert_eq!(port_view.subject_identity, source.identity);
        assert!(port_view.accessibility_name.contains("Output Port"));
        let cord = PatchbayCord {
            identity: "cord/0".into(),
            source_port: source.identity,
            sink_port: sink.identity,
            value_kind: kind_id("value/text@1"),
            temporal: PortTemporal::Value,
        };
        let view = cord_presentation(
            &cord,
            Some(CordLineAnnotation {
                line_id: LineId::from("line/one"),
                plan_id: PlanId::from("plan/one"),
                label: "active Line".into(),
            }),
        )
        .unwrap();
        assert_eq!(view.subject_identity, "cord/0");
        assert_eq!(view.line.as_ref().unwrap().line_id.as_str(), "line/one");
        assert_ne!(view.subject_identity, view.line.unwrap().line_id.as_str());

        for subject in [
            PatchbaySubjectPresentation::Port(port_view),
            PatchbaySubjectPresentation::Cord(cord_presentation(&cord, None).unwrap()),
        ] {
            let direct = realize_direct(subject.clone(), BackInspection::Explicit);
            let recursive = realize_recursive(subject, BackInspection::Explicit).unwrap();
            assert_eq!(normalized_subject(&direct), normalized_subject(&recursive));
            assert!(!direct.back_inspected);
            assert!(recursive.back_inspected);
        }
        assert_eq!(PATCHBAY_BACK_KINDS.len(), 3);
    }
}
