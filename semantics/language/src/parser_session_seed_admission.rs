//! Complete Native beam refinement of the one actual fixed Source seed.
use crate::{
    parser_canonical_refinement::PreparedParserCanonicalRefinement,
    parser_session_execution::ParserSessionEntry, parser_session_fixed_ingress::ParserFixedHistory,
    LanguageParserJointRuntimeRawBeam, LanguageParserSessionSeedProposal,
};
use alloc::vec::Vec;
use conduit_core::validate_canonical_structured_value;
use conduit_plot::rust_binding::{NativeBindingRefusal, PreparedNativeFamily};

pub(crate) struct ParserSeedBeamAdmission {
    pub(crate) seed_execution: usize,
    pub(crate) complete_beam: Vec<u8>,
}
#[derive(Debug)]
pub(crate) enum SeedBeamRefusal {
    Origin,
    Pressure,
    Refinement,
    Native(NativeBindingRefusal),
}
impl ParserSeedBeamAdmission {
    pub(crate) fn admit(
        seed_execution: usize,
        origin: &ParserFixedHistory,
        refinement: &mut PreparedParserCanonicalRefinement<
            LanguageParserSessionSeedProposal,
            LanguageParserJointRuntimeRawBeam,
        >,
        family: &mut PreparedNativeFamily,
        mut buffer: Vec<u8>,
    ) -> Result<Self, SeedBeamRefusal> {
        use SeedBeamRefusal as R;
        if origin.entry != ParserSessionEntry::Seed {
            return Err(R::Origin);
        }
        let source = validate_canonical_structured_value(&origin.output).map_err(|_| R::Origin)?;
        let beam = refinement.compose(source).map_err(|_| R::Refinement)?;
        if beam.len() > buffer.capacity() {
            return Err(R::Pressure);
        }
        // The reserved canonical frame moves into history. Full Native decode
        // uses the independently admitted active conversion reservation.
        buffer.clear();
        buffer.extend_from_slice(beam);
        drop(
            family
                .decode::<LanguageParserJointRuntimeRawBeam>(&buffer)
                .map_err(R::Native)?,
        );
        Ok(Self {
            seed_execution,
            complete_beam: buffer,
        })
    }
    pub(crate) fn readmit(
        &self,
        origin: &ParserFixedHistory,
        refinement: &mut PreparedParserCanonicalRefinement<
            LanguageParserSessionSeedProposal,
            LanguageParserJointRuntimeRawBeam,
        >,
        family: &mut PreparedNativeFamily,
    ) -> Result<(), SeedBeamRefusal> {
        use SeedBeamRefusal as R;
        if origin.entry != ParserSessionEntry::Seed {
            return Err(R::Origin);
        }
        let source = validate_canonical_structured_value(&origin.output).map_err(|_| R::Origin)?;
        if refinement.compose(source).map_err(|_| R::Refinement)? != self.complete_beam.as_slice() {
            return Err(R::Origin);
        }
        drop(
            family
                .decode::<LanguageParserJointRuntimeRawBeam>(&self.complete_beam)
                .map_err(R::Native)?,
        );
        Ok(())
    }
}
