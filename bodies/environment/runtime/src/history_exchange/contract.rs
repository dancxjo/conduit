//! Preparation-only contract derived from reviewed Conduit Types.

use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, Kind, KindIdentity, PortDescriptor, PortDirection,
    PortTemporal, PreparedStructuredValueValidator, StructuredInfoRefusal, StructuredInfoType,
};
use conduit_plot::{check_syntax_document, parse_syntax_document, ProfileCatalog, StartupCatalog};

pub const HISTORY_KIND: &str = "environment/history/read";
pub const HISTORY_CALL: &str = "conduit.host/environment-history-read@1";
pub const HISTORY_MAXIMUM_BYTES: u32 = 2304;
pub const HISTORY_TYPES: &str = include_str!("../../../profiles/history-exchange.conduit");

/// Exact schemas, prepared before Play. A definition is not an available Back,
/// selected endpoint, resource reservation or authority to perform a transfer.
pub struct HistoryReadContract {
    request: StructuredInfoType,
    result: StructuredInfoType,
    kind: Kind,
    types: Vec<conduit_plot::CheckedNativeType>,
}

impl HistoryReadContract {
    pub fn request_type(&self) -> &StructuredInfoType {
        &self.request
    }
    pub fn result_type(&self) -> &StructuredInfoType {
        &self.result
    }
    pub fn kind(&self) -> &Kind {
        &self.kind
    }

    pub fn prepare() -> Result<Self, StructuredInfoRefusal> {
        // Prepare Types before registering this primitive Kind; the composed Plot
        // is checked afterward against catalogs() by its planning caller.
        let types = HISTORY_TYPES
            .split("plot history-read")
            .next()
            .ok_or(StructuredInfoRefusal::WrongType)?;
        let syntax = parse_syntax_document(types);
        if !syntax.diagnostics.is_empty() {
            return Err(StructuredInfoRefusal::WrongType);
        }
        let checked = check_syntax_document(&syntax, &StartupCatalog::new())
            .map_err(|_| StructuredInfoRefusal::WrongType)?;
        let find = |name| {
            checked
                .native_types
                .iter()
                .find(|ty| ty.name == name)
                .map(|ty| ty.value_type.clone())
                .ok_or(StructuredInfoRefusal::WrongType)
        };
        let request = find("HistoryReadRequest")?;
        let result = find("SignedHistoryBatch")?;
        let port = |name: &str, ty: &StructuredInfoType, direction| {
            Ok(PortDescriptor {
                port_id: port_id(name),
                value_kind: ty.profile()?.value_kind().clone(),
                direction,
                temporal: PortTemporal::Flow { closes: true },
                abnormal_kind: None,
            })
        };
        let input = port("request", &request, PortDirection::Input)?;
        let output = port("result", &result, PortDirection::Output)?;
        let kind = Kind {
            kind_id: kind_id(HISTORY_KIND),
            kind_contract_revision: KindIdentity::from("environment/history/read@1"),
            startup_parameters: vec![],
            shorthand: Some((input.port_id.clone(), output.port_id.clone())),
            inputs: vec![input],
            outputs: vec![output],
            configuration: vec![],
            semantic_laws: vec![],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: HISTORY_MAXIMUM_BYTES,
            },
        };
        Ok(Self {
            request,
            result,
            kind,
            types: checked.native_types,
        })
    }

    /// Check the ordinary authored wrapper after registering its primitive Fore.
    /// This prepares Source only; admission still selects an actual offered Base.
    pub fn checked_document(
        &self,
    ) -> Result<conduit_plot::CheckedSyntaxDocument, StructuredInfoRefusal> {
        let (startup, _) = self.catalogs();
        let syntax = parse_syntax_document(HISTORY_TYPES);
        if !syntax.diagnostics.is_empty() {
            return Err(StructuredInfoRefusal::WrongType);
        }
        check_syntax_document(&syntax, &startup).map_err(|_| StructuredInfoRefusal::WrongType)
    }

    pub fn request_validator(
        &self,
    ) -> Result<PreparedStructuredValueValidator, StructuredInfoRefusal> {
        PreparedStructuredValueValidator::new(&self.request, HISTORY_MAXIMUM_BYTES as usize)
    }

    pub fn catalogs(&self) -> (StartupCatalog, ProfileCatalog) {
        let mut startup = StartupCatalog::new();
        for (name, path) in [
            ("HistoryReadRequest", "environment/history/read/request"),
            ("SignedHistoryBatch", "environment/history/read/result"),
        ] {
            let ty = self
                .types
                .iter()
                .find(|ty| ty.name == name)
                .expect("checked HISTORY Type");
            startup
                .insert_checked_native_type(path, ty)
                .expect("HISTORY import");
        }
        startup
            .insert(conduit_plot::KindSignature {
                kind: HISTORY_KIND.into(),
                startup_parameters: vec![],
            })
            .expect("HISTORY startup");
        startup
            .insert_fore(HISTORY_KIND, self.kind.checked_front())
            .expect("HISTORY Fore");
        let mut profile = ProfileCatalog::new();
        profile
            .insert_kind(self.kind.clone())
            .expect("HISTORY Kind");
        (startup, profile)
    }
}
