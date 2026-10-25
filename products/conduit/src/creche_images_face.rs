//! Fourth canonical Crèche stage over sealed, verified image production.
//! Save requests an owner-mediated destination grant; it is not filesystem
//! authority. This projection offers no carrier deployment or boot action.
use crate::creche_production::CrecheProducedImages;
use conduit_body_make::PreparedBirthArtifactReceipt;
use conduit_core::kind_id;
use conduit_face::*;
use sha2::{Digest, Sha256};

pub const MAX_IMAGE_HOSTS: usize = 64;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImagesFaceRefusal {
    InvalidBasis,
    InvalidProducts,
    Bounds,
    Face,
    StaleInteraction,
    UnknownAction,
}

pub enum ImagesRequest<'a> {
    Save {
        host_entry: &'a str,
        receipt: &'a PreparedBirthArtifactReceipt,
    },
    Done,
}

/// Borrowing keeps product ownership in the admitted make/save owner. The owner
/// supplies its current monotonic revision and actually acknowledged Mask Show.
pub struct CrecheImagesFace<'a> {
    products: &'a CrecheProducedImages,
    face: Face,
    save_actions: Vec<String>,
    done_action: String,
}
impl<'a> CrecheImagesFace<'a> {
    pub fn new(
        products: &'a CrecheProducedImages,
        basis: FaceBasis,
        revision: u64,
        maximum_face_bytes: usize,
    ) -> Result<Self, ImagesFaceRefusal> {
        use ImagesFaceRefusal as E;
        validate_basis(products, &basis)?;
        if products.artifacts().is_empty()
            || products.artifacts().len() > MAX_IMAGE_HOSTS
            || maximum_face_bytes == 0
            || maximum_face_bytes > MAX_FACE_TOTAL_BYTES
        {
            return Err(E::Bounds);
        }
        let request = format!("{:x}", Sha256::digest(products.request().0));
        let root = format!("creche/images/{request}");
        let article = format!("{root}/article");
        let navigation = format!("{root}/navigation");
        let mut subjects = vec![
            subject(&root, FaceRole::Plot, "Crèche"),
            subject(
                &article,
                FaceRole::Semantic(kind_id("document/article")),
                "Your Body's images",
            ),
            subject(
                &navigation,
                FaceRole::Semantic(kind_id("document/navigation")),
                "Image actions",
            ),
        ];
        let mut relationships = vec![contains(&root, &article), contains(&root, &navigation)];
        let mut properties = vec![
            property(&root, "stage", FacePropertyValue::Count(4)),
            property(
                &root,
                "stage-name",
                FacePropertyValue::Text("Images".into()),
            ),
            property(
                &article,
                "new-body-id",
                FacePropertyValue::Identity(products.biography().body_id.as_str().into()),
            ),
            property(
                &article,
                "host-count",
                FacePropertyValue::Count(products.artifacts().len() as u64),
            ),
        ];
        let mut text = vec![FaceText { subject: article.clone(), text: format!("{} is ready for its first home. Save each exact prepared image, then boot its selected Host. Boot and membership have not been observed.", products.biography().friendly_name) }];
        let mut actions = Vec::with_capacity(products.artifacts().len() + 1);
        let mut save_actions = Vec::with_capacity(products.artifacts().len());
        let mut hosts = std::collections::BTreeSet::new();
        for artifact in products.artifacts() {
            let receipt = artifact.receipt();
            if receipt.body_id != products.biography().body_id.as_str()
                || receipt.artifact_bytes == 0
                || receipt.boot_observed
                || receipt.join_observed
                || !hosts.insert(receipt.host_entry.as_str())
            {
                return Err(E::InvalidProducts);
            }
            let encoded = serde_json::to_vec(receipt).map_err(|_| E::InvalidProducts)?;
            let exact = format!("{:x}", Sha256::digest(encoded));
            let host = format!("{root}/host/{exact}");
            let save = format!("creche.images.save.{exact}");
            subjects.push(subject(&host, FaceRole::Info, &receipt.host_entry));
            relationships.push(contains(&article, &host));
            for (name, value) in [
                (
                    "host-entry",
                    FacePropertyValue::Text(receipt.host_entry.clone()),
                ),
                ("target", FacePropertyValue::Text(receipt.target.clone())),
                (
                    "profile-id",
                    FacePropertyValue::Identity(receipt.profile_id.clone()),
                ),
                (
                    "configuration-id",
                    FacePropertyValue::Identity(receipt.configuration_id.clone()),
                ),
                (
                    "artifact-sha256",
                    FacePropertyValue::Text(receipt.artifact_sha256.clone()),
                ),
                (
                    "artifact-bytes",
                    FacePropertyValue::Count(receipt.artifact_bytes),
                ),
                ("artifact-prepared", FacePropertyValue::Flag(true)),
                ("boot-observed", FacePropertyValue::Flag(false)),
                ("join-observed", FacePropertyValue::Flag(false)),
            ] {
                properties.push(property(&host, name, value));
            }
            text.push(FaceText {
                subject: host.clone(),
                text: format!(
                    "{} · {} · {} bytes. {}",
                    receipt.host_entry,
                    receipt.target,
                    receipt.artifact_bytes,
                    receipt.artifact_sha256
                ),
            });
            actions.push(action(
                &save,
                &host,
                &format!("Save image for {}…", receipt.host_entry),
            ));
            save_actions.push(save);
        }
        let done_action = format!("creche.images.done.{request}");
        actions.push(action(&done_action, &navigation, "Done"));
        let face = Face::new_with_semantics(
            revision,
            basis,
            subjects,
            relationships,
            properties,
            text,
            actions,
            vec![FaceDisclosure {
                subject: root,
                level: FaceDisclosureLevel::Primary,
            }],
        )
        .map_err(|_| E::Face)?;
        if serde_json::to_vec(&face).map_err(|_| E::Face)?.len() > maximum_face_bytes {
            return Err(E::Bounds);
        }
        Ok(Self {
            products,
            face,
            save_actions,
            done_action,
        })
    }
    pub fn face(&self) -> &Face {
        &self.face
    }

