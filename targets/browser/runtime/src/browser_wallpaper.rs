//! Prepare the checked wallpaper observation from Browser Host metadata.
use conduit_core::{
    validate_canonical_structured_value as canonical, BoundedResourceRef,
    PreparedStructuredComposer, ResourceExtent, ResourceLifetime, ResourceSemanticIdentity,
    ResourceVersionIdentity, StructuredInfoTypeShape, StructuredInfoValue,
};
use conduit_face::{ImageColorProfile, ImageFormat, ImagePixelExtent, ImageResource};
use conduit_semantic_catalog::{
    prepare_ambient_display_types, PreparedWallpaperSelection, WallpaperImageMetadata,
    RESIDENT_WALLPAPER_BYTES, WALLPAPER_DECK_LEN,
};
use serde::Deserialize;

const MAXIMUM_PREPARATION_BYTES: usize = 12 * 1024;
const PROFILE: &str = "media/image-rgba8@1";
const ACCESS: &str = "conduit.resource/image-content@1";

#[derive(Deserialize)]
#[serde(untagged)]
enum Deck {
    Ready {
        metadata: [Photo; WALLPAPER_DECK_LEN],
    },
    Unavailable {
        reason: Reason,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Reason {
    ProviderRateLimited,
    ProviderUnavailable,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Photo {
    reference: Reference,
    title: String,
    caption: String,
    attribution: String,
    license: String,
    source: String,
    image_frame: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Reference {
    identity: String,
    version: String,
    width: u64,
    height: u64,
    bytes: u64,
    content_profile: String,
    access_class: String,
}

/// One selected Browser Wallpaper Back can own a prepared observation.
pub struct BrowserWallpaperObservation {
    selection: Option<PreparedWallpaperSelection>,
    unavailable: Box<[u8]>,
}

impl BrowserWallpaperObservation {
    pub fn prepare(metadata: &[u8]) -> Result<Self, String> {
        if metadata.len() > MAXIMUM_PREPARATION_BYTES {
            return Err("browser wallpaper preparation exceeds its metadata bound".into());
        }
        let deck: Deck = serde_json::from_slice(metadata).map_err(debug_error)?;
        let types = prepare_ambient_display_types()?;
        let named = |name: &str| {
            types
                .native_types
                .iter()
                .find(|ty| ty.name == name)
                .ok_or_else(|| format!("missing wallpaper Type {name}"))
        };
        let observation = &named("AmbientWallpaperObservation")?.value_type;
        let (selection, reason) = match deck {
            Deck::Ready { metadata } => {
                let ty = &named("AmbientWallpaperImage")?.value_type;
                let mut values = Vec::with_capacity(WALLPAPER_DECK_LEN);
                let mut identities = [[0; 32]; WALLPAPER_DECK_LEN];
                for (index, photo) in metadata.into_iter().enumerate() {
                    let identity = digest(&photo.reference.identity)?;
                    if identities[..index].contains(&identity) {
                        return Err("browser wallpaper deck repeats an image identity".into());
                    }
                    identities[index] = identity;
                    let image = photo.reference.into_image(identity, photo.image_frame)?;
                    values.push(
                        WallpaperImageMetadata {
                            title: &photo.title,
                            caption: &photo.caption,
                            attribution: &photo.attribution,
                            license: &photo.license,
                            source: &photo.source,
                        }
                        .compose(ty, image)
                        .map_err(debug_error)?,
                    );
                }
                let values: [StructuredInfoValue; WALLPAPER_DECK_LEN] = values
                    .try_into()
                    .map_err(|_| "browser wallpaper deck has the wrong length")?;
                (
                    Some(PreparedWallpaperSelection::prepare(values, 300).map_err(debug_error)?),
                    "Photograph selection unavailable",
                )
            }
            Deck::Unavailable {
                reason: Reason::ProviderRateLimited,
            } => (None, "The photograph provider rate-limited acquisition"),
            Deck::Unavailable {
                reason: Reason::ProviderUnavailable,
            } => (None, "The photograph provider did not supply a usable deck"),
        };
        let StructuredInfoTypeShape::Variant { cases, .. } = observation.shape() else {
            return Err("expected wallpaper observation cases".into());
        };
        let payload = cases
            .iter()
            .find(|case| case.tag() == "unavailable")
            .ok_or("missing wallpaper absence case")?
            .payload_type();
        let mut message = PreparedStructuredComposer::new(payload, 256).map_err(debug_error)?;
        let message = canonical(message.leaf(reason.as_bytes()).map_err(debug_error)?)
            .map_err(debug_error)?;
        let mut output =
            PreparedStructuredComposer::new(observation, RESIDENT_WALLPAPER_BYTES as usize)
                .map_err(debug_error)?;
        let unavailable = output
            .variant("unavailable", message)
            .map_err(debug_error)?
            .into();
        Ok(Self {
            selection,
            unavailable,
        })
    }

    pub fn prepared_heap_bytes(&self) -> usize {
        self.selection
            .as_ref()
            .map_or(0, PreparedWallpaperSelection::prepared_heap_bytes)
            + self.unavailable.len()
    }

    pub fn observe_at(&mut self, unix_millis: u64) -> &[u8] {
        self.selection
            .as_mut()
            .and_then(|selection| selection.encode(unix_millis).ok())
            .unwrap_or(&self.unavailable)
    }
}

impl Reference {
    fn into_image(self, identity: [u8; 32], frame: String) -> Result<ImageResource, String> {
        if !(1..=1024).contains(&self.width)
            || !(1..=600).contains(&self.height)
            || self.width * self.height * 4 != self.bytes
            || self.content_profile != PROFILE
            || self.access_class != ACCESS
        {
            return Err("browser wallpaper reference exceeds its prepared pixel contract".into());
        }
        ImageResource::new(
            ImageColorProfile::absent(),
            BoundedResourceRef {
                identity: ResourceSemanticIdentity::from_digest(identity),
                content_profile: PROFILE.into(),
                access_class: ACCESS.into(),
                extent: ResourceExtent {
                    bytes: self.bytes,
                    items: Some(1),
                },
                lifetime: ResourceLifetime {
                    version: ResourceVersionIdentity::from_digest(digest(&self.version)?),
                    expires_at: None,
                },
            },
            ImagePixelExtent::new(self.height, self.width).map_err(debug_error)?,
            ImageFormat::rgba8(),
            frame,
        )
        .map_err(debug_error)
    }
}

fn digest(hex: &str) -> Result<[u8; 32], String> {
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("browser wallpaper resource digest is not canonical".into());
    }
    let mut digest = [0; 32];
    for (index, byte) in digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).map_err(debug_error)?;
    }
    Ok(digest)
}

fn debug_error(error: impl core::fmt::Debug) -> String {
    format!("browser wallpaper preparation: {error:?}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn prepared_browser_deck_rotates_and_rejects_wrong_pixel_extent() {
        let photos: [_; WALLPAPER_DECK_LEN] = std::array::from_fn(|index| {
            json!({
                "reference": {
                    "identity": format!("{:064x}", index + 1),
                    "version": format!("{:064x}", index + 10),
                    "width": 2, "height": 1, "bytes": 8,
                    "contentProfile": PROFILE, "accessClass": ACCESS
                },
                "title": format!("Photo {index}"), "caption": "", "attribution": "Observer",
                "license": "License not supplied by provider",
                "source": format!("NASA APOD/example-{index}"),
                "imageFrame": format!("apod/example-{index}")
            })
        });
        let mut request = json!({ "metadata": photos });
        let mut observation =
            BrowserWallpaperObservation::prepare(&serde_json::to_vec(&request).unwrap()).unwrap();
        let first = observation.observe_at(1_000).to_vec();
        assert_eq!(
            canonical(&first).unwrap().variant_tag().unwrap(),
            "available"
        );
        assert_eq!(observation.observe_at(300_999), first);
        assert_ne!(observation.observe_at(301_000), first);
        request["metadata"][0]["reference"]["bytes"] = json!(7);
        assert!(
            BrowserWallpaperObservation::prepare(&serde_json::to_vec(&request).unwrap()).is_err()
        );
    }

    #[test]
    fn unavailable_provider_stays_a_typed_observation() {
        let mut observation =
            BrowserWallpaperObservation::prepare(br#"{"reason":"provider-rate-limited"}"#).unwrap();
        assert_eq!(
            canonical(observation.observe_at(0))
                .unwrap()
                .variant_tag()
                .unwrap(),
            "unavailable"
        );
    }
}
