//! Local selected native Mask custody for the ordinary Crèche Body operation.
//! The main thread owns one renderer and scanout; worker-visible state contains
//! only bounded actual receipts. This does not acquire a display or start Play.
use crate::creche_encounter::{CrecheEncounterController, CrecheMaskShowCustody};
use conduit_body::{BodyPlan, BodyPlayIdentity, ResidentPlot};
use conduit_face::{Face, MaskInteractionCorrelation, MaskShow};
use conduit_human::KeyEvent;
use conduit_std_host::hosted_creche::CrecheExecutionContext;
use conduitos::{
    display::PixelTarget,
    native_compositor::CompositionReceipt,
    native_face_mask::{NativeFaceMask, NativeFaceMaskInput, NativeMaskPlayReceipt},
};
use std::{sync::Arc, time::Duration};
mod custody;
use custody::{NativeCustody, Shared};

/// The caller supplies actual initialized display custody. A CPU surface alone
/// is not a production scanout. Buffered targets must commit actual presentation
/// in PixelTarget::commit_frame before the Mask can acknowledge its Show.
pub trait LocalCrecheScanout: PixelTarget {
    /// Check the actual retained output lifetime before publication or input.
    fn require_output_live(&self) -> Result<(), String>;
    /// Revoke publication/input and drain this exact selected output. Idempotent.
    fn revoke_and_drain(&mut self) -> Result<(), String>;
}
impl<T: LocalCrecheScanout + ?Sized> LocalCrecheScanout for &mut T {
    fn require_output_live(&self) -> Result<(), String> {
        (**self).require_output_live()
    }
    fn revoke_and_drain(&mut self) -> Result<(), String> {
        (**self).revoke_and_drain()
    }
}

