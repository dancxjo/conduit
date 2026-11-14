//! Checked construction intent for a hosted TLS listener, never a Ready Line.
use conduit_host_make::{
    BaseInitializationContract, CheckedHostConfiguration, ConfigurationValue, ImplementationOffer,
    KindConfigurationField, KindConfigurationRule,
};
use std::{net::SocketAddr, path::PathBuf};

pub const TLS_LISTENER_BASE_KIND: &str = "network/tls-listener";
pub const TLS_LISTENER_IMPLEMENTATION: &str = "hosted/tls-websocket-listener@1";

pub fn tls_listener_initialization_contract() -> BaseInitializationContract {
    let text = |key: &str, maximum| KindConfigurationField {
        key: key.into(),
        default_value: ConfigurationValue::Text(String::new()),
        rule: KindConfigurationRule::DataLabel { maximum },
    };
    let count = |key: &str, maximum| KindConfigurationField {
        key: key.into(),
        default_value: ConfigurationValue::U64(1),
        rule: KindConfigurationRule::U64Range {
            minimum: 1,
            maximum,
        },
    };
    BaseInitializationContract {
        fields: vec![
            text("bind_address", 256),
            text("certificate_pem", 4096),
            text("private_key_pem", 4096),
            text("certificate_binding_sha256", 64),
            count("maximum_message_bytes", 128 * 1024),
            count("handshake_timeout_millis", 10_000),
            count("accept_poll_millis", 1000),
            // The initial provider owns exactly one accepted socket at a time.
            count("maximum_concurrent_sessions", 1),
        ],
    }
}

pub(super) fn offer() -> ImplementationOffer {
    ImplementationOffer {
        base_kind: TLS_LISTENER_BASE_KIND.into(),
        implementation_id: TLS_LISTENER_IMPLEMENTATION.into(),
        implementation_revision: 1,
        target_patterns: vec![
            "std/x86_64/computer".into(),
            "std/aarch64/raspberry-pi-3-model-b-plus-rev-1.3".into(),
            "std/aarch64/raspberry-pi-5-model-b-rev-1.1-8gb".into(),
        ],
        prerequisites: Vec::new(),
        build_feature: None,
    }
}

