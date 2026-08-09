use crate::RendererSnapshot;

pub fn demonstration_snapshot() -> Result<RendererSnapshot, String> {
    RendererSnapshot::from_portable(patchbay_model::portable_demonstration()?)
        .map_err(|error| error.to_string())
}
