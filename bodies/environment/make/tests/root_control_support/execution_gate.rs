//! Shared production gate; no actual task, Body installation or pin effects.
use crate::native_execution_gate as gate;
#[test]
fn start_is_held_and_release_requires_current_uncancelled_start() {
    for phase in [
        "unprepared",
        "registering",
        "running",
        "quiesced",
        "retired",
    ] {
        let mut held = phase;
        assert!(gate::accept_start(&mut held, false, true).is_err());
        assert_eq!(held, phase);
        assert!(gate::release(&mut held, false).is_err());
        assert_eq!(held, phase);
    }
    let mut phase = "ready-held";
    assert!(gate::release(&mut phase, false).is_err());
    assert!(gate::accept_start(&mut phase, false, false).is_err());
    assert!(gate::accept_start(&mut phase, true, true).is_err());
    assert_eq!(phase, "ready-held");
    gate::accept_start(&mut phase, false, true).unwrap();
    assert_eq!(phase, "started-held");
    assert!(gate::accept_start(&mut phase, false, true).is_err());
    assert!(gate::release(&mut phase, true).is_err());
    assert_eq!(phase, "started-held");
    gate::release(&mut phase, false).unwrap();
    assert_eq!(phase, "released");
    assert!(gate::release(&mut phase, false).is_err());
}
