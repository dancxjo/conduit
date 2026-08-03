use crate::{CommandFlowPolicy, RoboticsReason, validate_command_flow_policy};

/// The checked program's portable classification at the execution boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionCommandClass {
    /// Preserve arrival order and reject when the queue is full.
    Ordinary,
    /// Retain only the latest queued motion and renew an identical active one.
    MotionLatest,
    /// Retain only the latest queued recovery request.
    SafetyRecovery,
    /// Interrupt active work, clear queued work, and run first.
    Stop,
    /// Interrupt active work, clear queued work, and run first.
    EmergencyStop,
}

/// One opaque host command plus the policy classification supplied by the
/// checked program.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionCommand<T> {
    pub command_id: u32,
    pub class: ExecutionCommandClass,
    pub payload: T,
}

/// The active command facts needed by portable execution arbitration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActiveExecutionCommand<T> {
    pub command_id: u32,
    pub class: ExecutionCommandClass,
    pub payload: T,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionQueueDisposition {
    EnqueuedBack,
    EnqueuedFront,
    RenewedActive,
    SafetyPreempted,
}

/// Exact lifecycle identities returned by a queue clear.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionQueueClear<const N: usize> {
    pub interrupted: [Option<u32>; N],
    pub interrupted_count: usize,
}

impl<const N: usize> ExecutionQueueClear<N> {
    const EMPTY: Self = Self {
        interrupted: [None; N],
        interrupted_count: 0,
    };

    fn record(&mut self, command_id: u32, replacement_id: Option<u32>) {
        if replacement_id == Some(command_id)
            || self.interrupted[..self.interrupted_count].contains(&Some(command_id))
        {
            return;
        }
        let index = self.interrupted_count;
        debug_assert!(
            index < N,
            "queue cannot report more identities than entries"
        );
        self.interrupted[index] = Some(command_id);
        self.interrupted_count += 1;
    }
}

/// Bounded policy result. The host closes the named lifecycles and applies the
/// active-command action while holding its chosen synchronization boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionQueueTransition<const N: usize> {
    pub disposition: ExecutionQueueDisposition,
    /// Stop the current active implementation before dispatching queued work.
    pub preempt_active: bool,
    /// Close this active lifecycle; absent when preemption retains the same
    /// command identity or no active command exists.
    pub interrupted_active: Option<u32>,
    pub interrupted_queued: [Option<u32>; N],
    pub interrupted_queued_count: usize,
    /// The active command whose deadline is renewed without dispatch.
    pub renewed_active: Option<u32>,
}

/// Allocator-free command storage with portable, deterministic arbitration.
///
/// The payload is opaque to Conduit. A host chooses its representation,
/// synchronization, wakeup mechanism, threads, interrupts, and dispatch loop;
/// the checked program supplies each [`ExecutionCommandClass`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionQueue<T, const N: usize>
where
    T: Copy,
{
    entries: [Option<ExecutionCommand<T>>; N],
    head: usize,
    len: usize,
}

impl<T, const N: usize> Default for ExecutionQueue<T, N>
where
    T: Copy,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const N: usize> ExecutionQueue<T, N>
