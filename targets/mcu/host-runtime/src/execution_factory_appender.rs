//! Add one actual prepared native append owner to the original driver inventory.
use super::*;
use conduit_resource_snapshot::{
    append::MAXIMUM_RECORD_PAYLOAD_BYTES,
    native_append::{PreparedAssignedAppender, RecordAppendHostCall},
    nor::NorStorage,
};

/// Original Calls remain available alongside the exact additional native owner.
pub struct CallsWithAppender<B, S> {
    pub base: B,
    pub appender: RecordAppendHostCall<S>,
}

/// Affine prepared inventory. Only the checked constructor can reserve its gap.
pub struct PreparedWithAppender<'a, B, S> {
    base: B,
    appender: PreparedAssignedAppender<'a, S>,
    backing: usize,
}

impl<
        'a,
        'o,
        'f,
        R: PreparedResidents<E>,
        P: PulseReadProvider,
        T: OpenDrain,
        W: MonotonicWaitProvider,
        const E: usize,
        const N: usize,
        const A: usize,
    > PreparedDrivers<'a, 'o, 'f, R, P, T, W, E, N, A>
{
    /// Prepare a complete unique mapping including the ticket's exact node.
    /// No owner is bound, allocation made, or physical effect performed here.
    pub fn with_appender<'s, S: NorStorage>(
        residents: R,
        resident_nodes: [Option<u16>; E],
        software: PreparedSoftware<'o, 'f, N>,
        native: PreparedCalls<'a, P, T, W, A>,
        appender: PreparedAssignedAppender<'s, S>,
    ) -> Result<PreparedWithAppender<'s, Self, S>, &'static str> {
        let extra_backing = appender
            .retained_backing_bytes()
            .ok_or("appender-backing")?;
        let base = Self::new_with_extra(
            residents,
            resident_nodes,
            software,
            native,
            Some(appender.node().0),
        )?;
        let backing = base.backing.checked_add(extra_backing).ok_or("backing")?;
        Ok(PreparedWithAppender {
            base,
            appender,
            backing,
        })
    }
}

// SAFETY: the only constructor checks the complete unique original node mapping,
// charges all backing, and retains the actual opaque ticket until binding.
// Binding consumes only those owners and performs no allocation or media effect.
unsafe impl<
        'a,
        'o,
        'f,
        's,
        R: PreparedResidents<E>,
        P: PulseReadProvider,
        T: OpenDrain,
        W: MonotonicWaitProvider,
        S: NorStorage,
        const E: usize,
        const N: usize,
        const A: usize,
        const PORTS: usize,
    > PreparedBacks<N, PORTS>
    for PreparedWithAppender<'s, PreparedDrivers<'a, 'o, 'f, R, P, T, W, E, N, A>, S>
where
    R::Back: StepBack<PORTS>,
{
    type Driver = Driver<'f, R::Back>;
    type Calls = CallsWithAppender<Calls<P, T, W, A>, S>;
    fn retained_backing_bytes(&self) -> Option<usize> {
        Some(self.backing)
    }
    fn bind(self) -> ([Self::Driver; N], Self::Calls) {
        let node = usize::from(self.appender.node().0);
        let (mut drivers, base) = self.base.bind_slots::<PORTS>();
        debug_assert!(drivers[node].is_none());
        drivers[node] = Some(Driver::Native(HostCallBack::new(
            MAXIMUM_RECORD_PAYLOAD_BYTES as u32,
        )));
        let appender = self.appender.bind();
        (
            drivers.map(|driver| driver.expect("complete checked append inventory")),
            CallsWithAppender { base, appender },
        )
    }
}

/// Original Calls remain available alongside the exact additional native owner.
pub struct CallsWithOptionalAppender<B, S> {
    pub base: B,
    pub appender: Option<RecordAppendHostCall<S>>,
}

/// Affine prepared inventory. Only the checked constructor can reserve its gap.
pub struct PreparedWithOptionalAppender<'a, B, S> {
    base: B,
    appender: Option<PreparedAssignedAppender<'a, S>>,
    backing: usize,
}

impl<
        'a,
        'o,
        'f,
        R: PreparedResidents<E>,
        P: PulseReadProvider,
        T: OpenDrain,
        W: MonotonicWaitProvider,
        const E: usize,
        const N: usize,
        const A: usize,
    > PreparedDrivers<'a, 'o, 'f, R, P, T, W, E, N, A>
{
    /// Prepare a complete unique mapping including the ticket's exact node.
    /// No owner is bound, allocation made, or physical effect performed here.
    pub fn with_optional_appender<'s, S: NorStorage>(
        residents: R,
        resident_nodes: [Option<u16>; E],
        software: PreparedSoftware<'o, 'f, N>,
        native: PreparedCalls<'a, P, T, W, A>,
        appender: Option<PreparedAssignedAppender<'s, S>>,
    ) -> Result<PreparedWithOptionalAppender<'s, Self, S>, &'static str> {
        let extra_backing = match &appender {
            Some(ticket) => ticket.retained_backing_bytes().ok_or("appender-backing")?,
            None => 0,
        };
        let base = Self::new_with_extra(
            residents,
            resident_nodes,
            software,
            native,
            appender.as_ref().map(|ticket| ticket.node().0),
        )?;
        let backing = base.backing.checked_add(extra_backing).ok_or("backing")?;
        Ok(PreparedWithOptionalAppender {
            base,
            appender,
            backing,
        })
    }
}

// SAFETY: the only constructor checks the complete unique original node mapping,
// charges all backing, and retains the actual opaque ticket until binding.
// Binding consumes only those owners and performs no allocation or media effect.
unsafe impl<
        'a,
        'o,
        'f,
        's,
        R: PreparedResidents<E>,
        P: PulseReadProvider,
        T: OpenDrain,
        W: MonotonicWaitProvider,
        S: NorStorage,
        const E: usize,
        const N: usize,
        const A: usize,
        const PORTS: usize,
    > PreparedBacks<N, PORTS>
    for PreparedWithOptionalAppender<'s, PreparedDrivers<'a, 'o, 'f, R, P, T, W, E, N, A>, S>
where
    R::Back: StepBack<PORTS>,
{
    type Driver = Driver<'f, R::Back>;
    type Calls = CallsWithOptionalAppender<Calls<P, T, W, A>, S>;
    fn retained_backing_bytes(&self) -> Option<usize> {
        Some(self.backing)
    }
    fn bind(self) -> ([Self::Driver; N], Self::Calls) {
        let (mut drivers, base) = self.base.bind_slots::<PORTS>();
        let appender = self.appender.map(|ticket| {
            let node = usize::from(ticket.node().0);
            debug_assert!(drivers[node].is_none());
            drivers[node] = Some(Driver::Native(HostCallBack::new(
                MAXIMUM_RECORD_PAYLOAD_BYTES as u32,
            )));
            ticket.bind()
        });
        (
            drivers.map(|driver| driver.expect("complete checked append inventory")),
            CallsWithOptionalAppender { base, appender },
        )
    }
}
