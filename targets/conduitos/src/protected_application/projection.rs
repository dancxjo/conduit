//! Read-only biography projection through an existing owner or explicit cold lease.
use super::*;
use conduit_body::BodyBiographyEvidence;
use conduitos_application_core::data::TutorialScope;

fn query(
    service: &mut Service,
    evidence: &BodyBiographyEvidence,
    scope: TutorialScope,
) -> Result<ApplicationView, Refusal> {
    scope.validate(evidence)?;
    let reply = service.command(&Command::ProjectTutorial {
        evidence: evidence.clone(),
        scope: scope.clone(),
    })?;
    let Reply::Tutorial {
        scope: returned,
        view,
    } = reply
    else {
        return Err(Refusal::Malformed);
    };
    if returned != scope || view.len() > VIEW_BYTES {
        return Err(Refusal::Malformed);
    }
    let view = ApplicationView::decode(&view).map_err(|_| Refusal::Malformed)?;
    if view.revision != scope.evidence_revision {
        return Err(Refusal::Malformed);
    }
    authority::validate_context(Kind::Tutorial, &view, &Context::None)
        .map_err(|_| Refusal::Malformed)?;
    Ok(view)
}

/// An explicit stateless cold projection owns and drops this lease in one call.
/// It is never called as a fallback for an existing owner's failed transport.
pub(crate) fn project_tutorial(
    evidence: &BodyBiographyEvidence,
    scope: TutorialScope,
) -> Result<ApplicationView, Refusal> {
    let mut service = Service::install()?;
    let result = query(&mut service, evidence, scope);
    service.quarantine();
    result
}
impl Owner {
    pub fn project_tutorial(
        &self,
        evidence: &BodyBiographyEvidence,
        scope: TutorialScope,
    ) -> Result<ApplicationView, Refusal> {
        if self.failed.get() {
            return Err(Refusal::Order);
        }
        let result = if self.needed {
            // Never substitute a new instance for a failed or missing live owner.
            (|| {
                let mut slot = self.service.try_borrow_mut().map_err(|_| Refusal::Order)?;
                let service = slot.as_mut().ok_or(Refusal::Order)?;
                query(service, evidence, scope)
            })()
        } else {
            // A text-only workset has no retained Application owner to reuse.
            project_tutorial(evidence, scope)
        };
        if result.is_err() {
            // &self cannot clear outer mutable snapshots, so invalidate every
            // accessor immediately. The next mutable boundary retires siblings.
            self.failed.set(true);
            if let Ok(mut slot) = self.service.try_borrow_mut() {
                if let Some(mut service) = slot.take() {
                    service.quarantine();
                }
            }
        }
        result
    }
}
