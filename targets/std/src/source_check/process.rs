//! One dedicated preparation process. No portable catalogue importer or executor.
use conduit_plot::{
    SourceCheckContext, SourceCheckNativeSchema, SourceCheckProjection, SourceCheckRefusal,
    SourceCheckReply, SourceCheckRequest,
};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
/// Trusted parent startup data from already selected immutable Source custody.
/// Deserializing this input grants no access or Host execution authority.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceCheckProcessInput {
    pub source: String,
    pub owner: String,
    pub generation: u64,
    pub request: SourceCheckRequest,
}
#[derive(Clone, Copy)]
pub struct SourceCheckProcessLimits {
    /// Complete child virtual address space, including code, stack and heap.
    pub address_space_bytes: u64,
    /// Complete child CPU time. The parent separately owns wall-time/cancellation.
    pub cpu_seconds: u64,
}
pub(super) const MAXIMUM_INPUT_BYTES: u64 =
    6 * conduit_plot::MAXIMUM_PLOT_SOURCE_BYTES as u64 + 16 * 1024;
pub(super) const MAXIMUM_REPLY_BYTES: usize = 64 * 1024;
impl SourceCheckProcessLimits {
    fn validate(self) -> Result<(), String> {
        if !(8 * 1024 * 1024..=1024 * 1024 * 1024).contains(&self.address_space_bytes)
            || !(1..=30).contains(&self.cpu_seconds)
        {
            return Err("Source-check process limits outside finite Linux profile".into());
        }
        Ok(())
    }
    fn apply(self) -> Result<(), String> {
        self.validate()?;
        // SAFETY: affects only this dedicated child process. No inherited limit
        // is enlarged; failure prevents catalogue construction or Source reading.
        for (resource, requested) in [
            (libc::RLIMIT_CORE, 0),
            (libc::RLIMIT_CPU, self.cpu_seconds),
            (libc::RLIMIT_AS, self.address_space_bytes),
        ] {
            let mut old = libc::rlimit {
                rlim_cur: 0,
                rlim_max: 0,
            };
            if unsafe { libc::getrlimit(resource, &mut old) } != 0 {
                return Err(format!(
                    "Source-check getrlimit: {}",
                    std::io::Error::last_os_error()
                ));
            }
            // The finite profile is at most 1 GiB / 30 seconds, also on
            // 32-bit Linux targets where rlim_t may be narrower than u64.
            let value = (requested as libc::rlim_t)
                .min(old.rlim_cur)
                .min(old.rlim_max);
            let bound = libc::rlimit {
                rlim_cur: value,
                rlim_max: value,
            };
            if unsafe { libc::setrlimit(resource, &bound) } != 0 {
                return Err(format!(
                    "Source-check setrlimit: {}",
                    std::io::Error::last_os_error()
                ));
            }
        }
        Ok(())
    }
}
/// Call only as the dedicated worker entrance. Limits are applied before the
/// trusted executable constructs its actual private checked catalogue. No
/// catalogue, callback, path or revision-based lookup is accepted from input.
/// This is preparation, not an automatically installed Ready Host offer.
pub fn run(
    limits: SourceCheckProcessLimits,
    context: impl FnOnce() -> Result<SourceCheckContext, String>,
    input: impl Read,
    mut output: impl Write,
) -> Result<(), String> {
    limits.apply()?;
    let mut bytes = Vec::new();
    input
        .take(MAXIMUM_INPUT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Source-check input: {error}"))?;
    if bytes.len() as u64 > MAXIMUM_INPUT_BYTES {
        return Err("Source-check input exceeds fixed byte profile".into());
    }
    let input: SourceCheckProcessInput = serde_json::from_slice(&bytes)
        .map_err(|error| format!("Source-check input shape: {error}"))?;
    let context = context()?;
    let schema =
        SourceCheckNativeSchema::prepare().map_err(|e| format!("Source-check schema: {e:?}"))?;
    let result = analyze(input, context);
    let reply = match &result {
        Ok(report) => SourceCheckReply::Checked(report),
        Err(reason) => SourceCheckReply::Refused(*reason),
    };
    let reply = schema
        .encode_reply(reply, MAXIMUM_REPLY_BYTES)
        .map_err(|e| format!("Source-check reply: {e:?}"))?;
    output
        .write_all(&reply)
        .map_err(|error| format!("Source-check output: {error}"))
}
fn analyze(
    input: SourceCheckProcessInput,
    context: SourceCheckContext,
) -> Result<conduit_plot::SourceCheckInfoV1, SourceCheckRefusal> {
    let mut owner =
        SourceCheckProjection::new(&input.owner, input.generation, &input.source, context)?;
    owner.project(&input.request)
}

mod executable;
mod parent;
mod prepared;
pub use parent::{SourceCheckProcessFailure, SourceCheckProcessOutput, SourceCheckProcessWorker};

mod source;
pub use crate::source_file::SourceFileRefusal;
pub use source::OwnedSourceCheckInput;
