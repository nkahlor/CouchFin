//! Stable request sizes, BlurHash placeholders and the in-flight limit
//! (decision 016).

mod image_support;

use std::time::Duration;

use couchfin_api::{BlurHash, ImageKind};
use couchfin_core::{
    ImageCacheConfig, ImageLoad, ImageRef, ImageShape, ImageSize, LibraryFlow, ScreenState,
    placeholder,
};
use image_support::{cache_in, log_in, poster, server};
use tempfile::tempdir;

const HASH: &str = "LEHV6nWB2yk8pyo0adR*.7kCMdnj";

#[test]
fn sizes_are_fixed_per_shape_and_snap_to_a_few_scales() {
    let at = |shape, scale| ImageSize::of(shape, scale);
    let size = |width, height| ImageSize { width, height };

    assert_eq!(at(ImageShape::Poster, 1.0), size(280, 420));
    assert_eq!(at(ImageShape::Thumb, 1.0), size(448, 252));
    assert_eq!(at(ImageShape::Backdrop, 1.0), size(1280, 720));
    assert_eq!(at(ImageShape::Poster, 1.5), size(420, 630));
    assert_eq!(at(ImageShape::Poster, 2.0), size(560, 840));
    // In between rounds up to the next step; beyond the last stays there.
    assert_eq!(at(ImageShape::Poster, 1.2), at(ImageShape::Poster, 1.5));
    assert_eq!(at(ImageShape::Poster, 1.75), at(ImageShape::Poster, 2.0));
    assert_eq!(at(ImageShape::Poster, 3.0), at(ImageShape::Poster, 2.0));
    assert_eq!(at(ImageShape::Poster, 0.5), at(ImageShape::Poster, 1.0));
    // Nonsense scales don't panic or give an empty box.
    assert_eq!(
        at(ImageShape::Poster, f32::NAN),
        at(ImageShape::Poster, 1.0)
    );
    assert_eq!(at(ImageShape::Poster, -1.0), at(ImageShape::Poster, 1.0));
}

#[test]
fn an_image_ref_requests_its_size_as_a_box_to_fill() {
    let image = ImageRef {
        item: "abc".parse().unwrap(),
        kind: ImageKind::Primary,
        tag: "t".parse().unwrap(),
    };

    let request = image.request(ImageSize::of(ImageShape::Poster, 1.0));

    assert_eq!((request.max_width, request.fill_height), (280, Some(420)));
    assert_eq!(request.tag, Some("t".parse().unwrap()));
}

#[test]
fn a_hash_decodes_to_a_small_picture_of_the_shapes_proportions() {
    let hash: BlurHash = HASH.parse().unwrap();

    let poster = placeholder(&hash, ImageShape::Poster).expect("decodes");
    let thumb = placeholder(&hash, ImageShape::Thumb).expect("decodes");

    assert_eq!((poster.width(), poster.height()), (24, 36));
    assert_eq!((thumb.width(), thumb.height()), (32, 18));
    assert_eq!(poster.rgba().len(), 24 * 36 * 4);
    // Opaque, and not one flat colour.
    assert!(poster.rgba().chunks(4).all(|px| px[3] == 255));
    assert!(poster.rgba().chunks(4).any(|px| px != &poster.rgba()[..4]));
}

#[tokio::test]
async fn home_cards_carry_a_placeholder_when_the_server_sent_a_hash() {
    let api = server(5);
    let session = log_in(&api).await;
    let flow = LibraryFlow::new(api.clone(), session);
    flow.start().await;

    let state = flow.state();
    let ScreenState::Home(home) = state.current() else {
        panic!("Home is on top");
    };
    let cards: Vec<_> = home.rows.iter().flat_map(|row| &row.cards).collect();
    assert!(cards.len() > 1, "{cards:?}");
    for card in cards {
        assert_eq!(card.placeholder.is_some(), card.image.is_some(), "{card:?}");
    }
}

#[test]
fn eight_downloads_run_at_once_by_default() {
    assert_eq!(ImageCacheConfig::default().max_in_flight, 8);
}

#[tokio::test]
async fn no_more_than_the_limit_run_at_once_and_the_rest_wait_in_order() {
    let api = server(30);
    api.delay_images(Duration::from_millis(30));
    let session = log_in(&api).await;
    let dir = tempdir().unwrap();
    let cache = std::sync::Arc::new(cache_in(&api, &dir, ImageCacheConfig::default()));
    let mut tasks = Vec::new();
    for n in 0..30 {
        let request = poster(&api, &session, n).await;
        let (cache, session) = (cache.clone(), session.clone());
        tasks.push(tokio::spawn(
            async move { cache.load(&session, &request).await },
        ));
    }

    for task in tasks {
        assert!(matches!(task.await.unwrap(), Ok(ImageLoad::Ready(_))));
    }

    assert_eq!(api.peak_in_flight(), 8);
}