where
    T: Copy,
{
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: [None; N],
            head: 0,
            len: 0,
        }
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    pub const fn capacity(&self) -> usize {
        N
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Remove the next command for host dispatch.
    pub fn pop_front(&mut self) -> Option<ExecutionCommand<T>> {
        if self.len == 0 {
            return None;
        }
        let command = self.entries[self.head].take();
        self.len -= 1;
        if self.len == 0 {
            self.head = 0;
        } else {
            self.head = (self.head + 1) % N;
        }
        command
    }

    /// Clear queued commands and return each distinct accepted lifecycle.
    pub fn clear(&mut self) -> ExecutionQueueClear<N> {
        self.clear_except(None)
    }

    /// Apply the checked execution classification without performing a wake,
    /// dispatch, device effect, or physical safety response.
    ///
    /// On rejection the queue is unchanged. `independent_safety_recovery_active`
    /// is a host observation from the physical safety floor; it is never
    /// inferred from the program or from queued command labels.
    pub fn transition(
        &mut self,
        policy: CommandFlowPolicy<'_>,
        active: Option<ActiveExecutionCommand<T>>,
        independent_safety_recovery_active: bool,
        request: ExecutionCommand<T>,
    ) -> Result<ExecutionQueueTransition<N>, RoboticsReason>
    where
        T: Eq,
    {
        validate_command_flow_policy(policy)?;
        if N != usize::from(policy.maximum_execution_queue) || N == 0 {
            return Err(RoboticsReason::InvalidDescriptor);
        }
        if independent_safety_recovery_active
            && !matches!(
                request.class,
                ExecutionCommandClass::Stop | ExecutionCommandClass::EmergencyStop
            )
        {
            return Err(RoboticsReason::IndependentSafetyRecoveryActive);
        }

        match request.class {
            ExecutionCommandClass::Ordinary => {
                self.require_capacity_after_removing(None)?;
                self.push_back(request);
                Ok(transition(ExecutionQueueDisposition::EnqueuedBack))
            }
            ExecutionCommandClass::MotionLatest => {
                let matching_active = active.is_some_and(|active| {
                    active.class == ExecutionCommandClass::MotionLatest
                        && active.payload == request.payload
                });
                if !matching_active {
                    self.require_capacity_after_removing(Some(
                        ExecutionCommandClass::MotionLatest,
                    ))?;
                }
                let cleared = self.remove_class(
                    ExecutionCommandClass::MotionLatest,
                    Some(request.command_id),
                );
                if matching_active {
                    let active_id = active.map(|active| active.command_id);
                    return Ok(ExecutionQueueTransition {
                        disposition: ExecutionQueueDisposition::RenewedActive,
                        preempt_active: false,
                        interrupted_active: None,
                        interrupted_queued: cleared.interrupted,
                        interrupted_queued_count: cleared.interrupted_count,
                        renewed_active: active_id,
                    });
                }
                let interrupt_active = active
                    .filter(|active| active.class == ExecutionCommandClass::MotionLatest)
                    .and_then(|active| different_id(active.command_id, request.command_id));
                let disposition = if interrupt_active.is_some() {
                    self.push_front(request);
                    ExecutionQueueDisposition::EnqueuedFront
                } else {
                    self.push_back(request);
                    ExecutionQueueDisposition::EnqueuedBack
                };
                Ok(ExecutionQueueTransition {
                    disposition,
                    preempt_active: interrupt_active.is_some()
                        || active.is_some_and(|active| {
                            active.class == ExecutionCommandClass::MotionLatest
                        }),
                    interrupted_active: interrupt_active,
                    interrupted_queued: cleared.interrupted,
                    interrupted_queued_count: cleared.interrupted_count,
                    renewed_active: None,
                })
            }
            ExecutionCommandClass::SafetyRecovery => {
                self.require_capacity_after_removing(Some(ExecutionCommandClass::SafetyRecovery))?;
                let cleared = self.remove_class(
                    ExecutionCommandClass::SafetyRecovery,
                    Some(request.command_id),
                );
                self.push_back(request);
                Ok(ExecutionQueueTransition {
                    disposition: ExecutionQueueDisposition::EnqueuedBack,
                    preempt_active: false,
                    interrupted_active: None,
                    interrupted_queued: cleared.interrupted,
                    interrupted_queued_count: cleared.interrupted_count,
                    renewed_active: None,
                })
            }
            ExecutionCommandClass::Stop | ExecutionCommandClass::EmergencyStop => {
                let cleared = self.clear_except(Some(request.command_id));
                let interrupt_active =
                    active.and_then(|active| different_id(active.command_id, request.command_id));
                self.push_front(request);
                Ok(ExecutionQueueTransition {
                    disposition: ExecutionQueueDisposition::SafetyPreempted,
                    preempt_active: active.is_some(),
                    interrupted_active: interrupt_active,
                    interrupted_queued: cleared.interrupted,
                    interrupted_queued_count: cleared.interrupted_count,
                    renewed_active: None,
                })
            }
        }
    }

    fn require_capacity_after_removing(
        &self,
        removed_class: Option<ExecutionCommandClass>,
    ) -> Result<(), RoboticsReason> {
        let removed = removed_class.map_or(0, |class| {
            self.iter().filter(|command| command.class == class).count()
        });
        if self.len - removed >= N {
            return Err(RoboticsReason::ExecutionQueueFull);
        }
        Ok(())
    }

    fn iter(&self) -> impl Iterator<Item = &ExecutionCommand<T>> {
        (0..self.len).map(move |offset| {
            self.entries[(self.head + offset) % N]
                .as_ref()
                .expect("occupied ring segment contains a command")
        })
    }

    fn remove_class(
        &mut self,
        class: ExecutionCommandClass,
        replacement_id: Option<u32>,
    ) -> ExecutionQueueClear<N> {
        let mut cleared = ExecutionQueueClear::EMPTY;
        let original_len = self.len;
        for _ in 0..original_len {
            let command = self
                .pop_front()
                .expect("bounded removal visits the original queue exactly once");
            if command.class == class {
                cleared.record(command.command_id, replacement_id);
            } else {
                self.push_back(command);
            }
        }
        cleared
    }

    fn clear_except(&mut self, replacement_id: Option<u32>) -> ExecutionQueueClear<N> {
        let mut cleared = ExecutionQueueClear::EMPTY;
        while let Some(command) = self.pop_front() {
            cleared.record(command.command_id, replacement_id);
        }
        cleared
    }

    fn push_back(&mut self, command: ExecutionCommand<T>) {
        debug_assert!(self.len < N, "validated command queue capacity");
        let index = (self.head + self.len) % N;
        self.entries[index] = Some(command);
        self.len += 1;
    }

    fn push_front(&mut self, command: ExecutionCommand<T>) {
        debug_assert!(self.len < N, "validated command queue capacity");
        self.head = (self.head + N - 1) % N;
        self.entries[self.head] = Some(command);
        self.len += 1;
    }
}