#[derive(Clone, Copy)]
pub struct LocalCrecheNativeLimits {
    pub maximum_presentations: u32,
    pub maximum_input_events: u32,
    pub retirement_timeout: Duration,
}
impl LocalCrecheNativeLimits {
    fn valid(self) -> bool {
        self.maximum_presentations > 0
            && self.maximum_presentations <= 4096
            && self.maximum_input_events > 0
            && self.maximum_input_events <= 65536
            && self.retirement_timeout >= Duration::from_millis(1)
            && self.retirement_timeout <= Duration::from_secs(10)
    }
}
struct Execution {
    plan: BodyPlan,
    play: BodyPlayIdentity,
    plot: ResidentPlot,
}
/// Main-thread owner. It must keep pumping service_retirement while a Body
/// worker can request Stop. Joining that worker first can prevent its drain ACK.
pub struct LocalCrecheNativeOwner<D: LocalCrecheScanout> {
    mask: NativeFaceMask,
    display: D,
    shared: Arc<Shared>,
    execution: Option<Execution>,
    limits: LocalCrecheNativeLimits,
    presentations: u32,
    inputs: u32,
    next_sequence: u64,
}
impl<D: LocalCrecheScanout> LocalCrecheNativeOwner<D> {
    /// Accept only an already prepared, unused Mask and actual initialized
    /// scanout. Preparing/admitting the Linux Show Base remains with its Host;
    /// no synthetic provider or firmware framebuffer identity is created here.
    pub fn new(
        mask: NativeFaceMask,
        mut display: D,
        limits: LocalCrecheNativeLimits,
    ) -> Result<(Self, Box<dyn CrecheMaskShowCustody>), String> {
        if !limits.valid()
            || mask.show().is_some()
            || mask.scene().is_some()
            || mask.render_receipt().is_some()
        {
            display.revoke_and_drain()?;
            return Err("native Creche requires unused selected Mask and finite limits".into());
        }
        display.require_output_live()?;
        if let Err(error) = display.format().validate() {
            display.revoke_and_drain()?;
            return Err(error.as_str().to_owned());
        }
        let shared = Arc::new(Shared::new(limits.retirement_timeout));
        let custody = Box::new(NativeCustody(shared.clone()));
        Ok((
            Self {
                mask,
                display,
                shared,
                execution: None,
                limits,
                presentations: 0,
                inputs: 0,
                next_sequence: 1,
            },
            custody,
        ))
    }
    /// Called by the trusted execution owner with its actually issued matching
    /// Play. A prospective context alone cannot grant presentation authority.
    pub fn bind_execution(
        &mut self,
        context: &CrecheExecutionContext,
        issued: &BodyPlayIdentity,
    ) -> Result<(), String> {
        self.require_live()?;
        if self.execution.is_some()
            || context.play() != issued
            || !issued.validate_for(context.plan())
            || context.plan().verify_seal().is_err()
        {
            return Err("native Creche lacks exact issued execution".into());
        }
        let basis = self.mask.birth_basis("conduit/creche/local-mask-scope");
        if basis.host_id != context.host().host_id || basis.boot_id != context.host().boot_id {
            return Err("native Creche Mask belongs to another current Host/Boot".into());
        }
        self.execution = Some(Execution {
            plan: context.plan().clone(),
            play: issued.clone(),
            plot: context.plot().clone(),
        });
        Ok(())
    }
    /// Publish only the adapter's actual current Face, never a caller-built ACK.
    pub fn present_current(
        &mut self,
        controller: &CrecheEncounterController,
    ) -> Result<bool, String> {
        // Retirement may arrive between the event loop's service pass and
        // publication. Complete its actual Mask/scanout drain rather than
        // treating an expected terminal transition as a renderer failure.
        if self.service_retirement()? {
            return Ok(false);
        }
        if let Err(error) = self.require_live() {
            if self.service_retirement()? {
                return Ok(false);
            }
            return Err(error);
        }
        let Some(face) = controller.face()? else {
            return Ok(false);
        };
        if self.shared.same_face(&face)? {
            return Ok(false);
        }
        if self.presentations >= self.limits.maximum_presentations {
            return self.fail("native Creche presentation bound exhausted");
        }
        self.shared.withdraw()?;
        let sequence = self.sequence()?;
        let execution = self
            .execution
            .as_ref()
            .ok_or("native Creche issued execution absent")?;
        let composition = match self.mask.present_plot_owner(
            (*face).clone(),
            &execution.plan,
            &execution.play,
            &execution.plot,
            sequence,
            sequence,
            &mut self.display,
        ) {
            Ok(receipt) => receipt,
            Err(error) => return self.fail(error.as_str()),
        };
        self.presentations += 1;
        if let Err(error) = self.record_presented(face.clone(), composition) {
            if self.service_retirement()? {
                return Ok(false);
            }
            return self.fail(&error);
        }
        let show = self
            .mask
            .show()
            .ok_or("native Creche completed Show absent")?
            .clone();
        self.acknowledge_presented(controller, &face, show)
    }

