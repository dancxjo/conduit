//! Bounded development observer, not a product Back or Native-law admission.
//! Call only for an actual published Source row. A complete recorder does not
//! acknowledge recurrent state; the authored final PCM transaction owns that.
use conduit_core::*;

pub const MAXIMUM_TRACE_ROWS: usize = 256;
#[derive(Debug, PartialEq, Eq)]
pub enum TraceRefusal {
    Preparation,
    Structure,
    Anchor,
    Epoch,
    NonFinite,
    Capacity,
    Incomplete,
}
pub struct DevelopmentTraceRecorder {
    validator: PreparedStructuredValueValidator,
    anchor_type: StructuredInfoType,
    anchor_body: Vec<u8>,
    epoch_type: StructuredInfoType,
    rows: Vec<Vec<u8>>,
    first_epoch: u64,
    committed: usize,
    maximum: usize,
}
impl DevelopmentTraceRecorder {
    pub fn prepare(
        row_type: &StructuredInfoType,
        selected_anchor: &StructuredInfoValue,
        first_epoch: u64,
        count: usize,
        maximum: usize,
    ) -> Result<Self, TraceRefusal> {
        if count == 0
            || count > MAXIMUM_TRACE_ROWS
            || maximum == 0
            || maximum > 16384
            || first_epoch.checked_add(count as u64 - 1).is_none()
        {
            return Err(TraceRefusal::Preparation);
        }
        let prefix = selected_anchor
            .value_type()
            .canonical_bytes()
            .map_err(|_| TraceRefusal::Preparation)?;
        let encoded = selected_anchor
            .canonical_bytes()
            .map_err(|_| TraceRefusal::Preparation)?;
        let epoch_type = StructuredInfoType::leaf(KindId::new("value/u64")).unwrap();
        Ok(Self {
            validator: PreparedStructuredValueValidator::new(row_type, maximum)
                .map_err(|_| TraceRefusal::Preparation)?,
            anchor_type: selected_anchor.value_type().clone(),
            anchor_body: encoded[prefix.len()..].to_vec(),
            epoch_type,
            rows: (0..count).map(|_| Vec::with_capacity(maximum)).collect(),
            first_epoch,
            committed: 0,
            maximum,
        })
    }
    /// Validates borrowed canonical framing and correlation before copying into
    /// prepared storage. This observer deliberately does not grant Native laws.
    pub fn record(&mut self, encoded: &[u8]) -> Result<(), TraceRefusal> {
        if self.committed == self.rows.len() {
            return Err(TraceRefusal::Capacity);
        }
        let expected = self.first_epoch + self.committed as u64;
        let mut anchors = 0;
        let mut epochs = 0;
        self.validator
            .visit_nodes(encoded, |ty, body| {
                if ty == &self.anchor_type {
                    anchors += 1;
                    if body != self.anchor_body {
                        return Err(TraceRefusal::Anchor);
                    }
                }
                if ty == &self.epoch_type {
                    epochs += 1;
                    if body.len() != 13
                        || body[0] != 0
                        || body[1..5] != 8u32.to_le_bytes()
                        || u64::from_le_bytes(body[5..].try_into().unwrap()) != expected
                    {
                        return Err(TraceRefusal::Epoch);
                    }
                }
                if let StructuredInfoTypeShape::Leaf(kind) = ty.shape() {
                    if kind.as_str() == "value/ieee754-binary32"
                        && (body.len() != 9
                            || !f32::from_le_bytes(
                                body[5..].try_into().map_err(|_| TraceRefusal::Structure)?,
                            )
                            .is_finite())
                    {
                        return Err(TraceRefusal::NonFinite);
                    }
                }
                Ok(())
            })
            .map_err(|error| match error {
                StructuredNodeVisitRefusal::Structure(_) => TraceRefusal::Structure,
                StructuredNodeVisitRefusal::Visitor(error) => error,
            })?;
        if anchors != 1 {
            return Err(TraceRefusal::Anchor);
        }
        if epochs != 1 {
            return Err(TraceRefusal::Epoch);
        }
        self.rows[self.committed].extend_from_slice(encoded);
        self.committed += 1;
        Ok(())
    }
    pub fn finish(&self) -> Result<&[Vec<u8>], TraceRefusal> {
        if self.committed != self.rows.len() {
            return Err(TraceRefusal::Incomplete);
        }
        Ok(&self.rows)
    }
    pub fn prepared_payload_capacity(&self) -> usize {
        self.rows.len() * self.maximum
    }
}

/// Development preparation envelope derived from complete native8k timing.
/// It neither changes duration nor proves coverage/committed parser authority.
pub struct NativeTraceExtent {
    pub native_epochs: usize,
    pub output_rows: usize,
    pub aligned_samples_16k: usize,
}
impl NativeTraceExtent {
    pub fn prepare(samples_8k: usize) -> Result<Self, TraceRefusal> {
        if samples_8k == 0 || !samples_8k.is_multiple_of(80) {
            return Err(TraceRefusal::Preparation);
        }
        let native_epochs = samples_8k / 80;
        let output_rows = native_epochs
            .checked_add(1)
            .ok_or(TraceRefusal::Preparation)?;
        if output_rows > MAXIMUM_TRACE_ROWS {
            return Err(TraceRefusal::Capacity);
        }
        Ok(Self {
            native_epochs,
            output_rows,
            aligned_samples_16k: samples_8k.checked_mul(2).ok_or(TraceRefusal::Preparation)?,
        })
    }
}
