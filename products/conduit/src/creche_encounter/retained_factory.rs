//! Service ownership of an initialized factory without detaching IMAGE custody.
use super::*;
use conduit_std_host::{body_execution::ImagePreparedValue, StdHost};
use std::{
    cell::{RefCell, RefMut},
    ops::{Deref, DerefMut},
};

pub struct RetainedCrecheFactory(Storage);
enum Storage {
    Reference(CrecheEncounterFactory),
    Image(ImagePreparedValue<RefCell<CrecheEncounterFactory>>),
}
impl From<CrecheEncounterFactory> for RetainedCrecheFactory {
    fn from(factory: CrecheEncounterFactory) -> Self {
        Self(Storage::Reference(factory))
    }
}
impl RetainedCrecheFactory {
    pub fn from_image(
        host: &StdHost,
        prepared: ImagePreparedValue<RefCell<CrecheEncounterFactory>>,
    ) -> Result<Self, String> {
        host.verify_selected_image_prepared_value(&prepared)?;
        prepared
            .value()
            .try_borrow()
            .map_err(|_| "Crèche factory is already borrowed")?
            .verify_image_custody(host)?;
        Ok(Self(Storage::Image(prepared)))
    }
    pub fn is_image(&self) -> bool {
        matches!(self.0, Storage::Image(_))
    }
    pub fn verify_image_custody(&self, host: &StdHost) -> Result<(), String> {
        match &self.0 {
            Storage::Reference(_) => Err("Crèche factory has no retained IMAGE owner".into()),
            Storage::Image(prepared) => {
                host.verify_selected_image_prepared_value(prepared)?;
                prepared
                    .value()
                    .try_borrow()
                    .map_err(|_| "Crèche factory is already borrowed")?
                    .verify_image_custody(host)
            }
        }
    }
    pub fn resource_offer(&self) -> Result<ResourceOffer, String> {
        match &self.0 {
            Storage::Reference(factory) => Ok(factory.resource_offer()),
            Storage::Image(prepared) => Ok(prepared
                .value()
                .try_borrow()
                .map_err(|_| "Crèche factory is already borrowed")?
                .resource_offer()),
        }
    }
    pub fn borrow_mut(&mut self) -> Result<CrecheFactoryBorrow<'_>, String> {
        Ok(CrecheFactoryBorrow(match &mut self.0 {
            Storage::Reference(factory) => Borrow::Reference(factory),
            Storage::Image(prepared) => Borrow::Image(
                prepared
                    .value()
                    .try_borrow_mut()
                    .map_err(|_| "Crèche factory is already borrowed")?,
            ),
        }))
    }
}
pub struct CrecheFactoryBorrow<'a>(Borrow<'a>);
enum Borrow<'a> {
    Reference(&'a mut CrecheEncounterFactory),
    Image(RefMut<'a, CrecheEncounterFactory>),
}
impl Deref for CrecheFactoryBorrow<'_> {
    type Target = CrecheEncounterFactory;
    fn deref(&self) -> &Self::Target {
        match &self.0 {
            Borrow::Reference(factory) => factory,
            Borrow::Image(factory) => factory,
        }
    }
}
impl DerefMut for CrecheFactoryBorrow<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match &mut self.0 {
            Borrow::Reference(factory) => factory,
            Borrow::Image(factory) => factory,
        }
    }
}
