#![no_std]
//! Portable face of one finite Body invitation.

extern crate alloc;

#[allow(dead_code)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/semantic_types.rs"));
}
pub use generated::InvitationFaceRefusal;

use alloc::{format, vec, vec::Vec};
use conduit_face::{
    ActionAvailability, ApplicationEventKind, FaceMechanism, SemanticAction,
    SemanticApplicationView, SemanticFaceNode, StatusKind,
};

pub const MAX_INVITATION_TRANSFER_BYTES: usize = 8_192;

pub struct InvitationFace<'a> {
    pub invitation_id: &'a str,
    pub body_id: &'a str,
    pub body_name: &'a str,
    pub expires_at_millis: u64,
    pub transfer_uri: &'a str,
    pub clipboard_available: bool,
    pub share_available: bool,
}

impl InvitationFace<'_> {
    pub fn view(&self, revision: u32) -> Result<SemanticApplicationView, InvitationFaceRefusal> {
        if self.invitation_id.is_empty()
            || self.body_id.is_empty()
            || self.body_name.is_empty()
            || self.transfer_uri.is_empty()
            || self.transfer_uri.len() > MAX_INVITATION_TRANSFER_BYTES
            || !self.transfer_uri.is_ascii()
            || !self.transfer_uri.contains('#')
        {
            return Err(InvitationFaceRefusal::InvalidInvitation);
        }
        let mut children = vec![node(
            "invitation-status",
            FaceMechanism::Status {
                kind: StatusKind::Ordinary,
                title: "Single-use Body invitation".into(),
                detail: format!(
                    "Expires at {0}. It grants no membership or effect authority by itself.",
                    self.expires_at_millis
                ),
            },
            vec![],
        )];
        children.push(node(
            "invitation-link",
            FaceMechanism::CodeBlock {
                language: "uri-fragment".into(),
                code: self.transfer_uri.into(),
            },
            vec![],
        ));
        children.push(node(
            "invitation-actions",
            FaceMechanism::ActionGroup {
                label: "Transfer this invitation".into(),
            },
            vec![
                action("show-qr", "invitation.show-qr", "Show QR", true),
                action(
                    "copy-link",
                    "invitation.copy-link",
                    "Copy link",
                    self.clipboard_available,
                ),
                action("share", "invitation.share", "Share…", self.share_available),
                action("dismiss", "invitation.dismiss", "Back to members", true),
            ],
        ));
        let view = SemanticApplicationView {
            revision,
            root: node(
                "invitation-face",
                FaceMechanism::Panel {
                    title: self.body_name.into(),
                },
                children,
            ),
        };
        view.lower().map_err(|_| InvitationFaceRefusal::Face)?;
        Ok(view)
    }
}

fn action(key: &str, identity: &str, label: &str, available: bool) -> SemanticFaceNode {
    node(
        key,
        FaceMechanism::Action(SemanticAction {
            identity: identity.into(),
            event: ApplicationEventKind::Activate,
            label: label.into(),
            availability: if available {
                ActionAvailability::Available
            } else {
                ActionAvailability::Unavailable {
                    detail: "This host does not currently offer that transfer mechanism.".into(),
                }
            },
        }),
        vec![],
    )
}

fn node(key: &str, mechanism: FaceMechanism, children: Vec<SemanticFaceNode>) -> SemanticFaceNode {
    SemanticFaceNode {
        key: key.into(),
        mechanism,
        children,
    }
}
