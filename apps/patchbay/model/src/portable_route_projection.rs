//! Exact route-recovery subjects for portable Patchbay presentation.

use conduit_presentation::{PresentationPropertyValue, PresentationRole};

use crate::{portable_projection::ContentBuilder, PatchbayPresentation};

pub(super) fn append_routes(
    presentation: &PatchbayPresentation,
    document: &str,
    content: &mut ContentBuilder,
) {
    for route in &presentation.routes {
        let subject = content.subject(
            PresentationRole::Route,
            route.same_plan.plan.connection_id.as_str(),
            format!(
                "Route {} under Plan {}",
                route.same_plan.plan.connection_id.as_str(),
                route.same_plan.plan.plan_id.as_str()
            ),
        );
        content.describes(&subject, document);
        for line in route.linear_lines() {
            content.line(&subject, line);
        }
        append_route_identities(&subject, route, content);
        append_line_candidates(
            &subject,
            "prior",
            &route.new_plan.prior,
            Some(&route.new_plan.unavailable_binding_id),
            None,
            content,
        );
        append_line_candidates(
            &subject,
            "same-plan",
            &route.same_plan.plan,
            Some(&route.same_plan.unavailable_binding_id),
            Some(&route.same_plan.selected_binding_id),
            content,
        );
    }
}

fn append_route_identities(
    subject: &str,
    route: &crate::DistributedRoutePresentation,
    content: &mut ContentBuilder,
) {
    for (name, value) in [
        ("new-plan-prior-id", route.new_plan.prior.plan_id.as_str()),
        (
            "new-plan-replacement-id",
            route.new_plan.replacement_plan_id.as_str(),
        ),
        (
            "new-plan-unavailable-binding-id",
            route.new_plan.unavailable_binding_id.as_str(),
        ),
        ("same-plan-id", route.same_plan.plan.plan_id.as_str()),
        (
            "same-plan-unavailable-binding-id",
            route.same_plan.unavailable_binding_id.as_str(),
        ),
        (
            "same-plan-selected-binding-id",
            route.same_plan.selected_binding_id.as_str(),
        ),
        ("refused-binding-id", route.refused.binding_id.as_str()),
        (
            "sign-new-plan-unavailable",
            route.new_plan.unavailable_sign_id.as_str(),
        ),
        (
            "sign-new-plan-unsatisfied",
            route.new_plan.unsatisfied_sign_id.as_str(),
        ),
        (
            "sign-new-plan-requested",
            route.new_plan.planning_request_sign_id.as_str(),
        ),
        (
            "sign-new-plan-planned",
            route.new_plan.planning_success_sign_id.as_str(),
        ),
        (
            "sign-new-plan-installed",
            route.new_plan.installed_sign_id.as_str(),
        ),
        (
            "sign-same-plan-unavailable",
            route.same_plan.unavailable_sign_id.as_str(),
        ),
        (
            "sign-same-plan-selected",
            route.same_plan.selection_sign_id.as_str(),
        ),
        (
            "sign-refused-observation",
            route.refused.observation_sign_id.as_str(),
        ),
    ] {
        content.property(
            subject,
            name,
            PresentationPropertyValue::Identity(value.into()),
        );
    }
}

fn append_line_candidates(
    route: &str,
    phase: &str,
    plan: &crate::RoutePlanPresentation,
    unavailable: Option<&conduit_core::LinkBindingId>,
    selected: Option<&conduit_core::LinkBindingId>,
    content: &mut ContentBuilder,
) {
    for candidate in &plan.candidates {
        let subject = content.subject(
            PresentationRole::Cord,
            candidate.binding_id.as_str(),
            format!(
                "Route candidate {} in Plan {}",
                candidate.binding_id.as_str(),
                plan.plan_id.as_str()
            ),
        );
        content.contains(route, &subject);
        for (name, value) in [
            ("plan-id", plan.plan_id.as_str()),
            ("connection-id", plan.connection_id.as_str()),
            ("binding-id", candidate.binding_id.as_str()),
            ("base-instance-id", candidate.base_instance_id.as_str()),
        ] {
            content.property(
                &subject,
                name,
                PresentationPropertyValue::Identity(value.into()),
            );
        }
        content.property(
            &subject,
            "phase",
            PresentationPropertyValue::Text(phase.into()),
        );
        content.property(
            &subject,
            "base",
            PresentationPropertyValue::ConnectionBase(candidate.base),
        );
        content.property(
            &subject,
            "order",
            PresentationPropertyValue::Count(candidate.order as u64),
        );
        let status = if unavailable == Some(&candidate.binding_id) {
            "unavailable"
        } else if selected == Some(&candidate.binding_id) {
            "selected"
        } else {
            "admitted"
        };
        content.property(
            &subject,
            "route-status",
            PresentationPropertyValue::Text(status.into()),
        );
    }
}
