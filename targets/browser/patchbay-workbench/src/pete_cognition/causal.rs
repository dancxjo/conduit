//! Structured actual causal evidence, distinct from lossy debugger playback.
use super::*;
mod helpers;
use conduit_face::{FaceRelationship, FaceRelationshipKind};
use helpers::*;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn append(
    directory: &Path,
    plan: &BodyPlan,
    run: &Run,
    body_subject: &str,
    graphs: &[PatchbayGraph],
    content: &mut graph_face::Content<'_>,
) -> Result<()> {
    let graph_face::Content {
        subjects,
        relationships,
        properties,
        texts,
        ..
    } = content;
    let dag: Value = read(directory, "causal-final-response.json")?;
    let summary = dag
        .get("summary")
        .ok_or("causal document has no explicit summary")?;
    let plan_subject = format!("historical-plan/{}", plan.plan_id.as_str());
    let play_subject = format!("historical-play/{}", run.play.active_play_id.as_str());
    subject(
        subjects,
        &plan_subject,
        FaceRole::Plan,
        "Recorded Body Plan",
    )?;
    subject(
        subjects,
        &play_subject,
        FaceRole::Play,
        "Recorded Body Play (historical)",
    )?;
    prop(
        properties,
        &plan_subject,
        "plan-id",
        FacePropertyValue::Identity(plan.plan_id.as_str().into()),
    );
    prop(
        properties,
        &play_subject,
        "active-play-id",
        FacePropertyValue::Identity(run.play.active_play_id.as_str().into()),
    );
    prop(
        properties,
        &play_subject,
        "play-state",
        FacePropertyValue::Text(format!("Historical {}", run.terminal)),
    );
    relationships.push(FaceRelationship {
        source: play_subject.clone(),
        target: plan_subject.clone(),
        kind: FaceRelationshipKind::Describes,
    });
    for (partition, graph) in plan.plots.iter().zip(graphs) {
        for fragment in &partition.plan.fragments {
            let host = format!("historical-host/{}", fragment.host_id.as_str());
            subject(subjects, &host, FaceRole::Host, "Recorded execution Host")?;
            prop_once(
                properties,
                &host,
                "host-id",
                FacePropertyValue::Identity(fragment.host_id.as_str().into()),
            );
            prop_once(
                properties,
                &host,
                "boot-id",
                FacePropertyValue::Identity(fragment.boot_id.as_str().into()),
            );
            for placement in &fragment.placements {
                let gear = graph
                    .gears
                    .iter()
                    .find(|gear| gear.gear_id == placement.gear_id)
                    .ok_or("historical selected Gear missing")?;
                let id = format!("gear/{}", gear.identity);
                for (name, value) in [
                    ("placement-id", placement.placement_id.as_str()),
                    ("implementation-id", placement.implementation_id.as_str()),
                    ("host-id", placement.host_id.as_str()),
                    ("boot-id", placement.boot_id.as_str()),
                    ("artifact-id", placement.artifact_id.as_str()),
                    (
                        "execution-profile-id",
                        placement.execution_profile_id.as_str(),
                    ),
                ] {
                    prop_once(
                        properties,
                        &id,
                        name,
                        FacePropertyValue::Identity(value.into()),
                    );
                }
                relationships.push(FaceRelationship {
                    source: id,
                    target: host.clone(),
                    kind: FaceRelationshipKind::Describes,
                });
            }
        }
    }
    flags(properties, body_subject, summary)?;
    if let Some(refusal) = dag.get("refusal") {
        prop(
            properties,
            body_subject,
            "sign-causal-refusal",
            FacePropertyValue::Text(refusal.as_str().ok_or("invalid causal refusal")?.into()),
        );
        prop(
            properties,
            body_subject,
            "dag-rendered-evidence-verified",
            FacePropertyValue::Flag(false),
        );
        return Ok(());
    }
    if dag.get("schema").and_then(Value::as_str) != Some("conduit.pete/final-response-causal-dag@1")
    {
        return Err("unsupported actual causal DAG schema".into());
    }
    for (key, actual) in [
        ("body", plan.body_id.as_str()),
        ("wake", plan.wake_id.as_str()),
        ("plan", plan.plan_id.as_str()),
        ("play", run.play.active_play_id.as_str()),
    ] {
        if dag.get(key).and_then(Value::as_str) != Some(actual) {
            return Err(format!("causal DAG {key} differs from actual Body execution").into());
        }
    }
    let host = dag
        .get("host")
        .and_then(Value::as_str)
        .ok_or("causal Host missing")?;
    let boot = dag
        .get("boot")
        .and_then(Value::as_str)
        .ok_or("causal Boot missing")?;
    if plan
        .plots
        .iter()
        .flat_map(|p| &p.plan.fragments)
        .any(|f| f.host_id.as_str() != host || f.boot_id.as_str() != boot)
    {
        return Err("causal Host/Boot differs from exact sealed Plans".into());
    }
    let nodes = dag
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or("causal nodes missing")?;
    let edges = dag
        .get("edges")
        .and_then(Value::as_array)
        .ok_or("causal edges missing")?;
    if nodes.len() > 257
        || edges.len() > 128
        || summary.get("nodes").and_then(Value::as_u64) != Some(nodes.len() as u64)
        || summary.get("edges").and_then(Value::as_u64) != Some(edges.len() as u64)
    {
        return Err("causal DAG count/bound mismatch".into());
    }
    let root = dag
        .get("root_evidence")
        .ok_or("causal root evidence missing")?;
    let root_key = evidence(root)?;
    let lowered = helpers::lowered(plan)?;
    let mut indexed = BTreeMap::new();
    let mut evidence_seen = BTreeSet::new();
    let mut sequences = BTreeSet::new();
    let mut gear_sign_counts = BTreeMap::<String, usize>::new();
    let mut roots = 0;
    for node in nodes {
        let ordinal = node
            .get("ordinal")
            .and_then(Value::as_u64)
            .ok_or("causal ordinal missing")?;
        let raw = node.get("evidence").ok_or("causal evidence missing")?;
        let key = evidence(raw)?;
        if indexed.contains_key(&ordinal) || !evidence_seen.insert(key) {
            return Err("duplicate causal ordinal/evidence".into());
        }
        let id = format!("causal-sign/{}/{}/{}", key.0, key.1, key.2);
        let metadata = node
            .get("metadata")
            .and_then(Value::as_array)
            .ok_or("causal metadata not visible")?;
        if metadata.len() > 32 {
            return Err("causal node metadata exceeds finite bound".into());
        }
        let fact = |name: &str| -> Result<&Value> {
            let values = metadata
                .iter()
                .filter(|fact| fact.get("fact").and_then(Value::as_str) == Some(name))
                .collect::<Vec<_>>();
            if values.len() != 1 {
                return Err(format!("causal node has ambiguous {name}").into());
            }
            Ok(values[0])
        };
        let value = |name: &str| -> Result<&str> {
            fact(name)?
                .get("value")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("causal {name} value missing").into())
        };
        for (name, actual) in [
            ("wake", plan.wake_id.as_str()),
            ("plan", plan.plan_id.as_str()),
            ("play", run.play.active_play_id.as_str()),
            ("host", host),
            ("boot", boot),
        ] {
            if value(name)? != actual {
                return Err(format!("node {ordinal} mismatches actual {name}").into());
            }
        }
        let placement_id = value("placement")?;
        let mut found = plan
            .plots
            .iter()
            .enumerate()
            .flat_map(|(index, p)| {
                p.plan
                    .fragments
                    .iter()
                    .flat_map(move |f| f.placements.iter().map(move |gear| (index, gear)))
            })
            .filter(|(_, gear)| gear.placement_id.as_str() == placement_id);
        let (index, placement) = found
            .next()
            .ok_or("causal placement absent from sealed Plan")?;
        if found.next().is_some() {
            return Err("causal placement aliases multiple sealed Gears".into());
        }
        let semantic = fact("semantic_subject")?;
        if semantic.get("gear").and_then(Value::as_str) != Some(placement.gear_id.as_str())
            || semantic.get("kind").and_then(Value::as_str) != Some(placement.kind_id.as_str())
            || value("implementation")? != placement.implementation_id.as_str()
        {
            return Err(
                "causal semantic/implementation metadata differs from sealed placement".into(),
            );
        }
        let event = node
            .get("kernel_event")
            .ok_or("causal actual kernel event missing")?;
        let sequence = event
            .get("sequence")
            .and_then(Value::as_u64)
            .ok_or("causal kernel sequence missing")?;
        if fact("local_order")?.get("sequence").and_then(Value::as_u64) != Some(sequence)
            || !sequences.insert(sequence)
        {
            return Err("causal LocalOrder/kernel sequence mismatch".into());
        }
        let actual = run
            .kernel_events
            .iter()
            .find(|candidate| candidate.get("sequence").and_then(Value::as_u64) == Some(sequence))
            .ok_or("causal kernel sequence absent from actual Body run")?;
        if actual.get("kind") != event.get("kind") {
            return Err("causal kernel event kind differs from actual run".into());
        }
        let node_id = actual
            .get("node")
            .and_then(Value::as_u64)
            .and_then(|n| u16::try_from(n).ok())
            .map(conduit_kernel::NodeId)
            .ok_or("actual event node identity missing")?;
        let map = lowered
            .partitions
            .iter()
            .map(|p| &p.identity)
            .find(|map| map.placement_for_node(node_id).is_some())
            .ok_or("actual event node absent from sealed Plan lowering")?;
        if map.placement_for_node(node_id).map(|id| id.as_str()) != Some(placement_id) {
            return Err("causal placement does not match actual kernel event node".into());
        }
        let direction = match event.get("kind").and_then(Value::as_str) {
            Some("ValueRouted") => conduit_core::PortDirection::Output,
            Some("ValueConsumed") => conduit_core::PortDirection::Input,
            _ => return Err("unsupported causal source event".into()),
        };
        let port = actual
            .get("port")
            .and_then(Value::as_u64)
            .and_then(|n| u16::try_from(n).ok())
            .map(conduit_kernel::PortId)
            .ok_or("actual causal event port missing")?;
        if map
            .port_identity(node_id, direction, port)
            .map(|p| p.port_id.as_str())
            != event.get("port").and_then(Value::as_str)
        {
            return Err("causal port differs from exact directional event binding".into());
        }

        let root_flag = node
            .get("terminal")
            .and_then(Value::as_bool)
            .ok_or("causal query-root flag missing")?;
        if root_flag {
            roots += 1;
            if key != root_key
                || summary.get("root_sequence").and_then(Value::as_u64) != Some(sequence)
            {
                return Err("causal query root differs from declared final response".into());
            }
        }
        subject(
            subjects,
            &id,
            FaceRole::Sign,
            &format!("Causal Sign {ordinal} · kernel {sequence}"),
        )?;
        let graph_gear = graphs[index]
            .gears
            .iter()
            .find(|g| g.gear_id == placement.gear_id)
            .ok_or("causal graph Gear missing")?;
        let gear_id = format!("gear/{}", graph_gear.identity);
        relationships.push(FaceRelationship {
            source: id.clone(),
            target: gear_id.clone(),
            kind: FaceRelationshipKind::Describes,
        });
        prop(
            properties,
            &id,
            "kernel-sequence",
            FacePropertyValue::Count(sequence),
        );
        prop(
            properties,
            &id,
            "causal-ordinal",
            FacePropertyValue::Count(ordinal),
        );
        prop(
            properties,
            &id,
            "causal-root",
            FacePropertyValue::Flag(root_flag),
        );
        prop(
            properties,
            &id,
            "exact-sign-evidence",
            FacePropertyValue::Text(serde_json::to_string(raw)?),
        );
        prop(
            properties,
            &id,
            "sign-evidence-class",
            FacePropertyValue::Text(
                node.get("outcome")
                    .and_then(Value::as_str)
                    .ok_or("causal outcome missing")?
                    .into(),
            ),
        );
        prop(
            properties,
            &id,
            "sign-kernel-event",
            FacePropertyValue::Text(serde_json::to_string(event)?),
        );
        for (index, metadata) in metadata.iter().enumerate() {
            prop(
                properties,
                &id,
                &format!("sign-metadata-{index}"),
                FacePropertyValue::Text(serde_json::to_string(metadata)?),
            );
        }
        let count = gear_sign_counts.entry(gear_id.clone()).or_default();
        prop(
            properties,
            &gear_id,
            &format!("sign-{count}"),
            FacePropertyValue::Identity(id.clone()),
        );
        *count += 1;
        if root_flag {
            flags(properties, &id, summary)?;
            prop(
                properties,
                body_subject,
                "dag-final-sign",
                FacePropertyValue::Identity(id.clone()),
            );
        }
        indexed.insert(ordinal, (id, key, sequence));
    }
    if roots != 1 {
        return Err("causal DAG must have exactly one actual query root".into());
    }
    let mut unique = BTreeSet::new();
    for (index, edge) in edges.iter().enumerate() {
        let effect = edge
            .get("effect")
            .and_then(Value::as_u64)
            .ok_or("edge effect missing")?;
        let cause = edge
            .get("cause")
            .and_then(Value::as_u64)
            .ok_or("edge cause missing")?;
        let relation = edge
            .get("relationship")
            .and_then(Value::as_str)
            .ok_or("edge relationship missing")?;
        if ![
            "DerivedFrom",
            "CausedBy",
            "RequestedBy",
            "AdmittedBy",
            "RefusedBy",
            "RealizedBy",
            "ObservedFrom",
            "TerminatedBecause",
            "Supersedes",
            "Corrects",
        ]
        .contains(&relation)
            || !unique.insert((effect, cause, relation))
        {
            return Err("unknown/duplicate exact causal relationship".into());
        }
        let effect = indexed.get(&effect).ok_or("edge effect ordinal absent")?;
        let cause = indexed.get(&cause).ok_or("edge cause ordinal absent")?;
        if cause.2 >= effect.2 {
            return Err("causal edge is not backward in actual kernel sequence".into());
        }
        // Describes links annotate Gears only. These exact edge properties retain
        // actual causal meaning without relabeling annotation as causation.
        prop(
            properties,
            &effect.0,
            &format!("causal-edge-{index}-cause"),
            FacePropertyValue::Identity(cause.0.clone()),
        );
        prop(
            properties,
            &effect.0,
            &format!("causal-edge-{index}-relationship"),
            FacePropertyValue::Text(relation.into()),
        );
        prop(
            properties,
            &effect.0,
            &format!("causal-edge-{index}-cause-evidence"),
            FacePropertyValue::Text(serde_json::to_string(
                &serde_json::json!({"host_session":cause.1.0,"execution":cause.1.1,"sign":cause.1.2}),
            )?),
        );
        prop(
            properties,
            &effect.0,
            &format!("sign-cause-{index}"),
            FacePropertyValue::Text(format!("{relation} {}", cause.0)),
        );
    }
    texts.push(FaceText{subject:body_subject.into(),text:format!("Actual retained final-response causal DAG: {} Signs, {} exact edges. Query-root is an inspection root, not an abnormal semantic terminal. Completeness and missing branches remain explicit; no full cognition completion claim.",nodes.len(),edges.len())});
    prop(
        properties,
        body_subject,
        "dag-rendered-evidence-verified",
        FacePropertyValue::Flag(false),
    );
    Ok(())
}
