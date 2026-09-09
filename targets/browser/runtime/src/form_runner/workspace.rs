//! One pending acknowledgement of an actual Body start, including synchronous
//! completion. This receipt does not own execution. An empty kernel slot is not
//! a terminal receipt; the Host retains that outcome before requesting Lull.
use conduit_body::BodyPlayIdentity;
use std::cell::RefCell;

thread_local! {
    static STARTED: RefCell<Option<BodyPlayIdentity>> = const { RefCell::new(None) };
}

pub(super) fn record_start(play: &BodyPlayIdentity) {
    STARTED.with(|slot| *slot.borrow_mut() = Some(play.clone()));
}

pub(crate) fn acknowledge_start() {
    STARTED.with(|slot| *slot.borrow_mut() = None);
}

pub(crate) fn require_started(play: &BodyPlayIdentity) -> Result<(), String> {
    STARTED.with(|slot| {
        if slot.borrow().as_ref() == Some(play) {
            Ok(())
        } else {
            Err("Workspace start does not match the current browser Play".into())
        }
    })
}

pub(crate) fn require_empty() -> Result<(), String> {
    super::abi::SESSION.with(|slot| {
        if slot.borrow().is_some() {
            Err("Retire the browser Play before changing its Body lifecycle".into())
        } else {
            Ok(())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_cannot_record_a_fake_start_or_lull_a_live_kernel_slot() {
        let request = super::super::body_start::tests::request();
        let (session, started) = super::super::body_start::prepare(request).unwrap();
        assert!(require_started(&started.play).is_err());
        record_start(&started.play);
        super::super::abi::SESSION.with(|slot| *slot.borrow_mut() = Some(session));
        require_started(&started.play).unwrap();
        let mut wrong = started.play.clone();
        wrong.active_play_id = "play/unstarted".into();
        assert!(require_started(&wrong).is_err());
        assert!(require_empty().is_err());
        let mut session = super::super::abi::SESSION.with(|slot| slot.borrow_mut().take().unwrap());
        let receipt = session.cancel().unwrap();
        assert_eq!(receipt.active_play_id, started.play.active_play_id.as_str());
        require_empty().unwrap();
        // Completion may precede the workspace's durable acknowledgement.
        require_started(&started.play).unwrap();
        acknowledge_start();
        assert!(require_started(&started.play).is_err());
    }
}
