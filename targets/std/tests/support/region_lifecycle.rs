#[allow(dead_code)]
#[path = "region_accounting.rs"]
mod accounting;
pub(crate) use accounting::{region_resources, KernelResourceLedger};
#[path = "../../src/kernel_preparation/service.rs"]
pub(crate) mod service;
#[path = "../../src/kernel_preparation/worker.rs"]
mod worker;
