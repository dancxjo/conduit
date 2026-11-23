//! Compose one bounded Home observation in the admitted native source worker.
use crate::{
    ambient_clock::PreparedLiveClock,
    ambient_wallpaper::{
        acquisition::PreparedDeck, selection_wire::PreparedWallpaperSelectionWire,
    },
    NativeReadContext, NativeSourceWorkerProvider, WorkerFailure,
};
use conduit_core::{
    validate_canonical_structured_value as canonical, PreparedStructuredComposer as Composer,
    StructuredInfoTypeShape,
};
use std::net::SocketAddr;

pub const HOME_OBSERVATION_BYTES: usize =
    conduit_semantic_catalog::AMBIENT_HOME_OBSERVATION_BYTES as usize;
const CLOCK_OBSERVATION_BYTES: usize = 4096;

pub struct PreparedAmbientHomeObservation {
    clock: PreparedLiveClock,
    clock_bytes: Box<[u8]>,
    deck: PreparedDeck,
    wallpaper: PreparedWallpaperSelectionWire,
    unavailable_wallpaper: Box<[u8]>,
    home: Composer,
}

impl PreparedAmbientHomeObservation {
    /// The caller admits the peer, the worker, and the original photo custody.
    /// The deck remains available for separately admitted graphical realization.
    pub fn prepare(
        timezone: &str,
        endpoint: SocketAddr,
        deck: PreparedDeck,
    ) -> Result<Self, String> {
        let types = conduit_semantic_catalog::prepare_ambient_display_types()?;
        let ty = &types
            .native_types
            .iter()
            .find(|ty| ty.name == "AmbientHomeObservation")
            .ok_or("missing AmbientHomeObservation")?
            .value_type;
        let StructuredInfoTypeShape::Record {
            fields: [clock_field, wallpaper_field],
            ..
        } = ty.shape()
        else {
            return Err("unexpected Home observation fields".into());
        };
        if clock_field.name() != "clock_weather" || wallpaper_field.name() != "wallpaper" {
            return Err("unexpected Home observation field order".into());
        }
        let wallpaper_type = wallpaper_field.value_type();
        let StructuredInfoTypeShape::Variant { cases, .. } = wallpaper_type.shape() else {
            return Err("expected wallpaper observation cases".into());
        };
        let unavailable = cases
            .iter()
            .find(|case| case.tag() == "unavailable")
            .ok_or("missing wallpaper absence case")?;
        let text_type = unavailable.payload_type();
        let mut reason = Composer::new(text_type, 256).map_err(debug_error)?;
        let reason = canonical(
            reason
                .leaf(b"Wallpaper selection time unavailable")
                .map_err(debug_error)?,
        )
        .map_err(debug_error)?;
        let mut absence =
            Composer::new(wallpaper_type, HOME_OBSERVATION_BYTES).map_err(debug_error)?;
        let unavailable_wallpaper = absence
            .variant("unavailable", reason)
            .map_err(debug_error)?
            .into();
        Ok(Self {
            clock: PreparedLiveClock::prepare(timezone, endpoint)?,
            clock_bytes: vec![0; CLOCK_OBSERVATION_BYTES].into_boxed_slice(),
            wallpaper: PreparedWallpaperSelectionWire::prepare(&deck).map_err(debug_error)?,
            deck,
            unavailable_wallpaper,
            home: Composer::new(ty, HOME_OBSERVATION_BYTES).map_err(debug_error)?,
        })
    }

    pub fn photos(&self) -> &PreparedDeck {
        &self.deck
    }

    pub fn prepared_heap_bytes(&self) -> usize {
        self.deck.retained_bytes()
            + self.clock.prepared_heap_bytes()
            + self.clock_bytes.len()
            + self.wallpaper.prepared_heap_bytes()
            + self.unavailable_wallpaper.len()
            + self.home.prepared_heap_bytes()
    }
    fn compose(&mut self, len: usize) -> Result<&[u8], WorkerFailure> {
        let clock = canonical(&self.clock_bytes[..len]).map_err(|_| WorkerFailure::Provider)?;
        let instant = clock
            .record_field("observed_at_unix_ms")
            .map_err(|_| WorkerFailure::Provider)?
            .ok_or(WorkerFailure::Provider)?
            .primitive_bytes("value/u64")
            .map_err(|_| WorkerFailure::Provider)?;
        let instant = u64::from_le_bytes(instant.try_into().map_err(|_| WorkerFailure::Provider)?);
        let wallpaper = self
            .wallpaper
            .encode(instant)
            .unwrap_or(&self.unavailable_wallpaper);
        let wallpaper = canonical(wallpaper).map_err(|_| WorkerFailure::Provider)?;
        self.home
            .record(&[clock, wallpaper])
            .map_err(|_| WorkerFailure::InvalidOutput)
    }
}

fn debug_error(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}

