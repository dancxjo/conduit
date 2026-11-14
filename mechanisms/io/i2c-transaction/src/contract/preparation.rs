//! Fallible flat interface preparation; no owned Type tree or Source parser.
use super::*;
use alloc::{string::String, vec::Vec};
use conduit_core::{CapabilityLimits, KindIdentity, PortDescriptor, PortDirection, PortTemporal};
use core::mem::size_of;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum I2cContractPreparationRefusal {
    Capacity,
    Allocation,
}
impl I2cContract {
    pub fn prepare() -> Result<Self, I2cContractPreparationRefusal> {
        Self::prepare_bounded(usize::MAX)
    }
    /// Fund actual retained strings and port tables. Inline Kind and allocator
    /// overhead remain separate Host charges; no provider or authority is made.
    pub fn prepare_bounded(maximum_bytes: usize) -> Result<Self, I2cContractPreparationRefusal> {
        use I2cContractPreparationRefusal as E;
        let fields = [
            I2C_KIND,
            "machine/i2c/transact@1",
            "request",
            I2C_REQUEST_PROFILE,
            "result",
            I2C_RESULT_PROFILE,
            "request",
            "result",
        ];
        let required = fields
            .iter()
            .try_fold(
                size_of::<PortDescriptor>()
                    .checked_mul(2)
                    .ok_or(E::Capacity)?,
                |n, s| n.checked_add(s.len()),
            )
            .ok_or(E::Capacity)?;
        if required > maximum_bytes {
            return Err(E::Capacity);
        }
        let mut remaining = maximum_bytes;
        let kind_id = copy(fields[0], &mut remaining)?.into();
        let kind_contract_revision = KindIdentity::from(copy(fields[1], &mut remaining)?);
        let input = PortDescriptor {
            port_id: copy(fields[2], &mut remaining)?.into(),
            value_kind: copy(fields[3], &mut remaining)?.into(),
            direction: PortDirection::Input,
            temporal: PortTemporal::Flow { closes: true },
            abnormal_kind: None,
        };
        let output = PortDescriptor {
            port_id: copy(fields[4], &mut remaining)?.into(),
            value_kind: copy(fields[5], &mut remaining)?.into(),
            direction: PortDirection::Output,
            temporal: PortTemporal::Flow { closes: true },
            abnormal_kind: None,
        };
        let shorthand = Some((
            copy(fields[6], &mut remaining)?.into(),
            copy(fields[7], &mut remaining)?.into(),
        ));
        let mut inputs = one_port(&mut remaining)?;
        inputs.push(input);
        let mut outputs = one_port(&mut remaining)?;
        outputs.push(output);
        Ok(Self {
            kind: Kind {
                kind_id,
                kind_contract_revision,
                startup_parameters: Vec::new(),
                shorthand,
                inputs,
                outputs,
                configuration: Vec::new(),
                semantic_laws: Vec::new(),
                limits: CapabilityLimits {
                    max_active_instances: 1,
                    max_queue_items: 1,
                    max_queue_bytes: I2C_MAXIMUM_BYTES,
                },
            },
        })
    }
    /// Actual owned interface backing, excluding inline Self and allocator headers.
    pub fn retained_backing_bytes(&self) -> usize {
        let k = &self.kind;
        let mut bytes = k.kind_id.0.capacity()
            + k.kind_contract_revision.0.capacity()
            + (k.inputs.capacity() + k.outputs.capacity()) * size_of::<PortDescriptor>();
        for p in k.inputs.iter().chain(&k.outputs) {
            bytes += p.port_id.0.capacity() + p.value_kind.0.capacity();
        }
        if let Some((a, b)) = &k.shorthand {
            bytes += a.0.capacity() + b.0.capacity();
        }
        bytes
    }
}
fn copy(text: &str, remaining: &mut usize) -> Result<String, I2cContractPreparationRefusal> {
    use I2cContractPreparationRefusal as E;
    if text.len() > *remaining {
        return Err(E::Capacity);
    }
    let mut value = String::new();
    value
        .try_reserve_exact(text.len())
        .map_err(|_| E::Allocation)?;
    *remaining = remaining.checked_sub(value.capacity()).ok_or(E::Capacity)?;
    value.push_str(text);
    Ok(value)
}
fn one_port(remaining: &mut usize) -> Result<Vec<PortDescriptor>, I2cContractPreparationRefusal> {
    use I2cContractPreparationRefusal as E;
    let mut values = Vec::new();
    values.try_reserve_exact(1).map_err(|_| E::Allocation)?;
    *remaining = remaining
        .checked_sub(
            values
                .capacity()
                .checked_mul(size_of::<PortDescriptor>())
                .ok_or(E::Capacity)?,
        )
        .ok_or(E::Capacity)?;
    Ok(values)
}
