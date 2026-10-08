//! Exact bounded biography archive residence beneath the selected installed Owner state.
use super::{bounded_read, restrict_directory, write_bytes_atomic, MAXIMUM_STATE};
use conduit_body::{BodyBiographyArchiveSegment, BodyBiographyEvidence};
use std::{fs, path::Path};

pub(super) fn archive_path(root: &Path, ordinal: u64) -> std::path::PathBuf {
    root.join("body/archive")
        .join(format!("{ordinal:020}.json"))
}

fn read_archive(root: &Path, ordinal: u64) -> Result<BodyBiographyArchiveSegment, String> {
    let path = archive_path(root, ordinal);
    if !fs::symlink_metadata(&path)
        .map_err(|error| format!("inspect biography archive {ordinal}: {error}"))?
        .file_type()
        .is_file()
    {
        return Err("biography archive is not an ordinary file".into());
    }
    let segment: BodyBiographyArchiveSegment =
        serde_json::from_slice(&bounded_read(&path, MAXIMUM_STATE / 2)?)
            .map_err(|error| format!("decode biography archive {ordinal}: {error}"))?;
    segment
        .validate()
        .map_err(|error| format!("invalid biography archive {ordinal}: {error:?}"))?;
    if segment.ordinal != ordinal {
        return Err("biography archive ordinal differs from its selected slot".into());
    }
    Ok(segment)
}

pub(super) fn retain_archive_segments(
    root: &Path,
    biography: &BodyBiographyEvidence,
    segments: &[BodyBiographyArchiveSegment],
) -> Result<(), String> {
    validate_archive_segments(root, biography, segments)?;
    if segments.is_empty() {
        return Ok(());
    }
    let directory = root.join("body/archive");
    match fs::symlink_metadata(&directory) {
        Ok(metadata) if !metadata.file_type().is_dir() => {
            return Err("selected biography archive residence is not a directory".into());
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("inspect biography archive residence: {error}")),
    }
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    restrict_directory(&directory)?;
    for segment in segments {
        let path = archive_path(root, segment.ordinal);
        let bytes = serde_json::to_vec_pretty(segment).map_err(|error| error.to_string())?;
        if bytes.len() as u64 > MAXIMUM_STATE / 2 {
            return Err("biography archive segment exceeds its selected storage bound".into());
        }
        match fs::symlink_metadata(&path) {
            Ok(_) => {
                if read_archive(root, segment.ordinal)? != *segment {
                    return Err("biography archive slot already holds different evidence".into());
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                write_bytes_atomic(&path, &bytes)?;
            }
            Err(error) => return Err(format!("inspect biography archive slot: {error}")),
        }
    }
    Ok(())
}

pub(super) fn validate_archive_segments(
    root: &Path,
    biography: &BodyBiographyEvidence,
    segments: &[BodyBiographyArchiveSegment],
) -> Result<(), String> {
    if segments.is_empty() {
        if let Some(summary) = &biography.compaction {
            if summary.sealed_segments > 0 {
                read_archive(root, summary.sealed_segments)?
                    .validate_as_head_of(biography)
                    .map_err(|error| format!("biography archive head differs: {error:?}"))?;
            }
        }
        return Ok(());
    }
    if segments.len() > conduit_body::MAX_BODY_BIOGRAPHY_WAKES {
        return Err("biography archive transaction exceeds its finite segment bound".into());
    }
    let head = segments.last().expect("non-empty checked");
    head.validate_as_head_of(biography)
        .map_err(|error| format!("biography archive head differs: {error:?}"))?;
    for (index, segment) in segments.iter().enumerate() {
        segment
            .validate()
            .map_err(|error| format!("invalid biography archive segment: {error:?}"))?;
        if segment.body_id != biography.body_id {
            return Err("biography archive belongs to another Body".into());
        }
        let prior = if index > 0 {
            Some(&segments[index - 1])
        } else {
            None
        };
        if let Some(prior) = prior {
            if segment.ordinal.checked_sub(1) != Some(prior.ordinal)
                || segment.previous_digest != Some(prior.digest)
            {
                return Err("biography archive transaction breaks its digest chain".into());
            }
        } else if segment.ordinal == 1 {
            if segment.previous_digest.is_some() {
                return Err("first biography archive has a predecessor".into());
            }
        } else {
            let prior = read_archive(root, segment.ordinal - 1)?;
            if prior.body_id != biography.body_id || segment.previous_digest != Some(prior.digest) {
                return Err("biography archive predecessor differs".into());
            }
        }
    }
    for segment in segments {
        let bytes = serde_json::to_vec_pretty(segment).map_err(|error| error.to_string())?;
        if bytes.len() as u64 > MAXIMUM_STATE / 2 {
            return Err("biography archive segment exceeds its selected storage bound".into());
        }
    }
    Ok(())
}

pub(super) fn verify_retained_archive(root: &Path) -> Result<(), String> {
    let path = root.join("body/biography.json");
    if !path.exists() {
        return Ok(());
    }
    let biography: BodyBiographyEvidence =
        serde_json::from_slice(&bounded_read(&path, MAXIMUM_STATE)?)
            .map_err(|error| format!("retained biography archive basis: {error}"))?;
    biography
        .validate()
        .map_err(|error| format!("retained biography archive basis invalid: {error:?}"))?;
    let Some(summary) = &biography.compaction else {
        return Ok(());
    };
    if summary.sealed_segments == 0 {
        return Ok(());
    }
    let mut expected_digest = summary.archive_head_digest;
    for ordinal in (1..=summary.sealed_segments).rev() {
        let segment = read_archive(root, ordinal)?;
        if segment.body_id != biography.body_id || Some(segment.digest) != expected_digest {
            return Err("retained biography archive chain differs".into());
        }
        if ordinal == summary.sealed_segments {
            segment
                .validate_as_head_of(&biography)
                .map_err(|error| format!("retained biography archive head invalid: {error:?}"))?;
        }
        expected_digest = segment.previous_digest;
    }
    if expected_digest.is_some() {
        return Err("retained biography archive first segment has a predecessor".into());
    }
    Ok(())
}
