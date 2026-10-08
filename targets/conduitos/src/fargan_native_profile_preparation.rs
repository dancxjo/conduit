//! Exact archived FARGAN native-profile preparation capability.
//! This is an immutable host preparation profile, not a generic checker bound.
//! Reservation covers new requested payload allocations/peak during this fixed
//! decode/check/verify/factory algorithm. Existing full input Arc allocations are
//! charged separately by the pinned existing-owner reservation. Allocator
//! bookkeeping/stack/whole runtime are not claimed.
use alloc::{string::String, sync::Arc, vec::Vec};
use conduit_ai::{
    native_profile::PreparedNativeProfile,
    operation_owners::native_profile::NativeProfileOperationFactory,
};
use conduit_core::Plan;
use sha2::{Digest, Sha256};
#[derive(Clone, Copy)]
struct Pin {
    length: usize,
    sha256: [u8; 32],
}
impl Pin {
    fn accepts(self, bytes: &[u8]) -> bool {
        bytes.len() == self.length && Sha256::digest(bytes).as_slice() == self.sha256
    }
}
pub const DECLARATION_PATHS: [&str; 11] = [
    "semantics/ai/fixed_numeric.conduit",
    "semantics/ai/fixed_numeric_signal.conduit",
    "semantics/speech/fargan_conditioning.conduit",
    "semantics/speech/fargan_signal.conduit",
    "semantics/speech/fargan_subframe.conduit",
    "semantics/speech/fargan_model_identity.conduit",
    "semantics/speech/fargan_epoch_contracts.conduit",
    "semantics/speech/fargan_epoch_feedback.conduit",
    "semantics/speech/fargan_conditioning_epoch_contracts.conduit",
    "semantics/speech/fargan_feature_epoch_contracts.conduit",
    "semantics/speech/fargan_feature_direct16k_contracts.conduit",
];
const DECLARATIONS: [Pin; 11] = [
    Pin {
        length: 2546,
        sha256: [
            16, 167, 71, 87, 173, 136, 119, 88, 58, 158, 161, 96, 151, 202, 34, 52, 227, 11, 216,
            203, 245, 236, 145, 152, 49, 120, 233, 202, 146, 249, 195, 178,
        ],
    },
    Pin {
        length: 6268,
        sha256: [
            14, 66, 82, 138, 241, 110, 218, 76, 32, 2, 235, 131, 127, 87, 146, 41, 115, 236, 252,
            59, 186, 251, 161, 51, 2, 150, 224, 169, 143, 148, 116, 12,
        ],
    },
    Pin {
        length: 4825,
        sha256: [
            15, 230, 3, 134, 39, 152, 33, 184, 191, 119, 153, 153, 93, 58, 119, 54, 111, 253, 225,
            92, 212, 86, 18, 150, 201, 172, 59, 176, 198, 232, 142, 254,
        ],
    },
    Pin {
        length: 16236,
        sha256: [
            210, 43, 99, 164, 146, 7, 90, 26, 249, 24, 85, 141, 51, 203, 69, 253, 167, 161, 207,
            193, 126, 119, 150, 15, 34, 146, 125, 132, 80, 71, 156, 131,
        ],
    },
    Pin {
        length: 5008,
        sha256: [
            49, 3, 17, 251, 239, 199, 67, 236, 252, 88, 250, 12, 190, 167, 123, 27, 246, 196, 6,
            12, 52, 194, 110, 65, 139, 98, 231, 106, 172, 215, 203, 8,
        ],
    },
    Pin {
        length: 530,
        sha256: [
            112, 174, 129, 170, 27, 14, 170, 255, 160, 220, 232, 134, 254, 108, 121, 192, 174, 229,
            138, 52, 224, 28, 58, 133, 63, 102, 25, 99, 46, 13, 30, 135,
        ],
    },
    Pin {
        length: 2716,
        sha256: [
            143, 253, 10, 159, 179, 146, 87, 158, 87, 86, 171, 57, 149, 213, 132, 123, 113, 113,
            152, 47, 226, 124, 234, 84, 198, 84, 79, 221, 31, 214, 8, 187,
        ],
    },
    Pin {
        length: 1071,
        sha256: [
            117, 228, 6, 248, 125, 218, 93, 64, 23, 250, 166, 64, 165, 44, 17, 98, 151, 127, 164,
            155, 58, 129, 164, 249, 185, 198, 126, 10, 70, 36, 159, 34,
        ],
    },
    Pin {
        length: 811,
        sha256: [
            89, 50, 159, 36, 47, 242, 17, 3, 157, 73, 82, 120, 35, 248, 115, 168, 177, 38, 180,
            240, 208, 72, 3, 46, 196, 207, 43, 80, 91, 172, 155, 154,
        ],
    },
    Pin {
        length: 1330,
        sha256: [
            183, 213, 255, 234, 39, 33, 20, 167, 215, 124, 113, 52, 231, 211, 225, 244, 192, 164,
            49, 200, 189, 205, 114, 115, 116, 239, 238, 208, 200, 135, 170, 4,
        ],
    },
    Pin {
        length: 650,
        sha256: [
            56, 27, 152, 116, 209, 155, 128, 225, 38, 205, 102, 112, 160, 111, 14, 234, 44, 38,
            239, 221, 17, 7, 218, 57, 213, 192, 137, 176, 146, 119, 132, 45,
        ],
    },
];
const PLAN: Pin = Pin {
    length: 39352307,
    sha256: [
        218, 37, 200, 193, 113, 143, 211, 135, 205, 122, 37, 4, 177, 41, 186, 92, 179, 38, 162,
        105, 201, 32, 65, 79, 4, 93, 249, 38, 98, 209, 98, 232,
    ],
};
const SOURCE: Pin = Pin {
    length: 304884,
    sha256: [
        40, 155, 67, 232, 182, 41, 5, 54, 86, 121, 90, 250, 229, 102, 132, 43, 187, 74, 224, 104,
        56, 223, 214, 221, 193, 65, 129, 2, 254, 204, 199, 51,
    ],
};
const RESOURCES: [Pin; 3] = [
    Pin {
        length: 3272868,
        sha256: [
            211, 95, 5, 16, 218, 71, 103, 22, 24, 58, 51, 202, 175, 228, 92, 192, 149, 228, 141,
            73, 111, 6, 222, 45, 183, 101, 94, 120, 10, 56, 94, 71,
        ],
    },
    Pin {
        length: 872164,
        sha256: [
            87, 175, 170, 45, 248, 138, 149, 251, 9, 240, 100, 153, 52, 241, 85, 143, 248, 196,
            109, 130, 250, 118, 39, 137, 85, 53, 198, 94, 210, 46, 88, 145,
        ],
    },
    Pin {
        length: 28289,
        sha256: [
            102, 242, 14, 167, 253, 210, 48, 112, 68, 6, 56, 7, 10, 21, 131, 61, 88, 25, 100, 144,
            120, 30, 63, 226, 30, 156, 106, 37, 25, 189, 156, 236,
        ],
    },
];
/// Reviewed finite immutable-profile reservation. Unsupported bytes refuse before
/// any parser/checker/factory call. Upstream input construction is separate.
pub const PREPARATION_REQUESTED_BYTES: usize = 320 * 1024 * 1024;
pub const PREPARATION_PEAK_BYTES: usize = 96 * 1024 * 1024;
/// Conservative full existing input-owner requested storage for this pinned
/// profile, including shared allocation headers. Input construction prep is
/// upstream; callers must admit that separately before making these Arcs.
pub const EXISTING_INPUT_OWNER_BYTES: usize = 64 * 1024 * 1024;
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub preparation_requested_bytes: usize,
    pub preparation_peak_bytes: usize,
    pub existing_input_bytes: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Capacity,
    ForeignPlan,
    ForeignSource,
    ForeignDeclaration(usize),
    ForeignResource(usize),
    Decode,
    Source,
    Factory,
}
pub struct Inputs {
    pub plan: Arc<[u8]>,
    pub source: Arc<str>,
    pub declarations: [Arc<str>; 11],
    pub resources: [Arc<[u8]>; 3],
}
/// Full borrowed original material. No parser/checker is invoked during ownership preparation.
pub struct BorrowedInputs<'a> {
    pub plan: &'a [u8],
    pub source: &'a str,
    pub declarations: [&'a str; 11],
    pub resources: [&'a [u8]; 3],
}
/// Reviewed immutable copying profile, separate from filesystem/input acquisition
/// and from subsequent parser/checker preparation. Includes shared allocation headers.
pub const INPUT_COPY_REQUESTED_BYTES: usize = 64 * 1024 * 1024;
pub const INPUT_COPY_PEAK_BYTES: usize = 64 * 1024 * 1024;
#[derive(Clone, Copy, Debug)]
pub struct InputCopyLimits {
    pub requested_bytes: usize,
    pub peak_bytes: usize,
    pub retained_owner_bytes: usize,
}
#[derive(Clone, Copy, Debug)]
pub struct InputCopyReceipt {
    requested_bytes_bound: usize,
    peak_bytes_bound: usize,
    retained_owner_bytes_bound: usize,
    original_payload_bytes: usize,
}
impl InputCopyReceipt {
    pub fn requested_bytes_bound(&self) -> usize {
        self.requested_bytes_bound
    }
    pub fn peak_bytes_bound(&self) -> usize {
        self.peak_bytes_bound
    }
    pub fn retained_owner_bytes_bound(&self) -> usize {
        self.retained_owner_bytes_bound
    }
    pub fn original_payload_bytes(&self) -> usize {
        self.original_payload_bytes
    }
}
impl Inputs {
    /// Refuses unreserved/foreign complete inputs before any allocation. This is
    /// an immutable toolchain/profile reservation, not a generic allocator quota.
    /// The caller's original borrowed backing and its acquisition remain separate.
    pub fn copy_from_borrowed(
        input: BorrowedInputs<'_>,
        limits: InputCopyLimits,
    ) -> Result<(Self, InputCopyReceipt), Refusal> {
        if limits.requested_bytes < INPUT_COPY_REQUESTED_BYTES
            || limits.peak_bytes < INPUT_COPY_PEAK_BYTES
            || limits.retained_owner_bytes < EXISTING_INPUT_OWNER_BYTES
        {
            return Err(Refusal::Capacity);
        }
        if !PLAN.accepts(input.plan) {
            return Err(Refusal::ForeignPlan);
        }
        if !SOURCE.accepts(input.source.as_bytes()) {
            return Err(Refusal::ForeignSource);
        }
        for (index, (pin, material)) in DECLARATIONS.iter().zip(input.declarations).enumerate() {
            if !pin.accepts(material.as_bytes()) {
                return Err(Refusal::ForeignDeclaration(index));
            }
        }
        for (index, (pin, material)) in RESOURCES.iter().zip(input.resources).enumerate() {
            if !pin.accepts(material) {
                return Err(Refusal::ForeignResource(index));
            }
        }
        let owned = Self {
            plan: Arc::from(input.plan),
            source: Arc::from(input.source),
            declarations: input.declarations.map(Arc::from),
            resources: input.resources.map(Arc::from),
        };
        let receipt = InputCopyReceipt {
            requested_bytes_bound: INPUT_COPY_REQUESTED_BYTES,
            peak_bytes_bound: INPUT_COPY_PEAK_BYTES,
            retained_owner_bytes_bound: EXISTING_INPUT_OWNER_BYTES,
            original_payload_bytes: owned.owned_payload_bytes(),
        };
        Ok((owned, receipt))
    }
}
impl Inputs {
    pub fn owned_payload_bytes(&self) -> usize {
        self.declarations
            .iter()
            .map(|v| v.len())
            .chain(self.resources.iter().map(|v| v.len()))
            .fold(
                self.plan.len().saturating_add(self.source.len()),
                usize::saturating_add,
            )
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Receipt {
    preparation_requested_bytes_bound: usize,
    preparation_peak_bytes_bound: usize,
    original_input_payload_bytes: usize,
    original_input_owner_bytes_bound: usize,
}
impl Receipt {
    pub fn preparation_requested_bytes_bound(&self) -> usize {
        self.preparation_requested_bytes_bound
    }
    pub fn preparation_peak_bytes_bound(&self) -> usize {
        self.preparation_peak_bytes_bound
    }
    pub fn original_input_payload_bytes(&self) -> usize {
        self.original_input_payload_bytes
    }
    pub fn original_input_owner_bytes_bound(&self) -> usize {
        self.original_input_owner_bytes_bound
    }
}
pub struct PreparedNativeProfiles {
    inputs: Inputs,
    plan: Arc<Plan>,
    profiles: Vec<Arc<PreparedNativeProfile>>,
    factory: NativeProfileOperationFactory,
    receipt: Receipt,
}
impl PreparedNativeProfiles {
    pub fn inputs(&self) -> &Inputs {
        &self.inputs
    }
    pub fn plan(&self) -> &Arc<Plan> {
        &self.plan
    }
    pub fn profiles(&self) -> &[Arc<PreparedNativeProfile>] {
        &self.profiles
    }
    pub fn factory(&self) -> &NativeProfileOperationFactory {
        &self.factory
    }
    pub fn receipt(&self) -> Receipt {
        self.receipt
    }
    pub fn prepare(inputs: Inputs, limits: Limits) -> Result<Self, Refusal> {
        let input_bytes = inputs.owned_payload_bytes();
        if limits.preparation_requested_bytes < PREPARATION_REQUESTED_BYTES
            || limits.preparation_peak_bytes < PREPARATION_PEAK_BYTES
            || limits.existing_input_bytes < EXISTING_INPUT_OWNER_BYTES
        {
            return Err(Refusal::Capacity);
        }
        if !PLAN.accepts(&inputs.plan) {
            return Err(Refusal::ForeignPlan);
        }
        if !SOURCE.accepts(inputs.source.as_bytes()) {
            return Err(Refusal::ForeignSource);
        }
        for (i, (pin, material)) in DECLARATIONS.iter().zip(&inputs.declarations).enumerate() {
            if !pin.accepts(material.as_bytes()) {
                return Err(Refusal::ForeignDeclaration(i));
            }
        }
        for (i, (pin, material)) in RESOURCES.iter().zip(&inputs.resources).enumerate() {
            if !pin.accepts(material) {
                return Err(Refusal::ForeignResource(i));
            }
        }
        let plan: Plan = serde_json::from_slice(&inputs.plan).map_err(|_| Refusal::Decode)?;
        let parts: Vec<String> = inputs.declarations[..7]
            .iter()
            .enumerate()
            .map(|(index, source)| {
                if index < 2 {
                    source
                        .lines()
                        .filter(|line| {
                            line.starts_with("type NumericFiniteF32 ")
                                || line.starts_with("type NumericF32Vector")
                                || line.starts_with("type NumericI16Vector")
                                || line.starts_with("type NumericHistory")
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                } else {
                    declarations_only(source)
                }
            })
            .collect();
        let definition = parts.join("\n");
        let feedback = &inputs.declarations[7];
        let conditioning = &inputs.declarations[8];
        let mut profiles = Vec::new();
        for name in [
            "FarganFloatEpochInput",
            "FarganFloatPhase1",
            "FarganFloatPhase2",
            "FarganFloatPhase3",
            "FarganFloatEpochProposal",
            "FarganPcm16EpochResult",
        ] {
            profiles.push(Arc::new(
                PreparedNativeProfile::check_definition(&definition, name)
                    .map_err(|_| Refusal::Source)?,
            ));
        }
        let d = definition.clone() + "\n" + feedback;
        profiles.push(Arc::new(
            PreparedNativeProfile::check_definition(&d, "FarganSignalEpochFeedback")
                .map_err(|_| Refusal::Source)?,
        ));
        let d = definition.clone() + "\n" + feedback + "\n" + conditioning;
        for name in [
            "FarganConditioningInputEpoch",
            "FarganConditioningProposalEpoch",
            "FarganConditioningPendingHistory",
            "FarganConditioningEpochFeedback",
            "FarganSignalConditionEpoch",
        ] {
            profiles.push(Arc::new(
                PreparedNativeProfile::check_definition(&d, name).map_err(|_| Refusal::Source)?,
            ));
        }
        let d = definition.clone() + "\n" + conditioning;
        profiles.push(Arc::new(
            PreparedNativeProfile::check_definition(&d, "FarganFeatureConditionEpoch")
                .map_err(|_| Refusal::Source)?,
        ));
        let fd = definition + "\n" + feedback + "\n" + &inputs.declarations[9];
        let direct = fd.clone() + "\n" + &inputs.declarations[10];
        for name in [
            "FarganFeatureProposalEpoch",
            "FarganFeaturePendingState",
            "FarganFeatureEpochFeedback",
        ] {
            profiles.push(Arc::new(
                PreparedNativeProfile::check_definition(&fd, name).map_err(|_| Refusal::Source)?,
            ));
        }
        for name in ["FarganFeatureInputEpoch16k", "FarganFeaturePcmEpoch16k"] {
            profiles.push(Arc::new(
                PreparedNativeProfile::check_definition(&direct, name)
                    .map_err(|_| Refusal::Source)?,
            ));
        }
        let factory = NativeProfileOperationFactory::for_plan(&plan, &profiles)
            .map_err(|_| Refusal::Factory)?;
        if conduit_plot::syntax_source_document_identity(&inputs.source) != plan.source_document_id
        {
            return Err(Refusal::ForeignSource);
        }
        Ok(Self {
            inputs,
            plan: Arc::new(plan),
            profiles,
            factory,
            receipt: Receipt {
                preparation_requested_bytes_bound: PREPARATION_REQUESTED_BYTES,
                preparation_peak_bytes_bound: PREPARATION_PEAK_BYTES,
                original_input_payload_bytes: input_bytes,
                original_input_owner_bytes_bound: EXISTING_INPUT_OWNER_BYTES,
            },
        })
    }
}
fn declarations_only(source: &str) -> String {
    let mut active = false;
    let mut result = String::new();
    for line in source.lines() {
        if line.starts_with("type ") {
            active = true;
        }
        if line.starts_with("plot ") {
            active = false;
        }
        if active {
            result.push_str(line);
            result.push('\n');
        }
    }
    result
}
