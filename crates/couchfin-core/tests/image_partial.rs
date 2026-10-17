//! Partial image data is never decoded into pixels, cached, or returned: it is
//! an error, and the next request fetches the image again.

mod image_support;

use std::time::Duration;

use couchfin_api::{ImageKind, ImageRequest, ItemId, JellyfinClient, Session};
use couchfin_core::{CacheDir, ImageCache, ImageCacheConfig, ImageError, ImageLoad};
use image::ImageFormat;
use image_support::{cache_in, encoded, log_in, poster, server};
use tempfile::{TempDir, tempdir};
use wiremock::matchers::{method, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TOKEN: &str = "0123456789abcdef0123456789abcdef";
const ITEM: &str = "a1b2c3d4e5f60718293a4b5c6d7e8f90";

fn content_type(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Jpeg => "image/jpeg",
        ImageFormat::WebP => "image/webp",
        _ => "image/png",
    }
}

fn cache_files(dir: &TempDir) -> usize {
    std::fs::read_dir(dir.path()).map_or(0, Iterator::count)
}

fn request() -> ImageRequest {
    ImageRequest::new(
        ITEM.parse::<ItemId>().expect("valid id"),
        ImageKind::Primary,
        300,
    )
}

/// A real client and cache against `server`.
fn real_cache(server: &MockServer, dir: &TempDir) -> (ImageCache<JellyfinClient>, Session) {
    let client = JellyfinClient::new(
        "3f2b9c1e-8d4a-4b6f-9e2d-7a1c5b8e0f42"
            .parse()
            .expect("valid"),
    )
    .expect("client builds");
    let session = Session::new(
        server.uri().parse().expect("valid"),
        couchfin_api::AccessToken::new(TOKEN).expect("token is valid"),
    );
    let cache = ImageCache::new(
        client,
        &CacheDir::new(dir.path()),
        ImageCacheConfig::default(),
    );
    (cache, session)
}

async fn serve_once(server: &MockServer, format: ImageFormat, body: Vec<u8>) {
    Mock::given(method("GET"))
        .and(path_regex("/Images/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, content_type(format)))
        .up_to_n_times(1)
        .mount(server)
        .await;
}

#[tokio::test]
async fn a_truncated_image_is_an_error_is_not_cached_and_is_retried() {
    for format in [ImageFormat::Jpeg, ImageFormat::WebP, ImageFormat::Png] {
        let full = encoded(format);
        // Cut at several places: the decoders fail or pad differently on each.
        for tenths in [2, 5, 8, 9] {
            let cut = full.len() * tenths / 10;
            let server = MockServer::start().await;
            let dir = tempdir().unwrap();
            let (cache, session) = real_cache(&server, &dir);
            serve_once(&server, format, full[..cut].to_vec()).await;

            let result = cache.load(&session, &request()).await;

            assert!(
                matches!(result, Err(ImageError::Decode { .. })),
                "{format:?} cut at {cut}/{}: {result:?}",
                full.len()
            );
            assert_eq!(cache_files(&dir), 0, "{format:?} cut at {cut}");

            // The server has the whole image by the next request.
            serve_once(&server, format, full.clone()).await;
            let retried = cache.load(&session, &request()).await;
            let Ok(ImageLoad::Ready(pixels)) = retried else {
                panic!("{format:?}: the retry should succeed, got {retried:?}");
            };
            assert_eq!((pixels.width(), pixels.height()), (64, 96));
            assert_eq!(cache_files(&dir), 1);
        }
    }
}

#[tokio::test]
async fn a_jpeg_missing_only_its_last_bytes_is_still_partial() {
    let full = encoded(ImageFormat::Jpeg);
    let api = server(1);
    api.serve(full[..full.len() - 2].to_vec());
    let session = log_in(&api).await;
    let request = poster(&api, &session, 0).await;
    let dir = tempdir().unwrap();
    let cache = cache_in(&api, &dir, ImageCacheConfig::default());

    let result = cache.load(&session, &request).await;

    assert!(
        matches!(result, Err(ImageError::Decode { .. })),
        "{result:?}"
    );
    assert_eq!(cache_files(&dir), 0);
}

#[tokio::test]
async fn an_image_wider_than_requested_is_rejected() {
    let api = server(1);
    api.serve(encoded(ImageFormat::Png));
    let session = log_in(&api).await;
    let request = poster(&api, &session, 0).await;
    let narrow = ImageRequest {
        max_width: 32,
        ..request
    };
    let dir = tempdir().unwrap();
    let cache = cache_in(&api, &dir, ImageCacheConfig::default());

    let result = cache.load(&session, &narrow).await;

    assert!(
        matches!(result, Err(ImageError::Decode { .. })),
        "{result:?}"
    );
    assert_eq!(cache_files(&dir), 0);
}

#[tokio::test]
async fn a_truncated_file_in_the_disk_cache_is_discarded_and_fetched_again() {
    let api = server(1);
    let full = encoded(ImageFormat::Jpeg);
    api.serve(full.clone());
    let session = log_in(&api).await;
    let request = poster(&api, &session, 0).await;
    let dir = tempdir().unwrap();
    cache_in(&api, &dir, ImageCacheConfig::default())
        .load(&session, &request)
        .await
        .unwrap();
    let file = std::fs::read_dir(dir.path())
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    std::fs::write(&file, &full[..full.len() / 2]).unwrap();

    // A fresh cache, as after a restart, so memory can't answer.
    let cache = cache_in(&api, &dir, ImageCacheConfig::default());
    let load = cache.load(&session, &request).await.unwrap();

    assert!(matches!(load, ImageLoad::Ready(_)));
    assert_eq!(api.image_calls(), 2);
    assert_eq!(std::fs::read(&file).unwrap(), full);
}

#[tokio::test]
async fn a_load_cancelled_mid_download_leaves_nothing_behind() {
    let api = server(1);
    api.serve(encoded(ImageFormat::Jpeg));
    api.delay_images(Duration::from_millis(300));
    let session = log_in(&api).await;
    let request = poster(&api, &session, 0).await;
    let dir = tempdir().unwrap();
    let cache = cache_in(&api, &dir, ImageCacheConfig::default());

    let cancelled =
        tokio::time::timeout(Duration::from_millis(50), cache.load(&session, &request)).await;
    assert!(cancelled.is_err(), "the load should still be running");
    assert_eq!(cache_files(&dir), 0);

    api.delay_images(Duration::ZERO);
    let load = cache.load(&session, &request).await.unwrap();
    assert!(matches!(load, ImageLoad::Ready(_)));
    assert_eq!(cache_files(&dir), 1);
}

#[tokio::test]
async fn a_real_download_cancelled_mid_flight_is_retried() {
    let server = MockServer::start().await;
    let dir = tempdir().unwrap();
    let (cache, session) = real_cache(&server, &dir);
    let full = encoded(ImageFormat::Jpeg);
    Mock::given(method("GET"))
        .and(path_regex("/Images/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw(full.clone(), "image/jpeg")
                .set_delay(Duration::from_millis(300)),
        )
        .up_to_n_times(1)
        .mount(&server)
        .await;
    let cancelled =
        tokio::time::timeout(Duration::from_millis(50), cache.load(&session, &request())).await;
    assert!(cancelled.is_err());
    assert_eq!(cache_files(&dir), 0);

    serve_once(&server, ImageFormat::Jpeg, full).await;
    let load = cache.load(&session, &request()).await.unwrap();

    assert!(matches!(load, ImageLoad::Ready(_)));
}
