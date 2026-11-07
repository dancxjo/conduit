//! Original Root authorization precedes planning; opaque issuance follows it.
use super::*;
use alloc::string::String;
use alloc::vec::Vec;
use conduit_i2c_transaction::installation::I2cNativeIdentity;
use conduit_kernel::scheduler::KernelPreparationError;

pub(super) struct OriginalAuthority {
    pub(super) grant: c::AuthorityGrant,
    native: [u8; 32],
}
impl OriginalAuthority {
    pub(super) fn matches<P: I2cProvider>(
        &self,
        context: NativePreparationContext<'_>,
        native: &ReadyI2cBase<P>,
    ) -> bool {
        selected(context, native) && self.native == fingerprint(context, native)
    }
}
impl Issuer {
    /// Borrow only the original locally authorized description. It contains no
    /// bearer key, handle, resource reservation or permission to Start a Play.
    pub fn grant(&self) -> Option<&c::AuthorityGrant> {
        self.authorization.as_ref().map(|owner| &owner.grant)
    }

    /// Root must hold the actual initialized Base and explicitly select both
    /// Seed-owner preparation and execution policy. Membership or a ready offer
    /// alone never creates this authorization. No Plan is needed for this
    /// Host/Boot/capability description; later opaque issuance still requires it.
    pub fn authorize<P: I2cProvider>(
        &mut self,
        context: NativePreparationContext<'_>,
        native: &ReadyI2cBase<P>,
        maximum_bytes: usize,
    ) -> Result<(), E> {
        if !selected(context, native) {
            return Err(E::Scope);
        }
        if let Some(prior) = &self.authorization {
            return if prior.matches(context, native) {
                Ok(())
            } else {
                Err(E::Scope)
            };
        }
        let id = native.native_identity();
        let suffix = match config::I2C_CONTROLLER {
            0 => "/i2c/0/authority",
            1 => "/i2c/1/authority",
            _ => return Err(E::Scope),
        };
        let required = [
            id.boot_id.as_str(),
            suffix,
            I2C_AUTHORITY,
            I2C_CALL,
            I2C_KIND,
            id.host_id.as_str(),
            id.boot_id.as_str(),
            I2C_IMPLEMENTATION,
        ]
        .iter()
        .try_fold(core::mem::size_of::<OriginalAuthority>(), |n, text| {
            n.checked_add(text.len())
        })
        .ok_or(E::Bounds)?;
        if required > maximum_bytes {
            return Err(E::Bounds);
        }
        let mut remaining = maximum_bytes
            .checked_sub(core::mem::size_of::<OriginalAuthority>())
            .ok_or(E::Bounds)?;
        let grant = c::AuthorityGrant {
            grant_id: copy(&[id.boot_id.as_str(), suffix], &mut remaining)?.into(),
            contract_id: copy(&[I2C_AUTHORITY], &mut remaining)?.into(),
            host_call_contract_id: copy(&[I2C_CALL], &mut remaining)?.into(),
            subject_kind: copy(&[I2C_KIND], &mut remaining)?.into(),
            host_id: copy(&[id.host_id.as_str()], &mut remaining)?.into(),
            boot_id: copy(&[id.boot_id.as_str()], &mut remaining)?.into(),
            capability_id: copy(&[I2C_IMPLEMENTATION], &mut remaining)?.into(),
        };
        self.authorization = Some(OriginalAuthority {
            grant,
            native: fingerprint(context, native),
        });
        Ok(())
    }

    /// The Root caller must fence actual reservations before withdrawing this
    /// planning description. This does not cancel or retire an existing table.
    pub fn withdraw_authorization(&mut self) {
        self.authorization = None;
    }

