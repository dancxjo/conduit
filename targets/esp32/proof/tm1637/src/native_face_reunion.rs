//! Affine two-owner transition. Partial progress owns every remaining handle;
//! no publication handle escapes before both backing owners are replaced.
#![allow(clippy::type_complexity, clippy::result_large_err)] // Errors retain affine owners.
use super::{http, pair, read_scope, PreparedReplacement, Publisher, Reader};
use conduit_environment_runtime::publication_face_custody as custody;

pub struct Returning {
    phase: ReturnPhase,
}
enum ReturnPhase {
    Original(Publisher, Reader),
    Face(
        custody::Returned<'static>,
        http::Publisher,
        http::Control,
        [u8; 64],
    ),
}
pub struct Returned {
    face: custody::Returned<'static>,
    http: http::Returned,
    active: [u8; 64],
}
pub struct Replacing {
    phase: ReplacePhase,
}
#[allow(clippy::large_enum_variant)] // Keep exact prepared ownership inline; retry never allocates.
enum ReplacePhase {
    Original(Returned, PreparedReplacement),
    Face(
        custody::Publisher<'static>,
        custody::Reader<'static>,
        http::Returned,
        u32,
        u64,
    ),
}
pub(super) fn reunite(
    publisher: Publisher,
    reader: Reader,
) -> Result<Returned, (Returning, &'static str)> {
    Returning {
        phase: ReturnPhase::Original(publisher, reader),
    }
    .retry()
}
impl Returning {
    /// Recover the original pair only before either generation was advanced.
    pub fn into_original(self) -> Result<(Publisher, Reader), Self> {
        match self.phase {
            ReturnPhase::Original(publisher, reader) => Ok((publisher, reader)),
            _ => Err(self),
        }
    }
    #[inline(never)]
    pub fn retry(self) -> Result<Returned, (Self, &'static str)> {
        let (face, publisher, control, active) = match self.phase {
            ReturnPhase::Original(publisher, reader) => {
                let active = match read_scope(&publisher.reader) {
                    Ok(active) => active,
                    Err(reason) => {
                        return Err((
                            Self {
                                phase: ReturnPhase::Original(publisher, reader),
                            },
                            reason,
                        ))
                    }
                };
                let Publisher {
                    inner,
                    reader: lease,
                    http,
                } = publisher;
                let Reader {
                    inner: face_reader,
                    http: control,
                } = reader;
                match face_reader.reunite(inner) {
                    Ok(face) => {
                        http.withdraw();
                        (face, http, control, active)
                    }
                    Err((inner, face_reader, reason)) => {
                        return Err((
                            Self {
                                phase: ReturnPhase::Original(
                                    Publisher {
                                        inner,
                                        reader: lease,
                                        http,
                                    },
                                    Reader {
                                        inner: face_reader,
                                        http: control,
                                    },
                                ),
                            },
                            reason.code(),
                        ))
                    }
                }
            }
            ReturnPhase::Face(face, publisher, control, active) => {
                (face, publisher, control, active)
            }
        };
        match http::reunite(publisher, control) {
            Ok(http) => Ok(Returned { face, http, active }),
            Err((publisher, control, reason)) => Err((
                Self {
                    phase: ReturnPhase::Face(face, publisher, control, active),
                },
                reason,
            )),
        }
    }
}
impl Returned {
    #[inline(never)]
    pub fn replace(
        self,
        prepared: PreparedReplacement,
    ) -> Result<(Publisher, Reader), (Replacing, &'static str)> {
        Replacing {
            phase: ReplacePhase::Original(self, prepared),
        }
        .retry()
    }
}
impl Replacing {
    /// Recover unchanged returned custody and prepared scope before replacement.
    pub fn into_unstarted(self) -> Result<(Returned, PreparedReplacement), Self> {
        match self.phase {
            ReplacePhase::Original(returned, prepared) => Ok((returned, prepared)),
            _ => Err(self),
        }
    }

    #[inline(never)]
    pub fn retry(self) -> Result<(Publisher, Reader), (Self, &'static str)> {
        let (publisher, reader, http, count, stale_after_ms) = match self.phase {
            ReplacePhase::Original(returned, prepared) => {
                if returned.active == prepared.active {
                    return Err((
                        Self {
                            phase: ReplacePhase::Original(returned, prepared),
                        },
                        "face-replacement-same-play",
                    ));
                }
                let PreparedReplacement {
                    face,
                    count,
                    stale_after_ms,
                    active,
                } = prepared;
                match returned.face.replace(face) {
                    Ok((publisher, reader)) => {
                        (publisher, reader, returned.http, count, stale_after_ms)
                    }
                    Err((face_returned, face, reason)) => {
                        return Err((
                            Self {
                                phase: ReplacePhase::Original(
                                    Returned {
                                        face: face_returned,
                                        http: returned.http,
                                        active: returned.active,
                                    },
                                    PreparedReplacement {
                                        face,
                                        count,
                                        stale_after_ms,
                                        active,
                                    },
                                ),
                            },
                            reason.code(),
                        ))
                    }
                }
            }
            ReplacePhase::Face(publisher, reader, http, count, stale_after_ms) => {
                (publisher, reader, http, count, stale_after_ms)
            }
        };
        match http.replace(count, stale_after_ms) {
            Ok((http, control)) => Ok(pair(publisher, reader, http, control)),
            Err((http, reason)) => Err((
                Self {
                    phase: ReplacePhase::Face(publisher, reader, http, count, stale_after_ms),
                },
                reason,
            )),
        }
    }
}
