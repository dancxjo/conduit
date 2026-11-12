// Sampling alone retains the original carrier; Mask/output and renderer remain
// independent owned members of the SAME complete admitted/Started workset.
use super::*;
#[cfg(fore_carrier)]
use conduit_mcu_host_runtime::{
    fore_bridge::BoundForeBridge,
    root::{BoundForeCarrierKernels, BoundForeSessions, ForeCarrierKernels},
};
use conduit_mcu_host_runtime::{
    fore_bridge::PreparedForeBridge,
    root::{PreparedForeCarrier, PreparedForeSessions},
};
#[cfg(fore_carrier)]
pub mod installation {
    include!(concat!(
        env!("OUT_DIR"),
        "/source_play_fore_installation.rs"
    ));
}
#[derive(Clone, Copy)]
pub struct Selection<'p> {
    pub profile: c::AssignedLineForeProfile<'p>,
    pub bindings: &'p [c::AssignedRemoteBinding],
}
pub struct Components<'a, 'p> {
    pub carrier: PreparedForeCarrier<'a, 'p, crate::transport::Link, FORE_PORTS>,
    pub bridge: PreparedForeBridge<'a, 'p>,
    pub sessions: PreparedForeSessions,
}
pub enum PreparedSampling<'a, 'p, S: PreparedLocalKernels> {
    Local(S, core::marker::PhantomData<(&'a (), &'p ())>),
    #[cfg(fore_carrier)]
    Line(
        ForeCarrierKernels<
            'a,
            'p,
            crate::transport::Link,
            PreparedKernelGroup<S, PreparedForeBridge<'a, 'p>, PreparedForeSessions>,
            FORE_PORTS,
        >,
    ),
}
pub enum BoundSampling<'a, 'p, S> {
    Local(S, core::marker::PhantomData<(&'a (), &'p ())>),
    #[cfg(fore_carrier)]
    Line(
        BoundForeCarrierKernels<
            'a,
            'p,
            crate::transport::Link,
            (S, BoundForeBridge<'a, 'p>, BoundForeSessions),
            FORE_PORTS,
        >,
    ),
}
impl<S> BoundSampling<'_, '_, S> {
    pub fn into_local(self) -> Result<S, &'static str> {
        match self {
            Self::Local(sample, _) => Ok(sample),
            #[cfg(fore_carrier)]
            Self::Line(_) => Err("Line continuation unsupported"),
        }
    }
}
impl<'a, 'p> Components<'a, 'p> {
    pub fn wrap<S: PreparedLocalKernels>(self, sample: S) -> PreparedSampling<'a, 'p, S> {
        #[cfg(fore_carrier)]
        {
            PreparedSampling::Line(self.carrier.attach(PreparedKernelGroup(
                sample,
                self.bridge,
                self.sessions,
            )))
        }
        #[cfg(not(fore_carrier))]
        {
            let _ = (self, sample);
            panic!("Line carrier not installed")
        }
    }
}
// SAFETY: both variants preserve the original opaque workset's sole bind/Start.
// The wrapper counts its inline padding/discriminant, without cloning owners.
unsafe impl<'a, 'p, S: PreparedLocalKernels> PreparedLocalKernels for PreparedSampling<'a, 'p, S> {
    type Bound = BoundSampling<'a, 'p, S::Bound>;
    fn retained_backing_bytes(&self) -> Option<usize> {
        match self {
            Self::Local(s, _) => s
                .retained_backing_bytes()?
                .checked_add(core::mem::size_of::<Self>().checked_sub(core::mem::size_of::<S>())?),
            #[cfg(fore_carrier)]
            Self::Line(s) => s
                .retained_backing_bytes()?
                .checked_add(core::mem::size_of::<Self>().checked_sub(core::mem::size_of_val(s))?),
        }
    }
    unsafe fn bind(self) -> Self::Bound {
        match self {
            Self::Local(s, _) => {
                BoundSampling::Local(unsafe { s.bind() }, core::marker::PhantomData)
            }
            #[cfg(fore_carrier)]
            Self::Line(s) => BoundSampling::Line(unsafe { s.bind() }),
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn prepare_kernel<B: conduit_mcu_host_runtime::execution::PreparedBacks<SN, SP>>(
    approved: &b::DelegatedBodyFragment<'_>,
    profile: b::AssignedBodyPreparationProfile<'_>,
    all_fore: &[c::AssignedForeRequirement],
    line: Option<c::AssignedLineForeProfile<'_>>,
    values: HostedValueStore,
    maximum: usize,
    backs: B,
) -> Result<PreparedFragment<B, SN, SC, SP, SR, ST, SH, SS, HostedSignLog, SK>, Refusal<&'static str>>
{
    let layout = ResidentLayout {
        records: source::sampling_layout::RECORDS,
        counts: source::sampling_layout::COUNTS,
    };
    let signs = HostedSignLog::new(
        SS as u16,
        (SS * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32,
    )
    .map_err(|_| Refusal::Bounds)?;
    match line {
        Some(line_fore) => PreparedFragment::prepare_with_line_fore_terminal_transductions_ref(
            approved,
            b::AssignedBodyLineForePreparationProfile {
                preparation: profile,
                line_fore,
            },
            &layout,
            values,
            signs,
            maximum,
            || Ok::<_, &'static str>(backs),
            &SAMPLING_TERMINALS,
        ),
        None => PreparedFragment::prepare_with_terminal_transductions_ref(
            approved,
            profile,
            all_fore,
            &layout,
            values,
            signs,
            maximum,
            || Ok::<_, &'static str>(backs),
            &SAMPLING_TERMINALS,
        ),
    }
}

#[cfg(fore_carrier)]
pub struct Buffers {
    pub frame: alloc::vec::Vec<u8>,
    pub ingress: alloc::vec::Vec<u8>,
    pub egress: alloc::vec::Vec<u8>,
}
#[cfg(fore_carrier)]
pub fn buffers() -> Result<Buffers, &'static str> {
    let bounds = crate::config::FORE_CARRIER_BOUNDS.ok_or("missing-installed-fore")?;
    fn funded(bytes: usize) -> Result<alloc::vec::Vec<u8>, &'static str> {
        let mut value = alloc::vec::Vec::new();
        value
            .try_reserve_exact(bytes)
            .map_err(|_| "fore-buffer-capacity")?;
        if value.capacity() != bytes {
            return Err("fore-buffer-capacity");
        }
        value.resize(bytes, 0);
        Ok(value)
    }
    Ok(Buffers {
        frame: funded(bounds.maximum_buffered_bytes)?,
        ingress: funded(conduit_core::COUNT_ENCODED_LEN)?,
        egress: funded(conduit_resource_snapshot::read_result::READ_RESULT_BYTES)?,
    })
}

#[cfg(fore_carrier)]
#[allow(clippy::too_many_arguments)]
pub fn prepare_components<'a, 'p>(
    parts: &conduit_mcu_host_runtime::root::ExecutionParts<
        '_,
        '_,
        { crate::config::MAX_ASSIGNMENT_BYTES },
    >,
    context: b::NativePreparationContext<'_>,
    hot: &'p installation::Hot,
    cold: installation::Cold,
    link: &'a mut crate::transport::Link,
    buffers: &'a mut Buffers,
    approved: &b::DelegatedBodyFragment<'_>,
    fore: &b::ApprovedBodyForePorts<FORE_PORTS>,
    profile: b::AssignedBodyPreparationProfile<'_>,
) -> Result<Components<'a, 'p>, &'static str> {
    let (ingress, egress) = hot.specs()?;
    // SAFETY: the explicit checked Host Base selects THIS initialized original
    // BSP serial wrapper and the operator-established physical custody mapping
    // to the independently retained SeedOwner peer. Native UART/USB framed
    // receive is cancellation-safe with partial framing retained. This is no
    // network authentication or confidentiality claim and never frame-derived.
    let selected = unsafe {
        conduit_mcu_host_runtime::root::NativeForeCarrierSelection::from_native_custody(
            link,
            &mut buffers.frame,
            parts.configuration.image_id,
            parts.configuration.configuration_id,
            &parts.bootstrap.owner,
            hot.profile(),
            &cold.lines,
            &cold.session,
            installation::SESSION_CONTRACT,
            crate::config::FORE_CARRIER_BOUNDS.ok_or("missing-installed-fore")?,
        )
    }
    .map_err(|_| "fore-native-installation")?;
    let profile = b::AssignedBodyLineForePreparationProfile {
        preparation: profile,
        line_fore: hot.profile(),
    };
    let carrier = parts
        .prepare_fore_carrier::<_, 3, FORE_PORTS>(
            context.now_millis,
            Some(selected),
            approved,
            fore,
            profile,
        )
        .map_err(|_| "fore-original-custody")?;
    let bridge = PreparedForeBridge::prepare(
        *carrier.custody_digest(),
        ingress,
        egress,
        &mut buffers.ingress,
        &mut buffers.egress,
    )
    .map_err(|_| "fore-bridge")?;
    let sessions = carrier
        .prepare_bridge_sessions(&bridge, ingress, egress)
        .map_err(|_| "fore-sessions")?;
    drop(cold); // Retire full Lines/session strings before native/kernel funding.
    Ok(Components {
        carrier,
        bridge,
        sessions,
    })
}

#[allow(clippy::too_many_arguments)]
pub async fn run_started<'a, 'f>(
    sampling: BoundSampling<'a, 'a, SamplingBound<'a, 'f>>,
    mask: conduit_seven_segment_mask::native_kernel::PreparedAssignedMaskKernel,
    output: OutputBound<'a, 'f>,
    renderer: conduit_mcu_host_runtime::execution_mask::RendererService<'_, crate::renderer::Clock>,
    optional: conduit_kernel::scheduler::prepared_session::OptionalOutputStorage<6>,
    scopes: crate::play_service::Scopes,
    evidence: conduit_mcu_host_runtime::execution_workset::StartedEvidence,
    arena: conduit_mcu_host_runtime::image_memory::ImageMemoryLease<'_>,
    face: &conduit_environment_runtime::publication_face_custody::LocalPublicationFace,
    local_link: Option<&mut crate::transport::Link>,
    activity: &conduit_mcu_host_runtime::native_runner::NativeActivity,
    append_output: &mut [u8],
    append_status: conduit_mcu_host_runtime::execution_inspection::PreparedAppendStatus,
) -> ! {
    let (output, output_calls) = output;
    macro_rules! run {
        ($sample:expr,$calls:expr,$carriage:expr) => {
            crate::play_run::run(
                $sample,
                $calls,
                mask,
                output,
                output_calls,
                renderer,
                optional,
                scopes,
                evidence,
                arena,
                face,
                $carriage,
                activity,
                append_output,
                append_status,
            )
            .await
        };
    }
    #[cfg(fore_carrier)]
    let mut sampling = sampling;
    match sampling {
        BoundSampling::Local((mut sample, mut calls), _) => run!(
            &mut *sample,
            &mut calls,
            crate::play_run::Carriage::Local {
                link: local_link.expect("original local carrier"),
                control: None
            }
        ),
        #[cfg(fore_carrier)]
        BoundSampling::Line(ref mut bound) => {
            assert!(local_link.is_none(), "exclusive original Line reader");
            let ((sample, calls), observer, actor) = bound
                .activate_actor(&evidence)
                .expect("original preflighted Fore Start");
            run!(
                &mut **sample,
                calls,
                crate::play_run::Carriage::Line { actor, observer }
            )
        }
    }
}
