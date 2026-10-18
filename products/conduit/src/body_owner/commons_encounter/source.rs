//! Preparation of the actual checked Commons Source, never lifecycle authority.
use conduit_core::{
    ConfigurationValue, InfoBool, StructuredFieldValue, StructuredInfoType,
    StructuredInfoTypeShape, StructuredInfoValue, StructuredInfoValueShape,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    CheckedSyntaxDocument, PortableExpressionProgram, PreparedPortableExpressionEvaluator,
    ProfileCatalog, StartupCatalog,
};

pub(super) const SOURCE: &str = include_str!("../../../../../plots/commons/main.conduit");
pub(super) const FRAME_BYTES: usize = 32768;

pub(super) struct Source {
    pub checked: CheckedSyntaxDocument,
    pub places: StructuredInfoValue,
    request_type: StructuredInfoType,
    evaluator: PreparedPortableExpressionEvaluator,
    admission: [PreparedPortableExpressionEvaluator; 3],
}
impl Source {
    pub fn prepare() -> Result<Self, String> {
        let checked = check_syntax_document(&parse_syntax_document(SOURCE), &StartupCatalog::new())
            .map_err(super::debug)?;
        let mut graph = evaluator(&checked, "commons")?;
        let graph_bytes = graph
            .evaluate(&InfoBool::TRUE.encode())
            .map_err(super::debug)?;
        if graph_bytes.len() > 8192 {
            return Err("commons/places-pressure".into());
        }
        let places =
            StructuredInfoValue::from_canonical_bytes(graph_bytes).map_err(super::debug)?;
        let native = checked
            .native_types
            .iter()
            .find(|ty| ty.name == "CommonsPlaces")
            .ok_or("commons/places-type")?;
        conduit_plot::rust_binding::validate_native_contracts(&places, &native.value_contracts)
            .and_then(|_| {
                conduit_plot::rust_binding::validate_native_invariants(&places, &native.invariants)
            })
            .map_err(super::debug)?;
        let request_type = checked
            .native_types
            .iter()
            .find(|ty| ty.name == "CommonsEncounterRequest")
            .ok_or("commons/request-type")?
            .value_type
            .clone();
        let encounter = evaluator(&checked, "commons/encounter")?;
        let admission = [
            evaluator(&checked, "commons/refuse-invalid-places")?,
            evaluator(&checked, "commons/refuse-invalid-owner")?,
            evaluator(&checked, "commons/refuse-invalid-navigation")?,
        ];
        Ok(Self {
            checked,
            places,
            request_type,
            evaluator: encounter,
            admission,
        })
    }

