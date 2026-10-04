//! Bind a Unicode-scalar source range to exact borrowed text material.
//! Segment kind is retained; resolving a range does not prove segmentation.
use crate::semantic::{
    LanguageSegmentRef, LanguageText, LanguageTextReferenceMatch, LanguageTextSegmentRef,
};
use conduit_plot::rust_binding::NativeBindingRefusal;

#[derive(Debug)]
pub enum TextReferenceRefusal {
    ReferenceKind,
    Native(NativeBindingRefusal),
    Representation,
}

pub struct ResolvedText<'a> {
    reference: &'a LanguageSegmentRef,
    material: &'a LanguageText,
    text: &'a str,
    checked: LanguageTextReferenceMatch,
}
impl<'a> ResolvedText<'a> {
    pub fn reference(&self) -> &'a LanguageSegmentRef {
        self.reference
    }
    pub fn material(&self) -> &'a LanguageText {
        self.material
    }
    pub fn text(&self) -> &'a str {
        self.text
    }
    pub fn checked(&self) -> &LanguageTextReferenceMatch {
        &self.checked
    }
}

pub fn resolve_text<'a>(
    reference: &'a LanguageSegmentRef,
    material: &'a LanguageText,
) -> Result<ResolvedText<'a>, TextReferenceRefusal> {
    let LanguageSegmentRef::Text(value) = reference else {
        return Err(TextReferenceRefusal::ReferenceKind);
    };
    let text = material.text().as_str();
    let scalar_count =
        u32::try_from(text.chars().count()).map_err(|_| TextReferenceRefusal::Representation)?;
    let projected = LanguageTextSegmentRef::new(
        *value.kind(),
        value.language().clone(),
        value.range().clone(),
        value.revision_id().clone(),
        value.text_id().clone(),
    )
    .map_err(TextReferenceRefusal::Native)?;
    let checked = LanguageTextReferenceMatch::new(material.clone(), projected, scalar_count)
        .map_err(TextReferenceRefusal::Native)?;
    // The native laws establish ordered, in-range scalar indices. Convert only
    // for slicing this exact UTF-8 representation; never relabel the indices.
    let byte_at = |scalar: u32| {
        usize::try_from(scalar).ok().and_then(|index| {
            text.char_indices()
                .map(|(byte, _)| byte)
                .chain(core::iter::once(text.len()))
                .nth(index)
        })
    };
    let start = byte_at(*value.range().start()).ok_or(TextReferenceRefusal::Representation)?;
    let end = byte_at(*value.range().end()).ok_or(TextReferenceRefusal::Representation)?;
    let selected = text
        .get(start..end)
        .ok_or(TextReferenceRefusal::Representation)?;
    Ok(ResolvedText {
        reference,
        material,
        text: selected,
        checked,
    })
}
