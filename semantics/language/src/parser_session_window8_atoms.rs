//! Finite representation buffers for the closed Window8 driver's nested fields.
//! Prepared schemas come from exact ready original Native descriptors. Composed
//! bytes are representations only: full containing Native admission and original
//! Source execution remain mandatory before publication or commitment.
use crate::{
    generated::*,
    parser_canonical_composition::{
        ParserCompositionLimits, ParserCompositionRefusal, PreparedParserCanonicalComposer,
    },
    parser_canonical_nominal::{NominalRefusal, PreparedParserNominal},
    parser_canonical_schema::{select_steps, shape, SchemaStep, Shape},
    parser_session_window8_ports::family_for,
};
use alloc::{rc::Rc, vec::Vec};
use conduit_core::{validate_canonical_structured_value, ValidatedCanonicalStructuredValue};
use conduit_plot::rust_binding::{
    NativeFamilyTypeDescriptor, PreparedNativeFamily, PreparedNativeRustBinding,
};
use core::{cell::RefCell, mem::size_of};
type Family = Rc<RefCell<PreparedNativeFamily>>;
const MAXIMUM_RECORD_FIELDS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub(crate) enum Window8Atom {
    DependencyUnit,
    RelationBase,
    Subtype,
    DefaultRelation,
    AnalysisRevision,
    Unsigned,
    Signed,
    Boolean,
    Basis,
    Lexical,
    Projection,
    Ancestry,
    StateProof,
    CheckedHypothesis,
    Snapshot,
    Hypothesis,
    Beam,
    RawState,
}
const ATOMS: [Window8Atom; 18] = [
    Window8Atom::DependencyUnit,
    Window8Atom::RelationBase,
    Window8Atom::Subtype,
    Window8Atom::DefaultRelation,
    Window8Atom::AnalysisRevision,
    Window8Atom::Unsigned,
    Window8Atom::Signed,
    Window8Atom::Boolean,
    Window8Atom::Basis,
    Window8Atom::Lexical,
    Window8Atom::Projection,
    Window8Atom::Ancestry,
    Window8Atom::StateProof,
    Window8Atom::CheckedHypothesis,
    Window8Atom::Snapshot,
    Window8Atom::Hypothesis,
    Window8Atom::Beam,
    Window8Atom::RawState,
];
#[derive(Clone, Copy)]
pub(crate) enum Window8AtomField<'a> {
    Observed(ValidatedCanonicalStructuredValue<'a>),
    Prepared(Window8Atom),
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct Window8AtomLimits {
    pub(crate) maximum_frame_bytes: usize,
    pub(crate) maximum_preparation_requested_bytes: usize,
    pub(crate) maximum_retained_requested_bytes: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Window8AtomReceipt {
    pub(crate) preparation_requested_bytes_bound: usize,
    pub(crate) retained_requested_bytes_bound: usize,
    /// Static paths/selector table only. Original descriptor artifacts are
    /// separately charged once by the enclosing family's static inventory.
    pub(crate) static_selector_bytes_bound: usize,
}
#[derive(Debug)]
pub(crate) enum Window8AtomRefusal {
    Descriptor,
    Type,
    Pressure,
    Unavailable,
    SelfReference,
    Composition(ParserCompositionRefusal),
    Nominal(NominalRefusal),
}
struct Spec {
    descriptor: &'static NativeFamilyTypeDescriptor,
    steps: &'static [SchemaStep<'static>],
    nominal: bool,
    payload_bytes: Option<usize>,
}
fn spec(atom: Window8Atom) -> Spec {
    use SchemaStep::{Case as C, Field as F};
    use Window8Atom::*;
    let begin = LanguageParserWindow8Begin::PREPARED_DESCRIPTOR;
    let hypothesis = LanguageParserWindow8RawHypothesis::PREPARED_DESCRIPTOR;
    let fact = LanguageParserWindow8FactQuery::PREPARED_DESCRIPTOR;
    let (descriptor, steps, nominal, payload_bytes): (_, &[_], _, _) = match atom {
        DependencyUnit => (
            begin,
            &[F("default_relation"), F("base"), C("dep")],
            false,
            Some(0),
        ),
        RelationBase => (begin, &[F("default_relation"), F("base")], false, None),
        Subtype => (
            begin,
            &[F("default_relation"), F("subtype")],
            true,
            Some(32),
        ),
        DefaultRelation => (begin, &[F("default_relation")], false, None),
        AnalysisRevision => (begin, &[F("basis"), F("analysis_revision")], true, Some(64)),
        Unsigned => (hypothesis, &[F("identity")], false, Some(8)),
        Signed => (hypothesis, &[F("score")], false, Some(8)),
        Boolean => (hypothesis, &[F("active")], false, Some(1)),
        Basis => (begin, &[F("basis")], false, None),
        Lexical => (fact, &[F("snapshot"), F("lexical")], false, None),
        Projection => (
            LanguageParserWindow8RawFeatureQuery::PREPARED_DESCRIPTOR,
            &[F("projection")],
            false,
            None,
        ),
        Ancestry => (
            LanguageParserWindow8Ancestry::PREPARED_DESCRIPTOR,
            &[],
            false,
            None,
        ),
        StateProof => (
            LanguageParserWindow8StateProof::PREPARED_DESCRIPTOR,
            &[],
            false,
            None,
        ),
        CheckedHypothesis => (fact, &[F("snapshot"), F("candidate0")], false, None),
        Snapshot => (fact, &[F("snapshot")], false, None),
        Hypothesis => (hypothesis, &[], false, None),
        Beam => (
            LanguageParserWindow8RawBeam::PREPARED_DESCRIPTOR,
            &[],
            false,
            None,
        ),
        RawState => (hypothesis, &[F("state")], false, None),
    };
    Spec {
        descriptor,
        steps,
        nominal,
        payload_bytes,
    }
}
fn add(a: usize, b: usize) -> Result<usize, Window8AtomRefusal> {
    a.checked_add(b).ok_or(Window8AtomRefusal::Pressure)
}
fn output_bound(selected: &[u8], s: &Spec, maximum: usize) -> Result<usize, Window8AtomRefusal> {
    let bound = match s.payload_bytes {
        Some(payload) => add(add(selected.len(), 5)?, payload)?,
        None => maximum,
    };
    if maximum == 0
        || maximum > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
        || bound > maximum
        || bound <= selected.len()
    {
        return Err(Window8AtomRefusal::Pressure);
    }
    Ok(bound)
}
enum Owner {
    Composer(PreparedParserCanonicalComposer),
    Nominal(PreparedParserNominal),
}
struct Entry {
    owner: Owner,
    valid: bool,
}
impl Entry {
    fn encoded(&self) -> Result<&[u8], Window8AtomRefusal> {
        if !self.valid {
            return Err(Window8AtomRefusal::Unavailable);
        }
        Ok(match &self.owner {
            Owner::Composer(c) => c.encoded(),
            Owner::Nominal(n) => n.encoded(),
        })
    }
}
pub(crate) struct PreparedWindow8Atoms {
    entries: Vec<Entry>,
    receipt: Window8AtomReceipt,
}
impl PreparedWindow8Atoms {
    pub(crate) fn prepare(
        families: &[Family],
        limits: Window8AtomLimits,
    ) -> Result<Self, Window8AtomRefusal> {
        use Window8AtomRefusal as R;
        let slots = ATOMS
            .len()
            .checked_mul(size_of::<Entry>())
            .ok_or(R::Pressure)?;
        let mut preparation = slots;
        let mut retained = slots;
        let mut static_bytes = size_of::<[Window8Atom; 18]>();
        // Readiness, exact nested schema and all simultaneous reservations are
        // checked before the first owned bank allocation.
        for atom in ATOMS {
            let s = spec(atom);
            let index = family_for(families, s.descriptor).map_err(|_| R::Descriptor)?;
            let family = families[index].try_borrow().map_err(|_| R::Descriptor)?;
            let selected = select_steps(s.descriptor.type_bytes, s.steps).map_err(|_| R::Type)?;
            let expected_shape = match atom {
                Window8Atom::RelationBase => matches!(shape(selected), Ok(Shape::Variant(_))),
                Window8Atom::Subtype | Window8Atom::AnalysisRevision => {
                    matches!(shape(selected), Ok(Shape::Nominal { .. }))
                }
                Window8Atom::DependencyUnit
                | Window8Atom::Unsigned
                | Window8Atom::Signed
                | Window8Atom::Boolean => matches!(shape(selected), Ok(Shape::Leaf(_))),
                _ => matches!(shape(selected), Ok(Shape::Record(_))),
            };
            if !expected_shape {
                return Err(R::Type);
            }
            let output = output_bound(selected, &s, limits.maximum_frame_bytes)?;
            static_bytes = add(
                static_bytes,
                s.steps
                    .len()
                    .checked_mul(size_of::<SchemaStep<'_>>())
                    .ok_or(R::Pressure)?,
            )?;
            for step in s.steps {
                let (SchemaStep::Field(name) | SchemaStep::Case(name)) = step;
                static_bytes = add(static_bytes, name.len())?;
            }
            if s.nominal {
                if !family.contains_descriptor(s.descriptor)
                    || !matches!(shape(selected), Ok(Shape::Nominal { .. }))
                {
                    return Err(R::Type);
                }
                preparation = add(preparation, output)?;
                retained = add(retained, output)?;
            } else {
                let r = PreparedParserCanonicalComposer::descriptor_steps_reservation(
                    &family,
                    s.descriptor,
                    s.steps,
                    output,
                )
                .map_err(R::Composition)?;
                preparation = add(preparation, r.preparation_requested_bytes_bound)?;
                retained = add(retained, r.retained_requested_bytes_bound)?;
            }
        }
        if preparation > limits.maximum_preparation_requested_bytes
            || retained > limits.maximum_retained_requested_bytes
        {
            return Err(R::Pressure);
        }
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(ATOMS.len())
            .map_err(|_| R::Pressure)?;
        if entries.capacity() != ATOMS.len() {
            return Err(R::Pressure);
        }
        for atom in ATOMS {
            let s = spec(atom);
            let index = family_for(families, s.descriptor).map_err(|_| R::Descriptor)?;
            let family = families[index].try_borrow().map_err(|_| R::Descriptor)?;
            let selected = select_steps(s.descriptor.type_bytes, s.steps).map_err(|_| R::Type)?;
            let output = output_bound(selected, &s, limits.maximum_frame_bytes)?;
            let owner = if s.nominal {
                let path: &[&str] = match atom {
                    Window8Atom::Subtype => &["default_relation", "subtype"],
                    Window8Atom::AnalysisRevision => &["basis", "analysis_revision"],
                    _ => return Err(R::Type),
                };
                Owner::Nominal(
                    PreparedParserNominal::prepare::<LanguageParserWindow8Begin>(
                        &family, path, output, output, output,
                    )
                    .map_err(R::Nominal)?,
                )
            } else {
                Owner::Composer(
                    PreparedParserCanonicalComposer::prepare_descriptor_steps(
                        &family,
                        s.descriptor,
                        s.steps,
                        ParserCompositionLimits {
                            maximum_output_bytes: output,
                            maximum_preparation_requested_bytes: limits
                                .maximum_preparation_requested_bytes,
                            maximum_retained_requested_bytes: limits
                                .maximum_retained_requested_bytes,
                        },
                    )
                    .map_err(R::Composition)?,
                )
            };
            entries.push(Entry {
                owner,
                valid: false,
            });
        }
        let mut bank = Self {
            entries,
            receipt: Window8AtomReceipt {
                preparation_requested_bytes_bound: preparation,
                retained_requested_bytes_bound: retained,
                static_selector_bytes_bound: static_bytes,
            },
        };
        // Fixed representation defaults are not admitted linguistic relations.
        bank.leaf(Window8Atom::DependencyUnit, &[])?;
        bank.variant(
            Window8Atom::RelationBase,
            "dep",
            Window8AtomField::Prepared(Window8Atom::DependencyUnit),
        )?;
        bank.leaf(Window8Atom::Subtype, &[])?;
        bank.record(
            Window8Atom::DefaultRelation,
            &[
                Window8AtomField::Prepared(Window8Atom::RelationBase),
                Window8AtomField::Prepared(Window8Atom::Subtype),
            ],
        )?;
        Ok(bank)
    }
    pub(crate) fn receipt(&self) -> Window8AtomReceipt {
        self.receipt
    }
    pub(crate) fn encoded(&self, atom: Window8Atom) -> Result<&[u8], Window8AtomRefusal> {
        self.entries
            .get(atom as usize)
            .ok_or(Window8AtomRefusal::Unavailable)?
            .encoded()
    }
    pub(crate) fn leaf(
        &mut self,
        atom: Window8Atom,
        bytes: &[u8],
    ) -> Result<&[u8], Window8AtomRefusal> {
        let entry = self
            .entries
            .get_mut(atom as usize)
            .ok_or(Window8AtomRefusal::Unavailable)?;
        entry.valid = false;
        let result = match &mut entry.owner {
            Owner::Composer(c) => c.leaf(bytes).map_err(Window8AtomRefusal::Composition),
            Owner::Nominal(n) => n.leaf(bytes).map_err(Window8AtomRefusal::Nominal),
        };
        entry.valid = result.is_ok();
        result
    }
    pub(crate) fn unsigned(&mut self, value: u64) -> Result<&[u8], Window8AtomRefusal> {
        self.leaf(Window8Atom::Unsigned, &value.to_le_bytes())
    }
    pub(crate) fn signed(&mut self, value: i64) -> Result<&[u8], Window8AtomRefusal> {
        self.leaf(Window8Atom::Signed, &value.to_le_bytes())
    }
    pub(crate) fn boolean(&mut self, value: bool) -> Result<&[u8], Window8AtomRefusal> {
        self.leaf(Window8Atom::Boolean, &[u8::from(value)])
    }
    pub(crate) fn record(
        &mut self,
        atom: Window8Atom,
        fields: &[Window8AtomField<'_>],
    ) -> Result<&[u8], Window8AtomRefusal> {
        use Window8AtomRefusal as R;
        let index = atom as usize;
        let (before, rest) = self.entries.split_at_mut(index);
        let (entry, after) = rest.split_first_mut().ok_or(R::Unavailable)?;
        entry.valid = false;
        if fields.is_empty() || fields.len() > MAXIMUM_RECORD_FIELDS {
            return Err(R::Type);
        }
        let first = resolve(fields[0], index, before, after)?;
        let mut values = [first; MAXIMUM_RECORD_FIELDS];
        for (value, field) in values.iter_mut().zip(fields) {
            *value = resolve(*field, index, before, after)?;
        }
        let Owner::Composer(composer) = &mut entry.owner else {
            return Err(R::Type);
        };
        let result = composer
            .record(&values[..fields.len()])
            .map_err(R::Composition);
        entry.valid = result.is_ok();
        result
    }
    pub(crate) fn variant(
        &mut self,
        atom: Window8Atom,
        tag: &str,
        payload: Window8AtomField<'_>,
    ) -> Result<&[u8], Window8AtomRefusal> {
        use Window8AtomRefusal as R;
        let index = atom as usize;
        let (before, rest) = self.entries.split_at_mut(index);
        let (entry, after) = rest.split_first_mut().ok_or(R::Unavailable)?;
        entry.valid = false;
        let payload = resolve(payload, index, before, after)?;
        let Owner::Composer(composer) = &mut entry.owner else {
            return Err(R::Type);
        };
        let result = composer.variant(tag, payload).map_err(R::Composition);
        entry.valid = result.is_ok();
        result
    }
}
fn resolve<'a>(
    field: Window8AtomField<'a>,
    destination: usize,
    before: &'a [Entry],
    after: &'a [Entry],
) -> Result<ValidatedCanonicalStructuredValue<'a>, Window8AtomRefusal> {
    use Window8AtomRefusal as R;
    match field {
        Window8AtomField::Observed(value) => Ok(value),
        Window8AtomField::Prepared(atom) => {
            let index = atom as usize;
            if index == destination {
                return Err(R::SelfReference);
            }
            let entry = if index < destination {
                before.get(index)
            } else {
                after.get(index - destination - 1)
            }
            .ok_or(R::Unavailable)?;
            validate_canonical_structured_value(entry.encoded()?).map_err(|_| R::Type)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_plot::rust_binding::PreparedNativeFamilyLimits;

    fn families() -> Vec<Family> {
        let mut owners = Vec::new();
        // Prepare the actual descriptor closures; this does not fabricate a
        // ready family or bypass any generated Native preparation law.
        for atom in ATOMS {
            let descriptor = spec(atom).descriptor;
            if owners
                .iter()
                .any(|owner: &Family| owner.borrow().contains_descriptor(descriptor))
            {
                continue;
            }
            let family = PreparedNativeFamily::prepare(
                &[descriptor],
                PreparedNativeFamilyLimits {
                    maximum_types: 64,
                    maximum_laws_per_type: 256,
                    maximum_input_bytes: 262144,
                    maximum_retained_bytes: usize::MAX,
                    maximum_preparation_peak_bytes: usize::MAX,
                    maximum_conversion_requested_bytes: usize::MAX,
                },
            )
            .unwrap();
            owners.push(Rc::new(RefCell::new(family)));
        }
        owners
    }
    fn limits() -> Window8AtomLimits {
        Window8AtomLimits {
            maximum_frame_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES,
            maximum_preparation_requested_bytes: usize::MAX,
            maximum_retained_requested_bytes: usize::MAX,
        }
    }
    #[test]
    fn actual_nested_schemas_and_aggregate_storage_boundaries() {
        let families = families();
        let prepared = PreparedWindow8Atoms::prepare(&families, limits()).unwrap();
        let receipt = prepared.receipt();
        drop(prepared);
        for preparation in [true, false] {
            let mut bound = limits();
            bound.maximum_preparation_requested_bytes = receipt.preparation_requested_bytes_bound;
            bound.maximum_retained_requested_bytes = receipt.retained_requested_bytes_bound;
            if preparation {
                bound.maximum_preparation_requested_bytes -= 1;
            } else {
                bound.maximum_retained_requested_bytes -= 1;
            }
            assert!(matches!(
                PreparedWindow8Atoms::prepare(&families, bound),
                Err(Window8AtomRefusal::Pressure)
            ));
        }
        let mut exact = limits();
        exact.maximum_preparation_requested_bytes = receipt.preparation_requested_bytes_bound;
        exact.maximum_retained_requested_bytes = receipt.retained_requested_bytes_bound;
        let mut bank = PreparedWindow8Atoms::prepare(&families, exact).unwrap();
        for atom in [
            Window8Atom::DependencyUnit,
            Window8Atom::RelationBase,
            Window8Atom::Subtype,
            Window8Atom::DefaultRelation,
        ] {
            let frame = validate_canonical_structured_value(bank.encoded(atom).unwrap()).unwrap();
            let s = spec(atom);
            assert_eq!(
                frame.type_bytes(),
                select_steps(s.descriptor.type_bytes, s.steps).unwrap()
            );
        }
        assert!(matches!(
            bank.encoded(Window8Atom::Basis),
            Err(Window8AtomRefusal::Unavailable)
        ));
        assert!(bank.record(Window8Atom::DefaultRelation, &[]).is_err());
        assert!(matches!(
            bank.encoded(Window8Atom::DefaultRelation),
            Err(Window8AtomRefusal::Unavailable)
        ));
        bank.record(
            Window8Atom::DefaultRelation,
            &[
                Window8AtomField::Prepared(Window8Atom::RelationBase),
                Window8AtomField::Prepared(Window8Atom::Subtype),
            ],
        )
        .unwrap();
        bank.unsigned(u64::MAX).unwrap();
        bank.signed(i64::MIN).unwrap();
        bank.boolean(true).unwrap();
        assert!(matches!(
            bank.variant(
                Window8Atom::RelationBase,
                "dep",
                Window8AtomField::Prepared(Window8Atom::RelationBase)
            ),
            Err(Window8AtomRefusal::SelfReference)
        ));
        assert!(matches!(
            bank.encoded(Window8Atom::RelationBase),
            Err(Window8AtomRefusal::Unavailable)
        ));
        bank.variant(
            Window8Atom::RelationBase,
            "dep",
            Window8AtomField::Prepared(Window8Atom::DependencyUnit),
        )
        .unwrap();
        // A fully canonical but wrong-Type child must still refuse.
        assert!(bank
            .record(
                Window8Atom::DefaultRelation,
                &[
                    Window8AtomField::Prepared(Window8Atom::Unsigned),
                    Window8AtomField::Prepared(Window8Atom::Subtype),
                ]
            )
            .is_err());
        assert!(matches!(
            bank.encoded(Window8Atom::DefaultRelation),
            Err(Window8AtomRefusal::Unavailable)
        ));
    }
}
