//! Exclusive preparation custody over reservations appended to the same owner.
use super::*;

/// Reservations held for an unstarted preparation attempt. The exclusive owner
/// borrow prevents release, reassignment or replacement underneath this scope.
/// Drop releases only its appended reservations, without allocating or touching
/// earlier admissions/compute assignments. Forgetting it leaks reservations;
/// it never makes their resources available to another attempt.
///
/// This is not a native execution/retirement guard. Do not use its destruction
/// as evidence that an asynchronous device operation or worker has stopped.
pub struct ResourceAdmissionScope<'o> {
    owner: &'o mut ResourceAdmissionOwner,
    first: usize,
}

impl ResourceAdmissionOwner {
    /// Begin a preparation scope without copying inventory or prior admissions.
    pub fn preparation_scope(&mut self) -> ResourceAdmissionScope<'_> {
        let first = self.admissions.len();
        ResourceAdmissionScope { owner: self, first }
    }
}

impl ResourceAdmissionScope<'_> {
    pub fn host(&self) -> &HostAdvertisement {
        self.owner.host()
    }

    /// Only reservations acquired by this preparation attempt.
    pub fn admissions(&self) -> &[ResourceAdmission] {
        &self.owner.admissions[self.first..]
    }

    /// The same atomic batch law, including prior and already scoped bindings.
    /// A refusal preserves this scope as well as the owner's earlier state.
    pub fn admit_batch(
        &mut self,
        requests: Vec<ResourceAdmissionRequest>,
        observations: &[ResourceObservation],
    ) -> Result<&[ResourceAdmission], ResourceAdmissionRefusal> {
        self.owner.admit_batch(requests, observations)
    }
}

impl Drop for ResourceAdmissionScope<'_> {
    fn drop(&mut self) {
        // No mutable access or lane assignment escapes this scope, so the
        // immutable prefix and all earlier assignments remain exactly intact.
        self.owner.admissions.truncate(self.first);
    }
}
