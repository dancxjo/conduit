//! The shared general reading controller: native custody is solely in Mask.
use super::{Error, FrontDoor};
use conduitos_mask_core::control::{Operation, View};
impl FrontDoor {
    pub(super) fn reading(&self) -> Result<View, Error> {
        #[cfg(all(target_arch = "x86_64", target_os = "none"))]
        {
            self.workspace_mask
                .try_borrow_mut()
                .map_err(|_| Error::Scene)?
                .control(&self.host_id, &self.boot_id, Operation::Read)
                .map(|(_, view)| view)
                .map_err(|_| Error::Scene)
        }
        #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
        {
            Ok(self.reading)
        }
    }
    pub(super) fn reading_command(&mut self, operation: Operation) -> Result<bool, Error> {
        #[cfg(all(target_arch = "x86_64", target_os = "none"))]
        {
            self.workspace_mask
                .try_borrow_mut()
                .map_err(|_| Error::Scene)?
                .control(&self.host_id, &self.boot_id, operation)
                .map(|(changed, _)| changed)
                .map_err(|_| Error::Scene)
        }
        #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
        {
            conduitos_mask_core::control::apply(&mut self.reading, operation)
                .map_err(|_| Error::Face)
        }
    }
    pub fn exact_details_open(&self) -> bool {
        // A failed native controller must hold input rather than expose a
        // seemingly usable workspace mode. Fallible Face/scene paths refuse.
        self.reading().map_or(true, |view| view.exact_details_open)
    }
}
