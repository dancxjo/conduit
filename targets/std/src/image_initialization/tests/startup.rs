//! Real retained startup cue through the public selected IMAGE Body entrance.
use super::*;
use crate::body_execution::{BodyRunRequest, CommittedBodyStartup};
use crate::hosted_audio::{AlsaPlaybackObservation, FakePlaybackBehavior, HostedPlaybackSelection};
use conduit_body::*;
use conduit_kernel::HostCallDisposition;
mod admission;
mod execution;
mod fixture;
