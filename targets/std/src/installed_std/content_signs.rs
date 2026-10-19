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
}
impl StdSignLog {
    pub(super) fn prepare(
        inner: HostedSignLog,
        fragments: &[&PlanFragment],
        maximum_value_bytes: u32,
    ) -> Result<Self, String> {
        let expected = conduit_std_offers::routed_content_sign_requirement()
            .map_err(|why| format!("content Sign schema: {why:?}"))?;
        let mut requested = false;
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
        }
        if !requested {
            return Ok(Self::Plain(inner));
        }
        let event_items = inner.item_capacity();
        let binding_storage_bytes =
            content_sign_storage_bytes(event_items).ok_or("content Sign byte budget overflow")?;
        let admission = ContentSignAdmission {
            digest_domain: conduit_std_offers::ROUTED_CONTENT_DIGEST_DOMAIN,
            digest_scheme: conduit_std_offers::ROUTED_CONTENT_DIGEST_SCHEME,
            event_items,
            binding_storage_bytes,
            maximum_digest_input_bytes: maximum_value_bytes,
        };
        let log = HostedContentSignLog::new(
            inner,
            CoreContentDigest {
                maximum_bytes: maximum_value_bytes,
            },
            binding_storage_bytes,
        )
        .map_err(|why| format!("prepare content Sign storage: {why:?}"))?;
        Ok(Self::Content { log, admission })
    }
    fn base(&self) -> &HostedSignLog {
        match self {
            Self::Plain(log) => log,
            Self::Content { log, .. } => log.inner(),
        }
    }
    pub(super) fn events(&self) -> impl Iterator<Item = KernelEvent> + '_ {
        self.base().events()
    }
    pub(super) fn admission(&self) -> Option<&ContentSignAdmission> {
        match self {
            Self::Plain(_) => None,
            Self::Content { admission, .. } => Some(admission),
        }
    }
    pub(super) fn bindings(&self) -> impl Iterator<Item = ContentSignBinding> + '_ {
        let log = match self {
            Self::Plain(_) => None,
            Self::Content { log, .. } => Some(log),
        };
        log.into_iter().flat_map(|log| log.bindings())
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
