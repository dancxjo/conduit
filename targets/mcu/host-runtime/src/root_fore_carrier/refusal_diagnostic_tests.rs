//! Actual Root-attributed preparation and original carrier loans, without a Fore
//! activation or provider effect. Later paths deliberately remain pending.
use super::*;
use crate::preparation_refusal::{send_once, Details};
use core::{
    cell::RefCell,
    future::Future,
    task::{Context, Poll, Waker},
};
use std::{future::pending, vec::Vec};
struct Recording<'a> {
    writes: &'a RefCell<Vec<Vec<u8>>>,
    fail: bool,
}
impl Carrier for Recording<'_> {
    type Error = ();
    async fn connected(&mut self) {
        panic!("refusal must not reconnect")
    }
    async fn read(&mut self, _: &mut [u8], _: u32) -> Result<usize, ()> {
        panic!("refusal must not receive")
    }
    async fn write(&mut self, bytes: &[u8], _: u32) -> Result<(), ()> {
        self.writes.borrow_mut().push(bytes.to_vec());
        if self.fail {
            Err(())
        } else {
            Ok(())
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    PreparationRefusal,
    PreparedPending,
    AdmissionRefusal,
    StartedPending,
}
#[test]
fn only_initial_refusal_ends_original_loan_before_one_diagnostic() {
    for phase in [
        Phase::PreparationRefusal,
        Phase::PreparedPending,
        Phase::AdmissionRefusal,
        Phase::StartedPending,
    ] {
        for send_fails in [false, true] {
            let f = fixture();
            let bytes = seed(&f);
            let lines = [actual_line()];
            let pins = [pin(&lines[0])];
            let local = [docs::local()];
            let bindings = [binding()];
            let document = document(&pins[0]);
            let mut root = root(&f, &bytes, &document);
            let before_input = *root.input.payload_digest().unwrap();
            let before_ledger = serde_json::to_vec(root.resources.admissions()).unwrap();
            let bound = Cell::new(0);
            let writes = RefCell::new(Vec::new());
            let mut link = Link::new(Recording {
                writes: &writes,
                fail: send_fails,
            });
            let mut backing = [0; 64];
            // SAFETY: this CPU fixture exclusively models the same original
            // admitted heap lease; this counter is no physical fit evidence.
            let image =
                unsafe { crate::image_memory::StagedImageMemory::verified_heap(118784) }.unwrap();
            let mut arena = Some(image.reserve_remaining().unwrap());
            let mut response = [0; 4096];
            let mut outer = std::boxed::Box::pin(async {
                let reason = {
                    let attempt = async {
                        let parts = root.execution_parts().unwrap();
                        let context = b::NativePreparationContext {
                            bootstrap: parts.bootstrap,
                            member: parts.member,
                            actual: &f.host,
                            image_id: "image/1",
                            configuration_id: "config/1",
                            now_millis: 11,
                        };
                        let policy = crate::execution::Policy {
                            preparation: Some(&PREP),
                            execution: Some(&EXEC),
                            active_worksets: 0,
                            maximum_retained_bytes: 16384,
                        };
                        // SAFETY: this actual original fixture link/backing and
                        // installed Line/peer facts are independent of its Plan.
                        let selection = unsafe {
                            NativeForeCarrierSelection::from_native_custody(
                                &mut link,
                                &mut backing,
                                "image/1",
                                "config/1",
                                &f.issuer,
                                c::AssignedLineForeProfile {
                                    local: &local,
                                    lines: &pins,
                                },
                                &lines,
                                "session",
                                "serial-profile",
                                bounds(),
                            )
                        }
                        .unwrap();
                        let mut selection = Some(selection);
                        let prepared = PreparedLocalWorkset::<_, 1>::prepare_selected_profiles::<
                            4096,
                            2,
                            ForeCarrierRefusal,
                        >(
                            parts.workspace,
                            context,
                            policy,
                            |_, _| {
                                Ok(AssignedPreparationProfile::LineFore(docs::profile(
                                    &local, &pins, &bindings,
                                )))
                            },
                            |approved, fore| {
                                let carrier = parts
                                    .prepare_fore_carrier::<Link<Recording<'_>>, 1, 2>(
                                        11,
                                        selection.take(),
                                        &approved[0],
                                        &fore[0],
                                        docs::profile(&local, &pins, &bindings),
                                    )?;
                                let owned = carrier.attach(Ticket(&bound));
                                if phase == Phase::PreparationRefusal {
                                    // Returning Err drops the same original partial wrapper.
                                    return Err(ForeCarrierRefusal::Bounds);
                                }
                                Ok(owned)
                            },
                        );
                        let prepared = match prepared {
                            Err(crate::execution::Refusal::Back(ForeCarrierRefusal::Bounds)) => {
                                return "complete-source-preparation"
                            }
                            Err(_) => panic!("unexpected preparation failure"),
                            Ok(prepared) => prepared,
                        };
                        match phase {
                            Phase::PreparationRefusal => unreachable!(),
                            Phase::PreparedPending => {
                                pending::<()>().await;
                                core::hint::black_box(&prepared);
                            }
                            Phase::AdmissionRefusal => {
                                let refused = prepared.admit::<ForeCarrierRefusal>(
                                    parts.resources,
                                    &[],
                                    usize::MAX,
                                );
                                assert!(refused.is_err());
                                pending::<()>().await;
                                core::hint::black_box(&refused);
                            }
                            Phase::StartedPending => {
                                let mut committed = prepared
                                    .admit::<ForeCarrierRefusal>(parts.resources, &[], 0)
                                    .unwrap();
                                let started = committed
                                    .start::<4096, ForeCarrierRefusal>(
                                        parts.workspace,
                                        context,
                                        policy,
                                        parts.resources,
                                    )
                                    .unwrap();
                                let lease = arena.take().unwrap();
                                pending::<()>().await;
                                core::hint::black_box((&started, &lease));
                            }
                        }
                        pending::<&'static str>().await
                    };
                    let mut attempt = core::pin::pin!(attempt);
                    crate::poll_boundary::separate(attempt.as_mut()).await
                };
                // Rust can borrow the SAME original link only after the failed
                // attempt and every original carrier wrapper have retired.
                send_once(
                    &mut link,
                    &mut response,
                    &Details {
                        reason,
                        detail: "forced before admission",
                        heap_used_bytes: 0,
                        heap_peak_bytes: None,
                        heap_limit_bytes: 118784,
                    },
                )
                .await
            });
            let poll = outer.as_mut().poll(&mut Context::from_waker(Waker::noop()));
            if phase == Phase::PreparationRefusal {
                assert!(matches!(poll, Poll::Ready(result) if result.is_err()==send_fails));
                assert_eq!(bound.get(), 0);
                let messages = writes.borrow();
                if send_fails {
                    assert_eq!(messages.len(), 1, "one failed header write, no retry");
                } else {
                    let length = usize::from(u16::from_be_bytes(
                        messages[0].as_slice().try_into().unwrap(),
                    ));
                    assert_eq!(
                        messages.len(),
                        1 + length.div_ceil(crate::transport::PACKET_BYTES)
                            + usize::from(length.is_multiple_of(crate::transport::PACKET_BYTES))
                    );
                    let payload: Vec<_> = messages[1..].iter().flatten().copied().collect();
                    assert_eq!(payload.len(), length);
                    let json: serde_json::Value = serde_json::from_slice(&payload).unwrap();
                    assert_eq!(json["phase"], "refused-before-play");
                    assert_eq!(json["reason"], "complete-source-preparation");
                }
            } else {
                assert!(poll.is_pending());
                assert!(writes.borrow().is_empty());
                assert_eq!(bound.get(), usize::from(phase == Phase::StartedPending));
            }
            assert_eq!(
                image.remaining_bytes(),
                0,
                "original IMAGE remains held before terminal cleanup"
            );
            drop(outer);
            assert_eq!(root.input.payload_digest(), Some(&before_input));
            if phase != Phase::StartedPending {
                assert_eq!(
                    serde_json::to_vec(root.resources.admissions()).unwrap(),
                    before_ledger
                );
            }
            assert!(root.retained_input());
        }
    }
}