// SAFETY: both contributors use fixed prepared backing and the nested clock
// uses the same worker deadline and cancellation context. No borrow escapes.
unsafe impl NativeSourceWorkerProvider for PreparedAmbientHomeObservation {
    fn read(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        context: &NativeReadContext<'_>,
    ) -> Result<usize, WorkerFailure> {
        let len = self.clock.read(input, &mut self.clock_bytes, context)?;
        let combined = self.compose(len)?;
        if context.cancelled() {
            return Err(WorkerFailure::Cancelled);
        }
        if context.remaining().is_zero() {
            return Err(WorkerFailure::TimedOut);
        }
        output
            .get_mut(..combined.len())
            .ok_or(WorkerFailure::InvalidOutput)?
            .copy_from_slice(combined);
        Ok(combined.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ambient_clock::PreparedAmbientClockWire,
        ambient_wallpaper::{
            acquisition::{
                prepare_deck, PreparationAdmission, MAXIMUM_RETAINED_BYTES, PREPARATION_BYTES,
            },
            MAXIMUM_APOD_RESPONSE_BYTES,
        },
    };
    use image::{codecs::png::PngEncoder, ExtendedColorType, ImageEncoder};

    fn fixture_deck() -> PreparedDeck {
        let admission = PreparationAdmission::admit(
            chrono::NaiveDate::from_ymd_opt(2026, 11, 16).unwrap(),
            MAXIMUM_RETAINED_BYTES,
            PREPARATION_BYTES,
        )
        .unwrap();
        let mut number = 0;
        prepare_deck(admission, |requested, maximum| {
            if maximum == MAXIMUM_APOD_RESPONSE_BYTES {
                return Ok(serde_json::to_vec(&serde_json::json!({
                    "date": requested.rsplit('=').next().unwrap(),
                    "title": "Fixture photo",
                    "media_type": "image",
                    "hdurl": "https://apod.nasa.gov/fixture.png"
                }))
                .unwrap());
            }
            number += 1;
            let mut pixels = Vec::new();
            PngEncoder::new(&mut pixels)
                .write_image(&[number, 2, 3, 255], 1, 1, ExtendedColorType::Rgba8)
                .unwrap();
            Ok(pixels)
        })
        .unwrap()
    }

    #[test]
    fn prepared_home_rotates_from_the_clock_observation_without_allocations() {
        let mut home = PreparedAmbientHomeObservation::prepare(
            "America/Los_Angeles",
            "127.0.0.1:80".parse().unwrap(),
            fixture_deck(),
        )
        .unwrap();
        let mut clock =
            PreparedAmbientClockWire::prepare("America/Los_Angeles", "Weather unavailable")
                .unwrap();
        for (now, selected) in [(100_000, 100_000), (399_999, 100_000), (400_000, 400_000)] {
            let sample = clock.encode(now).unwrap();
            home.clock_bytes[..sample.len()].copy_from_slice(sample);
            let probe = crate::allocation_probe::begin();
            let encoded = home.compose(sample.len()).unwrap();
            assert_eq!(probe.finish_bytes(), 0);
            let observation = canonical(encoded).unwrap();
            let selection = observation
                .record_field("wallpaper")
                .unwrap()
                .unwrap()
                .variant_payload("available")
                .unwrap()
                .unwrap();
            let observed = selection
                .record_field("selected_at_unix_ms")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/u64")
                .unwrap();
            assert_eq!(u64::from_le_bytes(observed.try_into().unwrap()), selected);
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn ambient_home_worker_becomes_an_installed_offer_only_with_its_exact_grant_and_backing() {
        use crate::{NativeSourceDefinition, NativeWorkerBudget, StdHost, StdHostConfig};
        use conduit_core::*;
        use conduit_std_offers::*;
        use std::time::Duration;

        let mut host = StdHost::new_with_config(StdHostConfig {
            host_id: "test/ambient-host".into(),
            boot_id: "test/ambient-boot".into(),
            offer_generation: OfferGeneration(1),
        });
        let offer = ambient_home_source_offer().unwrap();
        let kind = conduit_semantic_catalog::ambient_home_observation_kind().unwrap();
        let budget = NativeWorkerBudget {
            requested_stack_bytes: 2 * 1024 * 1024,
            maximum_stack_mapping_bytes: 2 * 1024 * 1024 + 64 * 1024,
            capture_timeout: Duration::from_secs(3),
            cancellation_timeout: Duration::from_millis(250),
        };
        let grant = AuthorityGrant {
            grant_id: "test/ambient-grant".into(),
            contract_id: AMBIENT_HOME_SOURCE_AUTHORITY.into(),
            host_call_contract_id: AMBIENT_HOME_SOURCE_CALL.into(),
            subject_kind: kind.kind_id.clone(),
            host_id: host.advertisement().host_id.clone(),
            boot_id: host.advertisement().boot_id.clone(),
            capability_id: offer.capability_id.clone(),
        };
        let output_bytes = HOME_OBSERVATION_BYTES as u32;
        let input_bytes = conduit_time::TICK_ENCODED_LEN;
        let provider = PreparedAmbientHomeObservation::prepare(
            "America/Los_Angeles",
            "127.0.0.1:80".parse().unwrap(),
            fixture_deck(),
        )
        .unwrap();
        let backing = budget.backing_bytes(input_bytes, output_bytes).unwrap();
        let mut definition = NativeSourceDefinition {
            kind,
            grant,
            base: BaseProviderEntry {
                base_id: "test/ambient-base".into(),
                provider_instance_id: "test/ambient-session".into(),
                provider_generation: 1,
                implementation_id: AMBIENT_HOME_SOURCE_IMPLEMENTATION.into(),
                mechanism_family: conduit_semantic_catalog::AMBIENT_HOME_OBSERVE_KIND.into(),
                enforcement_class: BaseEnforcementClass::Cooperative,
                lifecycle: BaseLifecycle::Ready,
                capabilities: vec![offer.clone()],
                resources: vec![
                    resource_offer("test/ambient/source", AMBIENT_HOME_SOURCE_RESOURCE, 1),
                    resource_offer(
                        "test/ambient/backing",
                        RUNTIME_MEMORY_RESOURCE_CLASS,
                        backing,
                    ),
                ],
            },
            resource_generation: ResourceGenerationId("test/ambient/1".into()),
            configuration: vec![ConfigurationEntry {
                key: "timezone".into(),
                value: ConfigurationValue::Text("America/Los_Angeles".into()),
            }],
            input_bytes,
            output_bytes,
            resource_bytes: u64::from(output_bytes),
        };
        let correct_boot = definition.grant.boot_id.clone();
        definition.grant.boot_id = "test/stale-boot".into();
        // The exact prepared fixture owns fixed local photo bytes and can only
        // contact the declared loopback peer. No read is made by installation.
        let (mut definition, provider, _) =
            unsafe { host.install_native_source_worker(definition, budget, Box::new(provider)) }
                .expect_err("a stale grant must refuse before worker startup");
        assert!(!host.advertisement().capabilities.contains(&offer));
        definition.grant.boot_id = correct_boot;
        let grant = definition.grant.clone();
        unsafe { host.install_native_source_worker(definition, budget, provider) }
            .unwrap_or_else(|(_, _, reason)| panic!("{reason}"));
        assert!(host.advertisement().capabilities.contains(&offer));
        let plan = plan_fixture(&host, grant);
        let placement = plan
            .fragments
            .iter()
            .flat_map(|f| &f.placements)
            .find(|gear| gear.capability_id == offer.capability_id)
            .unwrap();
        assert_eq!(placement.host_calls, offer.host_calls);
        assert_eq!(placement.authority[0].boot_id, host.advertisement().boot_id);
    }

    #[cfg(target_os = "linux")]
    fn plan_fixture(
        host: &crate::StdHost,
        grant: conduit_core::AuthorityGrant,
    ) -> conduit_core::Plan {
        use conduit_core::*;
        use conduit_planner::{
            default_expanded_placements, plan_expanded_authoring_with_options, PlanningOptions,
        };
        use conduit_plot::{
            check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
            ProfileCatalog, StartupCatalog,
        };
        use conduit_semantic_catalog as catalog;
        use std::collections::BTreeMap;

        let mut startup = StartupCatalog::new();
        let mut profile = ProfileCatalog::new();
        catalog::install_ambient_display_types(&mut startup).unwrap();
        catalog::install_ambient_home_observation_kind(&mut startup, &mut profile).unwrap();
        startup
            .insert_value_kind_alias("Tick", kind_id(conduit_time::TICK_VALUE_KIND))
            .unwrap();
        let checked = check_syntax_document(
            &parse_syntax_document(
                r#"
plot home (
    >> pulse: Tick...
    current: AmbientHomeObservation... >>
) {
    source: ambient/resident-observation("America/Los_Angeles")
    pulse >> source.tick
    source.observation >> current
}
"#,
            ),
            &startup,
        )
        .unwrap();
        let authored = expand_canonical_plot_for_authoring(&checked, "home", &profile).unwrap();
        let hosts = [host.advertisement().clone()];
        let placements = default_expanded_placements(&authored.expanded, &hosts).unwrap();
        let boundaries = [
            (
                PortDirection::Input,
                "pulse",
                conduit_time::TICK_ENCODED_LEN,
            ),
            (
                PortDirection::Output,
                "current",
                HOME_OBSERVATION_BYTES as u32,
            ),
        ]
        .into_iter()
        .map(|(direction, name, byte_capacity)| {
            (
                ForeBoundaryKey {
                    direction,
                    front_port_id: name.into(),
                    track: ConnectionTrack::Payload,
                },
                ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity,
                },
            )
        })
        .collect();
        plan_expanded_authoring_with_options(
            &authored,
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
            PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: HOME_OBSERVATION_BYTES as u32,
                authority_grants: &[grant],
                protected_resource_grants: &[],
                line_offers: &[],
            },
            &boundaries,
        )
        .unwrap()
    }
}
