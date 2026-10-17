//! `ImageBlurHashes` in the item DTO: found by image kind and tag.

use couchfin_api::{BlurHash, ImageKind, Item};
use serde_json::json;

const HASH: &str = "LEHV6nWB2yk8pyo0adR*.7kCMdnj";

fn parse(value: serde_json::Value) -> Item {
    serde_json::from_value(value).expect("item parses")
}

#[test]
fn blur_hashes_are_found_by_image_kind_and_tag() {
    let item = parse(json!({
        "Id": "abc", "Name": "Hashed", "Type": "Movie",
        "ImageTags": { "Primary": "ptag", "Thumb": "ttag" },
        "BackdropImageTags": ["btag"],
        "ImageBlurHashes": {
            "Primary": { "ptag": HASH },
            "Backdrop": { "btag": HASH, "old": "x" }
        }
    }));
    let tags = &item.image_tags;

    let primary = tags.primary.as_ref().expect("primary tag");
    assert_eq!(
        item.blur_hashes
            .get(ImageKind::Primary, primary)
            .map(BlurHash::as_str),
        Some(HASH)
    );
    let backdrop = tags.backdrop.as_ref().expect("backdrop tag");
    assert!(
        item.blur_hashes
            .get(ImageKind::Backdrop, backdrop)
            .is_some()
    );
    // The thumb has a tag but no hash; a hash under another kind is no match.
    let thumb = tags.thumb.as_ref().expect("thumb tag");
    assert!(item.blur_hashes.get(ImageKind::Thumb, thumb).is_none());
    assert!(item.blur_hashes.get(ImageKind::Thumb, primary).is_none());
}

#[test]
fn malformed_blur_hashes_are_dropped_not_fatal() {
    let item = parse(json!({
        "Id": "abc", "Name": "Odd", "Type": "Movie",
        "ImageTags": { "Primary": "ptag" },
        "ImageBlurHashes": { "Primary": { "ptag": "not a hash!" }, "Thumb": null }
    }));

    let primary = item.image_tags.primary.as_ref().expect("primary tag");
    assert!(item.blur_hashes.get(ImageKind::Primary, primary).is_none());
}

#[test]
fn libraries_carry_their_covers_hash() {
    let library: couchfin_api::Library = serde_json::from_value(json!({
        "Id": "abc", "Name": "Movies", "CollectionType": "movies",
        "ImageTags": { "Primary": "ptag" },
        "ImageBlurHashes": { "Primary": { "ptag": HASH } }
    }))
    .expect("library parses");

    assert_eq!(
        library.primary_blur_hash.as_ref().map(BlurHash::as_str),
        Some(HASH)
    );
}

#[test]
fn blur_hash_validation() {
    assert!(HASH.parse::<BlurHash>().is_ok());
    for bad in [
        "",
        "short",
        "has space in it here",
        "LEHV6nWB2yk8pyo0adR*.7kCMdn",
    ] {
        assert!(bad.parse::<BlurHash>().is_err(), "{bad:?}");
    }
}
