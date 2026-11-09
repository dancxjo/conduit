// Current original-owner construction inputs; never approval or an executor.
use super::*;
use alloc::vec::Vec;
use conduit_mcu_host_runtime::{
    continuation_native::ContinuationNativeContext,
    execution_authority::ContinuationNativeDescription, native_authority::Issuers,
};
mod native {
    include!(concat!(
        env!("OUT_DIR"),
        "/source_play_continuation_native.rs"
    ));
}
#[derive(Debug)]
pub enum MaterialRefusal {
    Bounds,
    Scope,
    Authority,
}
struct Text<'a> {
    node: u16,
    instance: &'a str,
    grant: &'a str,
    generation: u64,
}
struct Entry<'a> {
    native: Vec<c::FreshAssignedNative>,
    texts: Vec<Text<'a>>,
    bindings: Vec<c::FragmentSealBinding<'a>>,
    resources: Vec<b::FragmentResource<'a>>,
}
/// Borrowed original facts and finite allocated construction rows only. Drop
/// before mutable Root admission; no capability/table is issued by this owner.
pub struct Material<'a> {
    actual: &'a c::HostAdvertisement,
    entries: [Entry<'a>; 3],
    bytes: usize,
}
macro_rules! pin {
    ($software:ident,$layout:ident) => {
        b::ContinuationBundleEntry {
            name: source::$layout::ENTRY,
            source_document: source::$software::SOURCE_DOCUMENT_ID,
            checked_plot: source::$software::CHECKED_PLOT_ID,
            expanded_plot: source::$software::EXPANDED_PLOT_ID,
            projection_digest: source::$layout::PROJECTION_TEMPLATE_DIGEST,
            recipe_digest: source::$layout::FRAGMENT_RECIPE_DIGEST,
            placement_ids: source::$layout::FRAGMENT_PLACEMENT_IDS,
            placement_nodes: source::$layout::FRAGMENT_SEAL_TO_KERNEL,
        }
    };
}
pub const SOURCE_DIGEST: [u8; 32] = source::SOURCE_SHA256;
pub const PINS: [b::ContinuationBundleEntry<'static>; 3] = [
    pin!(sampling, sampling_layout),
    pin!(mask, mask_layout),
    pin!(output, output_layout),
];
fn reserve<T>(count: usize, used: &mut usize, maximum: usize) -> Result<Vec<T>, MaterialRefusal> {
    let required = count
        .checked_mul(core::mem::size_of::<T>())
        .and_then(|n| n.checked_add(*used))
        .ok_or(MaterialRefusal::Bounds)?;
    if required > maximum {
        return Err(MaterialRefusal::Bounds);
    }
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| MaterialRefusal::Bounds)?;
    *used = values
        .capacity()
        .checked_mul(core::mem::size_of::<T>())
        .and_then(|n| n.checked_add(*used))
        .ok_or(MaterialRefusal::Bounds)?;
    if *used > maximum {
        return Err(MaterialRefusal::Bounds);
    }
    Ok(values)
}
impl<'a> Material<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        context: ContinuationNativeContext<'a>,
        policy: Policy<'_>,
        equipment: &'a crate::equipment::Equipment,
        issuers: &'a Issuers<WAIT_SLOTS>,
        operations: &Operations<'_, WAIT_SLOTS>,
        recording: Option<&Operation<'_>>,
        maximum_preparation_bytes: usize,
    ) -> Result<Self, MaterialRefusal> {
        let actual = context.actual();
        let mut bytes = 0;
        macro_rules! build {
            ($layout:ident,$software:ident,$index:expr) => {
                entry(
                    context,
                    policy,
                    equipment,
                    issuers,
                    operations,
                    recording,
                    &PINS[$index],
                    source::$layout::PROJECTION_TEMPLATE,
                    source::$layout::FRAGMENT_SEAL_RECIPE,
                    source::$software::NATIVE_KINDS,
                    source::$layout::CONTINUATION_RESOURCES,
                    &mut bytes,
                    maximum_preparation_bytes,
                )?
            };
        }
        let entries = [
            build!(sampling_layout, sampling, 0),
            build!(mask_layout, mask, 1),
            build!(output_layout, output, 2),
        ];
        Ok(Self {
            actual,
            entries,
            bytes,
        })
    }
    pub fn retained_backing_bytes(&self) -> usize {
        self.bytes
    }
    /// All slices live only inside this borrow. Caller supplies funded output
    /// and scratch to the existing canonical candidate and FreshScope checker.
    pub fn with_entries<R>(
        &self,
        use_entries: impl FnOnce(
            &[b::ContinuationBundleEntry<'static>; 3],
            &[b::ContinuationLocalEntry<'_>; 3],
            &[&[b::FragmentResource<'_>]; 3],
        ) -> R,
    ) -> R {
        macro_rules! local {
            ($layout:ident,$index:expr) => {
                b::ContinuationLocalEntry {
                    recipe: source::$layout::FRAGMENT_SEAL_RECIPE,
                    bindings: &self.entries[$index].bindings,
                    template: source::$layout::PROJECTION_TEMPLATE,
                    native: &self.entries[$index].native,
                    fore: source::$layout::FRESH_FORE,
                    profile: profile!(
                        $layout,
                        c::AssignedIdentity::from_text(self.actual.host_id.as_str()),
                        c::AssignedIdentity::from_text(self.actual.boot_id.as_str()),
                        &source::$layout::FORE
                    ),
                }
            };
        }
        let entries = [
            local!(sampling_layout, 0),
            local!(mask_layout, 1),
            local!(output_layout, 2),
        ];
        let resources = [
            self.entries[0].resources.as_slice(),
            self.entries[1].resources.as_slice(),
            self.entries[2].resources.as_slice(),
        ];
        use_entries(&PINS, &entries, &resources)
    }
}
#[allow(clippy::too_many_arguments)]
fn entry<'a>(
    context: ContinuationNativeContext<'a>,
    policy: Policy<'_>,
    equipment: &'a crate::equipment::Equipment,
    issuers: &'a Issuers<WAIT_SLOTS>,
    operations: &Operations<'_, WAIT_SLOTS>,
    recording: Option<&Operation<'_>>,
    pin: &b::ContinuationBundleEntry<'static>,
    template: &'static [u8],
    recipe: &'static [c::FragmentSealPiece<'static>],
    kinds: &[(u16, &str)],
    resources: &'static [(u16, u16, &str, &str, &str, u32)],
    bytes: &mut usize,
    maximum: usize,
) -> Result<Entry<'a>, MaterialRefusal> {
    let actual = context.actual();
    let count = c::AssignedPlanRecords::new(template)
        .map_err(|_| MaterialRefusal::Scope)?
        .filter_map(Result::ok)
        .filter(|r| {
            r.tag == c::ASSIGNED_HOST_CALL
                && c::assigned_native_binding(r.payload)
                    .ok()
                    .flatten()
                    .is_some()
        })
        .count();
    let mut row = Entry {
        native: reserve(count, bytes, maximum)?,
        texts: reserve(count, bytes, maximum)?,
        bindings: reserve(recipe.len(), bytes, maximum)?,
        resources: reserve(resources.len(), bytes, maximum)?,
    };
    for record in c::AssignedPlanRecords::new(template).map_err(|_| MaterialRefusal::Scope)? {
        let record = record.map_err(|_| MaterialRefusal::Scope)?;
        if record.tag != c::ASSIGNED_HOST_CALL {
            continue;
        }
        let Some(binding) =
            c::assigned_native_binding(record.payload).map_err(|_| MaterialRefusal::Scope)?
        else {
            continue;
        };
        let u16_at = |at| {
            u16::from_le_bytes(
                record.payload[at..at + 2]
                    .try_into()
                    .expect("checked native record width"),
            )
        };
        let node = u16_at(0);
        let kind = kinds
            .iter()
            .find(|n| n.0 == node)
            .ok_or(MaterialRefusal::Scope)?
            .1;
        let description = native::describe(
            context, policy, equipment, issuers, operations, recording, node, kind, binding,
        )?;
        row.native.push(c::FreshAssignedNative {
            node,
            call: u16_at(2),
            contract: c::AssignedIdentity(
                record.payload[4..20]
                    .try_into()
                    .expect("checked call identity"),
            ),
            binding: description.binding,
        });
        row.texts.push(Text {
            node,
            instance: description.instance,
            grant: description.grant,
            generation: description.binding.generation,
        });
    }
    for &(node, resource, placement, pool, class, units) in resources {
        let mut matching = actual
            .resources
            .iter()
            .filter(|p| p.pool_id.as_str() == pool);
        let held = matching.next().ok_or(MaterialRefusal::Scope)?;
        let ordinal = pin
            .placement_nodes
            .iter()
            .position(|n| *n == node)
            .ok_or(MaterialRefusal::Scope)?;
        if matching.next().is_some()
            || held.class_id.as_str() != class
            || units == 0
            || units > held.capacity_units
            || held.compute.is_some()
            || pin.placement_ids[ordinal] != placement
        {
            return Err(MaterialRefusal::Scope);
        }
        c::fragment_seal_resource_matches(
            recipe,
            ordinal.try_into().map_err(|_| MaterialRefusal::Bounds)?,
            resource,
            held,
            units,
        )
        .map_err(|_| MaterialRefusal::Scope)?;
        let content = held.content.as_ref().map(|content| b::FragmentContent {
            contract: b::FragmentContentContract::from_core(&content.contract),
            base: content.base_id.as_str(),
            residence_profile: content.residence_profile.as_str(),
        });
        if held
            .content
            .as_ref()
            .is_some_and(|c| c.owner_host != actual.host_id || c.owner_boot != actual.boot_id)
        {
            return Err(MaterialRefusal::Scope);
        }
        row.resources.push(b::FragmentResource {
            binding: b::PlainFragmentResource {
                node,
                resource,
                placement,
                pool: held.pool_id.as_str(),
                class: held.class_id.as_str(),
                units,
            },
            content,
        });
    }
    use c::{FragmentScopeRole as Role, FragmentSealValue as Value};
    for piece in recipe {
        let c::FragmentSealPiece::Field(field, _) = piece else {
            continue;
        };
        if row.bindings.iter().any(|b| b.field == *field) {
            continue;
        }
        let text = |s| Value::Text(s);
        let value = match field.placement {
            None => match field.role {
                Role::Host => text(actual.host_id.as_str()),
                Role::Boot => text(actual.boot_id.as_str()),
                Role::OfferGeneration => Value::U64(actual.offer_generation.0),
                _ => return Err(MaterialRefusal::Scope),
            },
            Some(p) => {
                let ordinal = usize::from(p);
                let node = *pin
                    .placement_nodes
                    .get(ordinal)
                    .ok_or(MaterialRefusal::Scope)?;
                let native = || {
                    row.texts
                        .iter()
                        .find(|n| n.node == node)
                        .ok_or(MaterialRefusal::Scope)
                };
                match field.role {
                    Role::PlacementId => text(pin.placement_ids[ordinal]),
                    Role::Host | Role::AuthorityHost | Role::ContentOwnerHost => {
                        text(actual.host_id.as_str())
                    }
                    Role::Boot | Role::AuthorityBoot | Role::ContentOwnerBoot => {
                        text(actual.boot_id.as_str())
                    }
                    Role::OfferGeneration => Value::U64(actual.offer_generation.0),
                    Role::ProviderInstance => text(native()?.instance),
                    Role::ProviderGeneration => Value::U64(native()?.generation),
                    Role::AuthorityGrant => text(native()?.grant),
                    Role::ResourcePool => text(
                        row.resources
                            .iter()
                            .find(|r| r.node == node && r.resource == field.entry)
                            .ok_or(MaterialRefusal::Scope)?
                            .pool,
                    ),
                    _ => return Err(MaterialRefusal::Scope),
                }
            }
        };
        row.bindings.push(c::FragmentSealBinding {
            field: *field,
            value,
        });
    }
    Ok(row)
}
