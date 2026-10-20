//! Independent finite equations for a numerical fixture, never product policy.
use std::f64::consts::PI;
pub struct Expected {
    pub phase: Vec<f64>,
    pub power: Vec<f64>,
    pub bands: Vec<f64>,
    pub features: Vec<f64>,
}
pub fn evaluate(history: &[f32], period: usize) -> Expected {
    assert_eq!(history.len(), 640);
    let window: Vec<_> = (0..320)
        .map(|i| {
            let j = i.min(319 - i) as f64;
            f64::from(history[320 + i])
                * (PI / 2. * (PI / 2. * (j + 0.5) / 160.).sin().powi(2)).sin()
        })
        .collect();
    let mut phase = Vec::new();
    let mut power = Vec::new();
    for k in 0..=160 {
        let real: f64 = window
            .iter()
            .enumerate()
            .map(|(n, x)| x * (2. * PI * k as f64 * n as f64 / 320.).cos())
            .sum();
        let imag: f64 = window
            .iter()
            .enumerate()
            .map(|(n, x)| -x * (2. * PI * k as f64 * n as f64 / 320.).sin())
            .sum();
        phase.extend([real, imag]);
        power.push(real * real + imag * imag);
    }
    // Integrate public triangular bands directly, not by reading generated matrix.
    let ends = [
        0, 1, 2, 3, 4, 5, 6, 7, 8, 10, 12, 14, 16, 20, 24, 28, 34, 40,
    ];
    let mut bands = vec![0.01; 18];
    for band in 0..17 {
        let start = 4 * ends[band];
        let width = 4 * (ends[band + 1] - ends[band]);
        for j in 0..width {
            let scaled = power[start + j] * (32768_f64 / 320.).powi(2);
            bands[band] +=
                scaled * (1. - j as f64 / width as f64) * if band == 0 { 2. } else { 1. };
            bands[band + 1] +=
                scaled * (j as f64 / width as f64) * if band + 1 == 17 { 2. } else { 1. };
        }
    }
    let (mut maximum, mut follow) = (-2_f64, -2_f64);
    let log: Vec<_> = bands
        .iter()
        .map(|energy| {
            let value = energy.log10().max(maximum - 8.).max(follow - 2.5);
            maximum = maximum.max(value);
            follow = (follow - 2.5).max(value);
            value
        })
        .collect();
    let mut features: Vec<_> = (0..18)
        .map(|j| {
            log.iter()
                .enumerate()
                .map(|(i, v)| {
                    v * ((i as f64 + 0.5) * j as f64 * PI / 18.).cos()
                        * (2_f64 / 18.).sqrt()
                        * if j == 0 { 0.5_f64.sqrt() } else { 1. }
                })
                .sum::<f64>()
                - if j == 0 { 4. } else { 0. }
        })
        .collect();
    features.push((256_f64 / period as f64).log2() - 1.5);
    let (mut xx, mut yy, mut xy) = (0., 0., 0.);
    for i in 480..640 {
        let x = f64::from(history[i]);
        let y = f64::from(history[i - period]);
        xx += x * x;
        yy += y * y;
        xy += x * y;
    }
    let correlation = (xy / (xx * yy + 1e-15).sqrt()).clamp(-1., 1.);
    features.push((1. + (5. * correlation).exp()).ln() / (1. + 5_f64.exp()).ln() - 0.5);
    Expected {
        phase,
        power,
        bands,
        features,
    }
}
