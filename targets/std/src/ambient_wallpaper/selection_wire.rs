//! Finite projection of a prepared deck; no acquisition, clock or authority.
use super::acquisition::{PreparedDeck, PreparedPhoto, DECK_LEN};
use conduit_core::{
    validate_canonical_structured_value as canonical, BoundedResourceRef,
    PreparedStructuredComposer as Composer, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity, StructuredFieldValue, StructuredInfoType,
    StructuredInfoTypeShape, StructuredInfoValue,
};
use conduit_face::{ImageColorProfile, ImageFormat, ImagePixelExtent, ImageResource};
use conduit_plot::{rust_binding, rust_binding::NativeRustBinding};
const LIMIT: usize = 8192;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionRefusal {
    InvalidInterval,
    InvalidMetadata,
    NativeContract,
    TimestampOverflow,
    BackwardsTime,
}
type Result<T> = core::result::Result<T, SelectionRefusal>;
fn contract<T>(_: T) -> SelectionRefusal {
    SelectionRefusal::NativeContract
}

#[derive(Clone, Copy)]
enum Field {
    Image,
    Selected,
    Next,
    Rotation,
}
struct Slot {
    field: Field,
    composer: Option<Composer>,
}
/// Retains canonical metadata, not pixels. The caller retains original deck
/// custody and separately admits installation of each exact resource reference.
pub struct PreparedWallpaperSelectionWire {
    images: [Box<[u8]>; DECK_LEN],
    slots: [Slot; 4],
    record: Composer,
    observation: Composer,
    rotation_seconds: u64,
    rotation: Box<[u8]>,
    selected: Option<u64>,
    last_reference: Option<u64>,
    index: usize,
}
impl PreparedWallpaperSelectionWire {
    pub fn prepare(deck: &PreparedDeck) -> Result<Self> {
        Self::prepare_with_interval(deck, 300)
    }
    pub fn prepare_with_interval(deck: &PreparedDeck, rotation_seconds: u64) -> Result<Self> {
        if !(30..=3600).contains(&rotation_seconds) {
            return Err(SelectionRefusal::InvalidInterval);
        }
        let types = conduit_semantic_catalog::prepare_ambient_display_types().map_err(contract)?;
        let named = |name: &str| {
            types
                .native_types
                .iter()
                .find(|t| t.name == name)
                .ok_or(SelectionRefusal::NativeContract)
        };
        let metadata_type = named("AmbientWallpaperImage")?;
        let selection = named("AmbientWallpaperSelection")?;
        let observation = named("AmbientWallpaperObservation")?;
        let rotation = primitive(
            &named("AmbientRotationSeconds")?.value_type,
            rotation_seconds,
        )?
        .canonical_bytes()
        .map_err(contract)?
        .into_boxed_slice();
        let mut images = Vec::with_capacity(DECK_LEN);
        for photo in deck.photos() {
            let metadata = photo_metadata(photo, &metadata_type.value_type)?;
            rust_binding::validate_native_contracts(&metadata, &metadata_type.value_contracts)
                .map_err(contract)?;
            rust_binding::validate_native_invariants(&metadata, &metadata_type.invariants)
                .map_err(contract)?;
            let bytes = metadata.canonical_bytes().map_err(contract)?;
            if bytes.len() > LIMIT {
                return Err(SelectionRefusal::InvalidMetadata);
            }
            images.push(bytes.into_boxed_slice());
        }
        let StructuredInfoTypeShape::Record { fields, .. } = selection.value_type.shape() else {
            return Err(contract(()));
        };
        let slots = fields
            .iter()
            .map(|f| {
                Ok(Slot {
                    field: match f.name() {
                        "image" => Field::Image,
                        "selected_at_unix_ms" => Field::Selected,
                        "next_rotation_unix_ms" => Field::Next,
                        "rotation_seconds" => Field::Rotation,
                        _ => return Err(contract(())),
                    },
                    composer: if matches!(f.name(), "image" | "rotation_seconds") {
                        None
                    } else {
                        Some(Composer::new(f.value_type(), LIMIT).map_err(contract)?)
                    },
                })
            })
            .collect::<Result<Vec<_>>>()?
            .try_into()
            .map_err(contract)?;
        // Exercise the independently checked domain once during preparation.
        let image = StructuredInfoValue::from_canonical_bytes(&images[0]).map_err(contract)?;
        let sample = selection_value(
            &selection.value_type,
            image,
            0,
            rotation_seconds * 1000,
            rotation_seconds,
        )?;
        rust_binding::validate_native_contracts(&sample, &selection.value_contracts)
            .map_err(contract)?;
        rust_binding::validate_native_invariants(&sample, &selection.invariants)
            .map_err(contract)?;
        Ok(Self {
            images: images.try_into().map_err(contract)?,
            slots,
            record: Composer::new(&selection.value_type, LIMIT).map_err(contract)?,
            observation: Composer::new(&observation.value_type, LIMIT).map_err(contract)?,
            rotation_seconds,
            rotation,
            selected: None,
            last_reference: None,
            index: 0,
        })
    }
    pub fn prepared_heap_bytes(&self) -> usize {
        self.rotation.len()
            + self.images.iter().map(|b| b.len()).sum::<usize>()
            + self.record.prepared_heap_bytes()
            + self.observation.prepared_heap_bytes()
            + self
                .slots
                .iter()
                .filter_map(|s| s.composer.as_ref())
                .map(Composer::prepared_heap_bytes)
                .sum::<usize>()
    }
    pub const fn inline_bytes() -> usize {
        core::mem::size_of::<Self>()
    }
    pub fn encode(&mut self, now: u64) -> Result<&[u8]> {
        if self.last_reference.is_some_and(|last| now < last) {
            return Err(SelectionRefusal::BackwardsTime);
        }
        let interval = self.rotation_seconds * 1000;
        let previous = self.selected.unwrap_or(now);
        let due = previous
            .checked_add(interval)
            .ok_or(SelectionRefusal::TimestampOverflow)?;
        let rotate = self.selected.is_some() && now >= due;
        // A late call performs one actual selection at its supplied reference
        // time; no unobserved intermediate selections or timers are invented.
        let selected = if rotate { now } else { previous };
        let next = selected
            .checked_add(interval)
            .ok_or(SelectionRefusal::TimestampOverflow)?;
        let index = if rotate {
            (self.index + 1) % DECK_LEN
        } else {
            self.index
        };
        let selected_bytes = selected.to_le_bytes();
        let next_bytes = next.to_le_bytes();
        let [a, b, c, d] = self.slots.each_mut().map(|s| {
            if matches!(s.field, Field::Image) {
                return canonical(&self.images[index]).map_err(contract);
            }
            if matches!(s.field, Field::Rotation) {
                return canonical(&self.rotation).map_err(contract);
            }
            s.composer
                .as_mut()
                .ok_or_else(|| contract(()))?
                .leaf(match s.field {
                    Field::Selected => &selected_bytes,
                    Field::Next => &next_bytes,
                    Field::Rotation | Field::Image => unreachable!(),
                })
                .map_err(contract)
                .and_then(|b| canonical(b).map_err(contract))
        });
        let bytes = self.record.record(&[a?, b?, c?, d?]).map_err(contract)?;
        let output = self
            .observation
            .variant("available", canonical(bytes).map_err(contract)?)
            .map_err(contract)?;
        self.selected = Some(selected);
        self.index = index;
        self.last_reference = Some(now);
        Ok(output)
    }
}
fn record(
    ty: &StructuredInfoType,
    mut value: impl FnMut(&str, &StructuredInfoType) -> Result<StructuredInfoValue>,
) -> Result<StructuredInfoValue> {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        return Err(contract(()));
    };
    let fields = fields
        .iter()
        .map(|f| {
            StructuredFieldValue::new(f.name(), value(f.name(), f.value_type())?).map_err(contract)
        })
        .collect::<Result<Vec<_>>>()?;
    StructuredInfoValue::record(ty.clone(), fields).map_err(contract)
}
fn selection_value(
    ty: &StructuredInfoType,
    image: StructuredInfoValue,
    selected: u64,
    next: u64,
    rotation: u64,
) -> Result<StructuredInfoValue> {
    record(ty, |name, ty| {
        if name == "image" {
            Ok(image.clone())
        } else {
            primitive(
                ty,
                match name {
                    "selected_at_unix_ms" => selected,
                    "next_rotation_unix_ms" => next,
                    "rotation_seconds" => rotation,
                    _ => return Err(contract(())),
                },
            )
        }
    })
}
// Authored named numeric Types retain their nominal identity. Preparation may
// construct the wrapper; runtime reuses the exact immutable interval bytes.
fn primitive(ty: &StructuredInfoType, value: u64) -> Result<StructuredInfoValue> {
    if let StructuredInfoTypeShape::Nominal { representation, .. } = ty.shape() {
        StructuredInfoValue::nominal(ty.clone(), primitive(representation, value)?)
            .map_err(contract)
    } else {
        rust_binding::primitive_into_structured(ty.clone(), &value).map_err(contract)
    }
}
fn photo_metadata(photo: &PreparedPhoto, ty: &StructuredInfoType) -> Result<StructuredInfoValue> {
    let pixel_bytes = u64::from(photo.width())
        .checked_mul(u64::from(photo.height()))
        .and_then(|n| n.checked_mul(4))
        .ok_or(SelectionRefusal::InvalidMetadata)?;
    if pixel_bytes != photo.rgba().len() as u64 || pixel_bytes == 0 {
        return Err(SelectionRefusal::InvalidMetadata);
    }
    let image = ImageResource::new(
        ImageColorProfile::absent(),
        BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest(*photo.original_sha256()),
            content_profile: "media/image-rgba8@1".into(),
            access_class: "conduit.resource/image-content@1".into(),
            extent: ResourceExtent {
                bytes: pixel_bytes,
                items: Some(1),
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest(*photo.version_sha256()),
                expires_at: None,
            },
        },
        ImagePixelExtent::new(u64::from(photo.height()), u64::from(photo.width()))
            .map_err(contract)?,
        ImageFormat::rgba8(),
        format!("apod/{}", photo.candidate().date),
    )
    .map_err(contract)?
    .into_structured()
    .map_err(contract)?;
    let source = format!("NASA APOD/{}", photo.candidate().date);
    record(ty, |name, ty| {
        if name == "image" {
            return Ok(image.clone());
        }
        let text = match name {
            "title" => photo.candidate().title.as_str(),
            "caption" => "",
            "attribution" => photo
                .candidate()
                .credit
                .as_deref()
                .unwrap_or("Creator credit not supplied by NASA APOD"),
            "license" => "License not supplied by provider",
            "source" => &source,
            _ => return Err(contract(())),
        };
        rust_binding::primitive_into_structured(ty.clone(), &text.to_owned()).map_err(contract)
    })
}
#[cfg(test)]
#[path = "selection_wire/tests.rs"]
mod tests;
