//! Protected owner-local compact-image return control, never transport authority.
//! This follows the existing owner's pending challenge and durable completion.
use crate::durable_host_control::{body::HostSource, DurableHostRuntime};
use conduit_body::{
    MemberReturnImagePresentation, MemberReturnProofDocument, PartReturnChallenge,
    SignedPortableAdmissionReceipt,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum Command {
    Begin {
        presentation: Box<MemberReturnImagePresentation>,
        images: Vec<Vec<u8>>,
    },
    Complete {
        proof: Box<MemberReturnProofDocument>,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum Reply {
    Challenge {
        challenge: Box<PartReturnChallenge>,
    },
    Committed {
        receipt: Box<SignedPortableAdmissionReceipt>,
    },
}
impl DurableHostRuntime {
    pub(in crate::durable_host_control) fn owned_body_image_member_return(
        &mut self,
        command: Command,
    ) -> Result<Reply, String> {
        let HostSource::Body { owner, root, .. } = &mut self.host else {
            return Err("installed Host does not own a live Body session".into());
        };
        match command {
            Command::Begin {
                presentation,
                images,
            } => owner
                .begin_image_member_return(*presentation, &images)
                .map(|challenge| Reply::Challenge {
                    challenge: Box::new(challenge),
                }),
            Command::Complete { proof } => owner
                .complete_image_member_return(
                    root,
                    &proof.decode().map_err(|_| "invalid return proof")?,
                )
                .map(|receipt| Reply::Committed {
                    receipt: Box::new(receipt),
                }),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::durable_host_control::{handle, Request, Response, PROTOCOL};
    fn request(token: Vec<u8>) -> Request {
        let proof = serde_json::from_value(serde_json::json!({
            "admission_id":"return/test", "body_id":"body/test", "part_id":"part/test",
            "host_id":"host/test", "boot_id":"boot/test", "nonce":vec![1;32], "signature":vec![7;64]
        }))
        .unwrap();
        Request::BodyMemberImageReturn {
            protocol: PROTOCOL,
            token,
            command: Box::new(Command::Complete {
                proof: Box::new(proof),
            }),
        }
    }
    #[test]
    fn image_return_requires_the_existing_control_token_and_owner_custody() {
        let host = conduit_std_host::StdHost::new();
        let mut runtime = DurableHostRuntime::new("fixture".into(), "fixture".into(), host);
        for (token, expected) in [
            (vec![0; 32], "unauthorized"),
            (
                vec![7; 32],
                "installed Host does not own a live Body session",
            ),
        ] {
            let encoded = serde_json::to_vec(&request(token)).unwrap();
            let request = serde_json::from_slice(&encoded).unwrap();
            assert!(matches!(handle(request, &[7;32], &mut runtime),
                Response::Refused { code, .. } if code == expected));
        }
    }
}

// Explicit transaction fixture only: no installed workspace constructor exists.
#[cfg(test)]
fn fixture_decode_borrowed(
    command: crate::durable_host_control::image_return_ingress::BorrowedCommand<'_>,
) -> Result<Command, serde_json::Error> {
    use crate::durable_host_control::image_return_ingress::BorrowedCommand;
    match command {
        BorrowedCommand::Begin {
            presentation,
            images,
            ..
        } => Ok(Command::Begin {
            presentation: Box::new(serde_json::from_str(presentation.get())?),
            images: serde_json::from_str(images.get())?,
        }),
        BorrowedCommand::Complete { proof } => Ok(Command::Complete {
            proof: Box::new(
                crate::durable_host_control::image_return_ingress::proof::inspect(proof.get())?
                    .decode()?,
            ),
        }),
    }
}
#[cfg(test)]
#[path = "image_member_return/ingress_tests.rs"]
mod ingress_tests;
