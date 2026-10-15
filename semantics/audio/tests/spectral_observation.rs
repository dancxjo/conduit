use conduit_audio::*;
use conduit_core::{validate_canonical_structured_value, IeeeF64, Quantity, Scalar, Unit};
use conduit_plot::rust_binding::NativeRustBinding;
fn frame(frames: u16, start: u64) -> Vec<u8> {
    let samples: Vec<u8> = (0..frames)
        .flat_map(|i| (0.5_f32 * (std::f32::consts::TAU * f32::from(i) / 16.0).sin()).to_le_bytes())
        .collect();
    PcmFrameHeader::new(
        PcmSampleRepresentation::Float32LittleEndian,
        48000,
        PcmChannelLayout::Mono,
        frames,
        55,
        start,
        false,
    )
    .unwrap()
    .encode_frame(&samples)
    .unwrap()
}
fn profile() -> PcmSpectralProfile {
    PcmSpectralProfile::new(
        PcmSpectralGeometry::new(1024, 256, 128).unwrap(),
        55,
        48000,
        PcmSpectralWindow::PeriodicHann,
    )
    .unwrap()
}
fn observation() -> AudioSpectrumObservation {
    profile()
        .prepare_observation(&mut PcmSpectralWorkspace::default(), &frame(1024, 10000), 0)
        .unwrap()
}
fn patch(mut encoded: Vec<u8>, name: &str, kind: &str, replacement: &[u8]) -> Vec<u8> {
    let (offset, len) = {
        let view = validate_canonical_structured_value(&encoded).unwrap();
        let value = view.record_field(name).unwrap().unwrap();
        let bytes = value.primitive_representation_bytes(kind).unwrap();
        (
            bytes.as_ptr() as usize - encoded.as_ptr() as usize,
            bytes.len(),
        )
    };
    assert_eq!(len, replacement.len());
    encoded[offset..offset + len].copy_from_slice(replacement);
    encoded
}
#[test]
fn exact_stride_windows_retain_original_custody_and_roundtrip_checked_source() {
    let p = profile();
    let encoded = frame(1024, 10000);
    let mut scratch = PcmSpectralWorkspace::default();
    let mut digest = None;
    assert_eq!(p.geometry().windows(), 7);
    for index in 0..p.geometry().windows() {
        let value = p
            .prepare_observation(&mut scratch, &encoded, index)
            .unwrap();
        let bytes = value.clone().encode().unwrap();
        assert!(bytes.len() < conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES);
        assert_eq!(AudioSpectrumObservation::decode(&bytes).unwrap(), value);
        assert_eq!(*value.window_index(), index);
        assert_eq!(*value.window_start_frame(), 10000 + u64::from(index) * 128);
        assert_eq!(*value.source_start_frame(), 10000);
        assert_eq!(*value.clock_id(), 55);
        assert_eq!(
            *value.bin_spacing(),
            Quantity::parse_plot_literal("187.5Hz").unwrap()
        );
        assert_eq!(value.peak_amplitudes().get().len(), 129);
        assert_eq!(value.mean_square().get().len(), 129);
        assert!(digest.as_ref().is_none_or(|d| d == value.source_digest()));
        digest = Some(value.source_digest().clone());
    }
    assert!(matches!(
        p.observe_window(&mut scratch, &encoded, 7),
        Err(PcmSpectralRefusal::WindowIndexOutsideGeometry)
    ));
    assert!(matches!(
        p.observe_window(&mut scratch, &frame(512, 10000), 0),
        Err(PcmSpectralRefusal::WrongSourceFrames)
    ));
}
#[test]
fn geometry_refuses_truncation_invalid_stride_and_unsupported_resolution() {
    assert!(matches!(
        PcmSpectralGeometry::new(2048, 16, 1),
        Err(PcmSpectralRefusal::TooManyWindows)
    ));
    assert!(matches!(
        PcmSpectralGeometry::new(1024, 256, 0),
        Err(PcmSpectralRefusal::InvalidStride)
    ));
    assert!(matches!(
        PcmSpectralGeometry::new(1024, 48, 64),
        Err(PcmSpectralRefusal::UnsupportedWindow)
    ));
    assert!(matches!(
        PcmSpectralGeometry::new(128, 256, 64),
        Err(PcmSpectralRefusal::WrongSourceFrames)
    ));
    assert!(matches!(
        PcmSpectralGeometry::new(2049, 256, 64),
        Err(PcmSpectralRefusal::WrongSourceFrames)
    ));
    assert!(PcmSpectralProfile::new(
        PcmSpectralGeometry::new(64, 64, 64).unwrap(),
        0,
        48000,
        PcmSpectralWindow::Rectangular
    )
    .is_err());
}
#[test]
fn native_where_laws_refuse_false_axis_time_resolution_and_nonfinite_output() {
    let bytes = observation().encode().unwrap();
    for changed in [
        patch(
            bytes.clone(),
            "bin_spacing",
            conduit_core::FREQUENCY_INFO_ID,
            &Quantity::new(1, Unit::Hertz).encode(),
        ),
        patch(
            bytes.clone(),
            "bin_spacing",
            conduit_core::FREQUENCY_INFO_ID,
            &Quantity::new(1, Unit::Second).encode(),
        ),
        patch(
            bytes.clone(),
            "window_frames",
            "value/u16",
            &48u16.to_le_bytes(),
        ),
        patch(
            bytes.clone(),
            "window_start_frame",
            "value/u64",
            &10001u64.to_le_bytes(),
        ),
        patch(bytes.clone(), "clock_id", "value/u64", &0u64.to_le_bytes()),
    ] {
        assert!(AudioSpectrumObservation::decode(&changed).is_err());
    }
    let mut changed = bytes;
    let offset = {
        let view = validate_canonical_structured_value(&changed).unwrap();
        let v = view
            .record_field("peak_amplitudes")
            .unwrap()
            .unwrap()
            .collection_index(0)
            .unwrap()
            .unwrap();
        v.primitive_representation_bytes(conduit_core::F64_INFO_ID)
            .unwrap()
            .as_ptr() as usize
            - changed.as_ptr() as usize
    };
    changed[offset..offset + 8].copy_from_slice(&f64::NAN.to_le_bytes());
    assert!(AudioSpectrumObservation::decode(&changed).is_err());
    assert!(AudioSpectrumAmplitude::new(IeeeF64::from_value(-1.0)).is_err());
}
#[test]
fn bin_query_refuses_fractional_or_outside_dense_axis() {
    let value = observation();
    for raw in [0, 128_000_000] {
        let q =
            AudioSpectrumBinRequest::new(Scalar::from_raw_microunits(raw), value.clone()).unwrap();
        assert_eq!(
            AudioSpectrumBinRequest::decode(&q.clone().encode().unwrap()).unwrap(),
            q
        );
    }
    for raw in [-1_000_000, 500_000, 129_000_000] {
        assert!(
            AudioSpectrumBinRequest::new(Scalar::from_raw_microunits(raw), value.clone()).is_err()
        );
    }
}
