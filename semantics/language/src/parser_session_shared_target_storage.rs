//! Exact immutable external target storage identity. This is an opaque
//! target declaration, not a measured Native-family or Session allocation proof.
//! Sharing never supplies model/fact/Source authority or permits dropping laws.
use alloc::{rc::Rc, string::String};
use conduit_plot::{CheckedSyntaxDocument, ProfileCatalog};

pub struct ParserSessionSharedTargetStorage {
    source_document: Rc<String>,
    checked_source: Rc<CheckedSyntaxDocument>,
    model_catalog: Rc<ProfileCatalog>,
    declared_retained_bytes: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SharedTargetStorageRefusal {
    Capacity,
    SourceOwner,
    Overflow,
}
impl ParserSessionSharedTargetStorage {
    /// Existing complete immutable target-owned inputs. The caller's prepared
    /// target owns the declaration; the Session still validates each original
    /// checked Source/expanded Plan and retains every whole execution frame.
    pub fn declare(
        source_document: Rc<String>,
        checked_source: Rc<CheckedSyntaxDocument>,
        model_catalog: Rc<ProfileCatalog>,
        declared_retained_bytes: usize,
    ) -> Result<Self, SharedTargetStorageRefusal> {
        let owner = core::mem::size_of::<Self>()
            .checked_add(2 * core::mem::size_of::<usize>())
            .and_then(|n| n.checked_add(4 * core::mem::align_of::<Self>()))
            .ok_or(SharedTargetStorageRefusal::Overflow)?;
        if declared_retained_bytes < owner || source_document.is_empty() {
            return Err(SharedTargetStorageRefusal::Capacity);
        }
        Ok(Self {
            source_document,
            checked_source,
            model_catalog,
            declared_retained_bytes,
        })
    }
    pub fn source_document(&self) -> &Rc<String> {
        &self.source_document
    }
    pub fn checked_source(&self) -> &Rc<CheckedSyntaxDocument> {
        &self.checked_source
    }
    pub fn model_catalog(&self) -> &Rc<ProfileCatalog> {
        &self.model_catalog
    }
    pub fn declared_retained_bytes(&self) -> usize {
        self.declared_retained_bytes
    }
    pub(crate) fn validate_source_owner(
        &self,
        source: &CheckedSyntaxDocument,
    ) -> Result<(), SharedTargetStorageRefusal> {
        if !core::ptr::eq(source, self.checked_source.as_ref()) {
            return Err(SharedTargetStorageRefusal::SourceOwner);
        }
        Ok(())
    }
}
/// Allocation-free exact-Rc aggregate. Call before the factory allocates its
/// other Session storage; the selected targets retain all owners.
/// Each complete target's checked_source must first match its declared owner.
pub(crate) fn shared_declared_bytes<'a>(
    owners: impl Clone + Iterator<Item = Option<&'a Rc<ParserSessionSharedTargetStorage>>>,
) -> Result<usize, SharedTargetStorageRefusal> {
    let mut bytes = 0usize;
    for (index, owner) in owners.clone().enumerate() {
        let Some(owner) = owner else {
            continue;
        };
        if owners
            .clone()
            .take(index)
            .flatten()
            .any(|prior| Rc::ptr_eq(prior, owner))
        {
            continue;
        }
        bytes = bytes
            .checked_add(owner.declared_retained_bytes)
            .ok_or(SharedTargetStorageRefusal::Overflow)?;
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_shared_owner_is_charged_once_and_foreign_source_refuses() {
        let text = Rc::new(String::from("type X = U64"));
        let checked = || {
            Rc::new(
                conduit_plot::check_syntax_document(
                    &conduit_plot::parse_syntax_document(&text),
                    &conduit_plot::StartupCatalog::new(),
                )
                .unwrap(),
            )
        };
        let source = checked();
        let catalog = Rc::new(ProfileCatalog::new());
        let first = Rc::new(
            ParserSessionSharedTargetStorage::declare(
                text.clone(),
                source.clone(),
                catalog.clone(),
                4096,
            )
            .unwrap(),
        );
        let second = Rc::new(
            ParserSessionSharedTargetStorage::declare(
                text.clone(),
                source.clone(),
                catalog.clone(),
                4096,
            )
            .unwrap(),
        );
        assert_eq!(
            shared_declared_bytes([Some(&first), None, Some(&first), Some(&second)].into_iter()),
            Ok(8192)
        );
        assert!(first.validate_source_owner(&source).is_ok());
        assert_eq!(
            first.validate_source_owner(&checked()),
            Err(SharedTargetStorageRefusal::SourceOwner)
        );
        let huge = Rc::new(
            ParserSessionSharedTargetStorage::declare(text, source, catalog, usize::MAX).unwrap(),
        );
        assert_eq!(
            shared_declared_bytes([Some(&first), Some(&huge)].into_iter()),
            Err(SharedTargetStorageRefusal::Overflow)
        );
    }
}
