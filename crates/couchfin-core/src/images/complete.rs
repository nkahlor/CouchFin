//! Telling whole image files from partial ones.
//!
//! The `image` crate's JPEG decoder (zune-jpeg) does not fail on a file that
//! ends early: it fills the missing blocks with grey and reports success, so
//! a cut-off download shows as a poster that is half grey (#50). PNG and WebP
//! decoders do report truncation, but their cheap structural checks are kept
//! here too, so the rule "only whole files are decoded or cached" lives in
//! one place.

/// Why a file is not a whole image.
pub(super) fn incomplete(bytes: &[u8]) -> Option<&'static str> {
    match bytes {
        [0xFF, 0xD8, ..] => jpeg_incomplete(bytes),
        [b'R', b'I', b'F', b'F', ..] => webp_incomplete(bytes),
        _ => None,
    }
}

/// A JPEG ends with the end-of-image marker `FF D9`; encoders and some
/// servers may pad after it with zeros.
fn jpeg_incomplete(bytes: &[u8]) -> Option<&'static str> {
    let end = bytes
        .iter()
        .rposition(|&byte| byte != 0)
        .map_or(0, |i| i + 1);
    (!bytes[..end].ends_with(&[0xFF, 0xD9])).then_some("the JPEG ends before its end marker")
}

/// A WebP file's RIFF header states the length of the rest of the file.
fn webp_incomplete(bytes: &[u8]) -> Option<&'static str> {
    let stated = bytes.get(4..8)?;
    let stated = u64::from(u32::from_le_bytes([
        stated[0], stated[1], stated[2], stated[3],
    ]));
    (stated + 8 > bytes.len() as u64).then_some("the WebP is shorter than its RIFF header says")
}
