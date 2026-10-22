//! Check the actual Conduitese through the shared Host and Body checkers.
//! No fixture offers, runtime admission, minting, deployment or hardware effects.
use std::{collections::BTreeMap, error::Error, fs, path::Path};

use conduit_body_make::{
    canonical_body_description_conduit, check_body_description, parse_body_description_conduit,
};
use conduit_host_make::{
    canonical_host_configuration_conduit, parse_host_configuration_conduit, MakeCatalog,
    MakePackageSet,
};

fn main() -> Result<(), Box<dyn Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let packages = MakePackageSet::compose(&[
        &conduit_host_rp2040::Rp2040MakePackage,
        &conduit_host_browser_make::BrowserMakePackage,
    ])
    .map_err(|why| format!("installed make packages: {why:?}"))?;
    let catalog = MakeCatalog::canonical().with_packages(&packages);
    let mut records = Vec::new();
    for file in [
        "environment.body.conduit",
        "environment-browser.body.conduit",
    ] {
        let path = root.join("bodies/environment/profiles").join(file);
        let body = parse_body_description_conduit(&fs::read_to_string(&path)?)
            .map_err(|why| format!("{}: {why:?}", path.display()))?;
        let round_trip = canonical_body_description_conduit(&body)
            .map_err(|why| format!("canonical Body: {why:?}"))?;
        let reparsed =
            parse_body_description_conduit(&round_trip).map_err(|why| format!("{why:?}"))?;
        assert_eq!(
            canonical_body_description_conduit(&reparsed).unwrap(),
            round_trip
        );
        let mut configurations = BTreeMap::new();
        let mut masks = Vec::new();
        for imported in &body.mask_imports {
            let source = fs::read_to_string(root.join(&imported.source).with_extension("conduit"))?;
            let mut startup = conduit_plot::StartupCatalog::new();
            let mut profiles = conduit_plot::ProfileCatalog::new();
            conduit_face::install_mask_plot_value_aliases(&mut startup)?;
            conduit_face::install_mask_mechanism_catalog(&mut startup, &mut profiles)?;
            let syntax = conduit_plot::parse_syntax_document(&source);
            if !syntax.diagnostics.is_empty() {
                return Err(format!("Mask syntax: {:?}", syntax.diagnostics).into());
            }
            let checked = conduit_plot::check_syntax_document(&syntax, &startup)
                .map_err(|why| format!("Mask check: {why:?}"))?;
            let expanded = conduit_plot::expand_canonical_plot_for_authoring(
                &checked,
                &imported.alias,
                &profiles,
            )
            .map_err(|why| format!("Mask expansion: {why:?}"))?;
            let mask = conduit_face::MaskPlot::admit(&expanded)
                .map_err(|why| format!("Mask role: {why:?}"))?;
            masks.push(
                serde_json::json!({"alias": imported.alias, "plot_identity": mask.plot_identity}),
            );
        }
        for host in &body.hosts {
            let source = fs::read_to_string(path.parent().unwrap().join(&host.configuration))?;
            let configuration = parse_host_configuration_conduit(&source)
                .map_err(|why| format!("Host {}: {why:?}", host.name))?;
            let canonical = canonical_host_configuration_conduit(&configuration)
                .map_err(|why| format!("canonical Host: {why:?}"))?;
            let reparsed =
                parse_host_configuration_conduit(&canonical).map_err(|why| format!("{why:?}"))?;
            assert_eq!(
                canonical_host_configuration_conduit(&reparsed).unwrap(),
                canonical
            );
            configurations.insert(host.configuration.clone(), configuration);
        }
        let checked = check_body_description(body, &configurations, &catalog, &packages)
            .map_err(|why| format!("Body {file}: {why:?}"))?;
        records.push(serde_json::json!({
            "source": file,
            "body_id": checked.description().body.id,
            "description_id": checked.description_id(),
            "masks": masks,
            "hosts": checked.hosts().iter().map(|host| serde_json::json!({
                "name": host.description.name,
                "join_mode": host.description.spore.join_mode,
                "configuration_id": host.configuration.configuration_id(),
                "selected_bases": host.configuration.resolved_bases(),
                "resource_needs": host.configuration.configuration().resources,
            })).collect::<Vec<_>>(),
        }));
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "proof_class": "checked-host-and-body-specifications",
            "runtime_admission_claimed": false,
            "specifications": records,
        }))?
    );
    Ok(())
}
