//! Two real, separately admitted placements around a checked storage Back.
use super::contract::{BLOCK_READ_MAXIMUM_BYTES, BlockReadContract};
use alloc::{vec, vec::Vec};
use conduit_core::*;
pub const AUTHORIZE_KIND: &str = "storage/block-read/authorize";
pub const SETTLE_KIND: &str = "storage/block-read/settle";
pub const AUTHORIZE_CALL: &str = "conduit.host/block-read-authorize@1";
pub const SETTLE_CALL: &str = "conduit.host/block-read-settle@1";

pub struct BlockReadTransactionContract {
    block: BlockReadContract,
    permit: StructuredInfoType,
    authorize: Kind,
    settle: Kind,
}
impl BlockReadTransactionContract {
    pub fn prepare() -> Result<Self, StructuredInfoRefusal> {
        let block = BlockReadContract::prepare()?;
        let StructuredInfoTypeShape::Variant { cases, .. } = block.result_type().shape() else {
            return Err(StructuredInfoRefusal::WrongType);
        };
        let permit = cases
            .iter()
            .find(|case| case.tag() == "unsupported")
            .ok_or(StructuredInfoRefusal::UnknownVariantTag)?
            .payload_type()
            .clone();
        let authorize = kind(AUTHORIZE_KIND, block.request_type(), &permit)?;
        let settle = kind(SETTLE_KIND, block.result_type(), block.result_type())?;
        Ok(Self {
            block,
            permit,
            authorize,
            settle,
        })
    }
    pub fn block(&self) -> &BlockReadContract {
        &self.block
    }
    pub fn permit_type(&self) -> &StructuredInfoType {
        &self.permit
    }
    pub fn authorize_kind(&self) -> &Kind {
        &self.authorize
    }
    pub fn settle_kind(&self) -> &Kind {
        &self.settle
    }
}
fn kind(
    name: &str,
    input: &StructuredInfoType,
    output: &StructuredInfoType,
) -> Result<Kind, StructuredInfoRefusal> {
    let port = |name, ty: &StructuredInfoType, direction| {
        Ok::<_, StructuredInfoRefusal>(PortDescriptor {
            port_id: port_id(name),
            value_kind: ty.profile()?.value_kind().clone(),
            direction,
            temporal: PortTemporal::Flow { closes: true },
            abnormal_kind: None,
        })
    };
    let input = port("request", input, PortDirection::Input)?;
    let output = port("result", output, PortDirection::Output)?;
    let value = |location, port: &PortDescriptor| {
        Ok::<_, StructuredInfoRefusal>(FrontValueContract {
            location,
            contract: CheckedValueContract::new(
                port.value_kind.clone(),
                BLOCK_READ_MAXIMUM_BYTES,
                Vec::new(),
            )
            .map_err(|_| StructuredInfoRefusal::WrongType)?,
        })
    };
    let laws = vec![KindSemanticLaw::ValueContracts(vec![
        value(FrontValueLocation::Input(input.port_id.clone()), &input)?,
        value(FrontValueLocation::Output(output.port_id.clone()), &output)?,
    ])];
    Ok(Kind {
        kind_id: kind_id(name),
        kind_contract_revision: KindIdentity::from(alloc::format!("{name}@1")),
        startup_parameters: vec![],
        shorthand: Some((input.port_id.clone(), output.port_id.clone())),
        inputs: vec![input],
        outputs: vec![output],
        configuration: vec![],
        semantic_laws: laws,
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: BLOCK_READ_MAXIMUM_BYTES,
        },
    })
}
