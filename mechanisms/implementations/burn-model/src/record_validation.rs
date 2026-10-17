//! Validate provider-owned records before committing or restoring numerical state.
use crate::Error;
use burn::{
    store::burn_pack::{Reader, Scalar},
    tensor::{Bytes, DType},
};
use std::collections::BTreeSet;

pub(crate) fn validate_record(bytes: Bytes, optimizer: bool) -> Result<BTreeSet<u64>, Error> {
    let reader = Reader::from_bytes(bytes).map_err(|_| Error::CorruptCheckpoint)?;
    if reader
        .scalars()
        .values()
        .any(|value| matches!(value,Scalar::Float(v) if !v.is_finite()))
    {
        return Err(Error::NumericFailure);
    }
    let mut parameter_ids = BTreeSet::new();
    for tensor in reader
        .into_tensors()
        .map_err(|_| Error::CorruptCheckpoint)?
    {
        if optimizer && tensor.param_id.is_none() {
            return Err(Error::CorruptCheckpoint);
        }
        if let Some(id) = tensor.param_id {
            parameter_ids.insert(id);
        }
        let (_, dtype, _, _, bytes) = tensor.into_parts().map_err(|_| Error::CorruptCheckpoint)?;
        match dtype {
            DType::F32 | DType::Flex32 => {
                let (values, remainder) = bytes.as_chunks::<4>();
                if !remainder.is_empty()
                    || values.iter().any(|v| !f32::from_le_bytes(*v).is_finite())
                {
                    return Err(Error::NumericFailure);
                }
            }
            DType::F64 | DType::F16 | DType::BF16 | DType::QFloat(_) => {
                return Err(Error::InvalidDescriptor)
            }
            // Integer/bool module buffers are allowed; their representation has no NaN/Inf.
            _ => {}
        }
    }
    Ok(parameter_ids)
}
