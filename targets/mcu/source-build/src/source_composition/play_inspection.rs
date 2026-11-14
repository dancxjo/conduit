// Read-only platform and retained Face inspection; no execution authority.
pub(super) fn encode(
    face: &conduit_environment_runtime::publication_face_custody::LocalPublicationFace,
    request: &[u8],
    output: &mut [u8],
) -> Result<Option<usize>, ()> {
    if request == b"H" {
        #[derive(serde::Serialize)]
        struct HeapReport {
            schema: &'static str,
            assurance: &'static str,
            phase: &'static str,
            heap_used_bytes: usize,
            heap_limit_bytes: usize,
            remaining_bytes: usize,
            inspection_buffer_bytes: usize,
        }
        return serde_json_core::to_slice(
            &HeapReport {
                schema: "conduit.host/playing-heap-inspection@1",
                assurance: "unsigned-inspection",
                phase: "playing",
                heap_used_bytes: crate::runtime::allocator_used(),
                heap_limit_bytes: crate::config::HEAP_BYTES,
                remaining_bytes: crate::runtime::remaining(),
                inspection_buffer_bytes: crate::workset::renderer::RENDERER_FACE_BYTES as usize,
            },
            output,
        )
        .map(Some)
        .map_err(|_| ());
    }
    // Read-only full-object binary inspection; this supplies no authority.
    if request == b"B" {
        return face
            .with_retained(|current, _| {
                conduit_face::postcard_wire::encode_face(current, output)
                    .map(|bytes| Some(bytes.len()))
                    .map_err(|_| ())
            })
            .map_err(|_| ())?;
    }
    #[cfg(target_arch = "xtensa")]
    if request == b"P" {
        use core::fmt::Write;
        #[derive(serde::Serialize)]
        struct CaptureReport<'a> {
            schema: &'static str,
            assurance: &'static str,
            phase: &'a str,
            completed_bits: u8,
            completed_octets: u8,
            elapsed_us: u32,
            wire: &'a [u8],
        }
        let Some(capture) = conduit_esp32_electrical::last_capture_observation() else {
            return Err(());
        };
        let mut phase = heapless::String::<24>::new();
        write!(&mut phase, "{:?}", capture.phase).map_err(|_| ())?;
        return serde_json_core::to_slice(
            &CaptureReport {
                schema: "conduit.host/pulse-capture-inspection@1",
                assurance: "unsigned-inspection",
                phase: phase.as_str(),
                completed_bits: capture.completed_bits,
                completed_octets: capture.completed_octets,
                elapsed_us: capture.elapsed_us,
                wire: &capture.wire[..usize::from(capture.completed_octets)],
            },
            output,
        )
        .map(Some)
        .map_err(|_| ());
    }
    if request != b"F" {
        return Ok(None);
    }
    face.with_retained(|current, status| {
        #[derive(serde::Serialize)]
        struct Report<'a> {
            schema: &'static str,
            assurance: &'static str,
            source_sequence: u64,
            publisher_retained: bool,
            refusal: Option<&'static str>,
            face: &'a conduit_face::Face,
        }
        serde_json_core::to_slice(
            &Report {
                schema: "conduit.host/retained-face-inspection@1",
                assurance: "unsigned-inspection",
                source_sequence: status.source_sequence,
                publisher_retained: status.publisher_retained,
                refusal: status.refusal,
                face: current,
            },
            output,
        )
        .map(Some)
        .map_err(|_| ())
    })
    .map_err(|_| ())?
}
