//! Exclusive preparation-owned storage association; no class policy or journal.
use super::*;
use crate::protocol_source::StorageBodyPreparation;
use crate::storage_base::request::BlockReadRequest;
use crate::storage_base::transaction_owner::{
    BlockReadTransactionOwner, StorageAuthorizationSubmission, StorageCancellationReceipt,
    StorageSettlementReceipt, StorageTransactionRefusal,
};

pub(super) struct StorageAssociation {
    pub(super) owner: BlockReadTransactionOwner,
    usb_nodes: [NodeId; 3],
    body: Option<StorageBodyPreparation>,
    submission: Option<StorageAuthorizationSubmission>,
    identity: Option<(BlockReadRequest, u32, [u8; 32])>,
    receipt: Option<StorageSettlementReceipt>,
    cancelled: bool,
    revoked: bool,
    maximum_retained_bytes: u64,
    preparation_retained_bytes: usize,
    output: [u8; 4096],
    output_length: usize,
}
impl StorageAssociation {
    pub(super) fn prepare(
        owner: BlockReadTransactionOwner,
        plan: &Plan,
        nodes: [NodeId; 3],
        maximum_retained_bytes: u64,
        dispatcher_retained_bytes: usize,
    ) -> Result<Self, ProtocolCallRefusal> {
        let [fragment] = plan.fragments.as_slice() else {
            return Err(crate::protocol_play::preparation_diagnostic::invalid(
                "storage/identity",
                (nodes, plan.plan_id.as_str()),
            ));
        };
        let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
        for claim in [owner.authorize_claim(), owner.settle_claim()] {
            if claim.plan_id != plan.plan_id
                || claim.host_id != fragment.host_id
                || claim.boot_id != fragment.boot_id
                || claim.active_play_id != active.active_play_id
            {
                return Err(crate::protocol_play::preparation_diagnostic::invalid(
                    "storage/identity",
                    (nodes, plan.plan_id.as_str()),
                ));
            }
        }
        let all = [
            nodes[0],
            nodes[1],
            nodes[2],
            owner.authorize_node(),
            owner.settle_node(),
        ];
        for (index, node) in all.iter().enumerate() {
            if all[..index].contains(node) {
                return Err(crate::protocol_play::preparation_diagnostic::invalid(
                    "storage/identity",
                    (nodes, plan.plan_id.as_str()),
                ));
            }
        }
        let preparation_retained_bytes = owner
            .retained_bytes()
            .checked_add(dispatcher_retained_bytes)
            .ok_or(ProtocolCallRefusal::Storage(
                StorageTransactionRefusal::Retention,
            ))?;
        crate::storage_base::transaction_owner::retention::charge(
            preparation_retained_bytes,
            core::mem::size_of::<Self>(),
            0,
            maximum_retained_bytes,
        )
        .map_err(ProtocolCallRefusal::Storage)?;
        Ok(Self {
            owner,
            usb_nodes: nodes,
            body: None,
            submission: None,
            identity: None,
            receipt: None,
            cancelled: false,
            revoked: false,
            maximum_retained_bytes,
            preparation_retained_bytes,
            output: [0; 4096],
            output_length: 0,
        })
    }
    pub(super) fn bind_body(
        &mut self,
        pin: StorageBodyPreparation,
    ) -> Result<(), ProtocolCallRefusal> {
        if self.body.is_some()
            || self.active()
            || pin.plan() != &self.owner.authorize_claim().plan_id
        {
            return Err(ProtocolCallRefusal::InvalidPlan);
        }
        crate::storage_base::transaction_owner::retention::charge(
            self.preparation_retained_bytes,
            core::mem::size_of::<Self>(),
            pin.plan().0.capacity(),
            self.maximum_retained_bytes,
        )
        .map_err(ProtocolCallRefusal::Storage)?;
        self.body = Some(pin);
        Ok(())
    }
    fn active(&self) -> bool {
        self.submission.is_some() || self.receipt.is_some()
    }
    pub(super) fn allows_usb(&self, node: NodeId) -> bool {
        self.body.is_some()
            && !self.cancelled
            && self.submission.is_some()
            && self.receipt.is_none()
            && self.usb_nodes.contains(&node)
    }
    pub(super) fn validate_active_play(
        &self,
        kernel: &KernelCompositeHost,
    ) -> Result<(), ProtocolCallRefusal> {
        if self.body.is_none()
            || self.cancelled
            || kernel
                .active_plays()
                .get(&self.owner.authorize_claim().host_id)
                != Some(&self.owner.authorize_claim().active_play_id)
        {
            return Err(ProtocolCallRefusal::InvalidPlan);
        }
        Ok(())
    }
    pub(super) fn authorize(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request: conduit_kernel::RequestId,
        input: &[u8],
    ) -> Result<&[u8], ProtocolCallRefusal> {
        if self.active() {
            return Err(ProtocolCallRefusal::Storage(
                StorageTransactionRefusal::Pressure,
            ));
        }
        let preparation = *self.owner.preparation();
        let (submission, bytes) = self
            .owner
            .authorize(node, call, request, input)
            .map_err(ProtocolCallRefusal::Storage)?;
        // The genuine original lease is retained before any permit can enable USB.
        self.identity = Some((
            submission.request(),
            submission.ordinal(),
            *submission.preparation(),
        ));
        let exact = submission.preparation() == &preparation;
        self.submission = Some(submission);
        if !exact {
            return Err(ProtocolCallRefusal::Storage(
                StorageTransactionRefusal::Stale,
            ));
        }
        Ok(bytes)
    }
    pub(super) fn settle(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request: conduit_kernel::RequestId,
        input: &[u8],
    ) -> Result<&[u8], ProtocolCallRefusal> {
        let submission = match self.receipt.take() {
            Some(receipt) => receipt.into_submission(),
            None => self.submission.take().ok_or(ProtocolCallRefusal::Storage(
                StorageTransactionRefusal::Stale,
            ))?,
        };
        if self.identity
            != Some((
                submission.request(),
                submission.ordinal(),
                *submission.preparation(),
            ))
        {
            self.submission = Some(submission);
            return Err(ProtocolCallRefusal::Storage(
                StorageTransactionRefusal::Stale,
            ));
        }
        // SAFETY: the only caller first validates the same original native Host,
        // dispatcher tokens and every associated scheduler obligation. This
        // genuine original token never came from Source bytes or another owner.
        let receipt = unsafe { StorageSettlementReceipt::acknowledged(submission) };
        if let Err((error, receipt)) = self.owner.settle(node, call, request, input, receipt) {
            self.receipt = Some(receipt);
            return Err(ProtocolCallRefusal::Storage(error));
        }
        self.identity = None;
        self.output[..input.len()].copy_from_slice(input);
        self.output_length = input.len();
        Ok(&self.output[..self.output_length])
    }
    pub(super) fn request_cancel(&mut self) -> Result<(), ProtocolCallRefusal> {
        self.cancelled = true;
        if !self.revoked {
            self.owner
                .request_cancel()
                .map_err(ProtocolCallRefusal::Storage)?;
            self.revoked = true;
        }
        Ok(())
    }
    pub(super) fn retire_cancelled(&mut self) -> Result<(), ProtocolCallRefusal> {
        if !self.cancelled || !self.revoked {
            return Err(ProtocolCallRefusal::Storage(
                StorageTransactionRefusal::Terminal,
            ));
        }
        let submission = match self.receipt.take() {
            Some(receipt) => Some(receipt.into_submission()),
            None => self.submission.take(),
        };
        let Some(submission) = submission else {
            return Ok(());
        };
        if self.identity
            != Some((
                submission.request(),
                submission.ordinal(),
                *submission.preparation(),
            ))
        {
            self.submission = Some(submission);
            return Err(ProtocolCallRefusal::Storage(
                StorageTransactionRefusal::Stale,
            ));
        }
        // SAFETY: only called after original native acknowledged stop and
        // kernel cancellation/retirement checks for this retained association.
        let receipt = unsafe { StorageCancellationReceipt::stopped(submission) };
        if let Err((error, receipt)) = self.owner.retire_cancelled(receipt) {
            self.submission = Some(receipt.into_submission());
            return Err(ProtocolCallRefusal::Storage(error));
        }
        self.identity = None;
        Ok(())
    }
}