    fn acknowledge_presented(
        &mut self,
        controller: &CrecheEncounterController,
        rendered: &Face,
        show: MaskShow,
    ) -> Result<bool, String> {
        if let Err(error) = controller.acknowledge_show(show) {
            // A make/save worker may publish a replacement while this thread
            // renders. The old Show remains stale and is never acknowledged.
            // Withdraw its input custody and lease before the next UI pump
            // realizes the actual current Face. Same-Face custody errors still
            // revoke the owner rather than disguising a failed acknowledgement.
            if controller.face()?.as_deref() != Some(rendered) {
                self.shared.withdraw()?;
                self.mask.suspend().map_err(|e| e.as_str().to_owned())?;
                return Ok(false);
            }
            return self.fail(&error);
        }
        Ok(true)
    }
    /// Native input leaves only through the selected Mask's typed return Fore.
    pub fn key(
        &mut self,
        controller: &CrecheEncounterController,
        event: KeyEvent,
    ) -> Result<bool, String> {
        self.require_live()?;
        if self.inputs >= self.limits.maximum_input_events {
            return self.fail("native Creche input bound exhausted");
        }
        if self.shared.has_returned()? {
            return Err("native Creche waits for queued semantic consumption".into());
        }
        let show = self
            .mask
            .show()
            .ok_or("native Creche has no acknowledged current Show")?
            .clone();
        let face = self.shared.face()?;
        let render = self
            .mask
            .render_receipt()
            .ok_or("native Creche renderer receipt absent")?
            .clone();
        let sequence = self.sequence()?;
        self.inputs += 1;
        match self.mask.key(event, sequence, &mut self.display) {
            Ok(NativeFaceMaskInput::Unchanged) => Ok(false),
            Ok(NativeFaceMaskInput::Redrawn(composition)) => {
                if let Err(error) = self.record_presented(face, composition) {
                    return self.fail(&error);
                }
                let show = self
                    .mask
                    .show()
                    .ok_or("native Creche redrawn Show absent")?
                    .clone();
                if let Err(error) = controller.acknowledge_show(show) {
                    return self.fail(&error);
                }
                Ok(false)
            }
            Ok(NativeFaceMaskInput::Submitted { correlation, .. }) => {
                if let Err(error) =
                    self.shared
                        .record_returned(&face, &show, &render, correlation.clone())
                {
                    return self.fail(&error);
                }
                if let Err(error) = controller.submit(correlation.interaction) {
                    return self.fail(&error);
                }
                Ok(true)
            }
            Err(error) => self.fail(error.as_str()),
        }
    }
    /// Service from the UI event loop before waiting/joining any Body worker.
    pub fn service_retirement(&mut self) -> Result<bool, String> {
        if !self.shared.retirement_requested()? {
            return Ok(false);
        }
        self.retire()?;
        Ok(true)
    }
    pub fn retire(&mut self) -> Result<(), String> {
        if matches!(self.shared.is_drained(), Ok(true)) {
            return Ok(());
        }
        let revocation = self.shared.revoke();
        let mask = self.mask.suspend().map_err(|e| e.as_str().to_owned());
        let display = self.display.revoke_and_drain();
        let result = revocation.and(mask).and(display);
        self.shared
            .complete_retirement(result.as_ref().err().cloned())?;
        result
    }
    fn require_live(&self) -> Result<(), String> {
        self.shared.require_live()?;
        self.display.require_output_live()
    }
    fn sequence(&mut self) -> Result<u64, String> {
        let sequence = self.next_sequence;
        self.next_sequence = sequence
            .checked_add(1)
            .ok_or("native Creche sequence exhausted")?;
        Ok(sequence)
    }
    fn record_presented(
        &mut self,
        face: Arc<Face>,
        composition: CompositionReceipt,
    ) -> Result<(), String> {
        let show = self
            .mask
            .show()
            .ok_or("native Creche completed Show absent")?
            .clone();
        let render = self
            .mask
            .render_receipt()
            .ok_or("native Creche completed renderer receipt absent")?
            .clone();
        self.shared
            .record_presented(face, show, render, composition)
    }
    fn fail<T>(&mut self, reason: &str) -> Result<T, String> {
        match self.retire() {
            Ok(()) => Err(reason.to_owned()),
            Err(cleanup) => Err(format!("{reason}; native Creche drain: {cleanup}")),
        }
    }
}
impl<D: LocalCrecheScanout> Drop for LocalCrecheNativeOwner<D> {
    fn drop(&mut self) {
        let _ = self.retire();
    }
}

#[cfg(test)]
mod tests;

/// SDL commit is synchronous on the owning thread. Revoking this prepared
/// scanout prevents further publication; no independent firmware provider exists.
#[cfg(all(target_os = "linux", feature = "native-creche-sdl"))]
impl LocalCrecheScanout for conduit_linux_display::Scanout<'_> {
    fn require_output_live(&self) -> Result<(), String> {
        conduitos::product_bases::InitializedNativeOutput::require_live(self)
            .map_err(|error| error.as_str().to_owned())
    }
    fn revoke_and_drain(&mut self) -> Result<(), String> {
        self.lose();
        Ok(())
    }
}

#[cfg(all(target_os = "linux", feature = "native-creche-sdl"))]
mod linux;
#[cfg(all(target_os = "linux", feature = "native-creche-sdl"))]
pub use linux::prepare_linux_creche_output;

#[cfg(all(target_os = "linux", feature = "native-creche-sdl"))]
mod linux_pump;
#[cfg(all(target_os = "linux", feature = "native-creche-sdl"))]
pub use linux_pump::{LinuxCrechePumpError, LinuxCrecheTurn};