    /// Fallible, byte-bounded descriptive carriage. The original stays here;
    /// neither this copy nor its signature exports opaque capability possession.
    pub fn copy_grants_for_return(
        &self,
        maximum_bytes: usize,
    ) -> Result<Vec<c::AuthorityGrant>, E> {
        let Some(grant) = self.grant() else {
            return Ok(Vec::new());
        };
        let fields = [
            grant.grant_id.as_str(),
            grant.contract_id.as_str(),
            grant.host_call_contract_id.as_str(),
            grant.subject_kind.as_str(),
            grant.host_id.as_str(),
            grant.boot_id.as_str(),
            grant.capability_id.as_str(),
        ];
        let required = fields
            .iter()
            .try_fold(core::mem::size_of::<c::AuthorityGrant>(), |n, text| {
                n.checked_add(text.len())
            })
            .ok_or(E::Bounds)?;
        if required > maximum_bytes {
            return Err(E::Bounds);
        }
        let mut grants = Vec::new();
        grants
            .try_reserve_exact(1)
            .map_err(|_| E::Storage(KernelPreparationError::Allocation))?;
        let mut remaining = maximum_bytes
            .checked_sub(grants.capacity() * core::mem::size_of::<c::AuthorityGrant>())
            .ok_or(E::Bounds)?;
        grants.push(c::AuthorityGrant {
            grant_id: copy(&[fields[0]], &mut remaining)?.into(),
            contract_id: copy(&[fields[1]], &mut remaining)?.into(),
            host_call_contract_id: copy(&[fields[2]], &mut remaining)?.into(),
            subject_kind: copy(&[fields[3]], &mut remaining)?.into(),
            host_id: copy(&[fields[4]], &mut remaining)?.into(),
            boot_id: copy(&[fields[5]], &mut remaining)?.into(),
            capability_id: copy(&[fields[6]], &mut remaining)?.into(),
        });
        Ok(grants)
    }
}
fn selected<P: I2cProvider>(
    context: NativePreparationContext<'_>,
    native: &ReadyI2cBase<P>,
) -> bool {
    let (Some(preparation), Some(execution)) =
        (config::PREPARATION_DELEGATION, config::EXECUTION_DELEGATION)
    else {
        return false;
    };
    let id = native.native_identity();
    let attachment = native.attachment();
    preparation.authorizer == conduit_body::HostPreparationAuthorizer::SeedOwner
        && preparation.has_finite_bounds()
        && execution.authorizer == conduit_body::HostExecutionAuthorizer::SeedOwner
        && execution.maximum_active_worksets == 1
        && config::I2C_CALL_MAXIMUM_OPERATIONS != 0
        && config::I2C_CALL_ADDRESS_RANGE
            == Some((attachment.minimum_address, attachment.maximum_address))
        && context.image_id == config::IMAGE_ID
        && context.configuration_id == config::CONFIGURATION_ID
        && context.actual.protocol_version == c::PROTOCOL_VERSION
        && context.actual.offer_generation.0 != 0
        && context.actual.profile.as_str() == config::CONFIGURATION_ID
        && id.host_id == context.actual.host_id
        && id.boot_id == context.actual.boot_id
        && id.artifact_id.as_str() == config::IMAGE_ID
        && context.member.body_id == context.bootstrap.body_id
        && context.member.host_id == context.actual.host_id
        && context.bootstrap.matches_native(
            id.host_id.as_str(),
            config::IMAGE_ID,
            config::CONFIGURATION_ID,
        )
}
fn fingerprint<P: I2cProvider>(
    context: NativePreparationContext<'_>,
    native: &ReadyI2cBase<P>,
) -> [u8; 32] {
    let id: &I2cNativeIdentity = native.native_identity();
    let mut hash = Sha256::new();
    hash.update(b"conduit.rp2040/original-i2c-authorization@1\0");
    for text in [
        id.host_id.as_str(),
        id.boot_id.as_str(),
        id.base_id.as_str(),
        id.provider_instance_id.as_str(),
        id.resource_pool_id.as_str(),
        id.resource_generation_id.0.as_str(),
        id.envelope_id.as_str(),
        id.artifact_id.as_str(),
        context.bootstrap.body_id.as_str(),
        context.bootstrap.owner.host_id.as_str(),
        context.bootstrap.owner.boot_id.as_str(),
    ] {
        hash.update((text.len() as u64).to_le_bytes());
        hash.update(text.as_bytes());
    }
    hash.update(id.provider_generation.to_le_bytes());
    hash.update(context.actual.offer_generation.0.to_le_bytes());
    hash.update(context.bootstrap.owner.offer_generation.0.to_le_bytes());
    hash.update(context.bootstrap.owner.verifying_key);
    let attachment = native.attachment();
    hash.update(attachment.generation.to_le_bytes());
    hash.update([attachment.minimum_address, attachment.maximum_address]);
    hash.update(attachment.resource_bytes.to_le_bytes());
    hash.finalize().into()
}
fn copy(parts: &[&str], remaining: &mut usize) -> Result<String, E> {
    let bytes = parts
        .iter()
        .try_fold(0usize, |n, s| n.checked_add(s.len()))
        .ok_or(E::Bounds)?;
    if bytes > *remaining {
        return Err(E::Bounds);
    }
    let mut text = String::new();
    text.try_reserve_exact(bytes)
        .map_err(|_| E::Storage(KernelPreparationError::Allocation))?;
    *remaining = remaining.checked_sub(text.capacity()).ok_or(E::Bounds)?;
    for part in parts {
        text.push_str(part);
    }
    Ok(text)
}
