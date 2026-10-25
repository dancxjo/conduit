//! Interpret the existing bounded native media module without inventing birth
//! or join. Invitation media retains its existing decoder and authority path.
use crate::make::MakeRecord;
use conduit_body::{PREPARED_BIRTH_MEDIA_MAGIC, PreparedBirthHostRole, PreparedBirthMedia};

/// This is only a discriminator. A matching prefix never proves validity.
pub fn is_prepared_birth(region: &[u8]) -> bool {
    region.starts_with(PREPARED_BIRTH_MEDIA_MAGIC)
}

/// Validate retained construction material against this actual compiled image.
/// Admission of the initialized current Host occurs separately in the Journey.
pub fn inspect(
    region: &[u8],
    make: &MakeRecord,
) -> Result<Option<PreparedBirthMedia>, &'static str> {
    if !is_prepared_birth(region) {
        return Ok(None);
    }
    let media = PreparedBirthMedia::decode(region).map_err(|_| "prepared-birth-media-invalid")?;
    let binding = media.binding();
    if binding.target != make.target
        || binding.profile_id != make.profile_id
        || binding.build_id != make.build_id
    {
        return Err("prepared-birth-image-binding-mismatch");
    }
    if media.role() != PreparedBirthHostRole::FirstHost {
        return Err("prepared-birth-awaiting-first-host-and-ordinary-join");
    }
    Ok(Some(media))
}

/// The image's existing fixed native-media slot carries exactly one variant:
/// an ordinary invitation or a retained birth. Neither variant is auto-trusted.
pub fn read_current(make: &MakeRecord) -> Result<Option<PreparedBirthMedia>, &'static str> {
    #[cfg(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "riscv64",
        target_arch = "loongarch64"
    ))]
    {
        crate::boot::spore_module().map_or(Ok(None), |region| inspect(region, make))
    }
    #[cfg(not(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "riscv64",
        target_arch = "loongarch64"
    )))]
    {
        let _ = make;
        Err("prepared-birth-boot-loader-unsupported")
    }
}

#[cfg(test)]
mod tests;
