{
 use conduit_language::{committed_discourse::*,committed_prosody::*};
 use conduit_plot::PortableExpressionProgram;
 use owner::checked;
 let discourse=prepare_committed_vocative_fact("committed/addressee".into(),&case.lexical,&commitments[1],LinguisticDerivationProvenance::deterministic_rule("language/committed-vocative-discourse".into(),"actual-retained-commit/1".into()).unwrap()).unwrap();
 let rich=prepare_rich_prosody_from_committed_vocative(&case.lexical,2,&discourse,case.rich.requested().profile()).unwrap();
 let trajectory=SpeechLinearPitchTrajectory::new(SpeechExactDuration::new(5,6).unwrap(),SpeechFundamentalCycle::new(300,1).unwrap(),SpeechFundamentalCycle::new(200,1).unwrap()).unwrap();
 let word_grid=PortableExpressionProgram::from_canonical_hex(GRID).unwrap();
 let fraction=PortableExpressionProgram::from_canonical_hex(FRACTION).unwrap();
 let end=PortableExpressionProgram::from_canonical_hex(END).unwrap();
 let cycle_at=|point:&checked::SpeechWordPitchPointRequest|{
  let fraction=checked::SpeechPitchCycleFraction::decode(&fraction.evaluate(&word_grid.evaluate(&point.clone().encode().unwrap()).unwrap()).unwrap()).unwrap();
  SpeechFundamentalCycle::new(*fraction.denominator(),*fraction.numerator()).unwrap()
 };
 let reduce=|original:SpeechFundamentalCycle|{
  let (mut a,mut b)=(*original.numerator_seconds(),*original.denominator());
  while b!=0 {let next=a%b;a=b;b=next;}
  let reduced=SpeechFundamentalCycle::new(*original.denominator()/a,*original.numerator_seconds()/a).unwrap();
  checked::SpeechWordPitchCycleReduction::new(a,original,reduced).unwrap()
 };
 let replace_cycle=|s:&SpeechPlannedSegmentIntent,c:&SpeechFundamentalCycle|{
  let prosody=SpeechSegmentProsodyIntent::new(s.prosody().duration().clone(),SpeechCycleSpecification::known(*c.denominator(),*c.numerator_seconds()).unwrap(),s.prosody().relative_intensity().clone()).unwrap();
  SpeechPlannedSegmentIntent::new(s.occurrence().clone(),s.phone().clone(),s.phoneme().clone(),prosody,s.provenance().clone(),s.sources().clone(),s.stress().clone(),s.word_position().clone()).unwrap()
 };
 let mut offset=SpeechExactDuration::new(5,0).unwrap();
 let mut admissions=Vec::new();
 for witness in witnesses.iter().filter(|w|*w.word_position()==1){
  let segment=witness.composite();
  let point=checked::SpeechWordPitchPointRequest::new(offset.clone(),trajectory.clone()).unwrap();
  let partition=checked::SpeechWordPitchSegmentPartitionRequest::new(segment.clone(),point.clone()).unwrap();
  let raw=checked::SpeechWordPitchRawOffset::decode(&end.evaluate(&partition.clone().encode().unwrap()).unwrap()).unwrap();
  offset=SpeechExactDuration::new(*raw.denominator(),*raw.numerator_seconds()).unwrap();
  let end_point=checked::SpeechWordPitchPointRequest::new(offset.clone(),trajectory.clone()).unwrap();
  let cycle=cycle_at(&point);
  let realized=replace_cycle(segment,&cycle);
  let realization=checked::SpeechWordPitchSegmentRealization::new(partition,realized.clone()).unwrap();
  let reduction=reduce(cycle);
  let reduced=replace_cycle(&realized,reduction.reduced());
  let realization=checked::SpeechWordPitchReducedSegmentRealization::new(reduction,realization,reduced).unwrap();
  let backend=checked::SpeechWordPitchBackendAdmission::new(reduce(cycle_at(&end_point)),end_point,realization,16000).unwrap();
  let requested=checked::SpeechWordPitchRequestedContourAdmission::new(rich.prepared().accepted().clone(),point).unwrap();
  admissions.push(checked::SpeechWordPitchRichBasisAdmission::new(backend,requested,0).unwrap());
 }
 let prepared=owner::prepare(&committed,&rich,&admissions).unwrap();
 assert!(core::ptr::eq(prepared.original(),&committed));
 assert!(core::ptr::eq(prepared.rich(),&rich));
 assert_eq!(prepared.admissions(),admissions);
 assert_eq!(prepared.segment_pitch().len(),6);
 assert_eq!(prepared.realized().events().len(),composite.events().len());
 assert_ne!(prepared.realized(),&composite);
 let realized_shared=PreparedSpeechUtteranceIntent::prepare(prepared.realized(),SpeechIntentComponents{
  context:&context,intended_text:Some(text),phonemes:&phoneme_sequence,phones:&phone_sequence,
  morphemes:&[],morpheme_texts:&[],syllables:&syllables,correspondences:&correspondences,
 }).unwrap();
 let realized_ipa=PreparedIpaSpeechUtteranceIntent::prepare(&realized_shared,&admitted_ipa).unwrap();
 assert!(core::ptr::eq(realized_ipa.shared().original(),prepared.realized()));
 assert_eq!(realized_ipa.phonemes().len(),10);
 assert!(core::ptr::eq(realized_ipa.ipa(),&admitted_ipa));
 // Reprepare contextual gestures from the SAME newly realized IPA owner.
 let realized_choices=positions.iter().enumerate().map(|(i,position)|select_intent_allophone(
  prepared.realized(),i,&linguistic_inventory,&policy,ExplicitAllophoneContext{
   syllable_position:position,prosodic_context:&unspecified_prosody,careful_style:&careful,
  },
 ).unwrap()).collect::<Vec<_>>();
 let realized_gestures=realized_choices.iter().zip(&timings).map(|(choice,timing)|
  conduit_speech::ipa_gestures::prepare_ipa_contextual_greeting_phone_gestures(
   &realized_ipa,choice,timing,SpeechGreetingLossPolicy::AcceptLateralRhoticAndStepDiphthongApproximationV2,
  ).unwrap()
 ).collect::<Vec<_>>();
 use conduit_speech::{linguistic_prosody::*,pitch_trajectory::*};
 let bindings=prepared.segment_pitch().iter().map(|pitch|SpeechLinguisticProsodyBinding::new(
  rich.prepared().accepted().choice().clone(),case.voice.identity().clone(),
  case.lexical.tape().source().material().language().clone(),rich.prepared().requested().profile().identity().clone(),
  language::speech_provenance(),SpeechLinguisticProsodyRealization::new(
   SpeechDurationSpecification::known(100,3).unwrap(),SpeechBoundarySpecification::Known(SpeechBoundaryKind::Phrase),
   pitch.segment().prosody().clone(),
  ).unwrap(),
 ).unwrap()).collect::<Vec<_>>();
 let realized_pitch=prepared.segment_pitch().iter().zip(&bindings).map(|(pitch,binding)|
  prepare_linguistic_pitch(LinguisticProsodyBasis::Rich(rich.prepared()),binding,pitch.segment(),pitch.trajectory()).unwrap()
 ).collect::<Vec<_>>();
 let cycle_projections=prepared.segment_pitch().iter().map(|pitch|{
  let SpeechCycleSpecification::Known(c)=pitch.segment().prosody().fundamental_cycle() else{panic!()};
  speech_cycle_to_audio(&SpeechFundamentalCycle::new(*c.denominator(),*c.numerator_seconds()).unwrap().encode().unwrap()).unwrap()
 }).collect::<Vec<_>>();
 let realized_renderers=realized_gestures.iter().skip(4).zip(&cycle_projections).map(|(gesture,cycle)|
  prepare_greeting_renderer_q8(gesture.contextual().profile(),&grid,cycle.admitted_canonical()).unwrap()
 ).collect::<Vec<_>>();
 use conduit_speech::bounded_pitch_programs::{PitchPrograms,Limits,PROGRAMS};
 let required=PitchPrograms::requirements(PROGRAMS).unwrap();
 let programs=PitchPrograms::prepare(PROGRAMS,Limits{retained:required.retained_bound,preparation:required.preparation_bound}).unwrap();
 let mut swapped=PROGRAMS;swapped.swap(0,1);
 let foreign_required=PitchPrograms::requirements(swapped).unwrap();
 let foreign=PitchPrograms::prepare(swapped,Limits{retained:foreign_required.retained_bound,preparation:foreign_required.preparation_bound}).unwrap();
 assert!(matches!(prepare_exact_material_linguistic_greeting_renderer(&realized_gestures[4],&realized_renderers[0],&realized_pitch[0],foreign),Err(SpeechGestureRenderRefusal::ForeignBasis)));
 let renderers=realized_gestures.iter().skip(4).zip(&realized_renderers).zip(&realized_pitch).map(|((gesture,renderer),pitch)|
  prepare_exact_material_linguistic_greeting_renderer(gesture,renderer,pitch,std::rc::Rc::clone(&programs)).unwrap()
 ).collect::<Vec<_>>();
 assert_eq!(std::rc::Rc::strong_count(&programs),7);
 assert!(matches!(renderers[0].next(&mut renderers[1].cursor()),Err(SpeechGestureRenderRefusal::ForeignBasis)));
 let mut first_frames=Vec::new();
 for renderer in &renderers{
  assert_eq!(renderer.endpoint_checks().len(),2);
  assert!(renderer.renderer().first_target().q8_initialization().is_some());
  let frame=renderer.next(&mut renderer.cursor()).unwrap().unwrap();
  let admission=frame.pitch().admission();
  assert_eq!(admission.original().pitch().frame(),&0);
  first_frames.push(serde_json::json!({
   "pitch_admission":admission.clone().encode().unwrap(),
   "dsp_input":frame.rendered().rendered().dsp_input_canonical(),
   "dsp_output":frame.rendered().rendered().dsp_output_canonical(),
   "sample":frame.rendered().rendered().sample(),
   "executions":frame.pitch().executions().iter().map(|x|serde_json::json!({"program":x.source_program_hex(),"input":x.input_canonical(),"output":x.output_canonical()})).collect::<Vec<_>>(),
  }));
 }
 assert_eq!(first_frames.len(),6);
 std::fs::write(std::env::var("CONDUIT_WORD_PITCH_OWNER_RECEIPT").unwrap(),serde_json::to_vec(&serde_json::json!({
  "scope":"Work-only actual committed complete greeting; continuous Travis word pitch realized into same original IPA inventory/context; all six actual gesture/DSP plans and endpoints; one first frame per Travis segment only; no complete PCM/WAV/neural/playback proof",
  "original_intent":composite.clone().encode().unwrap(),"realized_intent":prepared.realized().clone().encode().unwrap(),
  "rich_accepted":rich.prepared().accepted().clone().encode().unwrap(),
  "admissions":prepared.admissions().iter().map(|x|x.clone().encode().unwrap()).collect::<Vec<_>>(),
  "first_frames":first_frames,
 })).unwrap()).unwrap();


 for i in 0..4 {assert_eq!(prepared.realized().events().iter().nth(i),composite.events().iter().nth(i));}
 for pair in prepared.segment_pitch().windows(2){assert_eq!(pair[0].trajectory().end(),pair[1].trajectory().start());}
 assert!(matches!(owner::prepare(&committed,&rich,&admissions[..5]),Err(owner::Refusal::Count)));
 let mut swapped=admissions.clone();swapped.swap(0,1);
 assert!(matches!(owner::prepare(&committed,&rich,&swapped),Err(owner::Refusal::ForeignSegment)));
 // Equal Native commitment bytes at a foreign address do not grant custody.
 let foreign_commit=commitments[1].clone();
 let foreign_discourse=prepare_committed_vocative_fact("foreign/addressee".into(),&case.lexical,&foreign_commit,LinguisticDerivationProvenance::deterministic_rule("language/committed-vocative-discourse".into(),"actual-retained-commit/1".into()).unwrap()).unwrap();
 let foreign_rich=prepare_rich_prosody_from_committed_vocative(&case.lexical,2,&foreign_discourse,case.rich.requested().profile()).unwrap();
 assert!(matches!(owner::prepare(&committed,&foreign_rich,&admissions),Err(owner::Refusal::ForeignCommitment)));
 println!("PASS actual original four-revision committed greeting: opaque complete-coverage/rich-owner custody; six Source word-pitch admissions; new complete10segment intent; unchanged Hello; adjacent exact cycles; missing/swapped/foreign-address commitment refusals. Six actual first DSP frames only; no complete waveform or playback.");

 if let Ok(output_directory)=std::env::var("CONDUIT_WORD_PITCH_FULL_OUTPUT") {
 // Actual complete new IPA-owner realization; Hello retains its original cycles.
 let hello_renderers=realized_gestures.iter().take(4).zip(cycles.iter().take(4)).map(|(gesture,cycle)|prepare_greeting_renderer_q8(gesture.contextual().profile(),&grid,cycle.admitted_canonical()).unwrap()).collect::<Vec<_>>();
 let directory=std::path::PathBuf::from(output_directory);std::fs::create_dir_all(&directory).unwrap();
 use std::io::Write;
 let mut records=std::io::BufWriter::new(std::fs::File::create(directory.join("pitch-and-dsp-frames.bin")).unwrap());
 let mut all_samples=Vec::<i16>::new();let mut full_programs=Vec::<String>::new();
 let mut emit=|pitch:Vec<u8>,rendered:&GreetingRenderedFrame,pitch_executions:&[SpeechCommonAcousticExecution]|{
  let frame=rendered.rendered();let ordinal=all_samples.len();all_samples.push(i16::try_from(frame.sample()).unwrap());
  records.write_all(&u32::try_from(ordinal).unwrap().to_le_bytes()).unwrap();
  for bytes in [pitch.as_slice(),frame.dsp_input_canonical(),frame.dsp_output_canonical()] {records.write_all(&u32::try_from(bytes.len()).unwrap().to_le_bytes()).unwrap();records.write_all(bytes).unwrap();}
  let executions=pitch_executions.iter().chain(frame.executions()).chain(rendered.target_selection_and_reset_executions()).collect::<Vec<_>>();records.write_all(&u32::try_from(executions.len()).unwrap().to_le_bytes()).unwrap();
  for execution in executions {let program=full_programs.iter().position(|x|x==execution.source_program_hex()).unwrap_or_else(||{full_programs.push(execution.source_program_hex().into());full_programs.len()-1});records.write_all(&u32::try_from(program).unwrap().to_le_bytes()).unwrap();for bytes in [execution.input_canonical(),execution.output_canonical()] {records.write_all(&u32::try_from(bytes.len()).unwrap().to_le_bytes()).unwrap();records.write_all(bytes).unwrap();}}
 };
 // All ten original/new plans, six continuous word partitions, and twelve
 // pitch endpoints have already been prepared before any complete-run frame.
 for renderer in &hello_renderers {let mut cursor=renderer.cursor();while let Some(frame)=renderer.next(&mut cursor).unwrap(){emit(Vec::new(),&frame,&[]);}}
 for renderer in &renderers {let mut cursor=renderer.cursor();while let Some(frame)=renderer.next(&mut cursor).unwrap(){emit(frame.pitch().admission().clone().encode().unwrap(),frame.rendered(),frame.pitch().executions());}}
 records.flush().unwrap();assert_eq!(all_samples.len(),32000);assert!(all_samples.iter().any(|x|*x!=0));
 let bytes=u32::try_from(all_samples.len()*2).unwrap();let mut wav=Vec::new();wav.extend_from_slice(b"RIFF");wav.extend_from_slice(&(36+bytes).to_le_bytes());wav.extend_from_slice(b"WAVEfmt ");wav.extend_from_slice(&16u32.to_le_bytes());wav.extend_from_slice(&1u16.to_le_bytes());wav.extend_from_slice(&1u16.to_le_bytes());wav.extend_from_slice(&16000u32.to_le_bytes());wav.extend_from_slice(&32000u32.to_le_bytes());wav.extend_from_slice(&2u16.to_le_bytes());wav.extend_from_slice(&16u16.to_le_bytes());wav.extend_from_slice(b"data");wav.extend_from_slice(&bytes.to_le_bytes());for sample in &all_samples {wav.extend_from_slice(&sample.to_le_bytes());}std::fs::write(directory.join("hello-travis-continuous-word-pitch-16000.wav"),wav).unwrap();
 std::fs::write(directory.join("carrier-and-pitch.json"),serde_json::to_vec(&serde_json::json!({"scope":"Actual production workspace committed complete greeting, same original/new IPA owners; constant original Hello cycles and continuous Travis word pitch; Source exact-material Q8 and explicit fractional initialization; no pause/prominence/coarticulation/neural/played/attended claim", "record_format":"u32 ordinal; three length-prefixed buffers pitch admission (empty for original constant Hello), DSP input, DSP output; u32 executions, each u32 program index and length-prefixed input/output", "frames":32000,"sample_rate_hz":16000,"original_intent":composite.clone().encode().unwrap(),"realized_intent":prepared.realized().clone().encode().unwrap(),"rich_accepted":rich.prepared().accepted().clone().encode().unwrap(),"admissions":prepared.admissions().iter().map(|x|x.clone().encode().unwrap()).collect::<Vec<_>>(),"source_programs":full_programs,"shared_source_program_storage":{"retained":programs.storage().actual_retained,"retained_bound":programs.storage().retained_bound,"preparation_bound":programs.storage().preparation_bound}})).unwrap()).unwrap();

 }
}
