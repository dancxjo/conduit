// Membership-only return beside existing admitted kernels. No provider is
// reauthorized, no command bytes are replaced, and no Play is started here.
use super::*;
use conduit_mcu_host_runtime::{root::MemberControl, transport::MessageTransport};

pub(crate) struct ReturnControl<'a, 's> {
    member: MemberControl<'a, 's>,
    workspace: &'a mut [u8],
    grants: heapless::Vec<&'a c::AuthorityGrant, 32>,
    resident_images: [Option<&'static [u8]>; crate::resident::generated::ENTRY_COUNT],
}
impl<'a, 's> ReturnControl<'a, 's> {
    pub(crate) fn prepare(
        root: &'a mut conduit_mcu_host_runtime::root::Root<
            's,
            { crate::config::MAX_ASSIGNMENT_BYTES },
        >,
        issuers: &'a conduit_mcu_host_runtime::native_authority::Issuers<WAIT_SLOTS>,
        resident_images: [Option<&'static [u8]>; crate::resident::generated::ENTRY_COUNT],
    ) -> Option<Self> {
        let (member, workspace) = root.member_control_with_workspace().ok()?;
        let mut grants = heapless::Vec::new();
        for grant in issuers.grants() {
            grants.push(grant).ok()?;
        }
        Some(Self {
            member,
            workspace,
            grants,
            resident_images,
        })
    }
    async fn return_member(
        &mut self,
        link: &mut crate::transport::Link,
        output: &mut [u8],
    ) -> Result<(), ()> {
        // Refuse before temporary decoded/cryptographic work. This conservative
        // cold-phase ceiling does not enlarge the installed heap or a grant.
        // Actual whole-MCU peak remains a required deployment qualification.
        if crate::runtime::remaining() < 16 * 1024 {
            return Err(());
        }
        let native = [
            crate::workset::native_images::PULSE_IMAGE,
            crate::workset::native_images::DISPLAY_IMAGE,
            crate::workset::native_images::WAIT_IMAGE,
            crate::workset::native_images::RENDERER_IMAGE,
        ];
        let images = self
            .resident_images
            .into_iter()
            .flatten()
            .chain(native)
            .chain(crate::workset::native_images::APPEND_IMAGE)
            .chain(
                crate::workset::generated::sampling::OFFER_IMAGES
                    .iter()
                    .copied(),
            )
            .chain(
                crate::workset::generated::mask::OFFER_IMAGES
                    .iter()
                    .copied(),
            )
            .chain(
                crate::workset::generated::mask::MASK_ROUTING_IMAGES
                    .iter()
                    .copied(),
            )
            .chain(
                crate::workset::generated::output::OFFER_IMAGES
                    .iter()
                    .copied(),
            );
        self.member
            .return_member(link, images, self.grants.as_slice(), self.workspace, output)
            .await
            .map_err(|_| ())?;
        crate::runtime::observe_receipt(self.member.receipt_digest());
        Ok(())
    }
    fn clear_scratch(&mut self) {
        self.workspace.fill(0);
    }
}

pub(super) async fn serve(
    state: &State<3>,
    face: &conduit_environment_runtime::publication_face_custody::LocalPublicationFace,
    link: &mut crate::transport::Link,
    response: &mut [u8],
    control: &mut Option<ReturnControl<'_, '_>>,
    activity: &conduit_mcu_host_runtime::native_runner::NativeActivity,
) -> Result<(), ()> {
    let mut request = [0; conduit_mcu_host_runtime::execution_inspection::CONTROL_BYTES];
    loop {
        let bytes = link.receive(&mut request).await.map_err(|_| ())?;
        if bytes == conduit_body::PICO_MEMBER_RETURN_IMAGES_REQUEST {
            let Some(control) = control.as_mut() else {
                return Err(());
            };
            let result = embassy_time::with_timeout(
                embassy_time::Duration::from_secs(10),
                activity.with_blocking_polls(control.return_member(link, response), &Yield),
            )
            .await;
            control.clear_scratch();
            response.fill(0);
            result.map_err(|_| ())??;
        } else {
            let count = match inspection_encoder::encode(face, bytes, response)? {
                Some(count) if count <= response.len() => count,
                Some(_) => return Err(()),
                None => state.encode(bytes, response).map_err(|_| ())?,
            };
            link.send(&response[..count]).await.map_err(|_| ())?;
        }
    }
}
