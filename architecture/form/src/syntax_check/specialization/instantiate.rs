use super::substitution::{
    expression as substitute_expression, form as substitute_form,
    invocation as substitute_invocation, stages as substitute_stages, value_type as substitute,
};
use crate::prelude::*;
use crate::syntax::{BackStatement, FormSyntax, MatchedRoutePattern};
use alloc::collections::BTreeMap;

pub(super) fn exact(
    template: &FormSyntax,
    identity: &str,
    type_substitutions: &BTreeMap<String, String>,
    behavior_substitutions: &BTreeMap<String, String>,
) -> FormSyntax {
    let mut form = template.clone();
    form.name.text = identity.into();
    form.front.type_parameters.clear();
    form.front.kind_parameters.clear();
    for parameter in &mut form.front.startup_parameters {
        substitute(&mut parameter.value_type, type_substitutions);
        if let Some(default) = &mut parameter.default {
            substitute_expression(default, type_substitutions, behavior_substitutions);
        }
        if parameter.maximum_bytes.is_none() {
            parameter.maximum_bytes =
                crate::surface_parser::front::canonical_default_bound(&parameter.value_type.text);
        }
    }
    for port in &mut form.front.runtime_ports {
        substitute(&mut port.value_type, type_substitutions);
        if port.maximum_bytes.is_none() {
            port.maximum_bytes =
                crate::surface_parser::front::canonical_default_bound(&port.value_type.text);
        }
    }
    for statement in &mut form.back {
        match statement {
            BackStatement::NamedGear(gear) => {
                substitute_invocation(
                    &mut gear.invocation,
                    type_substitutions,
                    behavior_substitutions,
                );
                if let Some(retained) = &mut gear.retained {
                    substitute(&mut retained.value_type, type_substitutions);
                    if retained.maximum_bytes.is_none() {
                        retained.maximum_bytes =
                            crate::surface_parser::front::canonical_default_bound(
                                &retained.value_type.text,
                            );
                    }
                    if let Some(initial) = &mut retained.initial {
                        substitute_expression(initial, type_substitutions, behavior_substitutions);
                    }
                }
            }
            BackStatement::Cord(cord) => {
                substitute_stages(&mut cord.stages, type_substitutions, behavior_substitutions)
            }
            BackStatement::MatchedRoute(route) => {
                for arm in &mut route.arms {
                    if let MatchedRoutePattern::Variant { value_type, .. }
                    | MatchedRoutePattern::Guard { value_type, .. } = &mut arm.pattern
                    {
                        substitute(value_type, type_substitutions);
                    }
                    if let MatchedRoutePattern::Guard { expected, .. } = &mut arm.pattern {
                        substitute_expression(expected, type_substitutions, behavior_substitutions);
                    }
                    substitute_stages(&mut arm.stages, type_substitutions, behavior_substitutions);
                }
            }
            BackStatement::Pool(pool) => {
                if let Some(selected) = behavior_substitutions.get(&pool.member_form.text) {
                    pool.member_form.text.clone_from(selected);
                }
            }
            BackStatement::LocalValue(value) => {
                substitute_expression(&mut value.value, type_substitutions, behavior_substitutions)
            }
        }
    }
    for local in &mut form.local_forms {
        substitute_form(local, type_substitutions, behavior_substitutions);
    }
    form
}
