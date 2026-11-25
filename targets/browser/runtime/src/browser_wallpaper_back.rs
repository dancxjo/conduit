//! The selected Wallpaper gear samples the Host clock on each planned Plot tick.
use crate::browser_wallpaper::BrowserWallpaperObservation;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
    ValueRef,
};

pub struct BrowserWallpaperBack {
    observation: BrowserWallpaperObservation,
    output: Box<[u8]>,
    output_len: usize,
    empty: ValueRef,
    sequence: u32,
    pending: bool,
}

impl BrowserWallpaperBack {
    pub fn prepare(
        observation: BrowserWallpaperObservation,
        empty: ValueRef,
        admitted_bytes: usize,
    ) -> Result<Self, String> {
        let output_len = conduit_semantic_catalog::RESIDENT_WALLPAPER_BYTES as usize;
        if observation.prepared_heap_bytes() + output_len + core::mem::size_of::<Self>()
            > admitted_bytes
        {
            return Err("browser wallpaper observation exceeds its admitted backing".into());
        }
        Ok(Self {
            observation,
            output: vec![0; output_len].into_boxed_slice(),
            output_len: 0,
            empty,
            sequence: 0,
            pending: false,
        })
    }
}

impl<const PORTS: usize> StepBack<PORTS> for BrowserWallpaperBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        let port = PortId(0);
        if let Some((completed, outcome)) = io.host_completion() {
            if !self.pending
                || completed != RequestId(self.sequence)
                || outcome.disposition != HostCallDisposition::Completed
            {
                return outcome.failure.map_or_else(invalid, StepOutcome::Fail);
            }
            if !io.output_ready(port) {
                return StepOutcome::Await;
            }
            let Some(time) = bytes
                .host_output()
                .and_then(|reply| <[u8; 8]>::try_from(reply).ok())
            else {
                return invalid();
            };
            let encoded = self.observation.observe_at(u64::from_le_bytes(time));
            self.output_len = encoded.len();
            self.output[..self.output_len].copy_from_slice(encoded);
            io.consume_host_completion()
                .expect("observed wallpaper completion");
            io.send_prepared(port, self.output_len as u32)
                .expect("ready wallpaper output");
            self.pending = false;
            return StepOutcome::Progress;
        }
        if self.pending {
            return StepOutcome::Await;
        }
        if let Some(tick) = io.input(port) {
            if tick.byte_len != conduit_time::TICK_ENCODED_LEN {
                return invalid();
            }
            let Some(sequence) = self.sequence.checked_add(1) else {
                return StepOutcome::Fail(Failure {
                    code: FailureCode::IdentityCapacityExhausted,
                    detail: 224,
                });
            };
            io.consume(port).expect("observed wallpaper tick");
            io.request_host_call(
                RequestId(sequence),
                HostCallId(0),
                BoundedValueRef::new(self.empty, 0).expect("empty wallpaper clock request"),
            )
            .expect("admitted wallpaper clock Host Call");
            self.sequence = sequence;
            self.pending = true;
            return StepOutcome::Progress;
        }
        if io.input_closed(port) {
            io.consume_closed(port)
                .expect("observed wallpaper input closure");
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0)).then(|| &self.output[..self.output_len])
    }

    fn cancel(&mut self) {
        self.pending = false;
    }
}

fn invalid() -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail: 224,
    })
}
