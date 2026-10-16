//! Version two retains the full mathematical source and replays the finite profile.
use super::*;
pub(super) fn encode(
    value: &PreparedNumericalRealization,
    rounding: Binary64Rounding,
) -> Result<StructuredConfigurationValue, NumericalRealizationRefusal> {
    use NumericalRealizationRefusal as R;
    let NumericalTarget::EnclosedBinary64(prepared) = &value.target else {
        return Err(R::InvalidCertificate);
    };
    let mut bytes = alloc::vec![2, 3, float::mode(rounding)];
    bytes.extend_from_slice(&value.admitted_limb_work.to_le_bytes());
    bytes.extend_from_slice(&value.spent_limb_work.to_le_bytes());
    let mut source = alloc::vec![0;crate::MAXIMUM_REALIZATION_PROPERTY_BYTES];
    let length = value
        .source
        .encode(&mut source)
        .map_err(|_| R::CertificateExtentExceeded)?;
    bytes.extend_from_slice(&(length as u16).to_le_bytes());
    bytes.extend_from_slice(&source[..length]);
    rational(&mut bytes, value.prepared_error_limit)?;
    rational(&mut bytes, value.maximum_error)?;
    bytes.extend_from_slice(&prepared.rounded().target().bits().to_le_bytes());
    property(bytes)
}
pub(super) fn decode(
    bytes: &[u8],
    work: &mut ExactNumericWork,
) -> Result<PreparedNumericalRealization, NumericalRealizationRefusal> {
    use NumericalRealizationRefusal as R;
    let mut cursor = Cursor { bytes };
    if cursor.take(2)? != [2, 3] {
        return Err(R::InvalidCertificate);
    }
    let rounding = float::read_mode(&mut cursor)?;
    let admitted = cursor.u64()?;
    let spent = cursor.u64()?;
    let length = u16::from_le_bytes(
        cursor
            .take(2)?
            .try_into()
            .map_err(|_| R::InvalidCertificate)?,
    ) as usize;
    let source =
        MathematicalValue::decode(cursor.take(length)?, work).map_err(|_| R::InvalidCertificate)?;
    let error_limit = cursor.rational(work)?;
    let error = cursor.rational(work)?;
    let bits = cursor.u64()?;
    if !cursor.bytes.is_empty() {
        return Err(R::InvalidCertificate);
    }
    let ceiling = admitted.min(work.remaining());
    let mut verification = ExactNumericWork::new(ceiling);
    let attempt = PreparedNumericalRealization::prepare(
        &source,
        NumericalAlgorithm::EnclosedBinary64 { rounding },
        error_limit,
        &mut verification,
    );
    work.charge(verification.spent())?;
    let mut prepared = attempt?;
    let NumericalTarget::EnclosedBinary64(target) = &prepared.target else {
        return Err(R::InvalidCertificate);
    };
    if prepared.spent_limb_work != spent
        || spent > admitted
        || prepared.maximum_error != error
        || target.rounded().target().bits() != bits
    {
        return Err(R::InvalidCertificate);
    }
    prepared.admitted_limb_work = admitted;
    if let NumericalTarget::EnclosedBinary64(target) = &mut prepared.target {
        target.restore_admitted_work_delta(admitted - ceiling);
    }
    Ok(prepared)
}
