//! Stage Root preparation on the final original counter before Host construction.
use super::*;
use crate::{
    body_execution::{ImagePreparedValue, ImageRootPreparation, ImageRootPreparationStage},
    kernel_preparation::StagedImageMemory,
};

impl StdHost {
    /// Verify intended configuration/IMAGE/catalog and lend the final staged
    /// counter to a trusted synchronous recipe, before any Host initializer.
    ///
    /// The recipe must use `ImageRootPreparation::prepare_value` before its real
    /// factory/cold reads. The outer recipe, verified inputs, catalog backing and
    /// captures require independent bootstrap custody. This hook derives no
    /// factory bound and grants no Source/effect authority or provider readiness.
    /// Its affine output must enter the matching initialized-Base composition;
    /// creating a second counter would not preserve these prepared owners.
    pub fn prepare_selected_image_root<'a, T>(
        config: &'a StdHostConfig,
        image: &'a HostImage,
        checked: &conduit_host_make::CheckedHostConfiguration,
        catalog: &MakeCatalog,
        prepare: impl FnOnce(&ImageRootPreparation<'_>) -> Result<ImagePreparedValue<T>, String>,
    ) -> Result<ImageRootPreparationStage<'a, T>, String> {
        bounded_image_preflight(config, image)?;
        let memory = StagedImageMemory::verified(config, image, checked, catalog)?;
        let value = {
            let mut reserve = |bytes| memory.reserve(bytes);
            let preparation = ImageRootPreparation::new(&mut reserve);
            prepare(&preparation)?
        };
        value.verify_staged_owner(&memory)?;
        Ok(ImageRootPreparationStage {
            value,
            memory,
            config,
            image,
        })
    }

    /// Consume staged Root preparation with the existing timer-only initializer.
    /// This is not the absent CPU/source/native Root composition. Those actual
    /// initializers must transfer this same staged memory through InitialBases.
    /// The ordinary config clone and Host construction remain independent costs.
    pub fn from_selected_timer_image_with_prepared_root<T>(
        preparation: ImageRootPreparationStage<'_, T>,
        catalog: &MakeCatalog,
    ) -> Result<(Self, ImagePreparedValue<T>), String> {
        let ImageRootPreparationStage {
            value,
            memory,
            config,
            image,
        } = preparation;
        let host = Self::from_selected_initial_bases(
            config.clone(),
            image,
            catalog,
            InitialBases {
                memory: Some(memory),
                ..InitialBases::default()
            },
        )?;
        host.verify_selected_image_prepared_value(&value)?;
        Ok((host, value))
    }
}

#[cfg(test)]
mod tests;
