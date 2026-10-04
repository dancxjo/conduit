//! Exact phone definition lookup in a caller-supplied inventory.
//! This preserves material and definition metadata; it selects no voice model.
use crate::{reference_admission::ResolvedPhone, semantic::*};
use conduit_plot::rust_binding::NativeBindingRefusal;

#[derive(Debug)]
pub enum InventoryRefusal {
    Basis(NativeBindingRefusal),
    Unresolved(PhoneSpecification),
    MissingDefinition,
    AmbiguousDefinition,
    Definition(NativeBindingRefusal),
}

/// A borrowed material token and its unique definition in this supplied inventory.
/// Neither an external registry nor authority/authenticity is implied by lookup.
pub struct InventoryPhone<'a, 'material> {
    material: &'a ResolvedPhone<'material>,
    inventory: &'a SpeechInventory,
    definition: &'a SpeechPhone,
    basis: SpeechPhoneInventoryBasis,
    identity: SpeechPhoneDefinitionMatch,
}
impl<'a, 'material> InventoryPhone<'a, 'material> {
    pub fn material(&self) -> &'a ResolvedPhone<'material> {
        self.material
    }
    pub fn inventory(&self) -> &'a SpeechInventory {
        self.inventory
    }
    pub fn definition(&self) -> &'a SpeechPhone {
        self.definition
    }
    pub fn checked_basis(&self) -> &SpeechPhoneInventoryBasis {
        &self.basis
    }
    pub fn checked_identity(&self) -> &SpeechPhoneDefinitionMatch {
        &self.identity
    }
}

pub fn resolve_inventory_phone<'a, 'material>(
    material: &'a ResolvedPhone<'material>,
    inventory: &'a SpeechInventory,
) -> Result<InventoryPhone<'a, 'material>, InventoryRefusal> {
    let basis = SpeechPhoneInventoryBasis::new(
        material.snapshot().basis().clone(),
        inventory.identity().clone(),
        inventory.language().clone(),
    )
    .map_err(InventoryRefusal::Basis)?;
    let requested = match material.token().phone() {
        PhoneSpecification::Known(id) => id,
        state => return Err(InventoryRefusal::Unresolved(state.clone())),
    };
    // Exact opaque-ID indexing only. IPA, aliases and feature similarity are not IDs.
    let mut matches = inventory
        .phones()
        .as_slice()
        .iter()
        .filter(|definition| definition.identity() == requested);
    let definition = matches.next().ok_or(InventoryRefusal::MissingDefinition)?;
    if matches.next().is_some() {
        return Err(InventoryRefusal::AmbiguousDefinition);
    }
    let identity =
        SpeechPhoneDefinitionMatch::new(definition.identity().clone(), requested.clone())
            .map_err(InventoryRefusal::Definition)?;
    Ok(InventoryPhone {
        material,
        inventory,
        definition,
        basis,
        identity,
    })
}
