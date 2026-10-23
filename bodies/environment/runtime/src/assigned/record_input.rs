//! Borrowed representation dispatch; all validation remains in Core and the
//! existing fixed-table builder. No runtime allocation or execution lives here.
use conduit_core as c;
#[derive(Clone, Copy)]
pub(crate) enum RecordInput<'a> {
    Bytes(&'a [u8]),
    Records {
        header: &'a [u8],
        records: &'a [c::AssignedRecord<'a>],
    },
}
pub(crate) enum Records<'a> {
    Bytes(c::AssignedPlanRecords<'a>),
    Borrowed(core::slice::Iter<'a, c::AssignedRecord<'a>>),
}
impl<'a> Iterator for Records<'a> {
    type Item = Result<c::AssignedRecord<'a>, c::AssignedPlanRefusal>;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Bytes(records) => records.next(),
            Self::Borrowed(records) => records.next().copied().map(Ok),
        }
    }
}
impl<'a> RecordInput<'a> {
    pub(crate) fn header(self) -> &'a [u8] {
        match self {
            Self::Bytes(bytes) => bytes,
            Self::Records { header, .. } => header,
        }
    }
    pub(crate) fn records(self) -> Result<Records<'a>, c::AssignedPlanRefusal> {
        match self {
            Self::Bytes(bytes) => c::AssignedPlanRecords::new(bytes).map(Records::Bytes),
            Self::Records { records, .. } => Ok(Records::Borrowed(records.iter())),
        }
    }
    pub(crate) fn decode(
        self,
        maxima: c::AssignedPlanMaxima,
        required: c::AssignedPlanRequirements<'_>,
        computations: c::AssignedComputationProfile<'_>,
    ) -> Result<c::AssignedPlanView, c::AssignedPlanRefusal> {
        match self {
            Self::Bytes(bytes) => {
                c::decode_assigned_plan_with_computations(bytes, maxima, required, computations)
            }
            Self::Records { header, records } => c::decode_assigned_records_with_computations(
                header,
                records,
                maxima,
                required,
                computations,
            ),
        }
    }
}
