//! Isolated projection arithmetic worker; enter through xtask for retained proof.
#![allow(dead_code)]
#[allow(unexpected_cfgs)]
#[path = "../../../proof/performance/runtime/src/allocation.rs"]
mod allocation;
#[allow(unexpected_cfgs)]
#[path = "../../../proof/performance/runtime/src/artifact.rs"]
mod artifact;
#[path = "matrix_projection_performance/fixture.rs"]
mod fixture;
#[path = "matrix_projection_performance/reference.rs"]
mod reference;
#[path = "../../../proof/performance/runtime/src/timing.rs"]
mod timing;
use conduit_ai::{
    fixed_neural::FixedNumericRefusal, fixed_tensor::FixedMatrixOrder,
    fixed_tensor_linear::FixedTensorLinear,
};
use serde_json::json;
use std::{hint::black_box, path::Path};
#[global_allocator]
static ALLOCATOR: allocation::Allocator = allocation::Allocator;
#[inline(never)]
fn prepared<const I: usize, const O: usize>(
    linear: &FixedTensorLinear<'_, I, O>,
    input: &[f32; I],
    output: &mut [f32; O],
) -> Result<(), FixedNumericRefusal> {
    linear.apply(input, output)
}
fn case<const I: usize, const O: usize>(
    raw: &mut artifact::Raw,
    iterations: u32,
    repetitions: u32,
    order: FixedMatrixOrder,
) -> Result<(), Box<dyn std::error::Error>> {
    let ((tensor, bytes, input), fixture) = timing::measure(|| fixture::make::<I, O>(order));
    let (linear, preparation) =
        timing::measure(|| FixedTensorLinear::<I, O>::prepare(&tensor, &bytes, order).unwrap());
    let mut actual = [0.; O];
    let mut expected = [0.; O];
    reference::apply(&bytes, &input, &mut expected, order).unwrap();
    prepared(&linear, &input, &mut actual).unwrap();
    assert_eq!(actual.map(f32::to_bits), expected.map(f32::to_bits));
    let shape = format!("{I}x{O}");
    let order_name = format!("{order:?}");
    raw.row(&json!({"phase":"matrix-preparation","shape":shape,"order":order_name,"fixture":fixture,"preparation":preparation,"packed_weights_bytes":bytes.len(),"weight_sha256":artifact::digest(&bytes),"scope":"Synthetic validated tensor content; no Plan, Host resource authority, Back dispatch or canonical port codec measured."}))?;
    for repetition in 0..repetitions {
        let profiles = if repetition.is_multiple_of(2) {
            ["scalar-reference", "contiguous"]
        } else {
            ["contiguous", "scalar-reference"]
        };
        for profile in profiles {
            let ((), metrics) = timing::measure(|| {
                for _ in 0..iterations {
                    if profile == "contiguous" {
                        prepared(
                            black_box(&linear),
                            black_box(&input),
                            black_box(&mut actual),
                        )
                        .unwrap();
                    } else {
                        reference::apply(
                            black_box(&bytes),
                            black_box(&input),
                            black_box(&mut actual),
                            black_box(order),
                        )
                        .unwrap();
                    }
                    black_box(&actual);
                }
            });
            assert_eq!(
                metrics.allocator.allocations + metrics.allocator.reallocations,
                0
            );
            assert_eq!(actual.map(f32::to_bits), expected.map(f32::to_bits));
            let result: Vec<_> = actual.iter().flat_map(|v| v.to_le_bytes()).collect();
            raw.row(&json!({"phase":"warm-matrix-projection","shape":shape,"order":order_name,"profile":profile,"repetition":repetition,"iterations":iterations,"multiply_add_pairs":I as u64*O as u64*u64::from(iterations),"metrics":metrics,"output_sha256":artifact::digest(&result),"output_bits":actual.iter().map(|v|v.to_bits()).collect::<Vec<_>>(),"inspection_in_warm_timing":false}))?;
        }
    }
    Ok(())
}
fn run(output: &Path, iterations: u32, repetitions: u32) -> Result<(), Box<dyn std::error::Error>> {
    if cfg!(debug_assertions) {
        return Err("release measurements required".into());
    }
    let mut metadata = artifact::metadata()?;
    let worker = std::env::current_exe()?;
    metadata["worker_sha256"] = json!(artifact::digest(&std::fs::read(&worker)?));
    metadata["worker_path"] = json!(worker);
    artifact::json(&output.join("metadata.json"), &metadata)?;
    let mut raw = artifact::Raw::new(&output.join("raw.jsonl"))?;
    raw.row(&json!({"phase":"timer-control","samples_ns":timing::timer_control(2000),"subtracted":false}))?;
    for order in [FixedMatrixOrder::InputMajor, FixedMatrixOrder::OutputMajor] {
        case::<3, 2>(&mut raw, iterations, repetitions, order)?;
        case::<18, 18>(&mut raw, iterations, repetitions, order)?;
        case::<80, 1>(&mut raw, iterations, repetitions, order)?;
        case::<161, 18>(&mut raw, iterations, repetitions, order)?;
        case::<192, 4>(&mut raw, iterations, repetitions, order)?;
        case::<128, 40>(&mut raw, iterations, repetitions, order)?;
        case::<328, 192>(&mut raw, iterations, repetitions, order)?;
        case::<192, 192>(&mut raw, iterations, repetitions, order)?;
        case::<272, 480>(&mut raw, iterations, repetitions, order)?;
        case::<160, 480>(&mut raw, iterations, repetitions, order)?;
        case::<240, 384>(&mut raw, iterations, repetitions, order)?;
        case::<128, 384>(&mut raw, iterations, repetitions, order)?;
        case::<208, 384>(&mut raw, iterations, repetitions, order)?;
        case::<160, 160>(&mut raw, iterations, repetitions, order)?;
        case::<128, 128>(&mut raw, iterations, repetitions, order)?;
        case::<688, 128>(&mut raw, iterations, repetitions, order)?;

        case::<20, 40>(&mut raw, iterations, repetitions, order)?;
        case::<40, 128>(&mut raw, iterations, repetitions, order)?;
        case::<128, 160>(&mut raw, iterations, repetitions, order)?;
        case::<160, 128>(&mut raw, iterations, repetitions, order)?;
        case::<192, 160>(&mut raw, iterations, repetitions, order)?;
        case::<320, 160>(&mut raw, iterations, repetitions, order)?;
    }
    raw.finish()?;
    artifact::json(
        &output.join("summary.json"),
        &json!({"schema":"conduit.matrix-projection-performance@1","iterations":iterations,"repetitions":repetitions,"shapes":22,"orders":2,"shape_list":["3x2","18x18","80x1","161x18","192x4","128x40","328x192","192x192","272x480","160x480","240x384","128x384","208x384","160x160","128x128","688x128","20x40","40x128","128x160","160x128","192x160","320x160"],"scope":"Isolated ordered-f32 matrix arithmetic on validated immutable synthetic bytes. No full Back, Plan, FARGAN, realtime, hardware traffic or total RAM claim. No quiet timing acceptance without independent process controls.","accounting":"Fixture generation and tensor preparation measured separately; warm bulk includes function calls, finite checks, prepared/reference scratch and result guards. Both paths retain column accumulation order. Reference omits one view-field lookup. Output inspection/hashing and evidence writes occur outside warm timing; timer controls not subtracted."}),
    )?;
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let [output, iterations, repetitions] = args.as_slice() else {
        return Err("internal worker expects output/iterations/repetitions".into());
    };
    let iterations = iterations.parse()?;
    let repetitions = repetitions.parse()?;
    if !(1..=100000).contains(&iterations) || !(3..=101).contains(&repetitions) {
        return Err("finite matrix measurement bounds refused".into());
    }
    run(Path::new(output), iterations, repetitions)
}
