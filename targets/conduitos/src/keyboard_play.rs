//! One fixed production-kernel Play for the planned portable keyboard Gear.

use conduit_human::KeyEvent;
use conduit_kernel::scheduler::{
    CordCapacity, CordSpec, FixedScheduler, NodeSpec, StepInputBytes, StepIo, StepOperation,
    StepOutcome,
};
use conduit_kernel::{
    CordId, FixedRoutes, FixedSignLog, FixedValueStore, KernelEvent, NodeId, PortId, RouteRange,
    RouteTarget, ValueRef, ValueStorage,
};

use crate::{keyboard_plan::PreparedKeyboardPlay, ordinary_plan::PreparationError};

const PORTS: usize = 1;
const SIGN_EVENTS: usize = 32;

type Scheduler =
    FixedScheduler<Driver, FixedValueStore<2, 3>, FixedSignLog<SIGN_EVENTS>, 2, 1, PORTS, 1, 2, 1>;

#[derive(Clone, Copy)]
enum Role {
    Source {
        values: [Option<ValueRef>; 2],
        next: usize,
    },
    Boundary {
        values: [Option<KeyEvent>; 2],
        count: usize,
    },
}

#[derive(Clone, Copy)]
struct Driver {
    role: Role,
    cancelled: bool,
}

