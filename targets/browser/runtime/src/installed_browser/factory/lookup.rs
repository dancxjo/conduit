//! Resolve only installed browser implementation identities.
use super::*;

pub(crate) fn factory(
    implementation_id: &ImplementationId,
) -> Option<&'static BrowserInstallation> {
    if implementation_id.as_str() == super::super::current_sample::IMPLEMENTATION {
        return Some(&super::super::current_sample::INSTALLATION);
    }
    if implementation_id.as_str() == super::super::typed_state::IMPLEMENTATION {
        return Some(&super::super::typed_state::INSTALLATION);
    }
    if implementation_id.as_str() == super::super::structured_selector::IMPLEMENTATION {
        return Some(&super::super::structured_selector::INSTALLATION);
    }
    if implementation_id.as_str() == super::super::pure_expression::IMPLEMENTATION {
        return Some(&super::super::pure_expression::INSTALLATION);
    }
    if implementation_id.as_str() == super::super::pure_expression::FILTER_IMPLEMENTATION {
        return Some(&super::super::pure_expression::FILTER_INSTALLATION);
    }
    if implementation_id.as_str() == super::super::time_window::IMPLEMENTATION {
        return Some(&super::super::time_window::INSTALLATION);
    }
    if implementation_id.as_str() == super::super::flow_collect::IMPLEMENTATION {
        return Some(&super::super::flow_collect::INSTALLATION);
    }
    if implementation_id.as_str() == super::super::time_sample::IMPLEMENTATION {
        return Some(&super::super::time_sample::INSTALLATION);
    }
    if let Some(resource) = super::super::resource::factory(implementation_id.as_str()) {
        return Some(resource);
    }
    #[cfg(test)]
    if let Some(fixture) = super::super::test_json::factory(implementation_id.as_str()) {
        return Some(fixture);
    }
    #[cfg(test)]
    if implementation_id.as_str() == super::super::test_timing_sink::KIND {
        return Some(&super::super::test_timing_sink::SINK);
    }
    #[cfg(test)]
    if implementation_id.as_str() == super::super::test_replay_sink::KIND {
        return Some(&super::super::test_replay_sink::SINK);
    }
    #[cfg(test)]
    if implementation_id.as_str() == super::super::test_measurement_sink::KIND {
        return Some(&super::super::test_measurement_sink::SINK);
    }
    #[cfg(test)]
    if implementation_id.as_str() == super::super::test_measurement_observation_sink::KIND {
        return Some(&super::super::test_measurement_observation_sink::SINK);
    }
    #[cfg(test)]
    if implementation_id.as_str() == super::super::test_stroke_sink::KIND {
        return Some(&super::super::test_stroke_sink::SINK);
    }
    #[cfg(test)]
    if implementation_id.as_str() == super::super::test_garden_sink::KIND {
        return Some(&super::super::test_garden_sink::SINK);
    }
    #[cfg(test)]
    if implementation_id.as_str() == super::super::test_measurement_decision_sink::KIND {
        return Some(&super::super::test_measurement_decision_sink::SINK);
    }
    if implementation_id.as_str() == super::super::comparison_face::IMPLEMENTATION {
        return Some(&super::super::comparison_face::FACE);
    }
    if implementation_id.as_str() == super::super::normalized_face::IMPLEMENTATION {
        return Some(&super::super::normalized_face::FACE);
    }
    if implementation_id.as_str() == quantity_output::FACE_IMPLEMENTATION {
        return Some(&quantity_output::FACE);
    }
    if implementation_id.as_str() == quantity_output::DIRECT_FACE_IMPLEMENTATION {
        return Some(&quantity_output::DIRECT_FACE);
    }
    INSTALLATIONS
        .iter()
        .copied()
        .find(|factory| factory.implementation_id == implementation_id.as_str())
}
