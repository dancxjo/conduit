use crate::{brainstem_advertisement, catalogs};
use conduit_core::ConnectionBase;
use conduit_form::{check_syntax_document, expand_canonical_form, parse_syntax_document};
use conduit_planner::{default_expanded_placements, plan_expanded_canonical, PlannerError};

pub const OBSERVATION_FORM: &str = r#"form pete_describe_only {
    bump: robotics/observe-bump
    imu: robotics/observe-imu
}
"#;

pub const ACTUATOR_ATTEMPT_FORM: &str = r#"form forbidden_drive {
    drive: robotics/drive-differential
}
"#;

pub fn observation_plan() -> Result<conduit_core::Plan, String> {
    let (startup, profile) = catalogs()?;
    let syntax = parse_syntax_document(OBSERVATION_FORM);
    let checked = check_syntax_document(&syntax, &startup).map_err(|error| format!("{error:?}"))?;
    let expanded = expand_canonical_form(&checked, "pete_describe_only", &profile)
        .map_err(|error| error.to_string())?;
    let host = brainstem_advertisement();
    let placements = default_expanded_placements(&expanded, std::slice::from_ref(&host))
        .map_err(|error| error.to_string())?;
    plan_expanded_canonical(&expanded, &[host], &placements, &[ConnectionBase::Local])
        .map_err(|error| error.to_string())
}

pub fn attempt_actuator_plan() -> Result<(), PlannerError> {
    let (startup, profile) = catalogs().expect("fixed describe catalogs are valid");
    let syntax = parse_syntax_document(ACTUATOR_ATTEMPT_FORM);
    let checked = check_syntax_document(&syntax, &startup).expect("actuator meaning is valid");
    let expanded = expand_canonical_form(&checked, "forbidden_drive", &profile)
        .expect("actuator meaning expands independently of realization");
    default_expanded_placements(&expanded, &[brainstem_advertisement()]).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BUMP_KIND, DRIVE_KIND, IMU_KIND};

    #[test]
    fn observation_plan_seals_only_effect_free_sensor_offers() {
        let plan = observation_plan().unwrap();
        let placements = &plan.fragments[0].placements;
        assert_eq!(placements.len(), 2);
        assert!(placements.iter().all(|placement| {
            [BUMP_KIND, IMU_KIND].contains(&placement.kind_id.as_str())
                && placement.host_operations.is_empty()
                && placement.authority.is_empty()
        }));
        assert!(!serde_json::to_string(&plan).unwrap().contains(DRIVE_KIND));
    }

    #[test]
    fn valid_actuator_meaning_has_no_describe_only_realization() {
        assert!(matches!(
            attempt_actuator_plan(),
            Err(PlannerError::UnknownCapability(_))
        ));
        let profile = crate::pinned_profile();
        assert!(profile.effect_audit.is_effect_free());
    }
}
