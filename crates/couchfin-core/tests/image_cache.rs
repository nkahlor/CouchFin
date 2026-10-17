//! `ImageCache`: fetching, decoding, the blurred variant, and both caches.

mod image_support;

use couchfin_api::{ImageKind, ImageRequest, ItemId};
use couchfin_core::{ApiError, ImageCacheConfig, ImageError, ImageLoad, Pixels, Variant};
use image::ImageFormat;
use image_support::{cache_in, checkerboard, encoded, log_in, poster, server};
use tempfile::tempdir;

fn ready(load: ImageLoad) -> Pixels {
    match load {
        ImageLoad::Ready(pixels) => pixels,
        ImageLoad::NoImage => panic!("expected pixels, got no image"),
    }
}

#[tokio::test]
async fn fetches_and_decodes_the_fakes_png() {
    let api = server(1);
    let session = log_in(&api).await;
    let request = poster(&api, &session, 0).await;
    let dir = tempdir().unwrap();
    let cache = cache_in(&api, &dir, ImageCacheConfig::default());

    let pixels = ready(
        cache
            .load(&session, &request, Variant::Sharp)
            .await
            .unwrap(),
    );

    assert_eq!((pixels.width(), pixels.height()), (1, 1));
    assert_eq!(pixels.rgba().len(), 4);
    assert_eq!(api.image_calls(), 1);
}

#[tokio::test]
async fn decodes_jpeg_png_and_webp_to_the_same_size() {
    for format in [ImageFormat::Jpeg, ImageFormat::Png, ImageFormat::WebP] {
        let api = server(1);
        api.serve(encoded(format));
        let session = log_in(&api).await;
        let request = poster(&api, &session, 0).await;
        let dir = tempdir().unwrap();
        let cache = cache_in(&api, &dir, ImageCacheConfig::default());

        let pixels = ready(
            cache
                .load(&session, &request, Variant::Sharp)
                .await
                .unwrap(),
        );

        assert_eq!((pixels.width(), pixels.height()), (64, 96), "{format:?}");
    }
}

#[tokio::test]
async fn the_blurred_variant_differs_but_has_the_same_size() {
    let api = server(1);
    api.serve(encoded(ImageFormat::Png));
    let session = log_in(&api).await;
    let request = poster(&api, &session, 0).await;
    let dir = tempdir().unwrap();
    let cache = cache_in(&api, &dir, ImageCacheConfig::default());

    let sharp = ready(
        cache
            .load(&session, &request, Variant::Sharp)
            .await
            .unwrap(),
    );
    let blurred = ready(
        cache
            .load(&session, &request, Variant::Blurred)
            .await
            .unwrap(),
    );

    assert_eq!(sharp.rgba(), checkerboard().as_raw().as_slice());
    assert_eq!(
        (blurred.width(), blurred.height()),
        (sharp.width(), sharp.height())
    );
    assert_ne!(blurred.rgba(), sharp.rgba());
    assert_eq!(api.image_calls(), 1, "the blur reuses the fetched original");
}

#[tokio::test]
async fn a_second_request_is_served_from_memory() {
    let api = server(1);
    let session = log_in(&api).await;
    let request = poster(&api, &session, 0).await;
    let dir = tempdir().unwrap();
    let cache = cache_in(&api, &dir, ImageCacheConfig::default());

    let first = ready(
        cache
            .load(&session, &request, Variant::Sharp)
            .await
            .unwrap(),
    );
    let second = ready(
        cache
            .load(&session, &request, Variant::Sharp)
            .await
            .unwrap(),
    );

    assert_eq!(first, second);
    assert_eq!(api.image_calls(), 1);
}

#[tokio::test]
async fn after_a_restart_both_variants_come_from_disk() {
    let api = server(1);
    api.serve(encoded(ImageFormat::Jpeg));
    let session = log_in(&api).await;
    let request = poster(&api, &session, 0).await;
    let dir = tempdir().unwrap();
    let before = cache_in(&api, &dir, ImageCacheConfig::default());
    let sharp = ready(
        before
            .load(&session, &request, Variant::Sharp)
            .await
            .unwrap(),
    );
    let blurred = ready(
        before
            .load(&session, &request, Variant::Blurred)
            .await
            .unwrap(),
    );
    drop(before);

    let after = cache_in(&api, &dir, ImageCacheConfig::default());
    let sharp_again = ready(
        after
            .load(&session, &request, Variant::Sharp)
            .await
            .unwrap(),
    );
    let blurred_again = ready(
        after
            .load(&session, &request, Variant::Blurred)
            .await
            .unwrap(),
    );

    assert_eq!((sharp_again, blurred_again), (sharp, blurred));
    assert_eq!(api.image_calls(), 1);
}

