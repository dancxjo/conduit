//! Service task handshake, not physical socket or Body lifecycle evidence.
use conduit_environment_http::service_control::*;
use core::{
    future::Future,
    task::{Context, Poll, Waker},
};
fn poll<F: Future>(future: core::pin::Pin<&mut F>) -> Poll<F::Output> {
    future.poll(&mut Context::from_waker(Waker::noop()))
}
fn command(mailbox: &ServiceMailbox) -> ServiceCommand {
    match poll(core::pin::pin!(mailbox.next_command())) {
        Poll::Ready(command) => command,
        Poll::Pending => panic!("expected command"),
    }
}
#[test]
fn acknowledgement_is_required_and_control_generation_cannot_be_reused() {
    let mailbox = ServiceMailbox::new();
    let mut control = mailbox.claim_control(2).unwrap();
    assert!(matches!(
        mailbox.claim_control(2),
        Err(ServiceControlRefusal::AlreadyClaimed)
    ));
    let mut work = core::pin::pin!(control.request(ServiceMode::Running));
    assert!(poll(work.as_mut()).is_pending());
    let cmd = command(&mailbox);
    assert_eq!(cmd.mode, ServiceMode::Running);
    assert!(poll(work.as_mut()).is_pending());
    mailbox.acknowledge(cmd, ServiceState::Ready);
    assert_eq!(poll(work.as_mut()), Poll::Ready(Ok(())));
    assert_eq!(mailbox.observed_state(), Some(ServiceState::Ready));
}
#[test]
fn cancellation_and_old_acknowledgements_do_not_satisfy_a_new_stop() {
    let mailbox = ServiceMailbox::new();
    let mut control = mailbox.claim_control(3).unwrap();
    let old = {
        let mut work = core::pin::pin!(control.request(ServiceMode::Running));
        assert!(poll(work.as_mut()).is_pending());
        command(&mailbox)
    };
    let mut stop = core::pin::pin!(control.request(ServiceMode::Stopped));
    assert!(poll(stop.as_mut()).is_pending());
    let current = command(&mailbox);
    mailbox.acknowledge(old, ServiceState::Ready);
    assert!(poll(stop.as_mut()).is_pending());
    mailbox.acknowledge(current, ServiceState::Stopped);
    assert_eq!(poll(stop.as_mut()), Poll::Ready(Ok(())));
    mailbox.acknowledge(old, ServiceState::Ready);
    assert_eq!(mailbox.observed_state(), Some(ServiceState::Stopped));
}
#[test]
fn newer_desired_mode_supersedes_an_unobserved_cancelled_start() {
    let mailbox = ServiceMailbox::new();
    let mut control = mailbox.claim_control(2).unwrap();
    {
        let mut start = core::pin::pin!(control.request(ServiceMode::Running));
        assert!(poll(start.as_mut()).is_pending());
    }
    {
        let mut stop = core::pin::pin!(control.request(ServiceMode::Stopped));
        assert!(poll(stop.as_mut()).is_pending());
        let cmd = command(&mailbox);
        assert_eq!(cmd.mode, ServiceMode::Stopped);
        mailbox.acknowledge(cmd, ServiceState::Stopped);
        assert_eq!(poll(stop.as_mut()), Poll::Ready(Ok(())));
    }
    assert_eq!(
        poll(core::pin::pin!(control.request(ServiceMode::Running))),
        Poll::Ready(Err(ServiceControlRefusal::Exhausted))
    );
    assert!(poll(core::pin::pin!(mailbox.next_command())).is_pending());
}
#[test]
fn service_failure_revokes_readiness_and_wrong_terminal_is_not_success() {
    let mailbox = ServiceMailbox::new();
    let mut control = mailbox.claim_control(2).unwrap();
    let first = {
        let mut start = core::pin::pin!(control.request(ServiceMode::Running));
        assert!(poll(start.as_mut()).is_pending());
        let cmd = command(&mailbox);
        mailbox.acknowledge(cmd, ServiceState::Ready);
        assert_eq!(poll(start.as_mut()), Poll::Ready(Ok(())));
        cmd
    };
    mailbox.acknowledge(first, ServiceState::Failed);
    assert_eq!(mailbox.observed_state(), Some(ServiceState::Failed));
    let mut stop = core::pin::pin!(control.request(ServiceMode::Stopped));
    assert!(poll(stop.as_mut()).is_pending());
    let cmd = command(&mailbox);
    mailbox.acknowledge(cmd, ServiceState::Ready);
    assert_eq!(
        poll(stop.as_mut()),
        Poll::Ready(Err(ServiceControlRefusal::Failed))
    );
}
