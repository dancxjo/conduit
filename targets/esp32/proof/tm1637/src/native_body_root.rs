//! Root owns its installed Body; reports follow retained ownership.
use super::NativeRoot;
impl NativeRoot {
    #[cfg(feature = "body-image-root")]
    pub fn install_linked_body(&mut self) -> Result<(), &'static str> {
        if self.body.is_some() {
            return Err("body-image-already-installed");
        }
        let owner = crate::native_body_image::load(self)?;
        self.body = Some(owner);
        #[cfg(feature = "body-persistence")]
        self.admission.activate_after_checkpoint()?;
        Ok(())
    }
    #[cfg(all(feature = "body-image-root", feature = "body-persistence"))]
    pub(crate) fn restart_verifying_key(&self) -> [u8; 32] {
        self.admission.verifying_key()
    }
    #[cfg(all(feature = "body-image-root", feature = "body-persistence"))]
    pub fn checkpoint_body(&mut self) -> Result<(), &'static str> {
        let key = self.admission.verifying_key();
        let body = self.body.as_mut().ok_or("restart-body-not-installed")?;
        crate::native_restart::checkpoint(body, &key)?;
        self.admission.activate_after_checkpoint()
    }
    pub fn body_installed(&self) -> bool {
        #[cfg(feature = "body-image-root")]
        {
            self.body.is_some()
        }
        #[cfg(not(feature = "body-image-root"))]
        {
            false
        }
    }
}
