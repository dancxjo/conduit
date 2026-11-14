//! Canonical retained Part return over the already selected bounded USB carrier.
use super::*;
impl MembershipState {
    pub(super) async fn serve_return(
        &mut self,
        link: &mut Link,
        input: &mut [u8; LIMIT],
    ) -> Result<(), ()> {
        self.refresh_native_offer()?;
        let retained = self
            .receipt
            .as_ref()
            .or_else(|| self.seed.admission.credential())
            .ok_or(())?;
        // Readiness alone does not authorize the advertised operation. Root
        // independently selects explicit Seed policy while holding the actual
        // initialized Base; the fresh return will attribute its original grant.
        if self.native_offer.is_some()
            && crate::config::PREPARATION_DELEGATION.is_some()
            && crate::config::EXECUTION_DELEGATION.is_some()
        {
            let available = crate::config::HEAP_BYTES
                .checked_sub(crate::heap::usage().0)
                .ok_or(())?;
            let (native, _) = crate::i2c_installation::prepare(self.resources.host(), available)
                .map_err(|_| ())?;
            let available = crate::config::HEAP_BYTES
                .checked_sub(crate::heap::usage().0)
                .ok_or(())?;
            self.native_issuer
                .authorize(
                    conduit_body::NativePreparationContext {
                        bootstrap: &self.seed,
                        member: retained,
                        actual: self.resources.host(),
                        image_id: crate::config::IMAGE_ID,
                        configuration_id: crate::config::CONFIGURATION_ID,
                        now_millis: embassy_time::Instant::now().as_millis(),
                    },
                    &native,
                    available,
                )
                .map_err(|_| ())?;
            drop(native);
        } else {
            self.native_issuer.withdraw_authorization();
        }
        let owner = crate::resident::inventory().map_err(|_| ())?;
        let images = owner.offers();
        let images = images.into_iter().flatten().chain(self.native_offer);
        let mut output = [0; LIMIT];
        let accepted = conduit_mcu_host_runtime::member_return::RetainedReturn {
            bootstrap: &self.seed,
            actual: self.resources.host(),
            image_id: crate::config::IMAGE_ID,
            key: &self.key,
            retained,
        }
        .exchange(
            link,
            images,
            || {
                self.native_issuer
                    .copy_grants_for_return(
                        crate::config::HEAP_BYTES
                            .checked_sub(crate::heap::usage().0)
                            .ok_or(conduit_mcu_host_runtime::member_return::Refusal::Bounds)?,
                    )
                    .map_err(|_| conduit_mcu_host_runtime::member_return::Refusal::Bounds)
            },
            input,
            &mut output,
        )
        .await
        .map_err(|_| ())?;
        self.receipt = Some(accepted.credential);
        critical_section::with(|cs| {
            RECEIPT_DIGEST.borrow(cs).set(Some(accepted.receipt_digest));
        });
        Ok(())
    }
}

impl MembershipState {
    fn refresh_native_offer(&mut self) -> Result<(), ()> {
        if !self.resources.admissions().is_empty() || !self.resources.assignments().is_empty() {
            return Err(());
        }
        let Some(available) = crate::i2c_owner::native_available() else {
            return Ok(());
        };
        let available = available && crate::config::I2C_OFFER_IMAGE.is_some();
        if available == self.native_offer.is_some() {
            return Ok(());
        }
        let used = crate::heap::usage().0;
        let remaining = crate::config::HEAP_BYTES.checked_sub(used).ok_or(())?;
        let context =
            crate::native_publication::prepare(self.resources.host(), available, remaining)
                .map_err(|_| ())?;
        // The idle fence above forbids dropping any original reservation/lane.
        // Context preparation is fallible and leaves the old owner unchanged.
        self.native_issuer.withdraw_authorization();
        self.resources = conduit_core::ResourceAdmissionOwner::new(context);
        self.native_offer = if available {
            crate::config::I2C_OFFER_IMAGE
        } else {
            None
        };
        Ok(())
    }
}
