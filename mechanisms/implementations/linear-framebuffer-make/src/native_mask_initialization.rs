//! Checked native presenter intent. This does not initialize a Mask, issuer,
//! physical output or provider. Runtime preparation must admit its actual storage.
use crate::CheckedScanoutInitialization;
use conduit_host_make::{CheckedHostConfiguration, ResourceBudget};

/// One standard native graphical presenter and its explicitly declared pools.
/// The finite runtime-memory capacity is a caller-authored ceiling, not a size
/// inferred from incoming Face data or proof of sufficient preparation storage.
#[derive(Clone)]
pub struct CheckedNativeMaskInitialization {
    scanout: CheckedScanoutInitialization,
    surface: ResourceBudget,
    show: ResourceBudget,
    memory: ResourceBudget,
    input: Option<ResourceBudget>,
}
impl CheckedNativeMaskInitialization {
    pub fn from_configuration(checked: &CheckedHostConfiguration) -> Result<Self, String> {
        let mut selected = checked
            .profile()
            .presenters
            .iter()
            .filter(|presenter| presenter.implementation == "presenter/native-graphical@1");
        if selected.next().is_none() || selected.next().is_some() {
            return Err("native Mask requires one exact checked graphical presenter".into());
        }
        let scanout = CheckedScanoutInitialization::from_configuration(checked)?;
        let surface = one_pool(checked, "face/surface")?;
        if surface.slots != 1 || surface.bytes < scanout.surface_bytes() {
            return Err(
                "native Mask physical surface does not cover checked scanout geometry".into(),
            );
        }
        let show = one_pool(checked, conduit_face::SHOW_RESOURCE_CLASS)?;
        if show.slots != 1 || show.bytes == 0 {
            return Err("native Mask requires one finite single-slot Show pool".into());
        }
        let memory = one_pool(checked, conduit_core::RUNTIME_MEMORY_RESOURCE_CLASS)?;
        if memory.slots == 0 || memory.bytes < u64::from(memory.slots) {
            return Err("native Mask runtime memory lacks positive byte-backed capacity".into());
        }
        let mut inputs = checked
            .configuration()
            .resources
            .iter()
            .filter(|pool| pool.class == "conduit.resource/conduitos-human-input@1");
        let input = inputs.next().cloned();
        if inputs.next().is_some()
            || input
                .as_ref()
                .is_some_and(|pool| pool.slots != 1 || pool.bytes == 0)
        {
            return Err("native interaction requires one explicit single-slot input pool".into());
        }
        Ok(Self {
            scanout,
            surface,
            show,
            memory,
            input,
        })
    }
    pub fn configuration_id(&self) -> &str {
        self.scanout.configuration_id()
    }
    pub fn scanout(&self) -> &CheckedScanoutInitialization {
        &self.scanout
    }
    pub fn surface(&self) -> &ResourceBudget {
        &self.surface
    }
    pub fn show(&self) -> &ResourceBudget {
        &self.show
    }
    pub fn input(&self) -> Option<&ResourceBudget> {
        self.input.as_ref()
    }
    pub fn memory(&self) -> &ResourceBudget {
        &self.memory
    }
}
fn one_pool(checked: &CheckedHostConfiguration, class: &str) -> Result<ResourceBudget, String> {
    let mut matching = checked
        .configuration()
        .resources
        .iter()
        .filter(|pool| pool.class == class);
    let pool = matching
        .next()
        .ok_or_else(|| format!("native Mask missing declared {class} pool"))?;
    if matching.next().is_some() {
        return Err(format!("native Mask requires one unambiguous {class} pool"));
    }
    Ok(pool.clone())
}
