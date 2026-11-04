//! Ordinary offer metadata lives in the image; initialization retains its Back.
use crate::{
    ResidentExpressionRefusal as R, RESIDENT_EXPRESSION_IMPLEMENTATION, RESIDENT_EXPRESSION_PROFILE,
};
use conduit_core as c;
use conduit_plot::PortableExpressionProgram;

pub(crate) fn append(
    code: &mut String,
    program: &PortableExpressionProgram,
    temporal: c::PortTemporal,
    artifact: &c::ArtifactId,
    private_work: u16,
) -> Result<c::CapabilityOffer, R> {
    let kind = conduit_semantic_catalog::pure_expression_contract(program, temporal)
        .map_err(|_| R::Contract)?;
    let limits = c::CapabilityLimits {
        max_active_instances: 1,
        max_queue_items: 1,
        max_queue_bytes: program
            .maximum_prepared_input_bytes()
            .map_err(|_| R::Bounds)?
            .max(
                program
                    .maximum_prepared_output_bytes()
                    .map_err(|_| R::Bounds)?,
            )
            .max(1),
    };
    let back = c::Back {
        capability_id: format!("{RESIDENT_EXPRESSION_PROFILE}/{}", artifact.as_str()).into(),
        execution_profile_id: RESIDENT_EXPRESSION_PROFILE.into(),
        implementation_id: RESIDENT_EXPRESSION_IMPLEMENTATION.into(),
        artifact_id: artifact.clone(),
        host_calls: vec![],
        resource_requirements: vec![c::ResourceRequirement {
            class_id: c::COOPERATIVE_STEP_WORK_CLASS_ID.into(),
            units: u32::from(private_work),
            protected_role: None,
            compute: None,
            content: None,
        }],
        authority_requirements: vec![],
    };
    let offer = c::BackOfferBuilder::try_new(kind, back)
        .map_err(|_| R::Contract)?
        .narrow_capacity(limits)
        .map_err(|_| R::Contract)?
        .build();
    let json = serde_json::to_vec(&offer).map_err(|_| R::GenerationMetadata)?;
    let compressed = miniz_oxide::deflate::compress_to_vec_zlib(&json, 6);
    let document = conduit_wire::DeflatedDocument::from_parts(&compressed, json.len())
        .map_err(|_| R::GenerationMetadata)?;
    let mut image = vec![0; conduit_wire::DEFLATED_DOCUMENT_HEADER_BYTES];
    document
        .write_header(&mut image)
        .map_err(|_| R::GenerationMetadata)?;
    image.extend_from_slice(&compressed);
    // Keep only the lossless image, never a second raw catalog in firmware.
    code.push_str(&format!("static OFFER_IMAGE:&[u8]=&{image:?};\n"));
    code.push_str("/// One actually initialized Back; metadata is neither a grant nor a reservation.\npub struct Initialized { back:Back }\nimpl Initialized {\npub fn prepare()->Result<Self,PreparationRefusal>{Ok(Self{back:prepare_back()?})}\n/// Lend a lossless document image only while this initialized owner is retained.\n/// Resolution requires the recipient's independent bounded decode workspace.\npub fn offer_image(&self)->&[u8]{OFFER_IMAGE}\npub fn into_back(self)->Back{self.back}\n}\n");
    crate::resident_binding::append(
        code,
        &offer,
        &program.canonical_hex().map_err(|_| R::Program)?,
        private_work,
    )?;
    Ok(offer)
}
