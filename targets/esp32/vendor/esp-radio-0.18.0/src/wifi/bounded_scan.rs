//! Caller-owned bounded scanning with retained cancellation custody.
//!
//! This module grants no scan/radio authority. A selected Host operation must
//! supply mode/service ownership, absolute deadlines, output and cleanup custody.
#[path = "bounded_scan/custody.rs"]
mod custody;
use super::{
    WifiController,
    event::{EVENT_CHANNEL, EventInfo, EventSubscriber, MessageResult},
    scan::ScanConfig,
};
use crate::sys::include;
use core::mem::MaybeUninit;
pub use custody::{Completion, Record};

/// SDK/list failures remain distinct from missing terminal evidence.
#[derive(Debug)]
pub enum Error {
    /// The retained scan is not in the phase required by this operation.
    Busy,
    /// Caller storage or the requested finite record bound is invalid.
    Capacity,
    /// No persistent event subscriber was available before starting.
    Subscribers,
    /// Unmodified SDK return code; it is not terminal scan evidence.
    Sdk(i32),
    /// The SDK observation cannot fit the fixed representation.
    InvalidRecord,
    /// Caller conversion refused after consuming an exact SDK prefix.
    MappingRefused {
        /// Number of caller slots initialized before refusal.
        written: u8,
    },
    /// Ambiguous custody prevents successful result publication.
    Quarantined,
}
/// Exact terminal event and actual SDK list size; status zero alone means scan
/// success. A stopped/failed scan must not be published as a successful scan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Report {
    /// Actual terminal event observed by the retained subscriber.
    pub completion: Completion,
    /// SDK list count before consuming its bounded prefix.
    pub total: u16,
    /// Whether this owner requested stop before collecting this report.
    pub stop_requested: bool,
    /// Number of caller records initialized by this collection.
    pub copied: u8,
    /// Whether the original SDK list contained more records than copied.
    pub truncated: bool,
}
/// Owns the controller across cancellation of every borrowing future. Dropping
/// an unfinished owner deliberately quarantines the radio guard rather than
/// running WifiController's deinitialization without terminal evidence.
#[must_use = "retain until terminal scan evidence and SDK list cleanup"]
pub struct OwnedScan<'d> {
    controller: Option<WifiController<'d>>,
    subscriber: Option<EventSubscriber<'static>>,
    custody: custody::Custody,
    maximum: usize,
}
impl<'d> OwnedScan<'d> {
    /// Establish sequential, initially-quiescent SDK ownership.
    ///
    /// # Safety
    /// Caller exclusively owns this controller, with no pending scan/connect,
    /// unconsumed terminal event or retained SDK AP list. No other raw SDK user
    /// may start/stop a scan while this owner exists. SDK does not expose a start
    /// scan_id: correlation is this exclusive non-overlapping custody, never a
    /// guessed identifier. Configuration must already permit station scanning.
    pub unsafe fn from_quiescent(controller: WifiController<'d>) -> Self {
        Self {
            controller: Some(controller),
            subscriber: None,
            custody: custody::Custody::quiescent(),
            maximum: 0,
        }
    }
    /// Start one scan. At most sixteen records can ever be copied into caller
    /// output. SDK internal list allocation remains its separately bounded arena.
    pub fn start(&mut self, config: &ScanConfig, maximum: usize) -> Result<(), Error> {
        if !self.custody.is_quiescent() {
            return Err(Error::Busy);
        }
        if maximum == 0 || maximum > 16 {
            return Err(Error::Capacity);
        }
        let subscriber = EVENT_CHANNEL.subscriber().map_err(|_| Error::Subscribers)?;
        self.subscriber = Some(EventSubscriber::new(subscriber));
        self.maximum = config.max.unwrap_or(maximum).min(maximum);
        if self.maximum == 0 {
            self.subscriber = None;
            return Err(Error::Capacity);
        }
        self.custody.begin();
        if let Err(error) = sdk(super::wifi_start_scan(false, *config)) {
            // Even a refused SDK start is not recast as terminal radio evidence.
            self.custody.quarantine();
            return Err(error);
        }
        Ok(())
    }
    /// Request stop at most once. ESP_OK acknowledges the API request only;
    /// actual ScanDone and list cleanup are still required for owner return.
    pub fn request_stop(&mut self) -> Result<(), Error> {
        if self.custody.request_stop() {
            unsafe {
                sdk(include::esp_wifi_scan_stop())?;
            }
        }
        Ok(())
    }
    /// Wait without transferring the controller. A dropped wait retains state
    /// and subscriber in self. Lag poisons publication, but a later real terminal
    /// event may still reconcile cleanup under this same exclusive invocation.
    pub async fn wait_terminal(&mut self) -> Result<Completion, Error> {
        loop {
            if let Some(event) = self.custody.completion() {
                return Ok(event);
            }
            let subscriber = self.subscriber.as_mut().ok_or(Error::Busy)?;
            match subscriber.next_event().await {
                MessageResult::Lagged(_) => self.custody.quarantine(),
                MessageResult::Message(EventInfo::ScanDone {
                    status,
                    number,
                    scan_id,
                }) => {
                    self.custody.observed(Completion {
                        status,
                        number,
                        scan_id,
                    });
                }
                MessageResult::Message(_) => (),
            }
        }
    }
    /// Read the bounded prefix only after real terminal evidence. Preserve the
    /// actual raw SDK authentication value, avoiding upstream's unknown→Open
    /// enum conversion. Reading or clearing errors retain cleanup custody in self.
    pub async fn finish(&mut self, output: &mut [Record]) -> Result<Report, Error> {
        self.finish_mapped(output, Some).await
    }
    /// Convert each real SDK record directly into caller-owned storage, without
    /// retaining a second record array. Mapping refusal reports the exact
    /// initialized prefix and retains terminal/list custody for cleanup. The
    /// selected caller still decides whether any failed result is publishable.
    pub async fn finish_mapped<T>(
        &mut self,
        output: &mut [T],
        mut map: impl FnMut(Record) -> Option<T>,
    ) -> Result<Report, Error> {
        if output.len() < self.maximum {
            return Err(Error::Capacity);
        }
        let completion = self.wait_terminal().await?;
        if self.custody.is_quarantined() {
            return Err(Error::Quarantined);
        }
        let mut total = 0;
        unsafe {
            sdk(include::esp_wifi_scan_get_ap_num(&mut total))?;
        }
        let mut written = 0;
        let copied = if completion.status == 0 {
            let copy = custody::copy_prefix(&mut output[..self.maximum], total, &mut || {
                let mut record = MaybeUninit::<include::wifi_ap_record_t>::uninit();
                unsafe {
                    sdk(include::esp_wifi_scan_get_ap_record(record.as_mut_ptr()))?;
                    let raw = record.assume_init_ref();
                    let record = Record::from_sdk(&raw.ssid, raw.rssi, raw.authmode)
                        .ok_or(Error::InvalidRecord)?;
                    let mapped = map(record).ok_or(Error::MappingRefused { written })?;
                    written += 1;
                    Ok(mapped)
                }
            });
            match copy {
                Ok(count) => count,
                Err(error) => {
                    // A prefix was consumed from the SDK list. It cannot become
                    // a new full result by retrying finish with the remainder.
                    self.custody.quarantine();
                    return Err(error);
                }
            }
        } else {
            0
        };
        let stop_requested = self.custody.stop_requested();
        self.clear_terminal_list()?;
        Ok(Report {
            completion,
            total,
            stop_requested,
            copied: copied as u8,
            truncated: usize::from(total) > copied,
        })
    }
    /// Used after cancellation/deadline. Missing terminal evidence remains a
    /// retained owner; caller may bound this future but cannot claim cleanup.
    pub async fn cleanup(&mut self) -> Result<(), Error> {
        if self.custody.is_quiescent() {
            return Ok(());
        }
        self.request_stop()?;
        self.wait_terminal().await?;
        self.clear_terminal_list()
    }
    fn clear_terminal_list(&mut self) -> Result<(), Error> {
        if let Err(error) = unsafe { sdk(include::esp_wifi_clear_ap_list()) } {
            self.custody.quarantine();
            return Err(error);
        }
        if !self.custody.cleared() {
            return Err(Error::Quarantined);
        }
        self.subscriber = None;
        Ok(())
    }
    /// Return only an actually quiescent controller. Err retains this exact owner.
    pub fn try_into_controller(mut self) -> Result<WifiController<'d>, Self> {
        if !self.custody.is_quiescent() {
            return Err(self);
        }
        Ok(self.controller.take().expect("owned controller"))
    }
}
impl Drop for OwnedScan<'_> {
    fn drop(&mut self) {
        if !self.custody.is_quiescent() {
            if let Some(controller) = self.controller.take() {
                core::mem::forget(controller);
            }
        }
    }
}

fn sdk(code: i32) -> Result<(), Error> {
    if code == include::ESP_OK as i32 {
        Ok(())
    } else {
        Err(Error::Sdk(code))
    }
}
