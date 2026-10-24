//! Typed publication projection shared by Hosts. Body provenance, freshness and
//! transport authentication remain caller custody; Masks own formatting policy.
use crate::publication::{PublicationChannel as Channel, PublicationContract, PublicationSnapshot};
use conduit_core::{PreparedStructuredValueValidator, StructuredInfoRefusal as R};
use conduit_environment_face::{contribution, ContributionRefusal, SensorStatus};
use conduit_face::{FaceContributionBasis, FaceFragment};

#[derive(Debug, PartialEq, Eq)]
pub enum PublicationFaceRefusal {
    Structured(R),
    Contribution(ContributionRefusal),
}
impl From<R> for PublicationFaceRefusal {
    fn from(value: R) -> Self {
        Self::Structured(value)
    }
}
/// Preparation reserves the four exact channel validators. Projection allocates
/// ordinary Face objects and belongs in admitted Host Face construction, never
/// inside the allocation-sealed sensor Step.
pub struct PreparedPublicationFace {
    validators: [PreparedStructuredValueValidator; 4],
}
impl PreparedPublicationFace {
    pub fn prepare() -> Result<Self, R> {
        let validator = |channel: Channel| {
            PreparedStructuredValueValidator::new(
                PublicationContract::prepare(channel)?.value_type(),
                channel.maximum_bytes(),
            )
        };
        Ok(Self {
            validators: [
                validator(Channel::Reading)?,
                validator(Channel::CaptureTimeout)?,
                validator(Channel::ProviderFailure)?,
                validator(Channel::GeometryRefusal)?,
            ],
        })
    }
    /// `fresh` is established by the owning Host's admitted freshness policy;
    /// receipt time or bytes alone cannot establish it. Failures have no fresh
    /// observation and never inherit quantities from an earlier publication.
    pub fn project(
        &mut self,
        snapshot: PublicationSnapshot<'_>,
        fresh: bool,
        basis: FaceContributionBasis,
    ) -> Result<FaceFragment, PublicationFaceRefusal> {
        self.validators[snapshot.channel.index()].validate(snapshot.canonical)?;
        let (observation, status) = conduit_environment_face::decode_observation_result(
            snapshot.channel.index(),
            snapshot.canonical,
        )
        .map_err(|_| PublicationFaceRefusal::Structured(R::WrongType))?;
        let (temperature, humidity) = observation.map_or((None, None), |(t, h)| (Some(t), Some(h)));
        contribution(
            temperature,
            humidity,
            fresh && status == SensorStatus::Available,
            status,
            basis,
        )
        .map_err(PublicationFaceRefusal::Contribution)
    }
}
