use super::{
    HTTP_MAXIMUM_ENCODED_REQUEST_BYTES, HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES, HTTP_MAXIMUM_IN_FLIGHT,
};
use crate::{StandardKindContract, TerminalBehavior};
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    kind_id, port_id, CapabilityLimits, PortDescriptor, PortDirection, PortTemporal,
};

pub const HTTP_CLIENT_KIND: &str = "http/client";
pub const HTTP_SERVER_KIND: &str = "http/server";
pub const HTTP_CLIENT_REVISION: &str = "conduit.http/client@1";
pub const HTTP_SERVER_REVISION: &str = "conduit.http/server@1";

pub fn http_contracts() -> Vec<StandardKindContract> {
    vec![http_client_contract(), http_server_contract()]
}

pub fn http_client_contract() -> StandardKindContract {
    let request_info = super::http_request_type()
        .profile()
        .expect("finite HTTP request profile");
    let response_info = super::http_response_type()
        .profile()
        .expect("finite HTTP response profile");
    StandardKindContract {
        kind_id: kind_id(HTTP_CLIENT_KIND),
        plain_name: "HTTP client".to_string(),
        summary: "Perform finite, explicitly authorized HTTP exchanges without implicit redirects, retries, cookies, caching, credentials, or decompression.".to_string(),
        inputs: vec![port("request", request_info.value_kind().as_str(), PortDirection::Input)],
        outputs: vec![port("response", response_info.value_kind().as_str(), PortDirection::Output)],
        configuration: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: HTTP_MAXIMUM_IN_FLIGHT,
            max_queue_items: HTTP_MAXIMUM_IN_FLIGHT,
            max_queue_bytes: HTTP_MAXIMUM_IN_FLIGHT as u32 * (HTTP_MAXIMUM_ENCODED_REQUEST_BYTES + HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES),
        },
        terminal_behavior: TerminalBehavior::MirrorsInputTerminal,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "client: http/client".to_string(),
    }
}

pub fn http_server_contract() -> StandardKindContract {
    let request_info = super::http_request_type()
        .profile()
        .expect("finite HTTP request profile");
    let response_info = super::http_response_type()
        .profile()
        .expect("finite HTTP response profile");
    StandardKindContract {
        kind_id: kind_id(HTTP_SERVER_KIND),
        plain_name: "HTTP server".to_string(),
        summary: "Expose finite inbound HTTP requests and accept exactly correlated responses; routing and authentication remain surrounding semantic work.".to_string(),
        inputs: vec![port("response", response_info.value_kind().as_str(), PortDirection::Input)],
        outputs: vec![port("request", request_info.value_kind().as_str(), PortDirection::Output)],
        configuration: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: HTTP_MAXIMUM_IN_FLIGHT,
            max_queue_bytes: HTTP_MAXIMUM_IN_FLIGHT as u32 * (HTTP_MAXIMUM_ENCODED_REQUEST_BYTES + HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES),
        },
        terminal_behavior: TerminalBehavior::HostInputEndsOrFailsSource,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "server: http/server".to_string(),
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_http_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_form::{KindDefinition, KindSignature};
    for (contract, revision) in [
        (http_client_contract(), HTTP_CLIENT_REVISION),
        (http_server_contract(), HTTP_SERVER_REVISION),
    ] {
        startup.insert(KindSignature {
            kind: contract.kind_id.as_str().to_string(),
            startup_parameters: Vec::new(),
        })?;
        profile
            .insert(KindDefinition {
                kind_id: contract.kind_id,
                kind_contract_revision: conduit_core::KindContractRevision::from(revision),
                inputs: contract.inputs,
                outputs: contract.outputs,
                configuration: Vec::new(),
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn port(name: &str, info: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(info),
        direction,
        temporal: PortTemporal::Flow { closes: true },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        HTTP_AMBIENT_COOKIES, HTTP_AMBIENT_CREDENTIALS, HTTP_AUTOMATIC_REDIRECTS,
        HTTP_AUTOMATIC_RETRIES, HTTP_IMPLICIT_CACHING, HTTP_IMPLICIT_DECOMPRESSION,
    };

    #[test]
    fn client_and_server_are_exact_mirrored_semantic_faces() {
        let client = http_client_contract();
        let server = http_server_contract();
        assert_eq!(
            client.inputs[0].value_kind,
            super::super::http_request_type()
                .profile()
                .unwrap()
                .value_kind()
                .clone()
        );
        assert_eq!(
            client.outputs[0].value_kind,
            super::super::http_response_type()
                .profile()
                .unwrap()
                .value_kind()
                .clone()
        );
        assert_eq!(server.outputs[0].value_kind, client.inputs[0].value_kind);
        assert_eq!(server.inputs[0].value_kind, client.outputs[0].value_kind);
        assert_eq!(client.limits.max_active_instances, HTTP_MAXIMUM_IN_FLIGHT);
        assert!(client.configuration.is_empty());
        assert!(server.configuration.is_empty());
        const {
            assert!(!HTTP_AUTOMATIC_REDIRECTS);
            assert!(!HTTP_AUTOMATIC_RETRIES);
            assert!(!HTTP_AMBIENT_COOKIES);
            assert!(!HTTP_AMBIENT_CREDENTIALS);
            assert!(!HTTP_IMPLICIT_CACHING);
            assert!(!HTTP_IMPLICIT_DECOMPRESSION);
        }
    }

    #[cfg(feature = "form-catalog")]
    #[test]
    fn ordinary_form_check_keeps_http_distinct_from_realization() {
        let mut startup = conduit_form::StartupCatalog::new();
        let mut profile = conduit_form::ProfileCatalog::new();
        install_http_catalogs(&mut startup, &mut profile).unwrap();
        let checked = conduit_form::parse(
            "form 0\n\npair {\n client: http/client\n server: http/server\n server.request -> client.request\n client.response -> server.response\n}\n",
            &profile,
        )
        .unwrap();
        assert_eq!(checked.gears.len(), 2);
        assert_eq!(checked.connections.len(), 2);
    }
}
