//! Prepared optional content custody for the shared std execution kernel.
use conduit_core::PlanFragment;
use conduit_kernel::{
    content_sign::*, CordId, HostedSignLog, KernelEvent, KernelEventKind, NodeId, PortId,
    RemoteLifecycleIdentity, RequestId, SignError, SignQuery, SignRetentionGap, SignSink,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContentSignAdmission {
    pub digest_domain: &'static str,
    pub digest_scheme: &'static str,
    pub event_items: u16,
    pub binding_storage_bytes: u32,
    pub maximum_digest_input_bytes: u32,
    pub value_storage_bytes: Option<u32>,
    pub maximum_copy_passes: u8,
}
pub(super) struct CoreContentDigest {
    maximum_bytes: u32,
}
impl ContentDigestProvider for CoreContentDigest {
    fn digest(&self, bytes: &[u8]) -> [u8; 32] {
        debug_assert!(bytes.len() <= self.maximum_bytes as usize);
        conduit_core::semantic_digest(conduit_std_offers::ROUTED_CONTENT_DIGEST_DOMAIN, bytes)
    }
}
pub(super) enum StdSignLog {
    Plain(HostedSignLog),
    Content {
        log: HostedContentSignLog<HostedSignLog, CoreContentDigest>,
        admission: ContentSignAdmission,
    },
    Values {
        log: HostedContentSignLog<HostedSignLog, CoreContentDigest, Vec<u8>>,
        admission: ContentSignAdmission,
    },
}
mod evidence;
pub(super) use evidence::capture_report;
pub use evidence::{ContentEvidenceReport, RetainedRoutedValue};
impl StdSignLog {
    pub(super) fn prepare(
        inner: HostedSignLog,
        fragments: &[&PlanFragment],
        maximum_value_bytes: u32,
    ) -> Result<Self, String> {
        let expected = conduit_std_offers::routed_content_sign_requirement()
            .map_err(|why| format!("content Sign schema: {why:?}"))?;
        let values_expected = conduit_std_offers::routed_content_value_requirement()
            .map_err(|why| format!("content value schema: {why:?}"))?;
        let mut requested = false;
        let mut values_requested = false;
        for property in fragments
            .iter()
            .flat_map(|f| &f.placements)
            .flat_map(|p| &p.realization_properties)
        {
            if property.profile() == expected.profile() {
                if property != &expected {
                    return Err(
                        "content Sign requirement differs from the installed digest/storage policy"
                            .into(),
                    );
                }
                requested = true;
            }
            if property.profile() == values_expected.profile() {
                if property != &values_expected {
                    return Err("content value requirement differs from installed policy".into());
                }
                values_requested = true;
            }
        }
        if values_requested && !requested {
            return Err("content values require their exact digest profile".into());
        }
        if !requested {
            return Ok(Self::Plain(inner));
        }
        let event_items = inner.item_capacity();
        let binding_storage_bytes =
            content_sign_storage_bytes(event_items).ok_or("content Sign byte budget overflow")?;
        let value_storage_bytes = if values_requested {
            Some(
                content_value_storage_bytes(event_items, maximum_value_bytes)
                    .ok_or("content value byte budget overflow")?,
            )
        } else {
            None
        };
        let admission = ContentSignAdmission {
            digest_domain: conduit_std_offers::ROUTED_CONTENT_DIGEST_DOMAIN,
            digest_scheme: conduit_std_offers::ROUTED_CONTENT_DIGEST_SCHEME,
            event_items,
            binding_storage_bytes,
            maximum_digest_input_bytes: maximum_value_bytes,
            value_storage_bytes,
            maximum_copy_passes: u8::from(values_requested),
        };
        let log = HostedContentSignLog::new(
            inner,
            CoreContentDigest {
                maximum_bytes: maximum_value_bytes,
            },
            binding_storage_bytes,
        )
        .map_err(|why| format!("prepare content Sign storage: {why:?}"))?;
        if let Some(bytes) = value_storage_bytes {
            let length = usize::try_from(bytes).map_err(|_| "content value address space")?;
            let mut storage = Vec::new();
            storage
                .try_reserve_exact(length)
                .map_err(|_| "content value allocation refused")?;
            if storage.capacity() != length {
                return Err("content value capacity differs from admission".into());
            }
            storage.resize(length, 0);
            let log = log
                .with_value_storage(maximum_value_bytes, storage)
                .map_err(|why| format!("prepare content values: {why:?}"))?;
            Ok(Self::Values { log, admission })
        } else {
            Ok(Self::Content { log, admission })
        }
    }
    fn base(&self) -> &HostedSignLog {
        match self {
            Self::Plain(log) => log,
            Self::Content { log, .. } => log.inner(),
            Self::Values { log, .. } => log.inner(),
        }
    }
    pub(super) fn events(&self) -> impl Iterator<Item = KernelEvent> + '_ {
        self.base().events()
    }
    pub(super) fn admission(&self) -> Option<&ContentSignAdmission> {
        match self {
            Self::Plain(_) => None,
            Self::Content { admission, .. } | Self::Values { admission, .. } => Some(admission),
        }
    }
    pub(super) fn bindings(&self) -> impl Iterator<Item = ContentSignBinding> + '_ {
        let digest = match self {
            Self::Content { log, .. } => Some(log),
            _ => None,
        };
        let values = match self {
            Self::Values { log, .. } => Some(log),
            _ => None,
        };
        digest
            .into_iter()
            .flat_map(|log| log.bindings())
            .chain(values.into_iter().flat_map(|log| log.bindings()))
    }
    fn retained_value(&self, event: u32) -> Option<&[u8]> {
        match self {
            Self::Values { log, .. } => log.retained_value(event),
            _ => None,
        }
    }
}
impl SignSink for StdSignLog {
    fn item_capacity(&self) -> u16 {
        self.base().item_capacity()
    }
    fn byte_capacity(&self) -> u32 {
        self.base().byte_capacity()
    }
    fn len(&self) -> u16 {
        self.base().len()
    }
    fn used_bytes(&self) -> u32 {
        self.base().used_bytes()
    }
    fn ensure_capacity(&self, n: u16) -> Result<(), SignError> {
        self.base().ensure_capacity(n)
    }
    fn ensure_value_extent(&self, maximum_bytes: u32) -> Result<(), SignError> {
        match self {
            Self::Plain(log) => log.ensure_value_extent(maximum_bytes),
            Self::Content { log, .. } => log.ensure_value_extent(maximum_bytes),
            Self::Values { log, .. } => log.ensure_value_extent(maximum_bytes),
        }
    }
    fn ensure_remote_capacity(&self, n: u16) -> Result<(), SignError> {
        self.base().ensure_remote_capacity(n)
    }
    fn record(
        &mut self,
        node: NodeId,
        port: Option<PortId>,
        request: Option<RequestId>,
        kind: KernelEventKind,
    ) -> Result<KernelEvent, SignError> {
        match self {
            Self::Plain(log) => log.record(node, port, request, kind),
            Self::Content { log, .. } => log.record(node, port, request, kind),
            Self::Values { log, .. } => log.record(node, port, request, kind),
        }
    }
    fn record_value(
        &mut self,
        node: NodeId,
        port: PortId,
        cord: CordId,
        bytes: &[u8],
    ) -> Result<KernelEvent, SignError> {
        match self {
            Self::Plain(log) => log.record_value(node, port, cord, bytes),
            Self::Content { log, .. } => log.record_value(node, port, cord, bytes),
            Self::Values { log, .. } => log.record_value(node, port, cord, bytes),
        }
    }
    fn record_remote(
        &mut self,
        node: NodeId,
        port: PortId,
        kind: KernelEventKind,
        remote: RemoteLifecycleIdentity,
    ) -> Result<KernelEvent, SignError> {
        match self {
            Self::Plain(log) => log.record_remote(node, port, kind, remote),
            Self::Content { log, .. } => log.record_remote(node, port, kind, remote),
            Self::Values { log, .. } => log.record_remote(node, port, kind, remote),
        }
    }
}
impl SignQuery for StdSignLog {
    fn contains_kind(&self, kind: KernelEventKind) -> bool {
        self.base().contains_kind(kind)
    }
    fn remote_identity(&self, seq: u32) -> Option<RemoteLifecycleIdentity> {
        self.base().remote_identity(seq)
    }
    fn retention_gap(&self) -> Option<SignRetentionGap> {
        self.base().retention_gap()
    }
}

#[cfg(test)]
mod tests;