impl<P: I2cProvider> PreparedProtocolCalls<P> {
    pub(crate) fn bind_storage_body(
        &mut self,
        pin: Option<StorageBodyPreparation>,
    ) -> Result<(), ProtocolCallRefusal> {
        if let Some(storage) = &mut self.storage {
            storage.bind_body(pin.ok_or(ProtocolCallRefusal::InvalidPlan)?)?;
        }
        Ok(())
    }
    pub(super) fn dispatch_storage(
        &mut self,
        kernel: &mut KernelCompositeHost,
        admitted: &AdmittedKernelCompositeHostRequest,
        node: NodeId,
        call: HostCallId,
        request: conduit_kernel::RequestId,
        settle: bool,
    ) -> Result<(), ProtocolCallRefusal> {
        self.validate_usb_subordinate_idle(kernel)?;
        let storage = self
            .storage
            .as_mut()
            .ok_or(ProtocolCallRefusal::InvalidPlan)?;
        storage.validate_active_play(kernel)?;
        if settle
            && kernel
                .has_retained_host_call_for_nodes(
                    &self.identity.host_id,
                    &[storage.owner.authorize_node()],
                )
                .map_err(ProtocolCallRefusal::Kernel)?
        {
            return Err(ProtocolCallRefusal::Unsupported);
        }
        let input = kernel
            .host_request_input(admitted)
            .map_err(ProtocolCallRefusal::Kernel)?;
        let bytes = if settle {
            storage.settle(node, call, request, input)?
        } else {
            storage.authorize(node, call, request, input)?
        };
        kernel
            .complete_host_call_bytes(admitted, bytes)
            .map_err(ProtocolCallRefusal::Kernel)
    }
}

impl<P: I2cProvider> PreparedProtocolCalls<P> {
    pub(super) fn retire_cancelled_storage(
        &mut self,
        kernel: &KernelCompositeHost,
    ) -> Result<(), ProtocolCallRefusal> {
        use ProtocolCallRefusal as Refusal;
        if self.storage.is_some() {
            let storage = self.storage.as_mut().ok_or(Refusal::InvalidPlan)?;
            let usb_nodes = self
                .usb_duplex
                .as_ref()
                .ok_or(Refusal::InvalidPlan)?
                .subordinate_nodes();
            let nodes = [
                usb_nodes[0],
                usb_nodes[1],
                usb_nodes[2],
                storage.owner.authorize_node(),
                storage.owner.settle_node(),
            ];
            if kernel
                .has_retained_host_call_for_nodes(&self.identity.host_id, &nodes)
                .map_err(Refusal::Kernel)?
            {
                return Err(Refusal::Unsupported);
            }
            let host = self.usb_duplex.as_mut().ok_or(Refusal::InvalidPlan)?;
            host.discard_cancelled_terminals()
                .map_err(Refusal::UsbProtocol)?;
            host.validate_cancelled_settlement()
                .map_err(Refusal::UsbProtocol)?;
            storage.retire_cancelled()?;
        }
        Ok(())
    }
}
