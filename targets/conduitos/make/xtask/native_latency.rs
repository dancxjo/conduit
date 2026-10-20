//! Source-bound process observation around the ordinary graphical journey.
use super::{image, profile::Paths, report::git_head, ConduitosArch, ConduitosError};
use crate::cli::GlobalOpts;
use serde_json::json;
use std::fs;

pub(super) fn execute(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        return Err(refusal(
            "native latency observation needs a real image and guest",
        ));
    }
    let paths = Paths::new(ConduitosArch::X86_64)?;
    let source = git_head(&paths.root)?;
    let started = std::time::Instant::now();
    let built = image::execute_native_latency(opts)?;
    let preparation_ns = started.elapsed().as_nanos();
    let identity =
        super::journey_proof::execute_supplied(opts, &paths.iso, built.iso_sha256.clone())?;
    let total_ns = started.elapsed().as_nanos();
    if git_head(&paths.root)? != source {
        return Err(refusal("source changed during native observation"));
    }
    let serial = fs::read_to_string(paths.target.join("journey-serial.log")).map_err(io_error)?;
    let guest = super::native_timing::parse(&serial)?;
    if guest.is_null() {
        return Err(refusal("instrumented guest produced no timing records"));
    }
    fs::write(paths.target.join("journey-process-timing.json"), serde_json::to_vec_pretty(&json!({
        "schema":"conduit.conduitos/native-process-timing@1", "proof_class":"freestanding-emulator",
        "source_commit":source,"image_sha256":built.iso_sha256,"profile_id":identity.profile_id,
        "build_id":identity.build_id,"host_id":identity.host_id,"boot_id":identity.boot_id,
        "clock":"std::time::Instant","units":"nanoseconds","build_and_image_ns":preparation_ns,
        "journey_ns":total_ns - preparation_ns,"total_ns":total_ns,"guest":guest,
        "protocol":"One fresh QEMU process and boot; compiler and host OS caches are uncontrolled. Preparation includes supported image build. Journey includes emulator startup, inputs, receipt polling, frame capture and teardown. The process timer excludes xtask compilation and receipt serialization. Repeat records separately; this is not cold-machine, calibrated, or physical latency."
    })).expect("serializable timing report")).map_err(io_error)
}

fn refusal(detail: &str) -> ConduitosError {
    ConduitosError::refusal("native-latency-observation", detail)
}
fn io_error(error: std::io::Error) -> ConduitosError {
    ConduitosError::refusal("native-latency-observation-io", error.to_string())
}
