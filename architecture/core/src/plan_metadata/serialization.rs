//! Lossless serialization with one independently supplied configuration at a time.
use super::*;
use core::{cell::RefCell, marker::PhantomData};
use serde::{
    ser::{Error, SerializeSeq, SerializeStruct},
    Serialize, Serializer,
};

/// Complete canonical serialization borrowed from an opaque verified scope.
/// This view owns no whole rich Plan and establishes no execution authority.
/// Each serialization pass calls the supplier again; it must return the exact
/// original configuration for each placement. Errors can leave partial output;
/// callers must discard that output transactionally rather than publish it.
pub struct VerifiedPlanSerialization<'a, F, E> {
    verified: &'a VerifiedPlanMetadata,
    configuration: RefCell<F>,
    error: PhantomData<fn() -> E>,
}
impl VerifiedPlanMetadata {
    pub fn serialize_with_configurations<F, E>(
        &self,
        configuration: F,
    ) -> VerifiedPlanSerialization<'_, F, E>
    where
        F: FnMut(&PlannedGear) -> Result<Vec<ConfigurationEntry>, E>,
    {
        VerifiedPlanSerialization {
            verified: self,
            configuration: RefCell::new(configuration),
            error: PhantomData,
        }
    }
}
impl<F, E> Serialize for VerifiedPlanSerialization<'_, F, E>
where
    F: FnMut(&PlannedGear) -> Result<Vec<ConfigurationEntry>, E>,
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.verified.plan.serialize(ReplaceField {
            serializer,
            structure: "Plan",
            field: "fragments",
            replacement: Fragments(self),
        })
    }
}
struct Fragments<'a, 'p, F, E>(&'a VerifiedPlanSerialization<'p, F, E>);
impl<F, E> Serialize for Fragments<'_, '_, F, E>
where
    F: FnMut(&PlannedGear) -> Result<Vec<ConfigurationEntry>, E>,
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.0.verified.plan.fragments.len()))?;
        for index in 0..self.0.verified.plan.fragments.len() {
            seq.serialize_element(&Fragment {
                view: self.0,
                index,
            })?;
        }
        seq.end()
    }
}
struct Fragment<'a, 'p, F, E> {
    view: &'a VerifiedPlanSerialization<'p, F, E>,
    index: usize,
}
impl<F, E> Serialize for Fragment<'_, '_, F, E>
where
    F: FnMut(&PlannedGear) -> Result<Vec<ConfigurationEntry>, E>,
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.view.verified.plan.fragments[self.index].serialize(ReplaceField {
            serializer,
            structure: "PlanFragment",
            field: "placements",
            replacement: Placements {
                view: self.view,
                fragment: self.index,
            },
        })
    }
}
struct Placements<'a, 'p, F, E> {
    view: &'a VerifiedPlanSerialization<'p, F, E>,
    fragment: usize,
}
impl<F, E> Serialize for Placements<'_, '_, F, E>
where
    F: FnMut(&PlannedGear) -> Result<Vec<ConfigurationEntry>, E>,
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let fragment = &self.view.verified.plan.fragments[self.fragment];
        let mut seq = serializer.serialize_seq(Some(fragment.placements.len()))?;
        for (index, metadata) in fragment.placements.iter().enumerate() {
            let configuration = self
                .view
                .configuration
                .try_borrow_mut()
                .map_err(|_| S::Error::custom("configuration supplier already borrowed"))?(
                metadata,
            )
            .map_err(|_| S::Error::custom("configuration supplier refused"))?;
            if self.view.verified.configurations[self.fragment].get(index)
                != Some(&crate::plan_fingerprint::configuration_fingerprint(
                    &configuration,
                ))
            {
                return Err(S::Error::custom("configuration differs from verified Plan"));
            }
            // Keep immutable contracts/ports/resources borrowed. The canonical
            // derive still owns every field's order/tag/omission; only the
            // required, independently fingerprint-checked configuration changes.
            seq.serialize_element(&Placement {
                metadata,
                configuration: &configuration,
            })?;
        }
        seq.end()
    }
}

struct Placement<'a> {
    metadata: &'a PlannedGear,
    configuration: &'a [ConfigurationEntry],
}
impl Serialize for Placement<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.metadata.serialize(ReplaceField {
            serializer,
            structure: "PlannedGear",
            field: "configuration",
            replacement: self.configuration,
        })
    }
}