/// Source-bound equipment request. Contains no opened file, socket, peer,
/// membership, execution grant or permission to replace an existing session.
#[derive(Clone, Debug)]
pub struct CheckedTlsListenerInitialization {
    configuration_id: String,
    bind_address: SocketAddr,
    certificate_pem: PathBuf,
    private_key_pem: PathBuf,
    certificate_binding_sha256: [u8; 32],
    maximum_message_bytes: u32,
    handshake_timeout_millis: u32,
    accept_poll_millis: u32,
}
impl CheckedTlsListenerInitialization {
    pub fn from_configuration(checked: &CheckedHostConfiguration) -> Result<Self, String> {
        if checked.configuration().target.os.as_deref() != Some("linux") {
            return Err("TLS listener initialization requires a selected Linux target".into());
        }
        let mut bases = checked
            .configuration()
            .bases
            .iter()
            .filter(|base| base.kind == TLS_LISTENER_BASE_KIND);
        let base = bases.next().ok_or("no selected TLS listener Base")?;
        if bases.next().is_some()
            || !checked
                .resolved_bases()
                .iter()
                .any(|(kind, implementation)| {
                    kind == TLS_LISTENER_BASE_KIND && implementation == TLS_LISTENER_IMPLEMENTATION
                })
        {
            return Err("TLS listener requires one exact selected implementation".into());
        }
        let values = base
            .initialization
            .as_ref()
            .ok_or("TLS listener requires explicit initialization")?
            .checked_values(&tls_listener_initialization_contract())?;
        let text = |key: &str| match values.get(key) {
            Some(ConfigurationValue::Text(value)) => Ok(value.as_str()),
            _ => Err("TLS listener text field has wrong type".to_string()),
        };
        let count = |key: &str| match values.get(key) {
            Some(ConfigurationValue::U64(value)) => {
                u32::try_from(*value).map_err(|_| "TLS listener count overflow".to_string())
            }
            _ => Err("TLS listener count field has wrong type".into()),
        };
        let bind_address: SocketAddr = text("bind_address")?
            .parse()
            .map_err(|_| "TLS listener requires a numeric socket address")?;
        if bind_address.ip().is_loopback() || bind_address.port() == 0 {
            return Err("TLS listener requires an exact non-loopback network endpoint".into());
        }
        let certificate_pem = PathBuf::from(text("certificate_pem")?);
        let private_key_pem = PathBuf::from(text("private_key_pem")?);
        if !certificate_pem.is_absolute()
            || !private_key_pem.is_absolute()
            || certificate_pem == private_key_pem
        {
            return Err("TLS identity references require distinct absolute paths".into());
        }
        let encoded = text("certificate_binding_sha256")?;
        if encoded.len() != 64 || !encoded.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("TLS certificate binding requires exactly 32 hexadecimal bytes".into());
        }
        let mut certificate_binding_sha256 = [0; 32];
        for (index, pair) in encoded.as_bytes().as_chunks::<2>().0.iter().enumerate() {
            let digit = |byte: u8| (byte as char).to_digit(16).expect("checked hex") as u8;
            certificate_binding_sha256[index] = digit(pair[0]) * 16 + digit(pair[1]);
        }
        let maximum_message_bytes = count("maximum_message_bytes")?;
        // This lower bound covers the adapter's wire read/write and message
        // storage, not TLS/allocator overhead or whole-Host memory admission.
        let wire_buffer_minimum = u64::from(maximum_message_bytes) * 3 + 32;
        if checked.configuration().limits.line_sessions < 1
            || checked.configuration().limits.buffered_bytes < wire_buffer_minimum
        {
            return Err("TLS listener exceeds declared session or wire-buffer capacity".into());
        }
        Ok(Self {
            configuration_id: checked.configuration_id().into(),
            bind_address,
            certificate_pem,
            private_key_pem,
            certificate_binding_sha256,
            maximum_message_bytes,
            handshake_timeout_millis: count("handshake_timeout_millis")?,
            accept_poll_millis: count("accept_poll_millis")?,
        })
    }
    pub fn configuration_id(&self) -> &str {
        &self.configuration_id
    }
    pub fn bind_address(&self) -> SocketAddr {
        self.bind_address
    }
    pub fn certificate_pem(&self) -> &std::path::Path {
        &self.certificate_pem
    }
    pub fn private_key_pem(&self) -> &std::path::Path {
        &self.private_key_pem
    }
    pub fn certificate_binding_sha256(&self) -> [u8; 32] {
        self.certificate_binding_sha256
    }
    pub fn maximum_message_bytes(&self) -> u32 {
        self.maximum_message_bytes
    }
    pub fn handshake_timeout_millis(&self) -> u32 {
        self.handshake_timeout_millis
    }
    pub fn accept_poll_millis(&self) -> u32 {
        self.accept_poll_millis
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_host_make::{
        canonical_host_configuration_conduit, check_host_configuration,
        parse_host_configuration_conduit, MakeCatalog, MakePackageSet,
    };
    fn source() -> String {
        include_str!("../../profiles/linux-computer.host.conduit").replace(
            "  limits =",
            r#"  base = {kind: "network/tls-listener", implementation: "hosted/tls-websocket-listener@1", initialization: {bind_address: "0.0.0.0:8796", certificate_pem: "/nonexistent/certificate.pem", private_key_pem: "/nonexistent/private-key.pem", certificate_binding_sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", maximum_message_bytes: 8192, handshake_timeout_millis: 2000, accept_poll_millis: 100, maximum_concurrent_sessions: 1}}
  limits ="#,
        )
    }
    fn check(source: &str) -> Result<CheckedHostConfiguration, String> {
        let packages = MakePackageSet::compose(&[&crate::HostedMakePackage]).unwrap();
        check_host_configuration(
            parse_host_configuration_conduit(source).map_err(|e| format!("{e:?}"))?,
            &MakeCatalog::canonical().with_packages(&packages),
            &packages,
        )
        .map_err(|e| format!("{e:?}"))
    }
    #[test]
    fn tls_listener_intent_is_source_bound_without_opening_identity_or_socket() {
        let checked = check(&source()).unwrap();
        let intent = CheckedTlsListenerInitialization::from_configuration(&checked).unwrap();
        assert_eq!(intent.configuration_id(), checked.configuration_id());
        assert_eq!(intent.bind_address().port(), 8796);
        assert_eq!(intent.certificate_binding_sha256(), [0xaa; 32]);
        let canonical = canonical_host_configuration_conduit(checked.configuration()).unwrap();
        assert_eq!(
            check(&canonical).unwrap().configuration_id(),
            checked.configuration_id()
        );
        let changed = check(&source().replace("0.0.0.0:8796", "0.0.0.0:8797")).unwrap();
        assert_ne!(changed.configuration_id(), checked.configuration_id());
        assert!(
            checked.profile().lines.is_empty(),
            "intent does not mint an initialized Line"
        );
    }
    #[test]
    fn tls_listener_refuses_ambiguous_endpoint_identity_and_insufficient_bounds() {
        let source = source();
        for altered in [
            source.replace("0.0.0.0:8796", "localhost:8796"),
            source.replace("0.0.0.0:8796", "127.0.0.1:8796"),
            source.replace("0.0.0.0:8796", "0.0.0.0:0"),
            source.replace(
                "/nonexistent/private-key.pem",
                "/nonexistent/certificate.pem",
            ),
            source.replace("/nonexistent/certificate.pem", "certificate.pem"),
            source.replace("line_sessions: 64", "line_sessions: 0"),
            source.replace("buffered_bytes: 16777216", "buffered_bytes: 24576"),
            source.replace(
                "maximum_concurrent_sessions: 1",
                "maximum_concurrent_sessions: 2",
            ),
            source.replace(
                "maximum_message_bytes: 8192",
                "maximum_message_bytes: 131073",
            ),
            source.replace(
                "handshake_timeout_millis: 2000",
                "handshake_timeout_millis: 0",
            ),
            source.replace(&"a".repeat(64), &"g".repeat(64)),
            source.replace(", accept_poll_millis: 100", ""),
            source.replace(
                "implementation: \"hosted/tls-websocket-listener@1\"",
                "implementation: \"hosted/serial@1\"",
            ),
        ] {
            assert!(check(&altered)
                .and_then(|checked| CheckedTlsListenerInitialization::from_configuration(&checked))
                .is_err());
        }
    }
}
