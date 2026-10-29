use super::*;
use alloc::boxed::Box;
use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};

fn poll<F: Future>(future: Pin<&mut F>) -> Poll<F::Output> {
    future.poll(&mut Context::from_waker(Waker::noop()))
}

fn command(mailbox: &ServiceMailbox) -> ServiceCommand {
    let mut next = Box::pin(mailbox.next_command());
    match poll(next.as_mut()) {
        Poll::Ready(command) => command,
        Poll::Pending => panic!("request must have issued a real command"),
    }
}

#[test]
fn canceled_new_command_cannot_reuse_old_ready_acknowledgment() {
    let mailbox = ServiceMailbox::new();
    let mut control = mailbox.claim_control(3).unwrap();
    assert_eq!(control.acknowledged_state(), None);
    assert_eq!(control.remaining_commands(), 3);
    let first = {
        let mut running = Box::pin(control.request(ServiceMode::Running));
        assert!(poll(running.as_mut()).is_pending());
        let first = command(&mailbox);
        mailbox.acknowledge(first, ServiceState::Ready);
        assert_eq!(poll(running.as_mut()), Poll::Ready(Ok(())));
        first
    };
    assert_eq!(control.acknowledged_state(), Some(ServiceState::Ready));
    assert_eq!(control.remaining_commands(), 2);
    let stopped = {
        let mut stopping = Box::pin(control.request(ServiceMode::Stopped));
        assert!(poll(stopping.as_mut()).is_pending());
        command(&mailbox)
        // Cancel the future while the real service task still owes its ACK.
    };
    assert_eq!(mailbox.observed_state(), Some(ServiceState::Ready));
    assert_eq!(control.acknowledged_state(), None);
    assert_eq!(control.remaining_commands(), 1);
    mailbox.acknowledge(first, ServiceState::Ready);
    assert_eq!(control.acknowledged_state(), None);
    mailbox.acknowledge(stopped, ServiceState::Stopped);
    assert_eq!(control.acknowledged_state(), Some(ServiceState::Stopped));
    {
        let mut restoring = Box::pin(control.request(ServiceMode::Running));
        assert!(poll(restoring.as_mut()).is_pending());
        let restore = command(&mailbox);
        mailbox.acknowledge(stopped, ServiceState::Ready);
        assert!(poll(restoring.as_mut()).is_pending());
        mailbox.acknowledge(restore, ServiceState::Failed);
        assert_eq!(
            poll(restoring.as_mut()),
            Poll::Ready(Err(ServiceControlRefusal::Failed))
        );
    }
    assert_eq!(control.acknowledged_state(), Some(ServiceState::Failed));
    assert_eq!(control.remaining_commands(), 0);
    let mut exhausted = Box::pin(control.request(ServiceMode::Running));
    assert_eq!(
        poll(exhausted.as_mut()),
        Poll::Ready(Err(ServiceControlRefusal::Exhausted))
    );
    drop(exhausted);
    assert_eq!(control.remaining_commands(), 0);
    assert!(matches!(
        mailbox.claim_control(3),
        Err(ServiceControlRefusal::AlreadyClaimed)
    ));
}
