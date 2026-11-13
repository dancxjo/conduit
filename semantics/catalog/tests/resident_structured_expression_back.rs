#![cfg(feature = "prepared-expression-session")]
//! Generated native/no_std Back proof; no installed Body/offer/physical IO claim.
use conduit_core::*;
use conduit_plot::{rust_binding::*, *};
#[path = "../../../architecture/plot/tests/common/compiled_rust.rs"]
#[allow(dead_code)] // This consumer selects the explicit-library entrance.
mod compiled_rust;
fn field(ty: &StructuredInfoType, name: &str) -> StructuredInfoType {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|f| f.name() == name)
        .unwrap()
        .value_type()
        .clone()
}
#[test]
fn actual_display_request_uses_the_common_back_and_fixed_kernel() {
    proof(false, false);
}
#[test]
#[cfg(feature = "schema-value-storage")]
fn actual_display_request_accepts_separated_input_through_the_fixed_kernel() {
    proof(true, false);
}
#[test]
#[cfg(all(feature = "schema-value-storage", feature = "execution-workspace"))]
fn actual_display_request_lends_one_execution_workspace_for_separated_input() {
    proof(true, true);
}
#[test]
#[cfg(all(feature = "schema-value-storage", feature = "execution-workspace"))]
fn actual_display_request_lends_one_execution_workspace_for_canonical_input() {
    proof(false, true);
}
fn proof(separated: bool, shared: bool) {
    let types = check_syntax_document(
        &parse_syntax_document(include_str!(
            "../../../plots/device-protocols/i2c-types.conduit"
        )),
        &StartupCatalog::new(),
    )
    .unwrap();
    let mut catalog = StartupCatalog::new();
    for (name, path) in [
        ("I2cTransaction", "machine/i2c/transact/request"),
        ("I2cResult", "machine/i2c/transact/result"),
    ] {
        catalog
            .insert_checked_native_type_from_document(
                path,
                &types,
                &types
                    .native_types
                    .iter()
                    .find(|n| n.name == name)
                    .unwrap()
                    .identity,
            )
            .unwrap();
    }
    let document = check_syntax_document(
        &parse_syntax_document(include_str!(
            "../../../plots/device-protocols/ht16k33.conduit"
        )),
        &catalog,
    )
    .unwrap();
    let expanded = expand_canonical_plot_for_authoring(
        &document,
        "ht16k33-frame-request",
        &ProfileCatalog::new(),
    )
    .unwrap()
    .expanded;
    let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
        panic!("program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    let input = |address| {
        let ram = field(&program.input_type, "ram");
        let StructuredInfoTypeShape::Collection { element, .. } = ram.shape() else {
            panic!("collection")
        };
        StructuredInfoValue::record(
            program.input_type.clone(),
            vec![
                StructuredFieldValue::new(
                    "address",
                    StructuredInfoValue::leaf(field(&program.input_type, "address"), vec![address])
                        .unwrap(),
                )
                .unwrap(),
                StructuredFieldValue::new(
                    "ram",
                    StructuredInfoValue::collection(
                        ram.clone(),
                        (0..16)
                            .map(|n| StructuredInfoValue::leaf(element.clone(), vec![n]).unwrap())
                            .collect(),
                    )
                    .unwrap(),
                )
                .unwrap(),
            ],
        )
        .unwrap()
        .canonical_bytes()
        .unwrap()
    };
    let good = input(112);
    let bad = input(7);
    let expected = program.evaluate(&good).unwrap();
    let image = generate_static_structured_expression(
        &document,
        &program,
        PreparedNativeBounds {
            canonical_bytes: 4096,
            scopes: 64,
            invariants: 64,
            contracts: 64,
        },
        4096,
    )
    .unwrap();
    let mut additions = if shared {
        "pub type Back=conduit_semantic_catalog::WorkspaceStructuredExpressionBack<{input_native::CONTRACT_COUNT},{output_native::CONTRACT_COUNT},{CHILD_COUNT}>;".to_owned()
    } else {
        include_str!("resident_structured_expression_back/image_back.rs").to_owned()
    };
    if separated || shared {
        additions.push_str("pub fn input_schema()->&'static[u8]{input_native::prepare().unwrap().schema_bytes()} pub fn parts_back()->Back{Back::from_resident_structured_parts(prepare().unwrap(),conduit_core::PortTemporal::Value).unwrap()} pub fn canonical_back()->Back{Back::from_resident_structured(prepare().unwrap(),conduit_core::PortTemporal::Value).unwrap()}\n");
    }
    let generated = format!("mod image {{\n{image}\n{additions}\n}}");
    let probe = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../architecture/plot/tests/common/allocation_probe.rs");
    let mut exercise_source = if separated || shared {
        include_str!("resident_structured_expression_back/schema_harness.rs")
    } else {
        include_str!("resident_structured_expression_back/harness.rs")
    }
    .to_owned();
    if cfg!(feature = "execution-workspace") {
        exercise_source=exercise_source.replace("impl StepBack<1> for Back {", "impl StepBack<1> for Back { fn workspace_requirements(&self)->Option<StepWorkspaceRequirements>{match self{Self::Expression(b)=><image::Back as StepBack<1>>::workspace_requirements(b),_=>None}} fn step_with_workspace(&mut self,io:&mut StepIo<1>,input:&StepInputBytes<'_,1>,workspace:&mut StepWorkspace<'_>)->StepOutcome{match self{Self::Expression(b)=>b.step_with_workspace(io,input,workspace),_=>self.step(io,input)}}");
        exercise_source = format!("use conduit_kernel::scheduler::{{StepWorkspace,StepWorkspaceRequirements}};\n{exercise_source}");
    }
    if shared {
        exercise_source=exercise_source.replace("let (result, cost) = probe::observe", "let req=kernel.drivers()[1].workspace_requirements().unwrap(); let bytes=(req.output_bytes as usize).checked_add(req.scratch_bytes as usize).unwrap(); kernel.install_execution_workspace(conduit_kernel::scheduler::PreparedStepWorkspace::try_new(req,bytes).unwrap()).unwrap(); let (result, cost) = probe::observe");
    }
    let exercise=format!("#[path={:?}]mod probe;#[global_allocator]static ALLOC:probe::Allocator=probe::Allocator;const INPUT:&[u8]=&{good:?};const BAD:&[u8]=&{bad:?};const EXPECTED:&[u8]=&{expected:?};const SEPARATED:bool={separated};\n{exercise_source}",probe.to_str().unwrap());
    let mut libraries = vec![
        "conduit_plot",
        "conduit_core",
        "conduit_kernel",
        "conduit_semantic_catalog",
    ];
    if separated || shared {
        libraries.push("conduit_plan_lowering");
    }
    compiled_rust::verify_with_libraries(
        &generated,
        &exercise,
        if shared && separated {
            "structured-workspace-separated"
        } else if shared {
            "structured-workspace-canonical"
        } else if separated {
            "structured-common-back-separated"
        } else {
            "structured-common-back"
        },
        &libraries,
    );
}
