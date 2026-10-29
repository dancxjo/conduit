//! Scalar refusal-stage telemetry; never execution state or authority.
use super::IssuedCustody;

// Phased stages: 100 entry; 101/102 grants; 103 advertisement; 104 planner;
// 105 selection entry; 1051 tables; 1052 roles; 1053 attachment; 1054 placements;
// 1055 Root preparation; 1056/1057 expected credential/Join scopes;
// 1058 pair selection; 1059 numeric parts; 110 claim memory; 111/112 issuer memory; 113 reservation memory;
// 114 kernel/Sign backing; 115 admission callback; 120 actual Root reservation;
// 121/122 credential issue/check; 123/124 Join issue/check; 125 materialization.
// Scalar diagnostics only; no payload, authority or retained execution state.
#[inline(never)]
#[cold]
pub(super) fn trace_phased_refusal(
    stage: u16,
    custody: &IssuedCustody,
    reserved: bool,
    admitted: bool,
) {
    #[cfg(test)]
    PHASE_REFUSAL.with(|last| last.set(Some((stage, reserved, admitted))));
    #[cfg(target_os = "none")]
    {
        let custody = match custody {
            IssuedCustody::Empty => 0,
            IssuedCustody::Lease(_) => 1,
            IssuedCustody::First(_) => 2,
            IssuedCustody::FirstLease(_, _) => 3,
            IssuedCustody::Both(_, _) => 4,
        };
        esp_println::println!("{{\"event\":\"network-phased-install-refused\",\"stage\":{},\"admitted\":{},\"issued_custody\":{},\"active_reserved\":{}}}", stage, admitted, custody, reserved);
    }
    #[cfg(not(target_os = "none"))]
    let _ = (stage, custody, reserved, admitted);
}
#[cfg(test)]
std::thread_local! {
    static PHASE_REFUSAL: std::cell::Cell<Option<(u16, bool, bool)>> = const { std::cell::Cell::new(None) };
}
#[cfg(test)]
pub(crate) fn phased_refusal() -> Option<(u16, bool, bool)> {
    PHASE_REFUSAL.with(|last| last.take())
}

// One line per actual failed installation, no retained metadata. Stage codes:
// 1/2 grants; 3 advertisement; 4 planner; 5 selected output; 6 backing;
// 7 materialization; 10 tables; 11 kernel profile; 12 attachment;
// 13/14 placement; 15 Root reserve; 16 credential issue; 17 credential check;
// 18 join issue; 19 join check; 20 prepared kernel parts.
#[inline(never)]
#[cold]
pub(super) fn trace_flat_refusal(stage: u16, custody: &IssuedCustody, active: bool) {
    #[cfg(target_os = "none")]
    {
        let custody = match custody {
            IssuedCustody::Empty => 0,
            IssuedCustody::Lease(_) => 1,
            IssuedCustody::First(_) => 2,
            IssuedCustody::FirstLease(_, _) => 3,
            IssuedCustody::Both(_, _) => 4,
        };
        esp_println::println!("{{\"event\":\"network-flat-install-refused\",\"stage\":{},\"issued_custody\":{},\"active_reserved\":{}}}", stage, custody, active);
    }
    #[cfg(not(target_os = "none"))]
    let _ = (stage, custody, active);
}
