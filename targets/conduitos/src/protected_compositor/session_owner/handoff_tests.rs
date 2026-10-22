use super::*;

#[test]
fn withdrawn_commons_creche_birth_handoff_mints_fresh_session_without_replaying_handles() {
    let mut owner = owner();
    let commons = owner.activate_withdrawn().unwrap();
    assert_eq!(commons.session.namespace, 1);
    let commons_surface = admit(&mut owner, commons.session);
    owner
        .request(commons.session, Request::Remove(commons_surface))
        .unwrap();
    // Withdrawal alone keeps the old namespace: its final empty frame can
    // still account for old-rectangle damage before an explicit handoff.
    assert!(owner.is_active(commons.session));
    let creche = owner.activate_withdrawn().unwrap();
    assert_eq!(creche.retired, Some(commons.session));
    assert_eq!(creche.session.namespace, 2);
    assert_eq!(creche.removed, 0);
    let creche_surface = admit(&mut owner, creche.session);
    owner
        .request(creche.session, Request::Remove(creche_surface))
        .unwrap();
    let birth = owner.activate_withdrawn().unwrap();
    assert_eq!(birth.retired, Some(creche.session));
    assert_eq!(birth.session.namespace, 3);
    assert_eq!(birth.removed, 0);
    assert_ne!(birth.session, commons.session);
    assert_ne!(birth.session, creche.session);
    assert!(owner.transport.full_damage);
    let calls = owner.transport.commands.len();
    for old in [commons.session, creche.session] {
        assert_eq!(
            owner.request(old, Request::Keyboard),
            Err(OwnerError::InactiveSession)
        );
        assert_eq!(
            owner.upload_chunk(old, 0, &[0]),
            Err(OwnerError::InactiveSession)
        );
    }
    for old in [commons_surface, creche_surface] {
        assert_eq!(
            owner.request(birth.session, Request::Focus(old)),
            Err(OwnerError::ForeignHandle)
        );
    }
    assert_eq!(owner.transport.commands.len(), calls);
    let born = admit(&mut owner, birth.session);
    assert_eq!(born.slot, commons_surface.slot);
    assert!(born.generation > creche_surface.generation);
    assert_eq!(
        owner.request(birth.session, Request::Focus(born)),
        Ok(Response::Unit)
    );
    assert_eq!(owner.transport.quarantines, 0);
}

#[test]
fn withdrawn_handoff_refuses_live_surfaces_without_retiring_or_sending_commands() {
    let mut owner = owner();
    let active = owner.activate_withdrawn().unwrap().session;
    let surface = admit(&mut owner, active);
    let calls = owner.transport.commands.len();
    assert_eq!(owner.activate_withdrawn(), Err(OwnerError::ActiveSurfaces));
    assert_eq!(owner.active_session(), Some(active));
    assert_eq!(owner.transport.commands.len(), calls);
    assert_eq!(
        owner.transport.slots[usize::from(surface.slot)],
        Some(surface)
    );
    assert_eq!(
        owner.request(active, Request::Focus(surface)),
        Ok(Response::Unit)
    );
}

#[test]
fn withdrawn_handoff_failure_is_terminal_and_cannot_retry_uncertain_effects() {
    for failure_step in [1, 2] {
        for refused in [false, true] {
            let mut owner = owner();
            let old = owner.activate_withdrawn().unwrap().session;
            let calls = owner.transport.commands.len();
            if refused {
                owner.transport.refuse_at = Some(calls + failure_step);
            } else {
                owner.transport.fail_at = Some(calls + failure_step);
            }
            assert_eq!(
                owner.activate_withdrawn(),
                Err(if refused {
                    OwnerError::Protocol
                } else {
                    OwnerError::Transport(99)
                })
            );
            assert!(owner.is_terminated());
            assert!(!owner.is_active(old));
            let effects = owner.transport.commands.len();
            assert_eq!(owner.activate_withdrawn(), Err(OwnerError::Terminated));
            assert_eq!(
                owner.request(old, Request::Keyboard),
                Err(OwnerError::Terminated)
            );
            assert_eq!(owner.transport.commands.len(), effects);
            assert_eq!(owner.transport.quarantines, 1);
        }
    }
}

#[test]
fn withdrawn_handoff_never_wraps_namespace_generation() {
    let mut owner = owner();
    let old = owner.activate_withdrawn().unwrap().session;
    owner.namespace = u64::MAX;
    let calls = owner.transport.commands.len();
    assert_eq!(owner.activate_withdrawn(), Err(OwnerError::Exhausted));
    assert_eq!(owner.active_session(), Some(old));
    assert_eq!(owner.transport.commands.len(), calls);
}
