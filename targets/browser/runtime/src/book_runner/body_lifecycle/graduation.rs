use super::{
    protocol::{GraduationReadiness, GraduationReceipt},
    session,
};
use conduit_core::{bind_sign, BootId, HostId};
use conduit_planner::{
    default_expanded_placements, plan_expanded_canonical_with_options, PlanningOptions,
};
use std::collections::BTreeMap;

const PATCHBAY_SOURCE: &str = "form creche_graduation {\n    subject: text/literal(\"Body history\")\n    surface: presentation/patchbay\n    subject > surface.subject\n}\n";

pub(super) fn readiness() -> Result<GraduationReadiness, String> {
    let receipt =
        session::current().ok_or_else(|| "BIRTH is required before graduation".to_string())?;
    let durable_identity = !receipt.body_id.is_empty() && receipt.raw_body.validate().is_ok();
    let birth_evidence = !receipt.birth_sign_id.is_empty() && receipt.birth_sequence != 0;
    let current_admitted_part = receipt.here_part_id.is_some()
        && receipt.host_id.is_some()
        && receipt.boot_id.is_some()
        && !receipt.raw_membership.parts.is_empty();
    Ok(GraduationReadiness {
        schema: "conduit.creche/graduation-readiness@1",
        body_id: receipt.body_id,
        durable_identity,
        birth_evidence,
        current_admitted_part,
        intended_program: receipt.initial_program.clone(),
        ready: durable_identity
            && birth_evidence
            && current_admitted_part
            && receipt.initial_program == "morse-network@1",
    })
}

pub(super) fn graduate(
    choice: u32,
    sequence: u64,
) -> Result<super::protocol::BirthReceipt, String> {
    if sequence == 0 {
        return Err("graduation sequence must be nonzero".into());
    }
    let ready = readiness()?;
    if !ready.ready {
        return Err("the Body is not ready to graduate from the Crèche".into());
    }
    session::with_session(|body| {
        if body.receipt.graduation.is_some() {
            return Err("this Body has already graduated from the Crèche".into());
        }
        let host = body
            .receipt
            .host_id
            .clone()
            .ok_or_else(|| "graduation has no current Host".to_string())?;
        let boot = body
            .receipt
            .boot_id
            .clone()
            .ok_or_else(|| "graduation has no current Boot".to_string())?;
        let (choice_name, plan_id, implementation_id) = match choice {
            1 => {
                let plan = patchbay_plan(&host, &boot)?;
                let implementation = plan
                    .fragments
                    .iter()
                    .flat_map(|fragment| fragment.placements.iter())
                    .find(|placement| placement.kind_id.as_str() == "presentation/patchbay")
                    .ok_or_else(|| "planned Patchbay placement is absent".to_string())?
                    .implementation_id
                    .as_str()
                    .to_string();
                (
                    "host-patchbay",
                    Some(plan.plan_id.as_str().to_string()),
                    Some(implementation),
                )
            }
            2 => ("external-reader", None, None),
            _ => return Err("graduation choice must be hosted Patchbay or external reader".into()),
        };
        let sign = bind_sign(
            &HostId::from(host.as_str()),
            &BootId::from(boot.as_str()),
            None,
            sequence,
        );
        body.receipt.graduation = Some(GraduationReceipt {
            schema: "conduit.creche/graduation@1",
            body_id: body.receipt.body_id.clone(),
            sequence,
            sign_id: sign.sign_id.as_str().into(),
            choice: choice_name,
            patchbay_plan_id: plan_id,
            patchbay_implementation_id: implementation_id,
            creche_required: false,
        });
        Ok(body.receipt.clone())
    })
}

fn patchbay_plan(host: &str, boot: &str) -> Result<conduit_core::Plan, String> {
    let (startup, catalog) = crate::installed_browser::catalogs()?;
    let syntax = conduit_form::parse_syntax_document(PATCHBAY_SOURCE);
    if let Some(diagnostic) = syntax.diagnostics.first() {
        return Err(format!("parse Patchbay Form: {}", diagnostic.message));
    }
    let checked = conduit_form::check_syntax_document(&syntax, &startup)
        .map_err(|error| format!("check Patchbay Form: {error:?}"))?;
    let expanded = conduit_form::expand_canonical_form(&checked, "creche_graduation", &catalog)
        .map_err(|error| format!("expand Patchbay Form: {error:?}"))?;
    let hosts = [crate::installed_browser::advertisement(
        HostId::from(host),
        BootId::from(boot),
    )];
    let placements = default_expanded_placements(&expanded, &hosts)
        .map_err(|error| format!("place Patchbay Form: {error:?}"))?;
    plan_expanded_canonical_with_options(
        &expanded,
        &hosts,
        &placements,
        &crate::installed_browser::local_bases(),
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: crate::installed_browser::MAXIMUM_BROWSER_VALUE_BYTES as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .map_err(|error| format!("plan Patchbay Form: {error:?}"))
}
