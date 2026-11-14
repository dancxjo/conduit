//! Allocation-free routing of original attributed work to installed profiles.
//! A route is not fragment approval, possession, reservation or Start.
use conduit_body as b;
use conduit_core as c;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    Resident,
    MixedI2c,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    Retention(b::HostCommandRetentionRefusal),
    Workset(b::DelegatedWorksetRefusal),
    Execution(b::ExecutionDelegationRefusal),
    Shape,
}
pub fn select<const N: usize, const F: usize>(
    input: &b::HostCommandInput<N>,
    context: b::NativePreparationContext<'_>,
    preparation: Option<&b::HostPreparationDelegation>,
    execution: Option<&b::HostExecutionDelegation>,
) -> Result<Profile, Refusal> {
    let command = input
        .reborrow(
            context.member,
            context.actual,
            &context.bootstrap.owner,
            b::HOST_PREPARED_WORKSET_CONTRACT,
            context.now_millis,
        )
        .map_err(Refusal::Retention)?;
    let set = b::receive_delegated_body_workset::<F>(preparation, context, command)
        .map_err(Refusal::Workset)?;
    set.check_execution_delegation(execution, preparation, context, 0)
        .map_err(Refusal::Execution)?;
    let mut native = false;
    for index in 0..set.fragment_count() {
        let (metadata, _) =
            b::decode_body_fragment_document(set.document(index).ok_or(Refusal::Shape)?)
                .map_err(|_| Refusal::Shape)?;
        if metadata.host != context.actual.host_id.as_str()
            || metadata.boot != context.actual.boot_id.as_str()
            || metadata.offer_generation != context.actual.offer_generation.0
        {
            return Err(Refusal::Shape);
        }
        for resource in b::PlainFragmentResources::new(metadata.resource_bindings) {
            match resource.map_err(|_| Refusal::Shape)?.class {
                c::COOPERATIVE_STEP_WORK_CLASS_ID => {}
                conduit_i2c_transaction::installation::I2C_ATTACHMENT_CLASS => native = true,
                _ => return Err(Refusal::Shape),
            }
        }
    }
    // This installed native profile accepts exactly one native/resident pair.
    // Its preparer still validates every node, Fore, identity and resource.
    if native && set.fragment_count() != 1 {
        return Err(Refusal::Shape);
    }
    Ok(if native {
        Profile::MixedI2c
    } else {
        Profile::Resident
    })
}
