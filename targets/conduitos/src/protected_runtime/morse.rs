//! Tour fan-out/Morse uses the same broker with its independent service lease.
use super::*;
use conduitos_runtime_core::morse as canonical;
impl Scheduler {
    pub fn new_morse(admission: canonical::Admission) -> Result<Self, SchedulerError> {
        crate::tour_morse_kernel::contract::validate(admission.roles, &admission.bindings)?;
        let roles = admission.roles.map(|node| usize::from(node.0));
        let ledger = Ledger::new_morse(&admission)?;
        #[cfg(conduitos_runtime_image)]
        let bytes: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/runtime.elf"));
        #[cfg(not(conduitos_runtime_image))]
        let bytes: &[u8] = &[];
        let image = ServiceImage::for_profile(bytes, ServiceProfile::OrdinaryRuntime)
            .map_err(|_| SchedulerError::InvalidPlan)?;
        let mut domain = ServiceDomain::install(&image).map_err(|_| SchedulerError::InvalidPlan)?;
        let window = domain
            .transfer_window()
            .map_err(|_| SchedulerError::InvalidPlan)?;
        window.fill(0);
        let length = wire::morse_admission::encode(&admission, window)
            .map_err(|_| SchedulerError::InvalidPlan)?;
        let mut bindings = [None; NODES];
        bindings[..canonical::NODES].copy_from_slice(&admission.bindings);
        let mut expected_requests = [None; NODES];
        expected_requests[roles[1]] = Some(RequestId(1));
        expected_requests[roles[2]] = Some(RequestId(2));
        expected_requests[roles[3]] = Some(RequestId(5));
        expected_requests[roles[4]] = Some(RequestId(6));
        let mut result = Self {
            instance: domain.instance(),
            domain: Some(domain),
            sequence: 1,
            error: None,
            active: canonical::NODES,
            value_bytes: canonical::MAX_VALUE_BYTES,
            value_slots: 12,
            text_mode: true,
            sign_limit: canonical::SIGN_ITEMS as u16,
            pending_limit: 4,
            expected_requests,
            stats: wire::Stats {
                decisions: 0,
                signs: 0,
                pending: 0,
            },
            bindings,
            pending: [None; NODES],
            last_request: [None; NODES],
            issued: [false; NODES],
            ledger,
            input: None,
            input_bytes: [0; VALUE_BYTES],
            staged: None,
            staged_bytes: [0; VALUE_BYTES],
            gap: None,
        };
        if result.request(Request::InitMorse {
            bytes: length as u32,
        })? != Response::Ready
        {
            return result.fail(SchedulerError::InvalidPlan);
        }
        Ok(result)
    }
}
