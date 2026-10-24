//! Representation dispatch for the single selected native-operation binder.
//! Every input is validated by Core before any selection/possession check.
use conduit_core as c;

/// Storage form only. The binder validates either form before selecting facts;
/// borrowing firmware records supplies no provenance or native possession.
#[derive(Clone, Copy)]
pub enum AssignedOperationInput<'a> {
    Contiguous(&'a [u8]),
    Records {
        header: &'a [u8],
        records: &'a [c::AssignedRecord<'a>],
    },
}
pub(super) enum Records<'a> {
    Contiguous(c::AssignedPlanRecords<'a>),
    Borrowed(core::slice::Iter<'a, c::AssignedRecord<'a>>),
}
impl<'a> Iterator for Records<'a> {
    type Item = Result<c::AssignedRecord<'a>, c::AssignedPlanRefusal>;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Contiguous(records) => records.next(),
            Self::Borrowed(records) => records.next().copied().map(Ok),
        }
    }
}
impl<'a> AssignedOperationInput<'a> {
    pub(super) fn header(self) -> &'a [u8] {
        match self {
            Self::Contiguous(bytes) => bytes,
            Self::Records { header, .. } => header,
        }
    }
    pub(super) fn records(self) -> Result<Records<'a>, c::AssignedPlanRefusal> {
        match self {
            Self::Contiguous(bytes) => c::AssignedPlanRecords::new(bytes).map(Records::Contiguous),
            Self::Records { records, .. } => Ok(Records::Borrowed(records.iter())),
        }
    }
    pub(super) fn decode(
        self,
        maxima: c::AssignedPlanMaxima,
        required: c::AssignedPlanRequirements<'_>,
        computations: c::AssignedComputationProfile<'_>,
        fore: Option<&[c::AssignedForeRequirement]>,
    ) -> Result<c::AssignedPlanView, c::AssignedPlanRefusal> {
        if let Some(fore) = fore {
            return match self {
                Self::Contiguous(bytes) => {
                    c::decode_assigned_plan_with_fore(bytes, maxima, required, computations, fore)
                }
                Self::Records { header, records } => c::decode_assigned_records_with_fore(
                    header,
                    records,
                    maxima,
                    required,
                    computations,
                    fore,
                ),
            };
        }
        match self {
            Self::Contiguous(bytes) => {
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
