//! Actual emitted two-stage images; CPU/no_std proof, not admitted execution.
use super::{fixture::*, transition_equivalence};
use std::io::Write;
#[path = "../common/compiled_rust.rs"]
mod compiled_rust;

fn refusal_class(error: &conduit_plot::PortableExpressionEvaluationRefusal) -> u8 {
    use conduit_plot::PortableExpressionEvaluationRefusal as E;
    match error {
        E::InvalidInput | E::InputValueConstraint(_) => 1,
        E::Arithmetic => 2,
        other => panic!("unexpected portable refusal class: {other:?}"),
    }
}
fn write_case(writer: &mut impl Write, oracle: &mut Fixture, portable: &mut Fixture, input: &[u8]) {
    let expected = oracle.try_advance_bytes(input);
    let actual = portable.try_advance_bytes(input);
    writer
        .write_all(&u32::try_from(input.len()).unwrap().to_le_bytes())
        .unwrap();
    writer.write_all(input).unwrap();
    match (expected, actual) {
        (Ok(expected), Ok(actual)) => {
            assert_eq!(actual, expected);
            writer.write_all(&[0]).unwrap();
            writer
                .write_all(&u32::try_from(expected.len()).unwrap().to_le_bytes())
                .unwrap();
            writer.write_all(expected).unwrap();
        }
        (Err(expected), Err(actual)) => {
            let class = refusal_class(&expected);
            assert_eq!(refusal_class(&actual), class);
            writer.write_all(&[class]).unwrap();
            writer.write_all(&0u32.to_le_bytes()).unwrap();
        }
        (expected, actual) => panic!("portable/oracle disagreement: {expected:?}/{actual:?}"),
    }
}
#[test]
#[ignore = "finite full corpus and compiled owned/borrowed CPU/no_std images; run explicitly"]
fn exact_two_gear_generated_images_match_original_oracle_without_allocating() {
    let source = transition_equivalence::PRODUCTION;
    let checked = transition_equivalence::checked(source);
    let programs = transition_programs(&checked);
    assert_eq!(programs.len(), 2);
    let directory =
        std::env::temp_dir().join(format!("conduit-bme-static-corpus-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let corpus = directory.join("corpus.bin");
    let mut writer = std::io::BufWriter::new(std::fs::File::create(&corpus).unwrap());
    let mut oracle = Fixture::staged_oracle();
    let mut portable = Fixture::from_source(source);
    let mut records = 0usize;
    eprintln!("static execution stage=portable-oracle-corpus");
    let boundaries = transition_equivalence::visit_qualification_inputs(
        &mut oracle,
        &mut portable,
        |oracle, portable, input| {
            write_case(&mut writer, oracle, portable, input);
            records += 1;
        },
    );
    // A valid canonical record missing a required member is rejected, beyond
    // merely truncating a byte stream. This is malformed DATA, never a witness.
    let initial = oracle.initial();
    let valid = oracle.input_bytes(initial, oracle.tick(1));
    let schema = programs[0].input_type.canonical_bytes().unwrap();
    assert!(valid.starts_with(&schema));
    let mut missing = schema;
    missing.extend_from_slice(&[2, 0, 0, 0, 0]); // empty record node
    assert!(oracle.try_advance_bytes(&missing).is_err());
    write_case(&mut writer, &mut oracle, &mut portable, &missing);
    records += 1;
    writer.flush().unwrap();
    drop(writer);
    eprintln!(
        "static execution corpus boundary_cases={boundaries} records={records} file_bytes={}",
        std::fs::metadata(&corpus).unwrap().len()
    );
    let allocator =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/common/allocation_probe.rs");
    for borrowed in [false, true] {
        let mut generated = String::new();
        let mut work = Vec::new();
        for (name, program) in ["guards", "phases"].into_iter().zip(&programs) {
            let bounds = conduit_plot::rust_binding::PreparedNativeBounds {
                canonical_bytes: 4096,
                scopes: 64,
                invariants: 64,
                contracts: 64,
            };
            let image = if borrowed {
                conduit_plot::generate_borrowed_static_structured_expression_with_work(
                    &checked, program, bounds, 4096,
                )
            } else {
                conduit_plot::generate_static_structured_expression_with_work(
                    &checked, program, bounds, 4096,
                )
            }
            .expect("unchanged constructor/native bounds");
            work.push(image.computation_work);
            generated.push_str(&format!("mod {name}{{\n{}\n}}\n", image.source));
        }
        assert_eq!(work, [1723, 5379], "work charging changed");
        let exercise = format!("#[path={allocator:?}]mod probe;#[global_allocator]static ALLOC:probe::Allocator=probe::Allocator;const CORPUS:&str={corpus:?};const RECORDS:usize={records};\n{DRIVER}");
        eprintln!(
            "static execution stage=compile-run borrowed={borrowed} generated_rust_bytes={}",
            generated.len()
        );
        compiled_rust::verify(
            &generated,
            &exercise,
            if borrowed {
                "bme-transition-borrowed"
            } else {
                "bme-transition-owned"
            },
        );
    }
    std::fs::remove_dir_all(directory).unwrap();
}

const DRIVER: &str = r#"
fn class(error:conduit_plot::StaticStructuredRefusal)->u8 {
 use conduit_plot::{StaticStructuredRefusal as E,StaticProjectedRefusal as P,PortableExpressionEvaluationRefusal as R};
 match error {
  E::Input(_)=>1,
  E::Expression(R::InvalidInput|R::InputValueConstraint(_))=>1,
  E::Expression(R::Arithmetic)=>2,
  E::Child{refusal:P::Expression(R::Arithmetic),..}=>2,
  E::Child{refusal:P::Expression(R::InvalidInput|R::InputValueConstraint(_)),..}=>1,
  other=>panic!("unexpected generated refusal:{other:?}")
 }
}
fn length(bytes:&[u8], offset:&mut usize)->usize {
 let n=u32::from_le_bytes(bytes[*offset..*offset+4].try_into().unwrap())as usize;
 *offset+=4;n
}
fn main(){
 let data=std::fs::read(CORPUS).unwrap(); // CPU fixture allocation before observations
 let (mut guards,c)=probe::observe(||guards::prepare().unwrap());assert_eq!(c.allocations,0);assert_eq!(c.reallocations,0);
 let (mut phases,c)=probe::observe(||phases::prepare().unwrap());assert_eq!(c.allocations,0);assert_eq!(c.reallocations,0);
 assert_eq!(guards.computation_work(),1723);assert_eq!(phases.computation_work(),5379);
 assert!(guards.maximum_output_bytes()<=4096&&phases.maximum_output_bytes()<=4096);
 assert!(guards.scratch_bytes()<=4096&&phases.scratch_bytes()<=4096);
 let mut middle=[0xa5u8;4096];let mut output=[0xa5u8;4096];let mut gs=[0u8;4096];let mut ps=[0u8;4096];
 // Revisit changing inputs using the SAME prepared owners and scratch. Every
 // generated evaluation, including refusal, must remain allocation-free.
 for repetition in 0..2 {
  let mut offset=0usize;let mut count=0usize;
  while offset<data.len(){
   let n=length(&data,&mut offset);let input=&data[offset..offset+n];offset+=n;
   let expected_class=data[offset];offset+=1;let n=length(&data,&mut offset);let expected=&data[offset..offset+n];offset+=n;
   middle.fill(0xa5);output.fill(0xa5);
   let (actual,c)=probe::observe(||{
    let n=guards.evaluate_into(input,&mut middle,&mut gs).map_err(class)?;
    phases.evaluate_into(&middle[..n],&mut output,&mut ps).map_err(class)
   });
   assert_eq!(c.allocations,0,"case{count}");assert_eq!(c.reallocations,0,"case{count}");
   match actual {
    Ok(n)=>{assert_eq!(expected_class,0,"case{count}");assert_eq!(&output[..n],expected,"case{count}");assert!(output[n..].iter().all(|b|*b==0xa5));}
    Err(actual)=>{assert_eq!(actual,expected_class,"case{count}");assert!(output.iter().all(|b|*b==0xa5));}
   }
   count+=1;
  }
  assert_eq!(count,RECORDS);println!("actual generated BME pipeline repetition={repetition} cases={count} allocations=0 reallocations=0");
 }
}
"#;