#[tokio::test]
async fn a_blurred_request_after_a_restart_needs_no_fetch_even_without_the_blur_file() {
    let api = server(1);
    let session = log_in(&api).await;
    let request = poster(&api, &session, 0).await;
    let dir = tempdir().unwrap();
    let before = cache_in(&api, &dir, ImageCacheConfig::default());
    before
        .load(&session, &request, Variant::Sharp)
        .await
        .unwrap();
    drop(before);

    let after = cache_in(&api, &dir, ImageCacheConfig::default());
    let blurred = after
        .load(&session, &request, Variant::Blurred)
        .await
        .unwrap();

    assert!(matches!(blurred, ImageLoad::Ready(_)));
    assert_eq!(api.image_calls(), 1);
}

#[tokio::test]
async fn a_new_tag_is_a_new_image() {
    let api = server(1);
    let session = log_in(&api).await;
    let request = poster(&api, &session, 0).await;
    let changed = ImageRequest {
        tag: Some("changed-on-the-server".parse().unwrap()),
        ..request.clone()
    };
    let dir = tempdir().unwrap();
    let cache = cache_in(&api, &dir, ImageCacheConfig::default());

    cache
        .load(&session, &request, Variant::Sharp)
        .await
        .unwrap();
    cache
        .load(&session, &changed, Variant::Sharp)
        .await
        .unwrap();
    cache
        .load(&session, &changed, Variant::Sharp)
        .await
        .unwrap();

    assert_eq!(api.image_calls(), 2);
}

#[tokio::test]
async fn width_and_kind_are_part_of_the_key() {
    let api = server(1);
    let session = log_in(&api).await;
    let request = poster(&api, &session, 0).await;
    let wider = ImageRequest {
        max_width: 600,
        ..request.clone()
    };
    let dir = tempdir().unwrap();
    let cache = cache_in(&api, &dir, ImageCacheConfig::default());

    cache
        .load(&session, &request, Variant::Sharp)
        .await
        .unwrap();
    cache.load(&session, &wider, Variant::Sharp).await.unwrap();

    assert_eq!(api.image_calls(), 2);
}

#[tokio::test]
async fn an_item_without_the_image_is_no_image_not_an_error() {
    let api = server(1);
    let session = log_in(&api).await;
    let missing = ImageRequest::new(
        "00000000000000000000000000000000"
            .parse::<ItemId>()
            .unwrap(),
        ImageKind::Primary,
        300,
    );
    let dir = tempdir().unwrap();
    let cache = cache_in(&api, &dir, ImageCacheConfig::default());

    let sharp = cache.load(&session, &missing, Variant::Sharp).await;
    let blurred = cache.load(&session, &missing, Variant::Blurred).await;

    assert_eq!(sharp, Ok(ImageLoad::NoImage));
    assert_eq!(blurred, Ok(ImageLoad::NoImage));
}

#[tokio::test]
async fn other_api_errors_are_errors() {
    let api = server(1);
    let session = log_in(&api).await;
    let request = poster(&api, &session, 0).await;
    api.fake.fail_with(ApiError::InvalidCredentials);
    let dir = tempdir().unwrap();
    let cache = cache_in(&api, &dir, ImageCacheConfig::default());

    let result = cache.load(&session, &request, Variant::Sharp).await;

    assert_eq!(result, Err(ImageError::Api(ApiError::InvalidCredentials)));
}

#[tokio::test]
async fn bytes_that_are_not_an_image_are_a_decode_error_and_are_not_cached() {
    let api = server(1);
    api.serve(b"<html>not an image</html>".to_vec());
    let session = log_in(&api).await;
    let request = poster(&api, &session, 0).await;
    let dir = tempdir().unwrap();
    let cache = cache_in(&api, &dir, ImageCacheConfig::default());

    let result = cache.load(&session, &request, Variant::Sharp).await;

    assert!(
        matches!(result, Err(ImageError::Decode { .. })),
        "{result:?}"
    );
    assert_eq!(std::fs::read_dir(dir.path()).map_or(0, Iterator::count), 0);
}

#[tokio::test]
async fn clearing_empties_both_caches() {
    let api = server(1);
    let session = log_in(&api).await;
    let request = poster(&api, &session, 0).await;
    let dir = tempdir().unwrap();
    let cache = cache_in(&api, &dir, ImageCacheConfig::default());
    cache
        .load(&session, &request, Variant::Sharp)
        .await
        .unwrap();

    cache.clear().await.unwrap();
    cache
        .load(&session, &request, Variant::Sharp)
        .await
        .unwrap();

    assert_eq!(api.image_calls(), 2);
}
