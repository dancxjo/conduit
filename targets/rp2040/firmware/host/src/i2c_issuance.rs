//! Local native issuance after opaque workset approval; not admission or Start.
use crate::{config, native_preparation::Refusal as E};
use conduit_body::{DelegatedBodyFragment, NativePreparationContext};
use conduit_core as c;
use conduit_i2c_transaction::{
    contract::{I2C_CALL, I2C_KIND, I2C_MAXIMUM_BYTES},
    installation::{ReadyI2cBase, I2C_AUTHORITY, I2C_IMPLEMENTATION},
    owner::I2cCallRefusal,
    I2cProvider,
};
use sha2::{Digest, Sha256};
#[path = "i2c_issuance/construction.rs"]
mod construction;
use construction::Budget;
#[path = "i2c_issuance/authorization.rs"]
mod authorization;

pub type Issued = (
    c::BaseCapabilityTable,
    c::BaseCapabilityHandle,
    c::BaseOperationClaim,
);

/// Root retains this counter across attempts, independently of carrier state.
/// Consumed attempt numbers never roll back, including allocation refusals.
pub struct Issuer {
    next: u64,
    authorization: Option<authorization::OriginalAuthority>,
}
impl Issuer {
    pub const fn new() -> Self {
        Self {
            next: 1,
            authorization: None,
        }
    }
    pub fn issue<P: I2cProvider>(
        &mut self,
        context: NativePreparationContext<'_>,
        approved: &DelegatedBodyFragment<'_>,
        native: &ReadyI2cBase<P>,
        maximum_bytes: usize,
    ) -> Result<Issued, E> {
        let id = native.native_identity();
        let attachment = native.attachment();
        let m = approved.metadata();
        if context.image_id != config::IMAGE_ID
            || context.configuration_id != config::CONFIGURATION_ID
            || config::I2C_CALL_MAXIMUM_OPERATIONS == 0
            || config::I2C_CALL_ADDRESS_RANGE
                != Some((attachment.minimum_address, attachment.maximum_address))
            || !context.bootstrap.matches_native(
                context.actual.host_id.as_str(),
                config::IMAGE_ID,
                config::CONFIGURATION_ID,
            )
            || context.actual.profile.as_str() != config::CONFIGURATION_ID
            || context.actual.protocol_version != c::PROTOCOL_VERSION
            || context.actual.offer_generation.0 == 0
            || context.bootstrap.admission.secret() == &[0; 32]
            || context.member.body_id != context.bootstrap.body_id
            || context.member.host_id != context.actual.host_id
            || context.member.boot_id != context.actual.boot_id
            || m.body != context.member.body_id.as_str()
            || m.host != context.actual.host_id.as_str()
            || m.boot != context.actual.boot_id.as_str()
            || m.offer_generation != context.actual.offer_generation.0
            || approved.receiver_profile()
                != c::AssignedIdentity::from_text(context.actual.profile.as_str())
            || id.host_id != context.actual.host_id
            || id.boot_id != context.actual.boot_id
            || id.artifact_id.as_str() != config::IMAGE_ID
        {
            return Err(E::Scope);
        }
        let suffix = match config::I2C_CONTROLLER {
            0 => "/i2c/0/authority",
            1 => "/i2c/1/authority",
            _ => return Err(E::Scope),
        };
        let authorization = self.authorization.as_ref().ok_or(E::Scope)?;
        if !authorization.matches(context, native) {
            return Err(E::Scope);
        }
        let sequence = self.next;
        self.next = sequence.checked_add(1).ok_or(E::Bounds)?;
        // Domain-separated local bearer key; never exported or serialized. The
        // current Boot and monotonic attempt distinguish replacement tables.
        let mut hash = Sha256::new();
        hash.update(b"conduit.rp2040/i2c-capability-issuer@1\0");
        hash.update(context.bootstrap.admission.secret());
        hash.update(sequence.to_le_bytes());
        hash.update(context.actual.boot_id.as_str().as_bytes());
        let key = hash.finalize().into();
        let budget = Budget::prepare(id, m, suffix, maximum_bytes)?;
        construction::issue(budget, id, m, &authorization.grant, key)
    }
}
