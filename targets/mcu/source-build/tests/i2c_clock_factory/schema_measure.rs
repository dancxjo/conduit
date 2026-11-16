//! Independent process with compile-linked immutable schemas. It never imports
//! the authoring compiler, prepares a Root, or constructs a runtime assignment.
use conduit_core::BorrowedStructuredValueValidator as Validator;
use conduit_kernel::HostedValueStore;
use conduit_mcu_host_runtime::native_values::ValueSlotProfile as Profile;
use conduit_plan_lowering::shared_schema_value_store::{PreparationError, SharedSchemaValueStore};

#[allow(dead_code)]
mod profiles {
    include!(env!("CONDUIT_SCHEMA_PROFILE_FILE"));
}
mod allocation {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../architecture/plot/tests/common/allocation_probe.rs"
    ));
    pub fn live() -> usize {
        OBSERVATION.with(|o| o.get().live_bytes)
    }
}
#[global_allocator]
static ALLOCATOR: allocation::Allocator = allocation::Allocator;
const BACKING_CAP: usize = 49152;
type Store = SharedSchemaValueStore<'static, HostedValueStore>;
#[derive(Debug)]
enum Failure {
    Allocation,
    Geometry,
    Canonical(conduit_kernel::ValueStorePreparationError),
    Schema {
        error: PreparationError,
        canonical_backing: usize,
    },
}
impl Failure {
    fn capacity(&self) -> Option<(usize, usize)> {
        match self {
            Self::Canonical(conduit_kernel::ValueStorePreparationError::Capacity {
                required,
                available,
            })
            | Self::Schema {
                error:
                    PreparationError::Capacity {
                        required,
                        available,
                    },
                ..
            } => Some((*required, *available)),
            _ => None,
        }
    }
    fn canonical_backing(&self) -> Option<usize> {
        match self {
            Self::Schema {
                canonical_backing, ..
            } => Some(*canonical_backing),
            _ => None,
        }
    }
    fn detail(&self) -> String {
        match self {
            Self::Allocation => "temporary descriptor allocation".into(),
            Self::Geometry => "invalid geometry".into(),
            Self::Canonical(error) => format!("{error:?}"),
            Self::Schema { error, .. } => format!("{error:?}"),
        }
    }
}
fn profiles() -> impl Iterator<Item = &'static Profile> {
    profiles::CORD_VALUE_PROFILES
        .iter()
        .chain(profiles::STEP_VALUE_PROFILES)
}
fn maximum(profile: &Profile) -> u32 {
    match profile {
        Profile::Canonical(bytes)
        | Profile::Schema {
            maximum_bytes: bytes,
            ..
        } => *bytes,
    }
}
fn reserve<T>(count: usize) -> Result<Vec<T>, Failure> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Failure::Allocation)?;
    Ok(values)
}
fn canonical(capacities: &[u32], backing_cap: usize) -> Result<HostedValueStore, Failure> {
    let bytes = capacities
        .iter()
        .try_fold(0u32, |sum, n| sum.checked_add(*n))
        .ok_or(Failure::Geometry)?;
    HostedValueStore::try_new_with_slot_capacities(capacities, bytes, backing_cap)
        .map_err(Failure::Canonical)
}
fn dense_schema(backing_cap: usize) -> Result<Store, Failure> {
    // These construction descriptors are allocated inside the measured phase
    // and genuinely dropped before the returned store is inspected.
    let count = profiles().count();
    let mut logical = reserve(count)?;
    let mut capacities = reserve(count + profiles::NATIVE_RESULT_CAPACITIES.len())?;
    let mut logical_bytes = 0u32;
    for profile in profiles() {
        match *profile {
            Profile::Canonical(bytes) => capacities.push(bytes),
            Profile::Schema {
                canonical,
                maximum_bytes,
            } => {
                let validator = Validator::new(canonical, maximum_bytes as usize)
                    .map_err(|_| Failure::Geometry)?;
                logical.push((validator, 1));
                logical_bytes = logical_bytes
                    .checked_add(maximum_bytes)
                    .ok_or(Failure::Geometry)?;
            }
        }
    }
    capacities.extend_from_slice(profiles::NATIVE_RESULT_CAPACITIES);
    let canonical = canonical(&capacities, backing_cap)?;
    let remaining = backing_cap
        .checked_sub(canonical.reserved_storage_bytes())
        .ok_or(Failure::Geometry)?;
    // Dense physical stock equals every original logical declaration, including
    // heterogeneous extents. No fabricated owner reports or lifetime reduction.
    let canonical_backing = canonical.reserved_storage_bytes();
    SharedSchemaValueStore::try_new(canonical, &logical, &logical, logical_bytes, remaining)
        .map_err(|error| Failure::Schema {
            error,
            canonical_backing,
        })
}
type CanonicalResult = Result<(usize, usize), Failure>;
struct CanonicalObservation {
    result: CanonicalResult,
    observed: allocation::Observation,
    live_after: usize,
    backing_cap: usize,
}
fn canonical_case(backing_cap: usize) -> CanonicalObservation {
    let (result, observed) = allocation::observe(|| {
        let mut capacities =
            reserve(profiles().count() + profiles::NATIVE_RESULT_CAPACITIES.len())?;
        capacities.extend(profiles().map(maximum));
        capacities.extend_from_slice(profiles::NATIVE_RESULT_CAPACITIES);
        let store = canonical(&capacities, backing_cap)?;
        drop(capacities);
        let backing = store.reserved_storage_bytes();
        let live = allocation::live();
        assert_eq!(backing, live, "independent canonical allocation counter");
        drop(store);
        Ok::<_, Failure>((backing, live))
    });
    let live_after = allocation::live();
    assert_eq!(live_after, 0);
    CanonicalObservation {
        result,
        observed,
        live_after,
        backing_cap,
    }
}
impl CanonicalObservation {
    fn json(&self) -> serde_json::Value {
        let result = &self.result;
        let observed = self.observed;
        let detail = result.as_ref().err().map(Failure::detail);
        serde_json::json!({"backing_cap":self.backing_cap, "prepared":result.is_ok(), "refusal":detail, "required_by_ctor":result.as_ref().err().and_then(Failure::capacity).map(|v| v.0),
        "retained_backing":result.as_ref().ok().map(|r| r.0),
        "independent_live_at_store":result.as_ref().ok().map(|r| r.1),
        "peak_including_preparation_temporaries":observed.peak_bytes,
        "live_after_drop":self.live_after, "inline_owner_bytes":std::mem::size_of::<HostedValueStore>()})
    }
}

