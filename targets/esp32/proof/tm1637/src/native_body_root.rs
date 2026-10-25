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
        Ok(())
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