// Let canonical derives determine structure names, field counts/order and all
// default/omission decisions. Only required sequence fields are substituted.
// A future representation change refuses instead of silently emitting a partial
// Plan. Nested values serialize directly through the underlying serializer.
struct ReplaceField<S, R> {
    serializer: S,
    structure: &'static str,
    field: &'static str,
    replacement: R,
}
struct ReplacedStruct<S, R> {
    inner: S,
    field: &'static str,
    replacement: R,
    seen: bool,
}
impl<S: SerializeStruct, R: Serialize> SerializeStruct for ReplacedStruct<S, R> {
    type Ok = S::Ok;
    type Error = S::Error;
    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error> {
        if key == self.field {
            if self.seen {
                return Err(Self::Error::custom("duplicate canonical replacement field"));
            }
            self.seen = true;
            self.inner.serialize_field(key, &self.replacement)
        } else {
            self.inner.serialize_field(key, value)
        }
    }
    fn skip_field(&mut self, key: &'static str) -> Result<(), Self::Error> {
        if key == self.field {
            return Err(Self::Error::custom("missing canonical replacement field"));
        }
        self.inner.skip_field(key)
    }
    fn end(self) -> Result<Self::Ok, Self::Error> {
        if !self.seen {
            return Err(Self::Error::custom("missing canonical replacement field"));
        }
        self.inner.end()
    }
}
impl<S: Serializer, R: Serialize> Serializer for ReplaceField<S, R> {
    type Ok = S::Ok;
    type Error = S::Error;
    type SerializeSeq = serde::ser::Impossible<S::Ok, S::Error>;
    type SerializeTuple = serde::ser::Impossible<S::Ok, S::Error>;
    type SerializeTupleStruct = serde::ser::Impossible<S::Ok, S::Error>;
    type SerializeTupleVariant = serde::ser::Impossible<S::Ok, S::Error>;
    type SerializeMap = serde::ser::Impossible<S::Ok, S::Error>;
    type SerializeStructVariant = serde::ser::Impossible<S::Ok, S::Error>;
    type SerializeStruct = ReplacedStruct<S::SerializeStruct, R>;
    fn serialize_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        if name != self.structure {
            return Err(Self::Error::custom("unexpected canonical structure"));
        }
        Ok(ReplacedStruct {
            inner: self.serializer.serialize_struct(name, len)?,
            field: self.field,
            replacement: self.replacement,
            seen: false,
        })
    }
    fn is_human_readable(&self) -> bool {
        self.serializer.is_human_readable()
    }
    fn serialize_bool(self, _value: bool) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("unexpected canonical representation"))
    }
    fn serialize_i8(self, _value: i8) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("unexpected canonical representation"))
    }
    fn serialize_i16(self, _value: i16) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("unexpected canonical representation"))
    }
    fn serialize_i32(self, _value: i32) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("unexpected canonical representation"))
    }
    fn serialize_i64(self, _value: i64) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("unexpected canonical representation"))
    }
    fn serialize_i128(self, _value: i128) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("unexpected canonical representation"))
    }
    fn serialize_u8(self, _value: u8) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("unexpected canonical representation"))
    }
    fn serialize_u16(self, _value: u16) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("unexpected canonical representation"))
    }
    fn serialize_u32(self, _value: u32) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("unexpected canonical representation"))
    }
    fn serialize_u64(self, _value: u64) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("unexpected canonical representation"))
    }
    fn serialize_u128(self, _value: u128) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("unexpected canonical representation"))
    }
    fn serialize_f32(self, _value: f32) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("unexpected canonical representation"))
    }
    fn serialize_f64(self, _value: f64) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("unexpected canonical representation"))
    }
    fn serialize_char(self, _value: char) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("unexpected canonical representation"))
    }
    fn serialize_str(self, _value: &str) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("unexpected canonical representation"))
    }
    fn serialize_bytes(self, _value: &[u8]) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("unexpected canonical representation"))
    }
    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("unexpected canonical representation"))
    }
    fn serialize_some<T: ?Sized + Serialize>(self, _value: &T) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("unexpected canonical representation"))
    }
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("unexpected canonical representation"))
    }
    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("unexpected canonical representation"))
    }
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("unexpected canonical representation"))
    }
    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("unexpected canonical representation"))
    }
    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("unexpected canonical representation"))
    }
    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Err(Self::Error::custom("unexpected canonical representation"))
    }
    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Err(Self::Error::custom("unexpected canonical representation"))
    }
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(Self::Error::custom("unexpected canonical representation"))
    }
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(Self::Error::custom("unexpected canonical representation"))
    }
    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(Self::Error::custom("unexpected canonical representation"))
    }
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(Self::Error::custom("unexpected canonical representation"))
    }
}
