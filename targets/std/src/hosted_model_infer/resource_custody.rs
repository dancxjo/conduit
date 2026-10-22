//! The selected capability retains immutable model and imported-name custody.
use conduit_ai::{imported_onnx::ImportedOnnxResource, AdmittedModelResource};
use std::ops::Deref;

// Both variants are bounded preparation-owned descriptors. Keeping them inline
// avoids a new heap reservation for existing native preparations; Play only
// borrows this retained owner.
#[allow(clippy::large_enum_variant)]
pub(super) enum ModelCustody {
    Native(AdmittedModelResource),
    Imported {
        model: ImportedOnnxResource,
        admission: conduit_ai::imported_back_admission::ImportedBackAdmissionReceipt,
    },
}

impl ModelCustody {
    pub(super) fn imported(&self) -> Option<&ImportedOnnxResource> {
        match self {
            Self::Native(_) => None,
            Self::Imported { model, .. } => Some(model),
        }
    }

    pub(super) fn imported_runtime(&self) -> Option<&conduit_ai::ModelComputeRuntimeIdentity> {
        match self {
            Self::Native(_) => None,
            Self::Imported { admission, .. } => Some(&admission.profile().runtime),
        }
    }

    pub(super) fn selection_identity(&self) -> [u8; 32] {
        match self {
            Self::Native(model) => model.descriptor_identity(),
            Self::Imported { model, admission } => {
                let mut bytes = [0; 96];
                bytes[..32].copy_from_slice(&model.resource().descriptor_identity());
                bytes[32..64].copy_from_slice(&model.sidecar_identity());
                bytes[64..].copy_from_slice(&admission.identity());
                conduit_core::semantic_digest("model/imported-inference-selection@2", &bytes)
            }
        }
    }
}

impl Deref for ModelCustody {
    type Target = AdmittedModelResource;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Native(model) => model,
            Self::Imported { model, .. } => model.resource(),
        }
    }
}

/// Actual symbolic dimensions are invocation-local; independent maxima alone
/// cannot permit two ports carrying the same ONNX symbol to disagree.
pub(super) struct ImportedSymbols<'a> {
    model: Option<&'a ImportedOnnxResource>,
    values: [Option<(&'a str, u64)>; 256],
    count: usize,
}
impl ModelCustody {
    pub(super) fn input_symbols(
        &self,
        inputs: &conduit_data::NamedTensorBundleView<'_>,
    ) -> Result<ImportedSymbols<'_>, ()> {
        let mut symbols = ImportedSymbols {
            model: self.imported(),
            values: [None; 256],
            count: 0,
        };
        if let Some(model) = symbols.model {
            symbols.observe(&model.metadata().inputs, &model.sidecar().inputs, inputs)?;
        }
        Ok(symbols)
    }
}
impl<'a> ImportedSymbols<'a> {
    pub(super) fn outputs(
        &mut self,
        outputs: &conduit_data::NamedTensorBundleView<'_>,
    ) -> Result<(), ()> {
        if let Some(model) = self.model {
            self.observe(&model.metadata().outputs, &model.sidecar().outputs, outputs)?;
        }
        Ok(())
    }
    fn observe(
        &mut self,
        graph: &'a [conduit_ai::onnx_metadata::OnnxPort],
        bindings: &[conduit_ai::imported_onnx::ImportedPortBinding],
        tensors: &conduit_data::NamedTensorBundleView<'_>,
    ) -> Result<(), ()> {
        for binding in bindings {
            let port = graph
                .iter()
                .find(|p| p.name == binding.graph_name)
                .ok_or(())?;
            let tensor = tensors
                .values()
                .find(|t| t.name == binding.semantic_port)
                .ok_or(())?;
            if port.dimensions.len() != tensor.dimensions().len() {
                return Err(());
            }
            for (dimension, actual) in port.dimensions.iter().zip(tensor.dimensions()) {
                if let conduit_ai::onnx_metadata::OnnxDimension::Symbol(name) = dimension {
                    if let Some((_, previous)) = self.values[..self.count]
                        .iter()
                        .flatten()
                        .find(|(symbol, _)| *symbol == name.as_str())
                    {
                        if previous != actual {
                            return Err(());
                        }
                    } else {
                        *self.values.get_mut(self.count).ok_or(())? = Some((name, *actual));
                        self.count += 1;
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
