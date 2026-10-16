//! Borrowed semantic axis roles from the already admitted tensor descriptor.
use crate::{NamedTensorView, NamedTensorViewRefusal, TensorAxisRole};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TensorAxisRoleView<'a> {
    Batch,
    Time,
    Feature,
    Sensor,
    SpatialCoordinate,
    Frequency,
    Channel,
    Other(&'a str),
}
impl TensorAxisRoleView<'_> {
    pub fn matches(self, role: &TensorAxisRole) -> bool {
        match (self, role) {
            (Self::Batch, TensorAxisRole::Batch)
            | (Self::Time, TensorAxisRole::Time)
            | (Self::Feature, TensorAxisRole::Feature)
            | (Self::Sensor, TensorAxisRole::Sensor)
            | (Self::SpatialCoordinate, TensorAxisRole::SpatialCoordinate)
            | (Self::Frequency, TensorAxisRole::Frequency)
            | (Self::Channel, TensorAxisRole::Channel) => true,
            (Self::Other(actual), TensorAxisRole::Other(expected)) => actual == expected.identity(),
            _ => false,
        }
    }
}
impl<'a> NamedTensorView<'a> {
    pub fn axis_role(
        &self,
        index: usize,
    ) -> Result<TensorAxisRoleView<'a>, NamedTensorViewRefusal> {
        use NamedTensorViewRefusal::Encoding;
        if index >= self.dimensions().len() {
            return Err(Encoding);
        }
        let axis = self
            .axes()
            .collection_index(index as u16)
            .map_err(|_| Encoding)?
            .ok_or(Encoding)?
            .representation()
            .map_err(|_| Encoding)?;
        let role = axis
            .record_field("role")
            .map_err(|_| Encoding)?
            .ok_or(Encoding)?;
        Ok(match role.variant_tag().map_err(|_| Encoding)? {
            "batch" => TensorAxisRoleView::Batch,
            "time" => TensorAxisRoleView::Time,
            "feature" => TensorAxisRoleView::Feature,
            "sensor" => TensorAxisRoleView::Sensor,
            "spatial_coordinate" => TensorAxisRoleView::SpatialCoordinate,
            "frequency" => TensorAxisRoleView::Frequency,
            "channel" => TensorAxisRoleView::Channel,
            "other" => {
                let value = role
                    .variant_payload("other")
                    .map_err(|_| Encoding)?
                    .ok_or(Encoding)?
                    .representation()
                    .map_err(|_| Encoding)?
                    .record_field("identity")
                    .map_err(|_| Encoding)?
                    .ok_or(Encoding)?;
                TensorAxisRoleView::Other(
                    core::str::from_utf8(
                        value
                            .primitive_representation_bytes(conduit_core::TEXT_INFO_ID)
                            .map_err(|_| Encoding)?,
                    )
                    .map_err(|_| Encoding)?,
                )
            }
            _ => return Err(Encoding),
        })
    }
}
