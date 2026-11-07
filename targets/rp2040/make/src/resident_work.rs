//! Checked construction budget, not proof of live availability or a reservation.
use conduit_host_make::CheckedHostConfiguration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedResidentWorkPool {
    pub id: String,
    pub capacity_units: u32,
    pub bookkeeping_bytes: u64,
}
impl CheckedResidentWorkPool {
    pub fn from_configuration(checked: &CheckedHostConfiguration) -> Result<Self, String> {
        let configuration = checked.configuration();
        let mut pools = configuration
            .resources
            .iter()
            .filter(|pool| pool.class == "kernel/cooperative-step-work@1");
        let pool = pools
            .next()
            .ok_or("resident execution requires an explicit work-pool budget")?;
        if pools.next().is_some() {
            return Err("resident work-pool selection is ambiguous".into());
        }
        if pool.bytes > configuration.limits.heap_arena_bytes {
            return Err("resident work-pool bookkeeping exceeds the Host preparation arena".into());
        }
        Ok(Self {
            id: pool.id.clone(),
            capacity_units: pool.slots,
            bookkeeping_bytes: pool.bytes,
        })
    }
}