type SchemaResult = Result<(usize, usize, usize, usize, usize, u32, u16), Failure>;
struct SchemaObservation {
    result: SchemaResult,
    observed: allocation::Observation,
    live_after: usize,
    backing_cap: usize,
}
fn schema_case(backing_cap: usize) -> SchemaObservation {
    let (result, observed) = allocation::observe(|| {
        let store = dense_schema(backing_cap)?;
        let canonical = store.canonical_storage().reserved_storage_bytes();
        let metadata = store.reserved_metadata_bytes();
        let payload = store.reserved_payload_bytes();
        let backing = canonical + metadata + payload;
        let live = allocation::live();
        assert_eq!(
            backing, live,
            "independent allocator agrees with actual retained owner reports"
        );
        let logical_capacity = conduit_kernel::ValueStorage::byte_capacity(&store);
        let logical_items = conduit_kernel::ValueStorage::item_capacity(&store);
        drop(store);
        assert_eq!(
            allocation::live(),
            0,
            "all measured mutable storage was retired"
        );
        Ok::<_, Failure>((
            canonical,
            metadata,
            payload,
            backing,
            live,
            logical_capacity,
            logical_items,
        ))
    });
    let live_after = allocation::live();
    assert_eq!(live_after, 0);
    SchemaObservation {
        result,
        observed,
        live_after,
        backing_cap,
    }
}
impl SchemaObservation {
    fn json(&self) -> serde_json::Value {
        let result = &self.result;
        let observed = self.observed;
        let detail = result.as_ref().err().map(Failure::detail);
        serde_json::json!({"backing_cap":self.backing_cap, "prepared":result.is_ok(), "refusal":detail,
        "required_schema_by_ctor":result.as_ref().err().and_then(Failure::capacity).map(|v| v.0),
        "available_after_canonical":result.as_ref().err().and_then(Failure::capacity).map(|v| v.1),
        "canonical_backing_before_refusal":result.as_ref().err().and_then(Failure::canonical_backing),
        "combined_minimum_required_by_ctor":result.as_ref().err().and_then(|error|
            error.canonical_backing()?.checked_add(error.capacity()?.0)),
        "canonical_backend_bytes":result.as_ref().ok().map(|r| r.0),
        "schema_metadata_bytes":result.as_ref().ok().map(|r| r.1),
        "schema_payload_bytes":result.as_ref().ok().map(|r| r.2),
        "retained_backing":result.as_ref().ok().map(|r| r.3),
        "independent_live_at_store":result.as_ref().ok().map(|r| r.4),
        "logical_store_bytes":result.as_ref().ok().map(|r| r.5),
        "logical_store_items":result.as_ref().ok().map(|r| r.6),
        "peak_including_preparation_temporaries":observed.peak_bytes,
        "observed_attempt_peak_within_cap":observed.peak_bytes <= self.backing_cap,
        "live_after_drop":self.live_after, "inline_owner_bytes":std::mem::size_of::<Store>()})
    }
}

