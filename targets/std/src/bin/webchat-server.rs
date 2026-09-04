use conduit_core::{BootId, HostId, OfferGeneration};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use conduit_std_host::{StdHost, StdHostComposition, StdHostConfig, ThreadTimer};

const SOURCE: &str = include_str!("../../../../forms/webchat/main.conduit");

fn main() -> Result<(), String> {
    let bind = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:0".to_string());
    let source = SOURCE.replace("127.0.0.1:4178", &bind);
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_net::install_external_websocket_catalogs(&mut startup, &mut profile)?;
    conduit_chat::install_browser_chat_catalogs(&mut startup, &mut profile)?;
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup)
        .map_err(|error| format!("canonical webchat check: {error:?}"))?;
    let expanded = expand_canonical_form(&checked, "webchat-server-demo", &profile)
        .map_err(|error| format!("canonical webchat expansion: {error:?}"))?;
    let mut host = StdHost::new_with_composition(
        StdHostConfig {
            host_id: HostId::from("std-webchat"),
            boot_id: BootId::from("std-webchat-boot"),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::minimal().with_external_websocket(),
    );
    let plan = host
        .plan_expanded_local(&expanded)
        .map_err(|error| error.to_string())?;
    let mut output = std::io::stdout().lock();
    host.run_fragment_to(plan.fragments[0].clone(), &mut output, &mut ThreadTimer)?;
    Ok(())
}