    /// The owning installed adapter must also recheck actual selected renderer
    /// custody. A cloned encoded Available Show alone is not acknowledgement.
    pub fn request(
        &self,
        show: &MaskShow,
        interaction: &FaceInteraction,
    ) -> Result<ImagesRequest<'a>, ImagesFaceRefusal> {
        interaction
            .validate_against(&self.face, show)
            .map_err(|_| ImagesFaceRefusal::StaleInteraction)?;
        if interaction.action_id == self.done_action {
            return Ok(ImagesRequest::Done);
        }
        let index = self
            .save_actions
            .iter()
            .position(|action| action == &interaction.action_id)
            .ok_or(ImagesFaceRefusal::UnknownAction)?;
        let receipt = self.products.artifacts()[index].receipt();
        Ok(ImagesRequest::Save {
            host_entry: &receipt.host_entry,
            receipt,
        })
    }
}
fn validate_basis(
    products: &CrecheProducedImages,
    basis: &FaceBasis,
) -> Result<(), ImagesFaceRefusal> {
    let context = products.context();
    let partition = context
        .plan()
        .plots
        .iter()
        .find(|plot| &plot.plot == context.plot())
        .ok_or(ImagesFaceRefusal::InvalidBasis)?;
    if basis.body_id.as_ref() != Some(&context.plan().body_id)
        || basis.wake_id.as_ref() != Some(&context.plan().wake_id)
        || basis.source_document_id.as_ref() != Some(&partition.plan.source_document_id)
        || basis.checked_plot_id.as_ref() != Some(&partition.plan.checked_plot_id)
        || basis.expanded_plot_id.as_ref() != Some(&partition.plan.expanded_plot_id)
        || basis.plan_id.as_ref() != Some(&partition.plan.plan_id)
        || basis.active_play_id.as_ref() != Some(&context.play().active_play_id)
    {
        return Err(ImagesFaceRefusal::InvalidBasis);
    }
    Ok(())
}
fn subject(identity: &str, role: FaceRole, name: &str) -> FaceSubject {
    FaceSubject {
        identity: identity.into(),
        role,
        name: name.into(),
    }
}
fn property(subject: &str, name: &str, value: FacePropertyValue) -> FaceProperty {
    FaceProperty {
        subject: subject.into(),
        name: name.into(),
        value,
    }
}
fn contains(source: &str, target: &str) -> FaceRelationship {
    FaceRelationship {
        source: source.into(),
        target: target.into(),
        kind: FaceRelationshipKind::Contains,
    }
}
fn action(identity: &str, target: &str, name: &str) -> FaceAction {
    FaceAction {
        identity: identity.into(),
        intent: "conduit.intent/creche-image-request@1".into(),
        target: target.into(),
        name: name.into(),
        arguments: vec![],
        disclosure: FaceDisclosureLevel::Primary,
        availability: FaceActionAvailability::Available,
    }
}
