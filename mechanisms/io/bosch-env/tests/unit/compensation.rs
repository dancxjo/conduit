use super::*;
use std::vec::Vec;
fn registers() -> ([u8; 26], [u8; 7]) {
    let t = [
        27504_i32, 26435, -1000, 36477, -10685, 3024, 2855, 140, -7, 15500, -14600, 6000,
    ];
    let mut tp = [0; 26];
    for (i, v) in t.iter().enumerate() {
        tp[2 * i..2 * i + 2].copy_from_slice(&(*v as u16).to_le_bytes());
    }
    tp[25] = 75;
    let h2 = 362_i16.to_le_bytes();
    let (h4, h5) = (325_i16, 50_i16);
    let h = [
        h2[0],
        h2[1],
        0,
        (h4 >> 4) as u8,
        ((h5 & 15) << 4) as u8 | (h4 & 15) as u8,
        (h5 >> 4) as u8,
        30,
    ];
    (tp, h)
}
fn raw(p: u32, t: u32, h: u16) -> [u8; 8] {
    [
        (p >> 12) as u8,
        (p >> 4) as u8,
        (p << 4) as u8,
        (t >> 12) as u8,
        (t >> 4) as u8,
        (t << 4) as u8,
        (h >> 8) as u8,
        h as u8,
    ]
}
#[test]
fn official_datasheet_temperature_pressure_and_bmp_humidity() {
    let (tp, h) = registers();
    let cal = Calibration::parse(Chip::Bme280, &tp, &h).unwrap();
    let reading = cal
        .compensate(Chip::Bme280, &raw(415148, 519888, 30000))
        .unwrap();
    assert_eq!(reading.temperature_centi_celsius, 2508);
    assert!(reading.pressure_centi_pascal.abs_diff(10065325) <= 5);
    let bmp = Calibration::parse(Chip::Bmp280, &tp, &[0; 7])
        .unwrap()
        .compensate(Chip::Bmp280, &raw(415148, 519888, 0x8000))
        .unwrap();
    assert_eq!(
        bmp.humidity,
        Humidity::Unavailable(HumidityUnavailable::UnsupportedChip)
    );
    assert_eq!(
        bmp.temperature_centi_celsius,
        reading.temperature_centi_celsius
    );
    assert_eq!(bmp.pressure_centi_pascal, reading.pressure_centi_pascal);
}
#[test]
fn signed_humidity_calibration_and_invalid_factory_words() {
    let (mut tp, mut h) = registers();
    h[3] = 0xff;
    h[4] = 0x0f;
    h[5] = 0x80;
    h[6] = 0x80;
    let cal = Calibration::parse(Chip::Bme280, &tp, &h).unwrap();
    assert_eq!((cal.h4, cal.h5, cal.h6), (-1, -2048, -128));
    assert!(matches!(
        Calibration::parse(Chip::Bme280, &[0; 26], &h),
        Err(DataRefusal::Calibration)
    ));
    assert!(matches!(
        Calibration::parse(Chip::Bme280, &[255; 26], &h),
        Err(DataRefusal::Calibration)
    ));
    tp[6] = 0;
    tp[7] = 0;
    assert!(matches!(
        Calibration::parse(Chip::Bmp280, &tp, &h),
        Err(DataRefusal::Calibration)
    ));
    let (tp, h) = registers();
    let cal = Calibration::parse(Chip::Bme280, &tp, &h).unwrap();
    for value in [
        raw(0x80000, 519888, 30000),
        raw(415148, 0x80000, 30000),
        raw(415148, 519888, 0x8000),
    ] {
        assert_eq!(
            cal.compensate(Chip::Bme280, &value),
            Err(DataRefusal::SkippedData)
        );
    }
}
#[test]
fn arbitrary_register_bits_never_panic_or_export_outside_units() {
    let mut seed = 7_u64;
    for _ in 0..20000 {
        let mut next = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            (seed >> 32) as u8
        };
        let tp = core::array::from_fn(|_| next());
        let h = core::array::from_fn(|_| next());
        let raw = core::array::from_fn(|_| next());
        if let Ok(cal) = Calibration::parse(Chip::Bme280, &tp, &h) {
            if let Ok(v) = cal.compensate(Chip::Bme280, &raw) {
                assert!((-4000..=8500).contains(&v.temperature_centi_celsius));
                assert!((3000000..=11000000).contains(&v.pressure_centi_pascal));
                assert!(matches!(v.humidity, Humidity::PercentQ10(0..=102400)));
            }
        }
    }
}
#[test]
fn matches_pinned_official_bosch_integer_routines_for_calibration_trajectories() {
    use std::{
        fmt::Write,
        fs,
        process::{Command, Stdio},
        string::String,
    };
    let directory = std::env::temp_dir().join(std::format!(
        "conduit-bosch-reference-{}",
        std::process::id()
    ));
    fs::create_dir(&directory).unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(directory.clone());
    let exe = directory.join("reference");
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/reference_bosch.c");
    assert!(Command::new("cc")
        .args(["-std=c99", "-O1", "-fwrapv"])
        .arg(&source)
        .arg("-o")
        .arg(&exe)
        .status()
        .unwrap()
        .success());
    let mut input = String::new();
    let mut expected = Vec::new();
    let mut seed = 9_u64;
    for index in 0..2048 {
        let mut next = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            (seed >> 32) as u32
        };
        let (mut tp, mut h) = registers();
        let t2 = 26435 + (next() % 1001) as i16 - 500;
        tp[2..4].copy_from_slice(&t2.to_le_bytes());
        let p2 = -10685 + (next() % 1001) as i16 - 500;
        tp[8..10].copy_from_slice(&p2.to_le_bytes());
        let h2 = 362 + (next() % 21) as i16 - 10;
        h[..2].copy_from_slice(&h2.to_le_bytes());
        // Include negative signed H6 while keeping reference C intermediates in range.
        h[6] = if index % 2 == 0 { 30 } else { (-30_i8) as u8 };
        let h4 = if index % 4 == 0 { -7_i16 } else { 325 };
        let h5 = if index % 8 == 0 { -50_i16 } else { 50 };
        h[3] = (h4 >> 4) as u8;
        h[4] = (((h5 & 15) << 4) | (h4 & 15)) as u8;
        h[5] = (h5 >> 4) as u8;
        let cal = Calibration::parse(Chip::Bme280, &tp, &h).unwrap();
        let (p, t, hraw) = (
            420000 + next() % 20000,
            350000 + next() % 200000,
            (28000 + next() % 12000) as u16,
        );
        let t = if t == 0x80000 { t + 1 } else { t };
        let hraw = if hraw == 0x8000 { hraw + 1 } else { hraw };
        let result = cal.compensate(Chip::Bme280, &raw(p, t, hraw)).unwrap();
        expected.push(result);
        write!(
            &mut input,
            "{p} {t} {hraw} {} {} {} {}",
            cal.t1, cal.t2, cal.t3, cal.p1
        )
        .unwrap();
        for value in cal.p {
            write!(&mut input, " {value}").unwrap();
        }
        writeln!(
            &mut input,
            " {} {} {} {} {} {}",
            cal.h1, cal.h2, cal.h3, cal.h4, cal.h5, cal.h6
        )
        .unwrap();
    }
    assert!(expected.iter().any(|r| r.temperature_centi_celsius < 0));
    assert!(expected.iter().any(|r| r.temperature_centi_celsius > 0));
    let file = directory.join("cases");
    fs::write(&file, input).unwrap();
    let output = Command::new(&exe)
        .stdin(Stdio::from(fs::File::open(file).unwrap()))
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<_> = text.lines().collect();
    assert_eq!(lines.len(), expected.len());
    for (line, reading) in lines.into_iter().zip(expected) {
        let v: Vec<i64> = line
            .split_whitespace()
            .map(|v| v.parse().unwrap())
            .collect();
        assert_eq!(
            v,
            [
                i64::from(reading.temperature_centi_celsius),
                i64::from(reading.pressure_centi_pascal),
                match reading.humidity {
                    Humidity::PercentQ10(v) => i64::from(v),
                    _ => panic!("BME humidity missing"),
                }
            ]
        );
    }
}
