//! Current authority recheck and exact complete resource ledger at Start.
use super::*;

impl<T: PreparedLocalKernels, const F: usize> CommittedLocalWorkset<'_, T, F> {
    /// Original approved scopes needed to abort this unstarted reservation via
    /// Core and retain its release evidence. Providers remain unbound on refusal.
    pub fn fragments(&self) -> &[b::DelegatedBodyFragment<'_>; F] {
        &self.fragments
    }

    /// Recheck the unchanged original signed command and complete live ledger at
    /// the actual Start transition. Transport acknowledgement grants no Start.
    /// Refusal preserves this owner in place. Success consumes its original
    /// kernel tickets once; a repeated Start cannot bind them again.
    #[inline(always)] // The original committed owner stays in caller storage.
    pub fn start<const I: usize, E>(
        &mut self,
        workspace: &b::HostCommandInput<I>,
        context: b::NativePreparationContext<'_>,
        policy: Policy<'_>,
        resources: &c::ResourceAdmissionOwner,
    ) -> Result<StartedLocalWorkset<T::Bound>, Refusal<E>> {
        if self.prepared.is_none() {
            return Err(Refusal::Scope);
        }
        let StartOrigin::Online(stamp) = &self.origin else {
            return Err(Refusal::Scope);
        };
        let check = (|| {
            let command = workspace
                .reborrow(
                    context.member,
                    context.actual,
                    &context.bootstrap.owner,
                    b::HOST_PREPARED_WORKSET_CONTRACT,
                    context.now_millis,
                )
                .map_err(Refusal::Retention)?;
            stamp.check_command(&command).map_err(Refusal::Retention)?;
            let set = b::receive_delegated_body_workset::<F>(policy.preparation, context, command)
                .map_err(Refusal::Workset)?;
            set.check_execution_delegation(
                policy.execution,
                policy.preparation,
                context,
                policy.active_worksets,
            )
            .map_err(Refusal::Execution)?;
            if set.fragment_count() != F {
                return Err(Refusal::Scope);
            }
            check_resources(
                &self.fragments,
                context.actual,
                context.member.body_id.as_str(),
                resources,
            )?;
            Ok(())
        })();
        match check {
            Ok(()) => {
                // SAFETY: original whole-workset admission plus all current
                // authenticated Start checks above succeeded atomically.
                Ok(StartedLocalWorkset {
                    custody_identity: match &self.origin {
                        StartOrigin::Online(stamp) => stamp.custody_identity(),
                        StartOrigin::Continuation(_) => unreachable!("online origin checked"),
                    },
                    kernels: unsafe {
                        self.prepared
                            .take()
                            .expect("original unstarted kernels")
                            .bind()
                    },
                })
            }
            Err(e) => Err(e),
        }
    }
}

pub(super) fn check_resources<E, const F: usize>(
    fragments: &[b::DelegatedBodyFragment<'_>; F],
    actual: &c::HostAdvertisement,
    body: &str,
    resources: &c::ResourceAdmissionOwner,
) -> Result<(), Refusal<E>> {
    if resources.host() != actual {
        return Err(Refusal::Scope);
    }
    let mut entries = 0usize;
    for fragment in fragments.iter() {
        let m = fragment.metadata();
        if m.body != body
            || m.host != actual.host_id.as_str()
            || m.boot != actual.boot_id.as_str()
            || m.offer_generation != actual.offer_generation.0
        {
            return Err(Refusal::Scope);
        }
        for item in b::FragmentResources::new(m.resource_bindings) {
            let item = item.map_err(|_| Refusal::Scope)?;
            entries += 1;
            let matches = resources
                .admissions()
                .iter()
                .filter(|a| {
                    a.plan_id.as_str() == m.body_plan
                        && a.placement_id.as_str() == item.placement
                        && a.host_id.as_str() == m.host
                        && a.boot_id.as_str() == m.boot
                        && a.offer_generation.0 == m.offer_generation
                })
                .flat_map(|a| &a.items)
                .filter(|held| item.matches_admitted_item(held, m.host, m.boot))
                .count();
            if matches != 1 {
                return Err(Refusal::Scope);
            }
        }
    }
    if resources
        .admissions()
        .iter()
        .map(|a| a.items.len())
        .sum::<usize>()
        != entries
    {
        return Err(Refusal::Scope);
    }
    Ok(())
}
