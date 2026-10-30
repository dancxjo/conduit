//! One immutable local timer deadline; I/O has returned before this is called.
use super::*;
use core::{future::Future, pin::pin, task::Poll};
use embassy_time::Timer;
impl Owner {
    pub(crate) async fn wait(&mut self) {
        let deadline = Instant::now()
            .checked_add(Duration::from_secs(2))
            .expect("monotonic local wait exhausted");
        let mut timer = pin!(Timer::at(deadline));
        let mut receive = pin!(REQUEST.wait());
        let mut window = None;
        let mut tried_window = false;
        let mut attempted = false;
        let mut received = false;
        core::future::poll_fn(|cx| {
            // Register and poll the actual original timer before doing work.
            if timer.as_mut().poll(cx).is_ready() {
                return Poll::Ready(());
            }
            let now = Instant::now();
            if !tried_window {
                tried_window = true;
                window = self
                    .gate
                    .begin_wait(now.as_millis(), deadline.as_millis())
                    .ok();
            }
            if self.pending.is_none() && !received {
                if let Poll::Ready(request) = receive.as_mut().poll(cx) {
                    self.pending = Some(request);
                    received = true;
                }
            }
            if attempted {
                return Poll::Pending;
            }
            let Some(request) = self.pending.as_ref() else {
                return Poll::Pending;
            };
            if !self.ready
                || now
                    .checked_add(Duration::from_millis(u64::from(hardware::BUDGET_MS)))
                    .is_none_or(|end| end > request.deadline)
            {
                let request = self.pending.take().unwrap();
                RESPONSE.signal((
                    request.token,
                    Err(if self.ready {
                        "credential-request-expired"
                    } else {
                        "credential-storage-unavailable"
                    }),
                ));
                attempted = true;
                return Poll::Pending;
            }
            let Some(window) = window.as_ref() else {
                return Poll::Pending;
            };
            let started = Instant::now();
            let result = critical_section::with(|_| {
                if !hardware::quiescent() {
                    return None;
                }
                let now = Instant::now();
                // Gate milliseconds are rounded down; enforce the exact original
                // timer and request deadlines again in the hardware clock domain.
                if !policy::can_start(
                    now.as_micros(),
                    deadline.as_micros(),
                    request.deadline.as_micros(),
                ) {
                    return None;
                }
                let permit = window
                    .try_flash(now.as_millis(), hardware::BUDGET_MS)
                    .ok()?;
                let operation_started = Instant::now();
                if !policy::can_start(
                    operation_started.as_micros(),
                    deadline.as_micros(),
                    request.deadline.as_micros(),
                ) {
                    return None;
                }
                let access = hardware::Access::writable(&mut self.flash, &permit);
                let result = Journal::open(access)
                    .map_err(|_| "credential-journal-open")
                    .and_then(|mut journal| {
                        let mut current = Wire {
                            bytes: [0; MAX],
                            length: 0,
                        };
                        match journal.read(&mut current.bytes) {
                            Ok((_, length))
                                if current.bytes[..length]
                                    == request.wire.bytes[..request.wire.length] =>
                            {
                                return Ok((journal.revision(), false))
                            }
                            Ok(_) | Err(Refusal::Empty) => (),
                            Err(_) => return Err("credential-journal-read"),
                        }
                        journal
                            .commit(&request.wire.bytes[..request.wire.length])
                            .map(|revision| (revision, true))
                            .map_err(|_| "credential-journal-commit")
                    });
                if !policy::completed(
                    operation_started.as_micros(),
                    Instant::now().as_micros(),
                    deadline.as_micros(),
                    request.deadline.as_micros(),
                ) {
                    return Some(Err("credential-commit-overrun"));
                }
                Some(result)
            });
            if let Some(result) = result {
                attempted = true;
                let request = self.pending.take().unwrap();
                if result.is_err() {
                    self.ready = false;
                }
                let elapsed = started.elapsed().as_micros();
                critical_section::with(|cs| {
                    let mut state = STATE.borrow(cs).borrow_mut();
                    state.metrics.ready = self.ready;
                    state.metrics.maximum_us = state.metrics.maximum_us.max(elapsed);
                    if let Ok((revision, changed)) = result {
                        state.metrics.revision = revision;
                        state.metrics.commits =
                            state.metrics.commits.saturating_add(u32::from(changed));
                    }
                });
                RESPONSE.signal((request.token, result.map(|_| ())));
            } else {
                critical_section::with(|cs| {
                    let mut state = STATE.borrow(cs).borrow_mut();
                    state.metrics.deferred = state.metrics.deferred.saturating_add(1);
                });
            }
            Poll::Pending
        })
        .await;
    }
}
