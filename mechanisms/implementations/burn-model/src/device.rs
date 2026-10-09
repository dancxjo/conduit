use crate::Error;
use burn::tensor::Device;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceRequest {
    Cpu,
    Cuda(u32),
}

impl DeviceRequest {
    pub fn prepare(self) -> Result<Device, Error> {
        match self {
            Self::Cpu => Ok(Device::flex()),
            #[cfg(feature = "cuda")]
            Self::Cuda(index) => std::panic::catch_unwind(|| {
                let device = Device::cuda(index as usize);
                // Force actual preparation instead of advertising an unchecked selector.
                let _: f32 = burn::tensor::Tensor::<1>::zeros([1], &device).into_scalar();
                device
            })
            .map_err(|_| Error::UnsupportedDevice),
            #[cfg(not(feature = "cuda"))]
            Self::Cuda(_) => Err(Error::UnsupportedDevice),
        }
    }
    pub fn evidence(self) -> String {
        match self {
            Self::Cpu => "burn/flex-cpu".into(),
            Self::Cuda(i) => format!("burn/cuda/{i}"),
        }
    }
}
