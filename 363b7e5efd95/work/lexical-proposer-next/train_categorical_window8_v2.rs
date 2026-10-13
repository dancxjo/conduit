//! External numeric averaged-perceptron trainer, never a runtime parser.
use std::{env, fs};
const BINS: usize = 411;
const CLASSES: usize = 76;
const LOOKUPS: usize = 27;
#[derive(Clone)]
struct Sample {
    gold: [u16; LOOKUPS],
    target: usize,
    alternatives: Vec<[u16; LOOKUPS]>,
}
fn u16at(bytes: &[u8], cursor: &mut usize) -> Result<u16, String> {
    let v = bytes.get(*cursor..*cursor + 2).ok_or("truncated sample")?;
    *cursor += 2;
    Ok(u16::from_le_bytes([v[0], v[1]]))
}
fn tuple(bytes: &[u8], cursor: &mut usize) -> Result<[u16; LOOKUPS], String> {
    let mut v = [0; LOOKUPS];
    for i in &mut v {
        *i = u16at(bytes, cursor)?;
        if usize::from(*i) >= BINS {
            return Err("feature bound".into());
        }
    }
    Ok(v)
}
fn quantize(total: i64, steps: i64) -> Result<i16, String> {
    let negative = total < 0;
    let magnitude = total
        .unsigned_abs()
        .checked_mul(256)
        .ok_or("quantization overflow")?;
    let denominator = steps as u64;
    let mut q = magnitude / denominator;
    let r = magnitude % denominator;
    if r * 2 > denominator || r * 2 == denominator && q % 2 == 1 {
        q += 1
    }
    let value = if negative { -(q as i64) } else { q as i64 };
    i16::try_from(value).map_err(|_| "i16 precision overflow".into())
}
fn run() -> Result<(), String> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 4 {
        return Err("input samples, output artifact, epochs required".into());
    }
    let epochs: usize = args[3].parse().map_err(|_| "epochs")?;
    if !(1..=64).contains(&epochs) {
        return Err("epoch ceiling".into());
    }
    let bytes = fs::read(&args[1]).map_err(|e| e.to_string())?;
    if bytes.len() > 256 * 1024 * 1024 || bytes.get(..8) != Some(b"C27TRAIN") {
        return Err("input frame".into());
    }
    let mut cursor = 8;
    let mut samples = Vec::new();
    while cursor < bytes.len() {
        if samples.len() >= 1_000_000 {
            return Err("sample ceiling".into());
        }
        let gold = tuple(&bytes, &mut cursor)?;
        let target = usize::from(u16at(&bytes, &mut cursor)?);
        let count = usize::from(u16at(&bytes, &mut cursor)?);
        if target >= CLASSES || !(1..=16).contains(&count) {
            return Err("class/alternatives bound".into());
        }
        let mut alternatives = Vec::with_capacity(count);
        for _ in 0..count {
            alternatives.push(tuple(&bytes, &mut cursor)?)
        }
        alternatives.sort();
        alternatives.dedup();
        if !alternatives.contains(&gold) {
            return Err("gold outside candidate features".into());
        }
        samples.push(Sample {
            gold,
            target,
            alternatives,
        })
    }
    if samples.is_empty() {
        return Err("no samples".into());
    }
    let mut order: Vec<usize> = (0..samples.len()).collect();
    let mut random = 4907_u64;
    let mut weights = vec![0_i64; BINS * CLASSES];
    let mut totals = vec![0_i64; weights.len()];
    let mut times = vec![0_i64; weights.len()];
    let mut steps = 0_i64;
    for epoch in 0..epochs {
        for i in (1..order.len()).rev() {
            random ^= random << 13;
            random ^= random >> 7;
            random ^= random << 17;
            order.swap(i, (random as usize) % (i + 1))
        }
        let mut updates = 0;
        for &sample_index in &order {
            steps += 1;
            let s = &samples[sample_index];
            let mut best_score = i64::MIN;
            let mut chosen = 0;
            let mut chosen_indices = s.alternatives[0];
            for indices in &s.alternatives {
                for class in 0..CLASSES {
                    let score = indices
                        .iter()
                        .map(|i| weights[class * BINS + usize::from(*i)])
                        .sum::<i64>();
                    if score > best_score || score == best_score && class < chosen {
                        best_score = score;
                        chosen = class;
                        chosen_indices = *indices
                    }
                }
            }
            if chosen == s.target && chosen_indices == s.gold {
                continue;
            }
            updates += 1;
            for (class, indices, delta) in [(s.target, s.gold, 1), (chosen, chosen_indices, -1)] {
                for i in indices {
                    let index = class * BINS + usize::from(i);
                    totals[index] += (steps - times[index]) * weights[index];
                    times[index] = steps;
                    weights[index] += delta
                }
            }
        }
        eprintln!(
            "epoch={} samples={} updates={} steps={}",
            epoch + 1,
            samples.len(),
            updates,
            steps
        );
    }
    let mut artifact = Vec::with_capacity(20 + weights.len() * 2);
    artifact.extend_from_slice(b"CI16SUM1");
    for dimension in [BINS, CLASSES, LOOKUPS] {
        artifact.extend_from_slice(&(dimension as u32).to_le_bytes())
    }
    let mut max_weight = 0_i32;
    for i in 0..weights.len() {
        totals[i] += (steps - times[i]) * weights[i];
        let value = quantize(totals[i], steps)?;
        max_weight = max_weight.max(i32::from(value).abs());
        artifact.extend_from_slice(&value.to_le_bytes())
    }
    if max_weight as usize * LOOKUPS > 1000000 {
        return Err("unchanged Source score ceiling".into());
    }
    fs::write(&args[2], artifact).map_err(|e| e.to_string())?;
    println!(
        "samples={} steps={} maximum_weight={} conservative_score_bound={}",
        samples.len(),
        steps,
        max_weight,
        max_weight as usize * LOOKUPS
    );
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("refused: {error}");
        std::process::exit(1)
    }
}
