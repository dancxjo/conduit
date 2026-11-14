//! Membership-only view of the original Root. A returned credential is not
//! execution authority and this view cannot inspect or mutate command custody,
//! admissions, assigned resources, native tokens, kernels, or Start state.
use super::*;
use core::borrow::Borrow;

pub struct MemberControl<'a, 's> {
    bootstrap: &'a Seed<'s>,
    actual: &'a HostAdvertisement,
    key: &'a SigningKey,
    image_id: &'static str,
    receipt: &'a mut Option<MembershipCredential>,
    receipt_digest: &'a mut Option<[u8; 32]>,
}
impl<'s, const N: usize> Root<'s, N> {
    /// Split only membership fields. This makes no claim about an existing Play
    /// and supplies no authority to prepare, admit, start, or replace one.
    pub fn member_control(&mut self) -> MemberControl<'_, 's> {
        MemberControl {
            bootstrap: &self.seed,
            actual: self.resources.host(),
            key: &self.key,
            image_id: self.config.image_id,
            receipt: &mut self.receipt,
            receipt_digest: &mut self.receipt_digest,
        }
    }
    /// Lend the original command backing only when it holds no retained command.
    /// An online Play's retained command is never made writable. A caller must
    /// separately preserve every ongoing execution owner and fund its output.
    pub fn member_control_with_workspace(
        &mut self,
    ) -> Result<(MemberControl<'_, 's>, &mut [u8]), Refusal> {
        let workspace = self.input.writable().map_err(|_| Refusal::RetainedInput)?;
        Ok((
            MemberControl {
                bootstrap: &self.seed,
                actual: self.resources.host(),
                key: &self.key,
                image_id: self.config.image_id,
                receipt: &mut self.receipt,
                receipt_digest: &mut self.receipt_digest,
            },
            workspace,
        ))
    }
}
impl MemberControl<'_, '_> {
    pub fn actual(&self) -> &HostAdvertisement {
        self.actual
    }
    pub fn receipt_digest(&self) -> Option<[u8; 32]> {
        *self.receipt_digest
    }
    /// Uses the ordinary retained-Part signature, challenge and owner attestation.
    /// Only a fully verified acknowledgement changes the retained receipt fields.
    pub async fn return_member<'i, T, I, G>(
        &mut self,
        link: &mut T,
        images: I,
        grants: &[G],
        input: &mut [u8],
        output: &mut [u8],
    ) -> Result<(), Refusal>
    where
        T: DocumentTransport,
        I: Iterator<Item = &'i [u8]> + Clone,
        G: Borrow<AuthorityGrant>,
    {
        let retained = self
            .receipt
            .as_ref()
            .or_else(|| self.bootstrap.admission.credential())
            .ok_or(Refusal::NotMember)?;
        let accepted = RetainedReturn {
            bootstrap: self.bootstrap,
            actual: self.actual,
            image_id: self.image_id,
            key: self.key,
            retained,
        }
        .exchange_with_grants(link, images, grants, input, output)
        .await
        .map_err(Refusal::MemberReturn)?;
        *self.receipt = Some(accepted.credential);
        *self.receipt_digest = Some(accepted.receipt_digest);
        Ok(())
    }
}
