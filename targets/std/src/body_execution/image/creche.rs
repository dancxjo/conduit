//! Selected entry retains the actual funded owner across the ordinary runner.
use super::*;
use crate::hosted_creche::CrecheHost;
use std::cell::RefCell;

impl StdHost {
    /// Execute a prepared Crèche through the ordinary exact Host Call runner.
    /// Owner/context backing retain their original counter for the whole call;
    /// the result and escaped output bytes retain the kernel's separate charge.
    ///
    /// # Safety
    /// Root must independently qualify and retain adapter, worker, output
    /// adapter and every shared input backing, including asynchronous retirement
    /// and uncertainty paths, under this Host's admitted IMAGE limits. Owner
    /// preparation alone funds none of those. This cooperative entry grants no
    /// extra Source, deployment, membership or device authority.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn run_selected_image_body_plan_with_creche_to_with_start<
        W: Write,
        T: TimerAdapter,
        F,
    >(
        &mut self,
        request: BodyRunRequest<'_>,
        input: &ImageForeInput,
        owner: &ImagePreparedValue<RefCell<CrecheHost<'_>>>,
        output: &mut W,
        timer: &mut T,
        fore: &mut dyn BodyForeOutputAdapter,
        started: F,
    ) -> Result<ImageBodyRunReport, String>
    where
        F: FnMut(&BodyPlayIdentity, &Wake) -> Result<(), String>,
    {
        self.verify_selected_image_fore_input(input)?;
        self.verify_selected_image_prepared_value(owner)?;
        let mut owner = owner
            .value()
            .try_borrow_mut()
            .map_err(|_| "Crèche IMAGE owner is already borrowed")?;
        owner.verify_image_memory(self)?;
        if !self.kernel_resources.has_image_bounds() {
            return Err("selected IMAGE execution requires IMAGE-bound Host".into());
        }
        let mut custody = None;
        let result = self.run_body_plan_to_with_start_and_clock_and_creche(
            request,
            output,
            timer,
            None,
            None,
            Some((std::slice::from_ref(input.input()), false, fore)),
            None,
            None,
            None,
            Some(&mut owner),
            started,
            Some(&mut custody),
            None,
            None,
        );
        let report = match result {
            Ok(report) => report,
            Err(error) => {
                // The shared runner drains after reservation. Earlier failures
                // still own a prepared encounter and must drain it explicitly.
                if let Err(drain) = owner.retire() {
                    if self.kernel_resources.require_available().is_ok() {
                        self.kernel_resources.quarantine_retirement(Vec::new());
                    }
                    return Err(format!("{error}; Crèche pre-start retirement: {drain}"));
                }
                return Err(error);
            }
        };
        Ok(ImageBodyRunReport {
            report,
            _memory: custody.ok_or("selected IMAGE result lost its backing custody")?,
        })
    }
}
