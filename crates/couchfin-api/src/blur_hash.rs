//! BlurHash strings: the tiny placeholder Jellyfin stores for each image.

use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use crate::ids::ImageTag;
use crate::image::ImageKind;

/// The characters of BlurHash's base 83 alphabet.
const ALPHABET: &str =
    "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz#$%*+,-.:;=?@[]^_{|}~";

/// A string that isn't a BlurHash.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("not a BlurHash: {0}")]
pub struct InvalidBlurHash(&'static str);

/// A BlurHash: a short string describing a blurred version of an image,
/// shown while the image itself loads. It is checked for the base 83
/// alphabet and for the length its own header promises, so a decoder can
/// trust it.
///
/// # Examples
///
/// ```
/// use couchfin_api::BlurHash;
///
/// let hash: BlurHash = "LEHV6nWB2yk8pyo0adR*.7kCMdnj".parse()?;
/// assert_eq!(hash.as_str().len(), 28);
/// assert!("nonsense".parse::<BlurHash>().is_err());
/// # Ok::<(), couchfin_api::InvalidBlurHash>(())
/// ```
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct BlurHash(String);

impl BlurHash {
    /// The hash as Jellyfin sent it.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for BlurHash {
    type Err = InvalidBlurHash;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let digit = |c: char| ALPHABET.find(c);
        if text.len() < 6 || !text.chars().all(|c| digit(c).is_some()) {
            return Err(InvalidBlurHash("too short or not base 83"));
        }
        let size = text.chars().next().and_then(digit).unwrap_or(0);
        let (across, down) = (size % 9 + 1, size / 9 + 1);
        if text.len() != 4 + 2 * across * down {
            return Err(InvalidBlurHash("the length doesn't match the header"));
        }
        Ok(Self(text.to_owned()))
    }
}

impl fmt::Debug for BlurHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BlurHash({})", self.0)
    }
}

/// An item's BlurHashes, found by image kind and tag.
///
/// Jellyfin sends `{ "Primary": { "<tag>": "<hash>" }, ... }`; a hash that
/// doesn't validate is left out, since a placeholder is optional.
///
/// # Examples
///
/// ```
/// use couchfin_api::{ImageBlurHashes, ImageKind};
///
/// let hashes = ImageBlurHashes::default();
/// assert!(hashes.get(ImageKind::Primary, &"tag".parse()?).is_none());
/// # Ok::<(), couchfin_api::InvalidValueError>(())
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImageBlurHashes(HashMap<(ImageKind, ImageTag), BlurHash>);

/// The JSON shape: kind name, then tag, then hash.
pub(crate) type RawBlurHashes = HashMap<String, Option<HashMap<String, Option<String>>>>;

impl ImageBlurHashes {
    /// The hash of the image of `kind` with `tag`, if the server sent one.
    #[must_use]
    pub fn get(&self, kind: ImageKind, tag: &ImageTag) -> Option<&BlurHash> {
        self.0.get(&(kind, tag.clone()))
    }

    /// Adds a hash; the fake server uses it to give its items placeholders.
    pub(crate) fn with(mut self, kind: ImageKind, tag: ImageTag, hash: BlurHash) -> Self {
        self.0.insert((kind, tag), hash);
        self
    }

    pub(crate) fn from_raw(raw: &RawBlurHashes) -> Self {
        let mut hashes = HashMap::new();
        for kind in [ImageKind::Primary, ImageKind::Thumb, ImageKind::Backdrop] {
            let Some(Some(by_tag)) = raw.get(kind.as_str()) else {
                continue;
            };
            for (tag, hash) in by_tag {
                let parsed = tag.parse::<ImageTag>().ok().zip(
                    hash.as_deref()
                        .and_then(|hash| hash.parse::<BlurHash>().ok()),
                );
                if let Some((tag, hash)) = parsed {
                    hashes.insert((kind, tag), hash);
                }
            }
        }
        Self(hashes)
    }
}
