//! Source-owned name of the selected Commons encounter.
use super::*;
impl Commons {
    pub(in crate::front_door) fn heading(&self) -> Result<String, Error> {
        let source = self.source.borrow();
        let slot = if self.creche() { "creche" } else { "hub" };
        Ok(format!(
            "Commons / {}",
            place_text(&source.places, slot, "name")?
        ))
    }
}
