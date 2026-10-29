//! One explicitly Boot-pinned journal transaction; no implicit Boot adoption.
use super::*;
use crate::network_custody_client::{ack, resume};
use std::{net::SocketAddr, path::PathBuf, time::Instant};

pub struct Options<'a> {
    pub producer_boot: &'a str,
    pub private_run: &'a Path,
}
// Exactly one foreground transaction owns this finite inline state (608 bytes
// on the qualified CPU). Boxing a phase solely for enum size adds an allocation
// at its lifecycle boundary; this is not an extensible collection of sessions.
#[allow(clippy::large_enum_variant)]
enum State {
    Export(resume::Export),
    Acknowledge(ack::Acknowledgement),
    Terminal,
}
pub(super) struct Network {
    state: State,
    binding: PathBuf,
    continuity: PathBuf,
    next: Instant,
}
impl Network {
    pub(super) fn open(binding: &Path, continuity: &Path, options: Options<'_>) -> Result<Self> {
        let verified = validated_binding(binding)?;
        crate::observed_history::create_durable_directory(options.private_run)?;
        private_metadata(options.private_run, true)?;
        let export = resume::Export::open(
            options.private_run,
            &verified.credential().body_id,
            *verified.root(),
            &options.producer_boot.into(),
        )?;
        Ok(Self {
            state: State::Export(export),
            binding: binding.into(),
            continuity: continuity.into(),
            next: Instant::now(),
        })
    }
    pub(super) fn due(&self) -> Option<Instant> {
        (!matches!(self.state, State::Terminal)).then_some(self.next)
    }
    pub(super) fn tick(&mut self, peer: &str, address: SocketAddr) {
        if self.due().is_none_or(|next| Instant::now() < next) {
            return;
        }
        if let Err(error) = self.advance(peer, address) {
            self.state = State::Terminal;
            eprintln!("network custody stopped; retained evidence and observation collection continue: {error}");
        }
    }
    fn advance(&mut self, peer: &str, address: SocketAddr) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(3);
        let request =
            |method: &str, path: &str, body: &[u8], content_type: &str, deadline: Instant| {
                crate::body_join::http::http_resolved_until(
                    crate::body_join::http::Request {
                        peer,
                        method,
                        path,
                        body,
                        content_type,
                    },
                    address,
                    deadline,
                )
            };
        let state = std::mem::replace(&mut self.state, State::Terminal);
        match state {
            State::Export(mut export) => {
                self.next = Instant::now() + Duration::from_millis(400);
                match export.advance(deadline, |path, deadline| {
                    request("GET", path, &[], "application/json", deadline)
                })? {
                    resume::Progress::Pending => self.state = State::Export(export),
                    resume::Progress::Complete(token) => {
                        let binding = validated_binding(&self.binding)?;
                        let bytes = crate::body_join::sign_durable_network_custody(
                            &self.binding,
                            &self.continuity,
                            &token,
                        )?;
                        match ack::Acknowledgement::open(token, binding.credential().clone(), bytes)? {
                            ack::Start::Ready(owner) => self.state = State::Acknowledge(owner),
                            ack::Start::Complete(_receipt) => eprintln!("network custody: signed Clean receipt reopened; no erase requested"),
                            ack::Start::Interrupted => eprintln!("network custody: interrupted ACK session requires explicit reconciliation; no retry budget renewed"),
                        }
                    }
                }
            }
            State::Acknowledge(mut owner) => {
                self.next = Instant::now() + Duration::from_secs(2);
                match owner.advance(deadline, |bytes, deadline| request("POST", "/network/custody/ack", bytes, "application/octet-stream", deadline))? {
                    ack::Progress::Pending => self.state = State::Acknowledge(owner),
                    ack::Progress::Complete(_receipt) => eprintln!("network custody: exact signed Clean receipt durably retained; no erase requested"),
                }
            }
            State::Terminal => (),
        }
        Ok(())
    }
}
