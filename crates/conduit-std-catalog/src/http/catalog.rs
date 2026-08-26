//! Std-catalog descriptions projected from host-neutral HTTP contracts.

use crate::{StandardKindContract, TerminalBehavior};
use alloc::{string::ToString, vec, vec::Vec};

pub fn http_contracts() -> Vec<StandardKindContract> {
    vec![http_client_contract(), http_server_contract()]
}

pub fn http_client_contract() -> StandardKindContract {
    describe(
        conduit_web::http_client_semantics(),
        "HTTP client",
        "Perform finite, explicitly authorized HTTP exchanges without implicit redirects, retries, cookies, caching, credentials, or decompression.",
        TerminalBehavior::MirrorsInputTerminal,
        "client: http/client",
    )
}

pub fn http_server_contract() -> StandardKindContract {
    describe(
        conduit_web::http_server_semantics(),
        "HTTP server",
        "Expose finite inbound HTTP requests and accept exactly correlated responses; routing and authentication remain surrounding semantic work.",
        TerminalBehavior::HostInputEndsOrFailsSource,
        "server: http/server",
    )
}

fn describe(
    contract: conduit_web::PortableKindContract,
    plain_name: &str,
    summary: &str,
    terminal_behavior: TerminalBehavior,
    example: &str,
) -> StandardKindContract {
    StandardKindContract {
        kind_id: contract.kind_id,
        plain_name: plain_name.to_string(),
        summary: summary.to_string(),
        inputs: contract.inputs,
        outputs: contract.outputs,
        configuration: Vec::new(),
        limits: contract.limits,
        terminal_behavior,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: example.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_web::{
        HTTP_AMBIENT_COOKIES, HTTP_AMBIENT_CREDENTIALS, HTTP_AUTOMATIC_REDIRECTS,
        HTTP_AUTOMATIC_RETRIES, HTTP_IMPLICIT_CACHING, HTTP_IMPLICIT_DECOMPRESSION,
        HTTP_MAXIMUM_IN_FLIGHT,
    };

    #[test]
    fn catalog_description_preserves_the_exact_portable_faces() {
        let client = http_client_contract();
        let server = http_server_contract();
        let portable_client = conduit_web::http_client_semantics();
        let portable_server = conduit_web::http_server_semantics();
        assert_eq!(client.kind_id, portable_client.kind_id);
        assert_eq!(client.inputs, portable_client.inputs);
        assert_eq!(client.outputs, portable_client.outputs);
        assert_eq!(client.limits, portable_client.limits);
        assert_eq!(server.inputs, portable_server.inputs);
        assert_eq!(server.outputs, portable_server.outputs);
        assert_eq!(client.limits.max_active_instances, HTTP_MAXIMUM_IN_FLIGHT);
        const {
            assert!(!HTTP_AUTOMATIC_REDIRECTS);
            assert!(!HTTP_AUTOMATIC_RETRIES);
            assert!(!HTTP_AMBIENT_COOKIES);
            assert!(!HTTP_AMBIENT_CREDENTIALS);
            assert!(!HTTP_IMPLICIT_CACHING);
            assert!(!HTTP_IMPLICIT_DECOMPRESSION);
        }
    }
}
