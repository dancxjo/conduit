//! Exact hosted OCR operation over the finite image residence.

use super::{FiniteHostedVisionBase, HostedVisionRefusal};
use conduit_core::StructuredInfoValue;

impl FiniteHostedVisionBase {
    pub(crate) fn execute_ocr(
        &mut self,
        input: &[u8],
        run_id: &str,
        observed_at_micros: u64,
        clock_basis: &str,
    ) -> Result<&[u8], HostedVisionRefusal> {
        let (frame_index, pixels) = self.provider.resolve_exact_canonical(input)?;
        let source_value = StructuredInfoValue::from_canonical_bytes(
            self.observation_images
                .get(frame_index)
                .ok_or(HostedVisionRefusal::InvalidOutput)?,
        )
        .map_err(|_| HostedVisionRefusal::InvalidOutput)?;
        let source_image = conduit_semantic_catalog::image_observation_from_value(&source_value)
            .map_err(|_| HostedVisionRefusal::InvalidOutput)?;
        let provider = self
            .ocr_provider
            .as_mut()
            .ok_or(HostedVisionRefusal::InvalidOutput)?;
        let mut candidates = Vec::with_capacity(crate::MAXIMUM_OCR_ITEMS);
        provider
            .recognize(
                pixels,
                source_image.width,
                source_image.height,
                |candidate| {
                    candidates.push((
                        candidate.text.to_owned(),
                        candidate.region,
                        candidate.confidence_permille,
                    ));
                    Ok(())
                },
            )
            .map_err(HostedVisionRefusal::Ocr)?;
        let provider_identity = format!("tesseract/{}", provider.executable_sha256());
        let artifact_identity = format!(
            "tesseract/executable-sha256/{}",
            provider.executable_sha256()
        );
        let observations = candidates
            .into_iter()
            .enumerate()
            .map(|(index, (text, region, confidence_permille))| {
                conduit_human::VisibleTextObservation {
                    source_image: source_image.clone(),
                    text,
                    region: Some(region),
                    confidence_permille,
                    provenance: conduit_human::VisualObservationProvenance {
                        evidence_class: conduit_human::VisualEvidenceClass::StatisticalCandidate,
                        observation_sign_id: conduit_core::SignId::from(format!(
                            "{run_id}/observation-{index}"
                        )),
                        observed_at: conduit_core::TemporalInstant {
                            ticks: observed_at_micros,
                            scale: conduit_core::TemporalScale::Microseconds,
                            clock_basis: clock_basis.to_owned(),
                            resolution_ticks: 1,
                            uncertainty_ticks: 0,
                        },
                        implementation_id: conduit_core::BaseImplementationId::from(
                            conduit_std_offers::LOCAL_VISION_OCR_IMPLEMENTATION,
                        ),
                        provider_instance_id: conduit_core::BaseInstanceId::from(
                            provider_identity.clone(),
                        ),
                        artifact_id: conduit_core::ArtifactId::from(artifact_identity.clone()),
                        run_id: run_id.to_owned(),
                    },
                }
            })
            .collect::<Vec<_>>();
        let value = conduit_semantic_catalog::visible_text_observations_value(
            &observations,
            &source_image.content.content_profile,
        )
        .map_err(|_| HostedVisionRefusal::InvalidOutput)?;
        let encoded = value
            .canonical_bytes()
            .map_err(|_| HostedVisionRefusal::InvalidOutput)?;
        if encoded.len() > self.ocr_output.capacity() {
            return Err(HostedVisionRefusal::InvalidOutput);
        }
        self.ocr_output.clear();
        self.ocr_output.extend_from_slice(&encoded);
        Ok(&self.ocr_output)
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::hosted_vision::HostedVisionFrame;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::time::Duration;

    #[test]
    fn output_retains_exact_image_and_provider_provenance() {
        let image = conduit_semantic_catalog::deterministic_vision_fixture()
            .unwrap()
            .image;
        let encoded = image.canonical_bytes().unwrap();
        let (_, source, _, _) = crate::hosted_vision::decode_image_resource_for_test(&encoded);
        let root =
            std::env::temp_dir().join(format!("conduit-hosted-vision-ocr-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir(&root).unwrap();
        let executable = root.join("tesseract");
        fs::write(
            &executable,
            "#!/bin/sh\ncat >/dev/null\nprintf 'level\\tpage_num\\tblock_num\\tpar_num\\tline_num\\tword_num\\tleft\\ttop\\twidth\\theight\\tconf\\ttext\\n5\\t1\\t1\\t1\\t1\\t1\\t2\\t3\\t4\\t5\\t88.2\\tCANTUS\\n'\n",
        )
        .unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        let ocr = crate::TesseractOcrProvider::prepare(
            &executable,
            "eng",
            conduit_semantic_catalog::MAXIMUM_LOCAL_CV_PIXELS,
            Duration::from_secs(2),
        )
        .unwrap();
        let executable_sha256 = ocr.executable_sha256().to_owned();
        let mut vision = FiniteHostedVisionBase::new_with_ocr(
            vec![HostedVisionFrame {
                canonical_image: encoded.clone(),
                resource: source.clone(),
                width: 64,
                height: 48,
                grayscale_pixels: vec![0; 64 * 48],
            }],
            64,
            48,
            4,
            "finite-image-residence/ocr-proof",
            ocr,
        )
        .unwrap();
        let output = vision
            .execute_ocr(&encoded, "play/ocr/request-1", 41, "boot/ocr/monotonic")
            .unwrap();
        let value = StructuredInfoValue::from_canonical_bytes(output).unwrap();
        let observations = conduit_semantic_catalog::visible_text_observations_from_value(
            &value,
            &source.content_profile,
        )
        .unwrap();
        assert_eq!(observations.len(), 1);
        let observation = &observations[0];
        assert_eq!(observation.source_image.content, source);
        assert_eq!(observation.text, "CANTUS");
        assert_eq!(observation.confidence_permille, 882);
        assert_eq!(
            observation.provenance.evidence_class,
            conduit_human::VisualEvidenceClass::StatisticalCandidate
        );
        assert_eq!(
            observation.provenance.implementation_id.as_str(),
            conduit_std_offers::LOCAL_VISION_OCR_IMPLEMENTATION
        );
        assert!(observation
            .provenance
            .provider_instance_id
            .as_str()
            .ends_with(&executable_sha256));
        assert_eq!(observation.provenance.run_id, "play/ocr/request-1");
        fs::remove_dir_all(root).unwrap();
    }
}
