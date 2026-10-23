//! Correlated finite continuation of one immutable Graph preparation phase.
use super::{GraphPhase, Request, Response};
use crate::data::Refusal;

/// Root owns only transport progress; the guest retains all graph semantics.
pub struct GraphProgress {
    phase: GraphPhase,
    cursor: u32,
    limit: u32,
    complete: bool,
}
impl GraphProgress {
    pub fn new(phase: GraphPhase, limit: u32) -> Result<Self, Refusal> {
        if limit == 0 {
            return Err(Refusal::Bounds);
        }
        Ok(Self {
            phase,
            cursor: 0,
            limit,
            complete: false,
        })
    }
    pub fn request(&self) -> Result<Request, Refusal> {
        if self.complete || self.cursor >= self.limit {
            return Err(Refusal::Order);
        }
        Ok(match self.phase {
            GraphPhase::Serialize => Request::GraphSerialize {
                cursor: self.cursor,
            },
            GraphPhase::Validate => Request::GraphValidate {
                cursor: self.cursor,
            },
        })
    }
    /// true only for phase completion; no retries after a refusal or mismatch.
    pub fn accept(&mut self, response: Response) -> Result<bool, Refusal> {
        if self.complete || self.cursor >= self.limit {
            return Err(Refusal::Order);
        }
        match response {
            Response::Ready => {
                self.complete = true;
                Ok(true)
            }
            Response::Progress { phase, cursor }
                if phase == self.phase
                    && self.cursor.checked_add(1) == Some(cursor)
                    && cursor < self.limit =>
            {
                self.cursor = cursor;
                Ok(false)
            }
            _ => {
                self.complete = true;
                Err(Refusal::Malformed)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_phase_and_next_cursor_are_required_before_completion() {
        let mut phase = GraphProgress::new(GraphPhase::Serialize, 3).unwrap();
        assert_eq!(phase.request(), Ok(Request::GraphSerialize { cursor: 0 }));
        assert_eq!(
            phase.accept(Response::Progress {
                phase: GraphPhase::Serialize,
                cursor: 1
            }),
            Ok(false)
        );
        assert_eq!(phase.request(), Ok(Request::GraphSerialize { cursor: 1 }));
        assert_eq!(phase.accept(Response::Ready), Ok(true));
        assert_eq!(phase.request(), Err(Refusal::Order));
        assert_eq!(phase.accept(Response::Ready), Err(Refusal::Order));
    }
    #[test]
    fn wrong_phase_stale_skipped_and_over_limit_progress_are_terminal() {
        for response in [
            Response::Progress {
                phase: GraphPhase::Validate,
                cursor: 1,
            },
            Response::Progress {
                phase: GraphPhase::Serialize,
                cursor: 0,
            },
            Response::Progress {
                phase: GraphPhase::Serialize,
                cursor: 2,
            },
            Response::Blob { bytes: 1 },
        ] {
            let mut phase = GraphProgress::new(GraphPhase::Serialize, 2).unwrap();
            assert_eq!(phase.accept(response), Err(Refusal::Malformed));
            assert_eq!(phase.request(), Err(Refusal::Order));
        }
        let mut phase = GraphProgress::new(GraphPhase::Validate, 1).unwrap();
        assert_eq!(
            phase.accept(Response::Progress {
                phase: GraphPhase::Validate,
                cursor: 1
            }),
            Err(Refusal::Malformed)
        );
        assert_eq!(phase.request(), Err(Refusal::Order));
    }
}

#[cfg(test)]
mod wire_tests {
    use super::super::{self as wire, Envelope};
    use super::*;
    #[test]
    fn progress_wire_preserves_envelope_and_rejects_reserved_or_unknown_fields() {
        let envelope = Envelope {
            instance: 19,
            sequence: 91,
        };
        let mut bytes = [0; wire::BYTES];
        for response in [
            Response::Progress {
                phase: GraphPhase::Serialize,
                cursor: 1,
            },
            Response::Progress {
                phase: GraphPhase::Validate,
                cursor: 256,
            },
        ] {
            wire::encode_response(envelope, response, &mut bytes).unwrap();
            assert_eq!(wire::decode_response(&bytes), Ok((envelope, response)));
            bytes[40] = 1;
            assert_eq!(wire::decode_response(&bytes), Err(Refusal::Malformed));
            wire::encode_response(envelope, response, &mut bytes).unwrap();
            bytes[32] = 3;
            assert_eq!(wire::decode_response(&bytes), Err(Refusal::Malformed));
            wire::encode_response(envelope, response, &mut bytes).unwrap();
            bytes[36..40].fill(0);
            assert_eq!(wire::decode_response(&bytes), Err(Refusal::Malformed));
        }
        for request in [
            Request::GraphSerialize { cursor: 0 },
            Request::GraphSerialize { cursor: 17 },
            Request::GraphValidate { cursor: 29 },
        ] {
            wire::encode_request(envelope, request, &mut bytes).unwrap();
            assert_eq!(wire::decode_request(&bytes), Ok((envelope, request)));
            bytes[36] = 1;
            assert_eq!(wire::decode_request(&bytes), Err(Refusal::Malformed));
        }
    }
}
