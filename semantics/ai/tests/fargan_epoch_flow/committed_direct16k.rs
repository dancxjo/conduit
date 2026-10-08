//! Pinned private common-carrier fixture preparation, not a replacement common IR.
//! Root's original common owner supplies the admitted carrier and Audio projection.
//! This controller retains their terminal evidence and executes the numeric bridge.
use super::*;
use sha2::{Digest, Sha256};
use std::path::Path;

fn field<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value()
}
fn native_bytes(value: &serde_json::Value) -> Vec<u8> {
    serde_json::from_value(value.clone()).unwrap()
}
fn retained_file(root: &Path, name: &str, expected: &str, custody: &mut Vec<u8>) -> Vec<u8> {
    let bytes = std::fs::read(root.join(name)).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        expected,
        "pinned terminal original {name}"
    );
    custody.extend_from_slice(&(name.len() as u64).to_le_bytes());
    custody.extend_from_slice(name.as_bytes());
    custody.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    custody.extend_from_slice(&bytes);
    bytes
}
fn cycle_q8(entry: &serde_json::Value, grid: &[u8]) -> (u64, serde_json::Value) {
    let source = include_str!("../../../speech/fargan_direct16k_cycle_q8.conduit");
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let graph = expand_canonical_plot_for_authoring(
        &checked,
        "speech/fargan-direct16k-cycle-q8",
        &ProfileCatalog::new(),
    )
    .unwrap();
    let ConfigurationValue::Text(hex) = &graph.expanded.gears[0].configuration[0].value else {
        panic!("Source program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(hex).unwrap();
    let target =
        StructuredInfoValue::from_canonical_bytes(&native_bytes(&entry["integer_target"])).unwrap();
    let result =
        StructuredInfoValue::from_canonical_bytes(&native_bytes(&entry["result"])).unwrap();
    assert_eq!(field(&target, "basis").canonical_bytes().unwrap(), grid);
    assert_eq!(
        field(field(&result, "request"), "basis")
            .canonical_bytes()
            .unwrap(),
        grid
    );
    assert_eq!(
        field(&result, "request").canonical_bytes().unwrap(),
        native_bytes(&entry["request"])
    );
    let StructuredInfoValueShape::Variant { tag, payload } =
        field(field(&result, "request"), "quantity").shape()
    else {
        panic!("cycle variant")
    };
    assert_eq!(tag, "cycle");
    assert_eq!(
        payload.canonical_bytes().unwrap(),
        native_bytes(&entry["original_cycle_bytes"])
    );
    assert_eq!(
        field(&target, "whole_frames"),
        field(field(&result, "raw"), "whole_frames")
    );
    for execution in entry["source_executions"].as_array().unwrap() {
        let p =
            PortableExpressionProgram::from_canonical_hex(execution["program"].as_str().unwrap())
                .unwrap();
        assert_eq!(
            p.evaluate(&native_bytes(&execution["input"])).unwrap(),
            native_bytes(&execution["output"])
        );
    }
    let ty = &checked.native_types[0].value_type;
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("eligibility")
    };
    let copied = [
        (
            "sample_rate_hz",
            field(field(&target, "basis"), "sample_rate_hz"),
        ),
        ("whole_frames", field(&target, "whole_frames")),
        (
            "remainder_numerator",
            field(field(&result, "raw"), "remainder_numerator"),
        ),
    ];
    let input = StructuredInfoValue::record(
        ty.clone(),
        fields
            .iter()
            .map(|f| {
                StructuredFieldValue::new(
                    f.name(),
                    copied
                        .iter()
                        .find(|(n, _)| *n == f.name())
                        .unwrap()
                        .1
                        .clone(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap();
    let input = input.canonical_bytes().unwrap();
    super::interface::admit_retained_session_native(
        &checked,
        "FarganDirect16kCycleQ8Eligible",
        &input,
    )
    .unwrap();
    let output = program.evaluate(&input).unwrap();
    let admitted = StructuredInfoValue::leaf(program.output_type.clone(), output.clone()).unwrap();
    assert_eq!(
        StructuredInfoValue::from_canonical_bytes(&admitted.canonical_bytes().unwrap()).unwrap(),
        admitted
    );
    let q8 = u64::from_le_bytes(output.as_slice().try_into().unwrap());
    (
        q8,
        serde_json::json!({"original_projection":entry,"source":source,"program":hex,"eligible_input":input,"raw_output":output,"admitted_output":admitted.canonical_bytes().unwrap()}),
    )
}

#[test]
#[ignore = "full admitted common greeting and retained trained model; slow actual Source/Kernel inference"]
fn original_committed_common_greeting_runs_direct16k_trained_continuation() {
    std::thread::Builder::new().stack_size(64*1024*1024).spawn(|| {
        let root=std::path::PathBuf::from(std::env::var("CONDUIT_FARGAN_COMMON_CARRIER").unwrap());
        let mut custody=vec![];
        let wav=retained_file(&root,"hello-travis-16000.wav","7b8da729c301cc9ed56a309f85d6d381472157c0347e9f67a4e75f37f3835024",&mut custody);
        let carrier=retained_file(&root,"carrier-and-projection.json","6762a898323d49f94061ecac6659de947811308a58f5b711e1d2a8ee45e8b542",&mut custody);
        retained_file(&root,"dsp-frames.bin","083eb78c9fee3941084b8ae7b59a0ff0b558608cb880485ebd88cd554c536efa",&mut custody);
        retained_file(&root,"control-executions.bin","77e22d2c16725987a9f5e535d75a7ccc98657299937e365eda4be91cc6bee7d4",&mut custody);
        let cycles=retained_file(&root,"cycle-sample-grid.json","e027380533e0851e59fc6d03b91f6e84daecd2bab815f8bb3e23247bff1b1f94",&mut custody);
        assert!(custody.len()<=256*1024*1024,"finite original-evidence resource preparation profile");
        let carrier:serde_json::Value=serde_json::from_slice(&carrier).unwrap();
        let cycles:serde_json::Value=serde_json::from_slice(&cycles).unwrap();
        assert_eq!(cycles["original_carrier_sha256"],"6762a898323d49f94061ecac6659de947811308a58f5b711e1d2a8ee45e8b542");
        assert_eq!(carrier["commitments"],2);assert_eq!(carrier["segments"],10);
        assert_eq!(carrier["common_ipa_carrier"]["original_carrier_is_committed"],true);
        assert_eq!(carrier["formant_projection"]["rate_hz"],16000);
        assert_eq!(&wav[..4],b"RIFF");assert_eq!(&wav[8..16],b"WAVEfmt ");
        assert_eq!(u16::from_le_bytes(wav[20..22].try_into().unwrap()),1);
        assert_eq!(u16::from_le_bytes(wav[22..24].try_into().unwrap()),1);
        assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()),16000);
        assert_eq!(u16::from_le_bytes(wav[34..36].try_into().unwrap()),16);
        assert_eq!(&wav[36..40],b"data");assert_eq!(wav.len(),44+u32::from_le_bytes(wav[40..44].try_into().unwrap()) as usize);
        let pcm=wav[44..].as_chunks::<2>().0.iter().map(|b|i16::from_le_bytes(*b)).collect::<Vec<_>>();
        let policy=super::direct16k::explicit_analysis_policy();
        let grid=native_bytes(&carrier["formant_projection"]["grid_bytes"]);
        let mut observations=vec![];let mut q8_receipts=vec![];
        for (i,entry) in cycles["cycles"].as_array().unwrap().iter().enumerate(){
            assert_eq!(entry["segment_ordinal"],i);
            assert_eq!(entry["original_cycle_bytes"],carrier["formant_projection"]["cycle_bytes"][i]);
            let (q8,receipt)=cycle_q8(entry,&grid);q8_receipts.push(receipt);
            let frames=carrier["formant_projection"]["frame_counts"][i].as_u64().unwrap();
            assert!(frames.is_multiple_of(160));
            observations.extend(std::iter::repeat_n((false,q8),usize::try_from(frames/160).unwrap()));
        }
        custody.extend_from_slice(&serde_json::to_vec(&q8_receipts).unwrap());
        let tape=super::direct16k::RetainedDirect16kTape::prepare(&pcm,&custody,Some(&policy)).unwrap();
        assert_eq!(tape.epochs().len(),observations.len());assert_eq!(tape.epochs().len(),200);
        let periods=super::runtime::run_native_period_controls(&observations);
        let proposal=super::runtime::run_native_first_feature16k(&tape.epochs()[0],&periods[0]);
        let model_root=std::path::PathBuf::from(std::env::var("CONDUIT_FARGAN_MODEL_FIXTURE").unwrap());
        let mut model=super::custody::RetainedSignalModel::load(&model_root);
        let (_,conditioning)=model.conditioning_resources();
        for(name,resource)in conditioning {assert!(model.resources.insert(format!("conditioning_{name}"),resource).is_none());}
        let warm=super::runtime::run_native_warm_startup(&model,&proposal);
        let epochs=super::runtime::run_direct16k_trained_utterance(&model,&tape,&periods,&proposal,&warm);
        let (neural,alignment)=super::native_startup::align_native_pcm_for_epochs(&epochs,200);
        assert_eq!(neural.len(),pcm.len());
        let output=std::path::PathBuf::from(std::env::var("CONDUIT_FARGAN_COMMON_OUTPUT").unwrap());std::fs::create_dir_all(&output).unwrap();
        let mut neural_wav=wav[..44].to_vec();for sample in &neural{neural_wav.extend_from_slice(&sample.to_le_bytes());}
        std::fs::write(output.join("hello-travis-fargan-16000.wav"),&neural_wav).unwrap();
        std::fs::write(output.join("alignment-source.conduit"),&alignment).unwrap();
        std::fs::write(output.join("raw-model-epochs.json"),serde_json::to_vec(&epochs.iter().map(|e|e.canonical_bytes().unwrap()).collect::<Vec<_>>()).unwrap()).unwrap();
        std::fs::write(output.join("cycle-q8-executions.json"),serde_json::to_vec(&q8_receipts).unwrap()).unwrap();
        let receipt=serde_json::json!({"schema":"fargan/original-committed-common-greeting-direct16k@1","original_carrier_sha256":"6762a898323d49f94061ecac6659de947811308a58f5b711e1d2a8ee45e8b542","original_wav_sha256":"7b8da729c301cc9ed56a309f85d6d381472157c0347e9f67a4e75f37f3835024","neural_wav_sha256":format!("{:x}",Sha256::digest(&neural_wav)),"native_epochs":200,"raw_model_rows":epochs.len(),"source_continuation_epochs":2,"sample_rate_hz":16000,"frames":neural.len(),"complete_multiword_fixture":true,"analysis_policy":policy,"original_full_evidence_bytes":custody.len(),"actual_source_kernel_scheduler_drained":true,"model_blob_sha256":model.raw_blob_sha256,"model_compute_lifecycle_admitted":false,"rich_prosody":false,"continuous_coarticulation":false,"attended_listening":false,"physical_playback":false,"target_boot":false});
        std::fs::write(output.join("manifest.json"),serde_json::to_vec_pretty(&receipt).unwrap()).unwrap();
    }).unwrap().join().unwrap();
}
#[test]
fn direct16k_q8_profile_refuses_foreign_rate_fractional_cycle_and_bounds() {
    let source=include_str!("../../../speech/fargan_direct16k_cycle_q8.conduit");
    let checked=check_syntax_document(&parse_syntax_document(source),&StartupCatalog::new()).unwrap();
    let ty=&checked.native_types[0].value_type;
    let StructuredInfoTypeShape::Record{fields,..}=ty.shape() else {panic!("eligibility")};
    let graph=expand_canonical_plot_for_authoring(&checked,"speech/fargan-direct16k-cycle-q8",&ProfileCatalog::new()).unwrap();
    let ConfigurationValue::Text(hex)=&graph.expanded.gears[0].configuration[0].value else {panic!("Source")};
    let program=PortableExpressionProgram::from_canonical_hex(hex).unwrap();
    for (rate,frames,remainder,accepted) in [(16000u64,32u64,0u64,true),(16000,80,0,true),(16000,255,0,true),(8000,80,0,false),(48000,80,0,false),(16000,31,0,false),(16000,256,0,false),(16000,u64::MAX,0,false),(16000,80,1,false),(16000,0,0,false)] {
        let input=StructuredInfoValue::record(ty.clone(),fields.iter().map(|f|StructuredFieldValue::new(f.name(),StructuredInfoValue::leaf(f.value_type().clone(),match f.name(){"sample_rate_hz"=>rate,"whole_frames"=>frames,"remainder_numerator"=>remainder,_=>panic!("field")}.to_le_bytes().to_vec()).unwrap()).unwrap()).collect()).unwrap().canonical_bytes().unwrap();
        let admitted=super::interface::admit_retained_session_native(&checked,"FarganDirect16kCycleQ8Eligible",&input);
        assert_eq!(admitted.is_ok(),accepted);
        if accepted {let output=program.evaluate(&input).unwrap();assert_eq!(u128::from(u64::from_le_bytes(output.try_into().unwrap())),u128::from(frames)*256);}
    }
}
