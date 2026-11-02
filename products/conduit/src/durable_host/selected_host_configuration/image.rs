//! Recheck retained source and the actual existing Make IMAGE before native Boot.
use super::*;
use conduit_host_make::{
    validate_profile, verify_image_binding, HostImage, BUILD_MANIFEST_SCHEMA, IMAGE_SCHEMA,
};

pub(crate) fn check_seed_image(
    source: &str,
    image: &HostImage,
) -> Result<CheckedHostConfiguration, String> {
    let checked = check(source)?;
    let bytes = conduit_body_make::encode_joining_host_evidence(image, 64 * 1024)?;
    verify_image_binding(image, &bytes).map_err(|_| "Seed IMAGE payload identity changed")?;
    let profile = validate_profile(
        checked.profile().clone(),
        &conduit_workspace_make::catalog(),
    )
    .map_err(|_| "Seed Host profile is not checked")?;
    let manifest = &image.manifest;
    let payload = &image.payload;
    // The manifest cannot enlarge or replace semantics committed by the IMAGE
    // payload identity. Reuse the generic digest verifier, then compare all
    // shared fields without constructing a second metadata representation.
    macro_rules! same_payload_fields {
        ($first:ident $(, $field:ident)* $(,)?) => {
            if manifest.$first != payload.$first $(|| manifest.$field != payload.$field)* {
                return Err("Seed IMAGE manifest differs from its payload".into());
            }
        };
    }
    same_payload_fields!(
        declared_name,
        build_id,
        profile_id,
        source_configuration_id,
        target,
        make_descriptor,
        profile_fragments,
        implementations,
        host_calls,
        resources,
        resource_budgets,
        bases,
        base_selections,
        drivers,
        driver_selections,
        lines,
        presenters,
        facilities,
        bounds
    );
    if manifest.schema != BUILD_MANIFEST_SCHEMA
        || payload.schema != IMAGE_SCHEMA
        || profile.profile_id().as_str() != manifest.profile_id
        || manifest.profile_id != payload.profile_id
        || manifest.build_id != payload.build_id
        || manifest.source_configuration_id.as_deref() != Some(checked.configuration_id())
        || payload.source_configuration_id != manifest.source_configuration_id
        || checked.profile().target.key() != manifest.target
        || payload.target != manifest.target
        || checked.profile().bounds != manifest.bounds
        || payload.bounds != manifest.bounds
        || manifest.declared_name.as_ref().map(|name| name.as_str())
            != Some(checked.configuration().name.as_str())
    {
        return Err("Seed IMAGE differs from checked source/configuration/profile/bounds".into());
    }
    let source_profile = profile.profile();
    // A copied profile/configuration ID does not establish that IMAGE's actual
    // equipment and budgets came from that Source. Compare borrowed projection
    // fields without rebuilding or retaining a second IMAGE payload.
    let strings = |expected: Vec<&str>, actual: &[String]| {
        let mut expected = expected;
        let mut actual = actual.iter().map(String::as_str).collect::<Vec<_>>();
        expected.sort_unstable();
        actual.sort_unstable();
        expected == actual
    };
    if manifest.make_descriptor != source_profile.target.make_descriptor
        || !same_members(&source_profile.bases, &manifest.base_selections)
        || !same_members(&source_profile.drivers, &manifest.driver_selections)
        || !same_members(&source_profile.resources, &manifest.resource_budgets)
        || !strings(
            source_profile
                .fragments
                .iter()
                .map(String::as_str)
                .collect(),
            &manifest.profile_fragments,
        )
        || !strings(
            source_profile
                .capabilities
                .iter()
                .map(|item| item.implementation.as_str())
                .collect(),
            &manifest.implementations,
        )
        || !strings(
            source_profile
                .host_calls
                .iter()
                .map(String::as_str)
                .collect(),
            &manifest.host_calls,
        )
        || !strings(
            source_profile
                .resources
                .iter()
                .map(|item| item.id.as_str())
                .collect(),
            &manifest.resources,
        )
        || !strings(
            source_profile
                .bases
                .iter()
                .map(|item| item.id.as_str())
                .collect(),
            &manifest.bases,
        )
        || !strings(
            source_profile
                .drivers
                .iter()
                .map(|item| item.id.as_str())
                .collect(),
            &manifest.drivers,
        )
        || !strings(
            source_profile.lines.iter().map(String::as_str).collect(),
            &manifest.lines,
        )
        || !strings(
            source_profile
                .presenters
                .iter()
                .map(|item| item.implementation.as_str())
                .collect(),
            &manifest.presenters,
        )
        || !strings(
            source_profile
                .facilities
                .iter()
                .map(String::as_str)
                .collect(),
            &manifest.facilities,
        )
    {
        return Err("Seed IMAGE equipment and resources differ from checked Source".into());
    }
    Ok(checked)
}

fn same_members<T: PartialEq>(expected: &[T], actual: &[T]) -> bool {
    expected.len() == actual.len()
        && expected.iter().all(|item| {
            expected.iter().filter(|value| *value == item).count()
                == actual.iter().filter(|value| *value == item).count()
        })
}
