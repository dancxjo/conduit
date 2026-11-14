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
#[derive(Clone, Copy)]
pub(crate) enum AssignedForeProfile<'a> {
    Local(&'a [c::AssignedForeRequirement]),
    Line(c::AssignedLineForeProfile<'a>),
}
impl AssignedForeProfile<'_> {
    pub(crate) fn schema(self) -> u16 {
        match self {
            Self::Local(_) => c::ASSIGNED_FORE_NATIVE_PLAN_SCHEMA,
            Self::Line(_) => c::ASSIGNED_LINE_FORE_NATIVE_PLAN_SCHEMA,
        }
    }
}
pub(crate) enum Records<'a> {
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
    pub(crate) fn header(self) -> &'a [u8] {
        match self {
            Self::Contiguous(bytes) => bytes,
            Self::Records { header, .. } => header,
        }
    }
    pub(crate) fn records(self) -> Result<Records<'a>, c::AssignedPlanRefusal> {
        match self {
            Self::Contiguous(bytes) => c::AssignedPlanRecords::new(bytes).map(Records::Contiguous),
            Self::Records { records, .. } => Ok(Records::Borrowed(records.iter())),
        }
    }
    pub(crate) fn decode_profile(
        self,
        maxima: c::AssignedPlanMaxima,
        required: c::AssignedPlanRequirements<'_>,
        computations: c::AssignedComputationProfile<'_>,
        fore: Option<AssignedForeProfile<'_>>,
    ) -> Result<c::AssignedPlanView, c::AssignedPlanRefusal> {
        let fore = match fore {
            Some(AssignedForeProfile::Line(profile)) => {
                return match self {
                    Self::Contiguous(bytes) => c::decode_assigned_plan_with_line_fore(
                        bytes,
                        maxima,
                        required,
                        computations,
                        profile,
                    ),
                    Self::Records { .. } => Err(c::AssignedPlanRefusal::WrongSchema),
                }
            }
            Some(AssignedForeProfile::Local(fore)) => Some(fore),
            None => None,
        };
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