impl StepOperation<PORTS> for Driver {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        match &mut self.role {
            Role::Source { values, next } => {
                let Some(value) = values.get(*next).copied().flatten() else {
                    return StepOutcome::Complete;
                };
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.send(PortId(0), value)
                    .expect("planned key Cord admitted");
                *next += 1;
                StepOutcome::Progress
            }
            Role::Boundary { values, count } => {
                if io.input(PortId(0)).is_some() {
                    let Some(bytes) = input_bytes.input(PortId(0)) else {
                        return StepOutcome::Fail(conduit_kernel::Failure {
                            code: conduit_kernel::FailureCode::InvalidInput,
                            detail: 1,
                        });
                    };
                    let Ok(value) = KeyEvent::decode(bytes) else {
                        return StepOutcome::Fail(conduit_kernel::Failure {
                            code: conduit_kernel::FailureCode::InvalidInput,
                            detail: 2,
                        });
                    };
                    let Some(slot) = values.get_mut(*count) else {
                        return StepOutcome::Fail(conduit_kernel::Failure {
                            code: conduit_kernel::FailureCode::StorageExhausted,
                            detail: 3,
                        });
                    };
                    *slot = Some(value);
                    *count += 1;
                    io.consume(PortId(0)).expect("planned key value consumed");
                    StepOutcome::Progress
                } else if io.input_closed(PortId(0)) {
                    io.consume_closed(PortId(0))
                        .expect("planned key boundary closes");
                    StepOutcome::Complete
                } else {
                    StepOutcome::Await
                }
            }
        }
    }

    fn cancel(&mut self) {
        self.cancelled = true;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KeyboardPlayReport {
    pub values: [KeyEvent; 2],
    pub cord_item_capacity: u16,
    pub cord_byte_capacity: u32,
    pub completed: bool,
}

pub fn run(
    prepared: &PreparedKeyboardPlay,
    values: [KeyEvent; 2],
) -> Result<KeyboardPlayReport, PreparationError> {
    let fragment = prepared
        .plan
        .fragments
        .first()
        .ok_or(PreparationError::PlanRejected)?;
    if fragment.placements.len() != 1
        || fragment.placements[0].kind_id.as_str() != conduit_semantic_catalog::KEYBOARD_KIND
        || prepared.active_play.plan_id != prepared.plan.plan_id
        || prepared.active_play.host_id != fragment.host_id
        || prepared.active_play.boot_id != fragment.boot_id
    {
        return Err(PreparationError::PlanRejected);
    }
    let mut store = FixedValueStore::new(6).map_err(|_| PreparationError::KernelRejected)?;
    let references = [
        Some(
            store
                .store(&values[0].encode())
                .map_err(|_| PreparationError::KernelRejected)?,
        ),
        Some(
            store
                .store(&values[1].encode())
                .map_err(|_| PreparationError::KernelRejected)?,
        ),
    ];
    let mut routes = FixedRoutes::new(PORTS as u16);
    routes
        .install(
            NodeId(0),
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: CordId(0),
                sink: conduit_kernel::CordEndpoint::local(NodeId(1), PortId(0)),
            }],
        )
        .map_err(|_| PreparationError::KernelRejected)?;
    routes
        .seal()
        .map_err(|_| PreparationError::KernelRejected)?;
    let sign_bytes = u32::try_from(SIGN_EVENTS * core::mem::size_of::<KernelEvent>())
        .map_err(|_| PreparationError::KernelRejected)?;
    let mut scheduler = Scheduler::new(
        [
            NodeSpec {
                input_cords: [None],
                maximum_step_work: 1,
            },
            NodeSpec {
                input_cords: [Some(CordId(0))],
                maximum_step_work: 1,
            },
        ],
        [CordSpec::local(
            CordId(0),
            (NodeId(0), PortId(0)),
            (NodeId(1), PortId(0)),
            CordCapacity {
                slot_start: 0,
                item_capacity: 1,
                byte_capacity: conduit_human::KEY_EVENT_ENCODED_LEN as u32,
            },
        )],
        routes,
        [
            Driver {
                role: Role::Source {
                    values: references,
                    next: 0,
                },
                cancelled: false,
            },
            Driver {
                role: Role::Boundary {
                    values: [None; 2],
                    count: 0,
                },
                cancelled: false,
            },
        ],
        store,
        FixedSignLog::new(sign_bytes).map_err(|_| PreparationError::KernelRejected)?,
    )
    .map_err(|_| PreparationError::KernelRejected)?;
    scheduler
        .run(32)
        .map_err(|_| PreparationError::KernelRejected)?;
    let Role::Boundary {
        values: observed,
        count,
    } = scheduler.drivers()[1].role
    else {
        return Err(PreparationError::KernelRejected);
    };
    if count != 2 || observed != values.map(Some) {
        return Err(PreparationError::KernelRejected);
    }
    Ok(KeyboardPlayReport {
        values,
        cord_item_capacity: 1,
        cord_byte_capacity: conduit_human::KEY_EVENT_ENCODED_LEN as u32,
        completed: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        identity::BootIdentities,
        keyboard_offer::KeyboardRealization,
        keyboard_plan,
        offer::{CpuFeatures, HostOffer},
    };
    use conduit_human::{KeyModifiers, KeyTransition};

    #[test]
    fn planned_keyboard_values_cross_the_fixed_kernel_and_close() {
        assert_eq!(
            conduit_human::KEY_EVENT_DELIVERY_CONTRACT,
            conduit_semantic_catalog::reviewed_delivery_contract(&conduit_core::KindId::from(
                conduit_human::KEY_EVENT_INFO_ID,
            ))
            .unwrap()
        );
        assert_eq!(
            conduit_human::KEY_EVENT_DELIVERY_CONTRACT.pressure_policy,
            conduit_core::DeliveryPressurePolicy::PreserveOrder
        );
        let identities = BootIdentities {
            host: [1; 32],
            boot: [2; 32],
        };
        let offer = HostOffer::new(
            &identities,
            "build",
            CpuFeatures {
                sse2: true,
                rdrand: true,
                invariant_tsc: true,
            },
            1_048_576,
        )
        .with_keyboard(
            KeyboardRealization {
                controller_id: [3; 32],
                device_id: [4; 32],
                interface_id: [5; 32],
                endpoint_id: [6; 32],
                report_buffers: 2,
                transition_slots: 8,
                operation_slots: 2,
            },
            "build",
        )
        .unwrap();
        let prepared = keyboard_plan::prepare(&identities, &offer, "build").unwrap();
        let values = [
            KeyEvent::new(4, KeyTransition::Pressed, KeyModifiers::NONE).unwrap(),
            KeyEvent::new(4, KeyTransition::Released, KeyModifiers::NONE).unwrap(),
        ];
        let report = run(&prepared, values).unwrap();
        assert_eq!(report.values, values);
        assert_eq!(report.cord_byte_capacity, 3);
        assert!(report.completed);
    }
}
