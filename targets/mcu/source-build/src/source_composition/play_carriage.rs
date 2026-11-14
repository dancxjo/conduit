// Exactly one framed reader owns the original platform wrapper. Line mode is
// entered only with the observer/actor returned by Root's checked activation;
// it never shares that loan with diagnostic/control IO or bypasses BOOTSEL.
use conduit_mcu_host_runtime::{
    fore_bridge::{ForeObserver, ObserverPort, ObserverRefusal},
    root::ForeIoActor,
};
pub enum Carriage<'io, 'bridge, 'pins, 'control, 'state> {
    Local {
        link: &'io mut crate::transport::Link,
        control: Option<ReturnControl<'control, 'state>>,
    },
    Line {
        actor: ForeIoActor<'io, 'bridge, 'pins, crate::transport::Link>,
        observer: ObserverPort<'io, 'bridge, 'pins>,
    },
}
pub(super) enum Io<'io, 'bridge, 'pins, 'control, 'state> {
    Local {
        link: &'io mut crate::transport::Link,
        control: Option<ReturnControl<'control, 'state>>,
    },
    Line(ForeIoActor<'io, 'bridge, 'pins, crate::transport::Link>),
}
impl<'io, 'bridge, 'pins, 'control, 'state> Carriage<'io, 'bridge, 'pins, 'control, 'state> {
    pub(super) fn split(
        self,
    ) -> (
        Io<'io, 'bridge, 'pins, 'control, 'state>,
        Option<ObserverPort<'io, 'bridge, 'pins>>,
    ) {
        match self {
            Self::Local { link, control } => (Io::Local { link, control }, None),
            Self::Line { actor, observer } => (Io::Line(actor), Some(observer)),
        }
    }
}
impl Io<'_, '_, '_, '_, '_> {
    pub(super) async fn run(
        self,
        inspection: &State<3>,
        face: &conduit_environment_runtime::publication_face_custody::LocalPublicationFace,
        activity: &conduit_mcu_host_runtime::native_runner::NativeActivity,
        arena: &ImageMemoryLease<'_>,
    ) -> ! {
        match self {
            Self::Local { link, mut control } => {
                let mut response = inspection_buffer();
                loop {
                    core::hint::black_box(arena);
                    link.connected().await;
                    let _ = control::serve(
                        inspection,
                        face,
                        link,
                        &mut response,
                        &mut control,
                        activity,
                    )
                    .await;
                    link.reset();
                    embassy_time::Timer::after_millis(1).await;
                }
            }
            Self::Line(mut actor) => {
                let yield_to_host = Yield;
                loop {
                    core::hint::black_box(arena);
                    if actor.pump(&yield_to_host).await.is_err() {
                        // Retire only this bridge; independent native runners,
                        // undelivered Values and the original loan remain owned.
                        actor.retire();
                        loop {
                            core::hint::black_box(&actor);
                            core::hint::black_box(arena);
                            embassy_time::Timer::after_secs(1).await;
                        }
                    }
                    yield_to_host.pause().await;
                }
            }
        }
    }
}
pub(super) enum SampleObserverMode<'a, 'io, 'bridge, 'pins> {
    Local(SampleObserver<'a>),
    Line(ForeObserver<'io, 'bridge, 'pins, SampleObserver<'a>>),
}
impl<'a, 'io, 'bridge, 'pins> SampleObserverMode<'a, 'io, 'bridge, 'pins> {
    pub(super) fn new(
        existing: SampleObserver<'a>,
        port: Option<ObserverPort<'io, 'bridge, 'pins>>,
    ) -> Self {
        match port {
            Some(port) => Self::Line(ForeObserver::new(existing, port)),
            None => Self::Local(existing),
        }
    }
}
impl<'r, 'f> Observer<crate::play::SamplingKernel<'r, 'f>> for SampleObserverMode<'_, '_, '_, '_> {
    type Error = ObserverRefusal<observers::BridgeRefusal>;
    fn before_step(
        &mut self,
        kernel: &mut crate::play::SamplingKernel<'r, 'f>,
    ) -> Result<(), Self::Error> {
        match self {
            Self::Local(observer) => observer
                .before_step(kernel)
                .map_err(ObserverRefusal::Existing),
            Self::Line(observer) => observer.before_step(kernel),
        }
    }
    fn completed(
        &mut self,
        kernel: &mut crate::play::SamplingKernel<'r, 'f>,
        dispatch: Dispatch,
    ) -> Result<(), Self::Error> {
        match self {
            Self::Local(observer) => observer
                .completed(kernel, dispatch)
                .map_err(ObserverRefusal::Existing),
            Self::Line(observer) => observer.completed(kernel, dispatch),
        }
    }
}
