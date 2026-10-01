use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum JourneyActionKind {
    Bootstrap,
    Birth,
    UsefulWork,
    MaskInspectInitialShow,
    MaskWearAlternate,
    MaskPreferAlternate,
    MaskWithdrawSelectedRoute,
    MaskInspectUnavailableShow,
    MaskAddFaceHost,
    MaskAdmitReplacementPlan,
    MaskInspectReplannedShow,
    MaskDoffAlternate,
    MaskInspectRestoredShow,
    BreakAndRecover,
    RestAndFinish,
}

impl JourneyActionKind {
    pub(crate) const fn title(self) -> &'static str {
        match self {
            Self::Bootstrap => "Begin with no Body",
            Self::Birth => "Birth and become usable",
            Self::UsefulWork => "Do useful work",
            Self::MaskInspectInitialShow => "Inspect the initial Show",
            Self::MaskWearAlternate => "Wear an alternate Mask",
            Self::MaskPreferAlternate => "Prefer the alternate Mask",
            Self::MaskWithdrawSelectedRoute => "Withdraw the selected route",
            Self::MaskInspectUnavailableShow => "Inspect truthful unavailability",
            Self::MaskAddFaceHost => "Add a Face Host",
            Self::MaskAdmitReplacementPlan => "Admit a replacement Plan",
            Self::MaskInspectReplannedShow => "Inspect the replanned Show",
            Self::MaskDoffAlternate => "Doff the alternate Mask",
            Self::MaskInspectRestoredShow => "Inspect the restored Show",
            Self::BreakAndRecover => "Break and recovery outcome",
            Self::RestAndFinish => "Rest and finish",
        }
    }
}

pub(crate) const REQUIRED_ACTIONS: [JourneyActionKind; 15] = [
    JourneyActionKind::Bootstrap,
    JourneyActionKind::Birth,
    JourneyActionKind::UsefulWork,
    JourneyActionKind::MaskInspectInitialShow,
    JourneyActionKind::MaskWearAlternate,
    JourneyActionKind::MaskPreferAlternate,
    JourneyActionKind::MaskWithdrawSelectedRoute,
    JourneyActionKind::MaskInspectUnavailableShow,
    JourneyActionKind::MaskAddFaceHost,
    JourneyActionKind::MaskAdmitReplacementPlan,
    JourneyActionKind::MaskInspectReplannedShow,
    JourneyActionKind::MaskDoffAlternate,
    JourneyActionKind::MaskInspectRestoredShow,
    JourneyActionKind::BreakAndRecover,
    JourneyActionKind::RestAndFinish,
];
