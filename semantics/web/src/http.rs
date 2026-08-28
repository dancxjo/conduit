//! Portable, bounded HTTP protocol-domain semantics.

mod codec;
mod model;
mod schema;
mod state;

pub use codec::{decode_request, decode_response, encode_request, encode_response};
pub use model::*;
pub use schema::{http_request_type, http_response_type};
pub use state::*;

use crate::PortableKindContract;
#[cfg(feature = "form-catalog")]
use alloc::string::ToString;
use alloc::vec;
use conduit_core::{
    kind_id, port_id, CapabilityLimits, KindContractRevision, PortDescriptor, PortDirection,
    PortTemporal,
};

pub const HTTP_CLIENT_KIND: &str = "http/client";
pub const HTTP_SERVER_KIND: &str = "http/server";
pub const HTTP_CLIENT_REVISION: &str = "conduit.http/client@1";
pub const HTTP_SERVER_REVISION: &str = "conduit.http/server@1";

pub fn http_client_semantics() -> PortableKindContract {
    let request = http_request_type()
        .profile()
        .expect("finite HTTP request profile");
    let response = http_response_type()
        .profile()
        .expect("finite HTTP response profile");
    PortableKindContract {
        kind_id: kind_id(HTTP_CLIENT_KIND),
        kind_contract_revision: KindContractRevision::from(HTTP_CLIENT_REVISION),
        inputs: vec![port(
            "request",
            request.value_kind().as_str(),
            PortDirection::Input,
            PortTemporal::Flow { closes: true },
        )],
        outputs: vec![port(
            "response",
            response.value_kind().as_str(),
            PortDirection::Output,
            PortTemporal::Flow { closes: true },
        )],
        limits: CapabilityLimits {
            max_active_instances: HTTP_MAXIMUM_IN_FLIGHT,
            max_queue_items: HTTP_MAXIMUM_IN_FLIGHT,
            max_queue_bytes: HTTP_MAXIMUM_IN_FLIGHT as u32
                * (HTTP_MAXIMUM_ENCODED_REQUEST_BYTES + HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES),
        },
    }
}

pub fn http_server_semantics() -> PortableKindContract {
    let client = http_client_semantics();
    PortableKindContract {
        kind_id: kind_id(HTTP_SERVER_KIND),
        kind_contract_revision: KindContractRevision::from(HTTP_SERVER_REVISION),
        inputs: vec![port(
            "response",
            client.outputs[0].value_kind.as_str(),
            PortDirection::Input,
            PortTemporal::Flow { closes: true },
        )],
        outputs: vec![port(
            "request",
            client.inputs[0].value_kind.as_str(),
            PortDirection::Output,
            PortTemporal::Flow { closes: true },
        )],
        limits: CapabilityLimits {
            max_active_instances: 1,
            ..client.limits
        },
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_http_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    install(
        startup,
        profile,
        [http_client_semantics(), http_server_semantics()],
    )
}

fn port(
    name: &str,
    value: &str,
    direction: PortDirection,
    temporal: PortTemporal,
) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value),
        direction,
        temporal,
    }
}

#[cfg(feature = "form-catalog")]
fn install<const N: usize>(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
    contracts: [PortableKindContract; N],
) -> Result<(), alloc::string::String> {
    use alloc::vec::Vec;
    use conduit_form::{KindDefinition, KindSignature};
    for contract in contracts {
        startup.insert(KindSignature {
            kind: contract.kind_id.as_str().to_string(),
            startup_parameters: Vec::new(),
        })?;
        profile
            .insert(KindDefinition {
                kind_id: contract.kind_id,
                kind_contract_revision: contract.kind_contract_revision,
                inputs: contract.inputs,
                outputs: contract.outputs,
                configuration: Vec::new(),
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}