    /// All callers are preparing a projection, outside kernel Play. The owner
    /// supplies the observation; masks supply only a proposed finite navigation.
    pub fn encounter(
        &mut self,
        body_id: &str,
        life: &str,
        revision: u64,
        current: &str,
        target: &str,
        back: bool,
    ) -> Result<StructuredInfoValue, String> {
        let owner_type = member_type(&self.request_type, "owner")?;
        let owner = record(
            owner_type,
            vec![
                (
                    "body_id",
                    leaf(member_type(owner_type, "body_id")?, body_id.as_bytes())?,
                ),
                (
                    "life",
                    leaf(member_type(owner_type, "life")?, life.as_bytes())?,
                ),
                (
                    "revision",
                    leaf(
                        member_type(owner_type, "revision")?,
                        &revision.to_le_bytes(),
                    )?,
                ),
            ],
        )?;
        let request = record(
            &self.request_type,
            vec![
                ("places", self.places.clone()),
                ("owner", owner),
                (
                    "current",
                    leaf(
                        member_type(&self.request_type, "current")?,
                        current.as_bytes(),
                    )?,
                ),
                (
                    "target",
                    leaf(
                        member_type(&self.request_type, "target")?,
                        target.as_bytes(),
                    )?,
                ),
                (
                    "back",
                    leaf(
                        member_type(&self.request_type, "back")?,
                        &InfoBool::new(back).encode(),
                    )?,
                ),
            ],
        )?;
        let native = self
            .checked
            .native_types
            .iter()
            .find(|ty| ty.name == "CommonsEncounterRequest")
            .ok_or("commons/request-type")?;
        conduit_plot::rust_binding::validate_native_contracts(&request, &native.value_contracts)
            .and_then(|_| {
                conduit_plot::rust_binding::validate_native_invariants(&request, &native.invariants)
            })
            .map_err(|error| format!("commons/source-refused:{error:?}"))?;
        let owner_native = self
            .checked
            .native_types
            .iter()
            .find(|ty| ty.name == "CommonsOwnerObservation")
            .ok_or("commons/owner-type")?;
        let owner = field(&request, "owner")?;
        conduit_plot::rust_binding::validate_native_contracts(owner, &owner_native.value_contracts)
            .and_then(|_| {
                conduit_plot::rust_binding::validate_native_invariants(
                    owner,
                    &owner_native.invariants,
                )
            })
            .map_err(|error| format!("commons/source-refused:{error:?}"))?;
        let request = request.canonical_bytes().map_err(super::debug)?;
        if request.len() > FRAME_BYTES {
            return Err("commons/frame-pressure".into());
        }
        for (evaluator, refusal) in
            self.admission
                .iter_mut()
                .zip(["invalid-places", "invalid-owner", "invalid-navigation"])
        {
            let refused = evaluator.evaluate(&request).map_err(super::debug)?;
            if InfoBool::decode(refused).map_err(super::debug)?.get() {
                return Err(format!("commons/source-refused:{refusal}"));
            }
        }
        let output = self
            .evaluator
            .evaluate(&request)
            .map_err(|error| format!("commons/source-refused:{error:?}"))?;
        if output.len() > FRAME_BYTES {
            return Err("commons/frame-pressure".into());
        }
        StructuredInfoValue::from_canonical_bytes(output).map_err(super::debug)
    }
}
fn evaluator(
    checked: &CheckedSyntaxDocument,
    root: &str,
) -> Result<PreparedPortableExpressionEvaluator, String> {
    let plot = expand_canonical_plot_for_authoring(checked, root, &ProfileCatalog::new())
        .map_err(|error| format!("commons/{root}: {error:?}"))?;
    if plot.expanded.gears.len() != 1 {
        return Err("commons/source-shape".into());
    }
    let ConfigurationValue::Text(program) = &plot.expanded.gears[0]
        .configuration
        .iter()
        .find(|entry| entry.key == "program")
        .ok_or("commons/source-program")?
        .value
    else {
        return Err("commons/source-program".into());
    };
    let program = PortableExpressionProgram::from_canonical_hex(program).map_err(super::debug)?;
    if root != "commons"
        && (program
            .maximum_prepared_input_bytes()
            .map_err(super::debug)? as usize
            > FRAME_BYTES
            || program
                .maximum_prepared_output_bytes()
                .map_err(super::debug)? as usize
                > FRAME_BYTES)
    {
        return Err("commons/preparation-pressure".into());
    }
    PreparedPortableExpressionEvaluator::new(&program).map_err(super::debug)
}
fn member_type<'a>(
    ty: &'a StructuredInfoType,
    name: &str,
) -> Result<&'a StructuredInfoType, String> {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        return Err("commons/record-type".into());
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .map(|field| field.value_type())
        .ok_or("commons/field-type".into())
}
fn leaf(ty: &StructuredInfoType, bytes: &[u8]) -> Result<StructuredInfoValue, String> {
    if bytes.len() > conduit_core::MAXIMUM_STRUCTURED_LEAF_BYTES {
        return Err("commons/leaf-pressure".into());
    }
    if let StructuredInfoTypeShape::Nominal { representation, .. } = ty.shape() {
        return StructuredInfoValue::nominal(ty.clone(), leaf(representation, bytes)?)
            .map_err(super::debug);
    }
    StructuredInfoValue::leaf(ty.clone(), bytes.to_vec()).map_err(super::debug)
}
fn record(
    ty: &StructuredInfoType,
    fields: Vec<(&str, StructuredInfoValue)>,
) -> Result<StructuredInfoValue, String> {
    StructuredInfoValue::record(
        ty.clone(),
        fields
            .into_iter()
            .map(|(name, value)| StructuredFieldValue::new(name, value).map_err(super::debug))
            .collect::<Result<_, _>>()?,
    )
    .map_err(super::debug)
}
pub(super) fn field<'a>(
    value: &'a StructuredInfoValue,
    name: &str,
) -> Result<&'a StructuredInfoValue, String> {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        return Err("commons/record-value".into());
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .map(|field| field.value())
        .ok_or("commons/field-value".into())
}
pub(super) fn text(value: &StructuredInfoValue) -> Result<&str, String> {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        return Err("commons/text-value".into());
    };
    std::str::from_utf8(bytes).map_err(super::debug)
}
