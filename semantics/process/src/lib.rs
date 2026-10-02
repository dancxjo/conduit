#![no_std]

extern crate alloc;

#[allow(dead_code, clippy::too_many_arguments)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/semantic_types.rs"));
}

pub use generated::*;

pub const JOB_ARGUMENT_SLOTS: usize = 8;
pub const JOB_ENVIRONMENT_SLOTS: usize = 8;
pub const JOB_MAXIMUM_TEXT_BYTES: usize = 256;
pub const JOB_MAXIMUM_OUTPUT_BYTES: u32 = 65_536;
pub const JOB_MAXIMUM_TIMEOUT_MILLIS: u64 = 86_400_000;
pub const JOB_EXECUTABLE_CONTENT_PROFILE: &str = "process/executable-image@1";
pub const JOB_EXECUTABLE_ACCESS_CLASS: &str = "conduit.resource/executable@1";
pub const JOB_EXECUTABLE_AUTHORITY: &str = "conduit.authority/execute-resource@1";

impl JobRequest {
    pub fn validate_job(&self) -> Result<(), JobRequestRefusal> {
        self.executable()
            .get()
            .validate()
            .map_err(|_| JobRequestRefusal::InvalidExecutable)?;
        if self.executable().get().content_profile.as_str() != JOB_EXECUTABLE_CONTENT_PROFILE
            || self.executable().get().access_class.as_str() != JOB_EXECUTABLE_ACCESS_CLASS
        {
            return Err(JobRequestRefusal::InvalidExecutable);
        }
        for (index, entry) in self.environment().get().iter().enumerate() {
            if entry.name().get().is_empty() {
                return Err(JobRequestRefusal::EmptyEnvironmentName);
            }
            if self
                .environment()
                .get()
                .iter()
                .take(index)
                .any(|previous| previous.name().get() == entry.name().get())
            {
                return Err(JobRequestRefusal::DuplicateEnvironmentName);
            }
        }
        Ok(())
    }
}