fn parts(profiles: &[Profile]) -> (usize, usize, usize) {
    profiles.iter().fold(
        (0, 0, 0),
        |(canonical_bytes, headers, payload), profile| match profile {
            Profile::Canonical(bytes) => (canonical_bytes + *bytes as usize, headers, payload),
            Profile::Schema {
                canonical,
                maximum_bytes,
            } => (
                canonical_bytes,
                headers + canonical.len(),
                payload + *maximum_bytes as usize - canonical.len(),
            ),
        },
    )
}
fn main() {
    let canonical_extent: usize = profiles().map(|p| maximum(p) as usize).sum();
    let cords = parts(profiles::CORD_VALUE_PROFILES);
    let steps = parts(profiles::STEP_VALUE_PROFILES);
    let native_results: usize = profiles::NATIVE_RESULT_CAPACITIES
        .iter()
        .map(|v| *v as usize)
        .sum();
    let mut unique_headers = 0;
    let mut unique_header_bytes = 0;
    let mut duplicated_header_bytes = 0;
    let mut dense_node_payload = 0;
    let mut schema_slots = 0;
    for (index, profile) in profiles().enumerate() {
        if let Profile::Schema {
            canonical,
            maximum_bytes,
        } = profile
        {
            schema_slots += 1;
            duplicated_header_bytes += canonical.len();
            dense_node_payload += *maximum_bytes as usize - canonical.len();
            if !profiles()
                .take(index)
                .any(|p| matches!(p, Profile::Schema { canonical: old, .. } if old == canonical))
            {
                unique_headers += 1;
                unique_header_bytes += canonical.len();
            }
        }
    }
    // No heap-owned report or previous store survives into the next observed
    // phase. Only scalars and allocation-free refusal enums remain on stack.
    let canonical = canonical_case(BACKING_CAP);
    let dense_schema = schema_case(BACKING_CAP);
    // Separately authorized CPU-only preflight caps, never Host grants or
    // peak guarantees. No heap-owned report is created between any phases.
    let diagnostics = option_env!("CONDUIT_SCHEMA_EXACT_DIAGNOSTIC") == Some("1");
    let canonical_diagnostic = diagnostics.then(|| canonical_case(297604));
    let schema_diagnostic = diagnostics.then(|| schema_case(113946));
    let report = serde_json::json!({
        "scope":"CPU dense store representation only; no application/Root/Play/target fit claim",
        "word_bits":usize::BITS, "backing_cap":BACKING_CAP,
        "assigned_cord_slots":profiles::ASSIGNED_CORD_SLOTS,
        "assigned_cord_logical_bytes":profiles::ASSIGNED_CORD_BYTES,
        "cord_profiles":profiles::CORD_VALUE_PROFILES.len(), "step_profiles":profiles::STEP_VALUE_PROFILES.len(),
        "schema_logical_slots":schema_slots, "unique_linked_schemas":unique_headers,
        "unique_linked_schema_bytes":unique_header_bytes,
        "cpu_readonly_profile_table_bytes":std::mem::size_of_val(profiles::CORD_VALUE_PROFILES)
            + std::mem::size_of_val(profiles::STEP_VALUE_PROFILES)
            + std::mem::size_of_val(profiles::NATIVE_RESULT_CAPACITIES),
        "canonical_schema_bytes_repeated_per_slot":duplicated_header_bytes,
        "dense_schema_node_payload_extent":dense_node_payload,
        "cord_canonical_payload":cords.0, "cord_repeated_schema_headers":cords.1,
        "cord_schema_node_payload":cords.2,
        "step_canonical_payload":steps.0, "step_repeated_schema_headers":steps.1,
        "step_schema_node_payload":steps.2,
        "dense_mutable_payload_floor_before_metadata":cords.0 + steps.0 + native_results + dense_node_payload,
        "all_profile_canonical_extent":canonical_extent,
        "native_result_capacities":profiles::NATIVE_RESULT_CAPACITIES,
        "canonical":canonical.json(), "dense_schema":dense_schema.json(),
        "cpu_only_exact_cap_diagnostics":diagnostics.then(|| serde_json::json!({
            "canonical":canonical_diagnostic.as_ref().unwrap().json(),
            "dense_schema":schema_diagnostic.as_ref().unwrap().json(),
            "caps_are_not_Host_grants_or_peak_guarantees":true,
        })),
        "static_schema_source":"compile-linked read-only arrays, no heap leak or counter subtraction",
        "allocator_counter":"requested Layout bytes; allocator overhead and stack highwater not measured",
        "missing_whole_host_costs":["Root retained input/transport", "Ready providers/native tickets", "software/resident Back owners", "kernel/queues/Signs", "execution workspace", "allocator overhead and stack"],
        "runtime_admission":false, "hardware_effects":false,
    });
    println!("{report}");
}