fn transition<const N: usize>(
    disposition: ExecutionQueueDisposition,
) -> ExecutionQueueTransition<N> {
    ExecutionQueueTransition {
        disposition,
        preempt_active: false,
        interrupted_active: None,
        interrupted_queued: [None; N],
        interrupted_queued_count: 0,
        renewed_active: None,
    }
}

fn different_id(command_id: u32, replacement_id: u32) -> Option<u32> {
    (command_id != replacement_id).then_some(command_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audited_profile;

    const CAPACITY: usize = 16;

    fn command(command_id: u32, class: ExecutionCommandClass, payload: u8) -> ExecutionCommand<u8> {
        ExecutionCommand {
            command_id,
            class,
            payload,
        }
    }

    fn enqueue(
        queue: &mut ExecutionQueue<u8, CAPACITY>,
        command_id: u32,
        class: ExecutionCommandClass,
        payload: u8,
    ) -> ExecutionQueueTransition<CAPACITY> {
        queue
            .transition(
                audited_profile().command_flow,
                None,
                false,
                command(command_id, class, payload),
            )
            .unwrap()
    }

    #[test]
    fn ordinary_pressure_rejects_without_mutating_the_full_queue() {
        let mut queue = ExecutionQueue::<u8, CAPACITY>::new();
        for id in 0..CAPACITY as u32 {
            enqueue(&mut queue, id, ExecutionCommandClass::Ordinary, id as u8);
        }
        let before = queue;
        assert_eq!(
            queue.transition(
                audited_profile().command_flow,
                None,
                false,
                command(20, ExecutionCommandClass::Ordinary, 20),
            ),
            Err(RoboticsReason::ExecutionQueueFull)
        );
        assert_eq!(queue, before);

        let mut wrong_capacity = ExecutionQueue::<u8, 15>::new();
        assert_eq!(
            wrong_capacity.transition(
                audited_profile().command_flow,
                None,
                false,
                command(21, ExecutionCommandClass::Ordinary, 21),
            ),
            Err(RoboticsReason::InvalidDescriptor)
        );
        assert!(wrong_capacity.is_empty());
    }

    #[test]
    fn latest_motion_preserves_ordinary_order_and_closes_displaced_lifecycles() {
        let mut queue = ExecutionQueue::<u8, CAPACITY>::new();
        for queued in [
            command(1, ExecutionCommandClass::Ordinary, 1),
            command(2, ExecutionCommandClass::MotionLatest, 2),
            command(3, ExecutionCommandClass::Ordinary, 3),
            command(4, ExecutionCommandClass::MotionLatest, 4),
        ] {
            queue.push_back(queued);
        }

        let changed = queue
            .transition(
                audited_profile().command_flow,
                Some(ActiveExecutionCommand {
                    command_id: 5,
                    class: ExecutionCommandClass::MotionLatest,
                    payload: 5,
                }),
                false,
                command(6, ExecutionCommandClass::MotionLatest, 6),
            )
            .unwrap();
        assert_eq!(
            changed.disposition,
            ExecutionQueueDisposition::EnqueuedFront
        );
        assert_eq!(changed.interrupted_active, Some(5));
        assert!(changed.preempt_active);
        assert_eq!(changed.interrupted_queued_count, 2);
        assert_eq!(changed.interrupted_queued[..2], [Some(2), Some(4)]);
        assert_eq!(queue.pop_front().unwrap().command_id, 6);
        assert_eq!(queue.pop_front().unwrap().command_id, 1);
        assert_eq!(queue.pop_front().unwrap().command_id, 3);

        let same_lifecycle = queue
            .transition(
                audited_profile().command_flow,
                Some(ActiveExecutionCommand {
                    command_id: 8,
                    class: ExecutionCommandClass::MotionLatest,
                    payload: 1,
                }),
                false,
                command(8, ExecutionCommandClass::MotionLatest, 2),
            )
            .unwrap();
        assert!(same_lifecycle.preempt_active);
        assert_eq!(same_lifecycle.interrupted_active, None);
    }

    #[test]
    fn matching_active_motion_renews_without_dispatch_or_lifecycle_transfer() {
        let mut queue = ExecutionQueue::<u8, CAPACITY>::new();
        enqueue(&mut queue, 7, ExecutionCommandClass::MotionLatest, 7);
        let renewed = queue
            .transition(
                audited_profile().command_flow,
                Some(ActiveExecutionCommand {
                    command_id: 41,
                    class: ExecutionCommandClass::MotionLatest,
                    payload: 9,
                }),
                false,
                command(42, ExecutionCommandClass::MotionLatest, 9),
            )
            .unwrap();
        assert_eq!(
            renewed.disposition,
            ExecutionQueueDisposition::RenewedActive
        );
        assert_eq!(renewed.renewed_active, Some(41));
        assert!(!renewed.preempt_active);
        assert_eq!(renewed.interrupted_queued[..1], [Some(7)]);
        assert!(queue.is_empty());
    }

    #[test]
    fn stop_preempts_active_and_every_distinct_queued_lifecycle() {
        let mut queue = ExecutionQueue::<u8, CAPACITY>::new();
        enqueue(&mut queue, 10, ExecutionCommandClass::Ordinary, 1);
        enqueue(&mut queue, 10, ExecutionCommandClass::Ordinary, 2);
        enqueue(&mut queue, 11, ExecutionCommandClass::SafetyRecovery, 3);
        let stopped = queue
            .transition(
                audited_profile().command_flow,
                Some(ActiveExecutionCommand {
                    command_id: 9,
                    class: ExecutionCommandClass::Ordinary,
                    payload: 0,
                }),
                true,
                command(12, ExecutionCommandClass::Stop, 4),
            )
            .unwrap();
        assert_eq!(
            stopped.disposition,
            ExecutionQueueDisposition::SafetyPreempted
        );
        assert_eq!(stopped.interrupted_active, Some(9));
        assert!(stopped.preempt_active);
        assert_eq!(stopped.interrupted_queued_count, 2);
        assert_eq!(stopped.interrupted_queued[..2], [Some(10), Some(11)]);
        assert_eq!(queue.len(), 1);
        assert_eq!(queue.pop_front().unwrap().command_id, 12);
    }

    #[test]
    fn independent_recovery_blocks_program_commands_but_not_emergency_stop() {
        let mut queue = ExecutionQueue::<u8, CAPACITY>::new();
        assert_eq!(
            queue.transition(
                audited_profile().command_flow,
                None,
                true,
                command(20, ExecutionCommandClass::SafetyRecovery, 1),
            ),
            Err(RoboticsReason::IndependentSafetyRecoveryActive)
        );
        let emergency = queue
            .transition(
                audited_profile().command_flow,
                None,
                true,
                command(21, ExecutionCommandClass::EmergencyStop, 2),
            )
            .unwrap();
        assert_eq!(
            emergency.disposition,
            ExecutionQueueDisposition::SafetyPreempted
        );
    }
}
