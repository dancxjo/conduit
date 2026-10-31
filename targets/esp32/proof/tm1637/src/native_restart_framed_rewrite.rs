//! Same canonical JSON split at the immutable archive-array boundaries.
use super::*;
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(super) fn emit<E>(
    evidence: &BodyBiographyEvidence,
    pending: &[BodyBiographyArchiveSegment],
    admissions: &AdmissionManager,
    previous_span: archive::Span,
    prefix: Option<&wire::Receipt>,
    local_count: u64,
    layouts: &layout::Layouts,
    network: Option<&network::Cursor>,
    summary: archive::Summary,
    reuse: Option<frames::Range>,
    mut read: impl FnMut(&mut [u8; 128]) -> Result<usize, ()>,
    sink: &mut dyn FnMut(&[u8]) -> Result<(), E>,
) -> Result<(archive::Summary, archive::Span, frames::Range), StreamRefusal<E>> {
    let encoded = core::cell::Cell::new(0usize);
    let mut failure = None;
    let capacity = core::cell::Cell::new(false);
    let mut output = |bytes: &[u8]| -> Result<(), ()> {
        let next = encoded.get().checked_add(bytes.len()).ok_or(())?;
        if next > MAX_BYTES {
            capacity.set(true);
            return Err(());
        }
        sink(bytes).map_err(|e| {
            failure = Some(e);
        })?;
        encoded.set(next);
        Ok(())
    };
    let result = (|| {
        let mut index = compress::MatchIndex::new();
        let mut encoder = frames::Encoder::with_index(&mut output, &mut index);
        encoder.raw(frames::MAGIC)?;
        let head = encoder
            .frame(frames::PREFIX, |plain| {
                plain(b"{\"schema\":")?;
                json::stream(
                    &if network.is_some() {
                        SCHEMA_V4
                    } else if !layouts.is_empty() {
                        SCHEMA_V3
                    } else if prefix.is_some() {
                        SCHEMA_V2
                    } else {
                        SCHEMA
                    },
                    &mut |b: &[u8]| plain(b).map_err(|_| json::CodecError::Sink),
                )
                .map_err(|_| ())?;
                plain(b",\"evidence\":")?;
                json::stream(&profile::EvidenceRef::from(evidence), &mut |b: &[u8]| {
                    plain(b).map_err(|_| json::CodecError::Sink)
                })
                .map_err(|_| ())?;
                if let Some(prefix) = prefix {
                    plain(b",\"biography_prefix\":")?;
                    json::stream(prefix, &mut |b: &[u8]| {
                        plain(b).map_err(|_| json::CodecError::Sink)
                    })
                    .map_err(|_| ())?;
                }
                if !layouts.is_empty() {
                    plain(b",\"observation_layouts\":")?;
                    json::stream(layouts, &mut |b: &[u8]| {
                        plain(b).map_err(|_| json::CodecError::Sink)
                    })
                    .map_err(|_| ())?;
                }
                if let Some(network) = network {
                    plain(b",\"network_cursor\":")?;
                    json::stream(network, &mut |b: &[u8]| {
                        plain(b).map_err(|_| json::CodecError::Sink)
                    })
                    .map_err(|_| ())?;
                }
                plain(b",\"archives\":[")
            })
            .map_err(|e| {
                capacity.set(capacity.get() || e == compress::Error::Capacity);
            })?;
        // Derive encoded boundaries through a counting sink, never source JSON.
        let archive_start = encoded.get();
        let old_plain = previous_span
            .end
            .checked_sub(previous_span.start)
            .ok_or(())?;
        if let Some(reuse) = reuse {
            let mut at = 0usize;
            let mut buffer = [0; 128];
            let copied = (|| {
                loop {
                    let n = read(&mut buffer)?;
                    if n > 128 {
                        return Err(());
                    }
                    if n == 0 {
                        break;
                    }
                    let a = at.max(reuse.encoded.start);
                    let z = (at + n).min(reuse.encoded.end);
                    if a < z {
                        encoder.raw(&buffer[a - at..z - at])?;
                    }
                    at = at.checked_add(n).ok_or(())?;
                }
                if at < reuse.encoded.end {
                    return Err(());
                }
                Ok(())
            })();
            buffer.fill(0);
            copied?;
        } else if local_count != 0 {
            encoder
                .frame(frames::ARCHIVE, |plain| {
                    with_plain(&mut read, |source| {
                        let mut at = 0usize;
                        let mut buffer = [0; 128];
                        let result = (|| {
                            loop {
                                let n = source(&mut buffer).map_err(|_| "restart-source")?;
                                if n > 128 {
                                    return Err("restart-source");
                                }
                                if n == 0 {
                                    break;
                                }
                                let a = at.max(previous_span.start);
                                let z = (at + n).min(previous_span.end);
                                if a < z {
                                    plain(&buffer[a - at..z - at]).map_err(|_| "restart-write")?;
                                }
                                at = at.checked_add(n).ok_or("restart-source")?;
                            }
                            if at < previous_span.end {
                                return Err("restart-source");
                            }
                            Ok(())
                        })();
                        buffer.fill(0);
                        result
                    })
                    .map_err(|_| ())
                })
                .map_err(|e| {
                    capacity.set(capacity.get() || e == compress::Error::Capacity);
                })?;
        } else {
            let mut buffer = [0; 128];
            let drained = (|| {
                loop {
                    let n = read(&mut buffer)?;
                    if n > 128 {
                        return Err(());
                    }
                    if n == 0 {
                        break;
                    }
                }
                Ok(())
            })();
            buffer.fill(0);
            drained?;
        }
        let append = if pending.is_empty() {
            0
        } else {
            encoder
                .frame(frames::ARCHIVE, |plain| {
                    let mut comma = local_count != 0;
                    for segment in pending {
                        if comma {
                            plain(b",")?;
                        }
                        comma = true;
                        json::stream(&profile::ArchiveRef::from(segment), &mut |b: &[u8]| {
                            plain(b).map_err(|_| json::CodecError::Sink)
                        })
                        .map_err(|_| ())?;
                    }
                    Ok(())
                })
                .map_err(|e| {
                    capacity.set(capacity.get() || e == compress::Error::Capacity);
                })?
                .plain
        };
        let archive_end = encoded.get();
        let tail = encoder
            .frame(frames::SUFFIX, |plain| {
                plain(b"],\"admissions\":")?;
                json::stream(admissions, &mut |b: &[u8]| {
                    plain(b).map_err(|_| json::CodecError::Sink)
                })
                .map_err(|_| ())?;
                plain(b"}")
            })
            .map_err(|e| {
                capacity.set(capacity.get() || e == compress::Error::Capacity);
            })?;
        encoder.raw(&[frames::END])?;
        let span = archive::Span {
            start: head.plain,
            end: head
                .plain
                .checked_add(old_plain)
                .and_then(|n| n.checked_add(append))
                .ok_or(())?,
        };
        if span.end.checked_add(tail.plain).ok_or(())? > MAX_PLAIN_BYTES {
            capacity.set(true);
            return Err(());
        }
        Ok((
            summary,
            span,
            frames::Range {
                encoded: archive::Span {
                    start: archive_start,
                    end: archive_end,
                },
                plain: span,
            },
        ))
    })();
    if let Some(e) = failure {
        return Err(StreamRefusal::Sink(e));
    }
    result.map_err(|_| {
        if capacity.get() {
            StreamRefusal::Capacity
        } else {
            StreamRefusal::Invalid
        }
    })
}
