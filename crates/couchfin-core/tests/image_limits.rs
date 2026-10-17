//! `ImageCache` limits and failure handling: the size cap, damaged files,
//! shared fetches, the in-flight limit and dropped callers.

mod image_support;

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use couchfin_core::{ImageCacheConfig, ImageLoad};
use image_support::{cache_in, log_in, poster, server};
use tempfile::tempdir;

fn files_ending(dir: &Path, suffix: &str) -> Vec<std::path::PathBuf> {
    fs::read_dir(dir)
        .expect("directory is readable")
        .map(|entry| entry.expect("entry is readable").path())
        .filter(|path| path.to_string_lossy().ends_with(suffix))
        .collect()
}

fn dir_size(dir: &Path) -> u64 {
    fs::read_dir(dir)
        .expect("directory is readable")
        .map(|entry| entry.expect("entry").metadata().expect("metadata").len())
        .sum()
}

async fn pause() {
    // File modification times are what order the eviction.
    tokio::time::sleep(Duration::from_millis(20)).await;
}

#[tokio::test]
async fn the_size_cap_evicts_the_least_recently_used_file() {
    let api = server(3);
    let session = log_in(&api).await;
    let (a, b, c) = (
        poster(&api, &session, 0).await,
        poster(&api, &session, 1).await,
        poster(&api, &session, 2).await,
    );
    let dir = tempdir().unwrap();
    // No memory cache, so every request reaches the disk and refreshes it.
    let probe = cache_in(&api, &dir, ImageCacheConfig::default());
    probe.load(&session, &a).await.unwrap();
    let one = dir_size(dir.path());
    probe.clear().await.unwrap();
    let config = ImageCacheConfig::default()
        .with_memory_limit_bytes(0)
        .with_disk_limit_bytes(one * 2 + one / 2);
    let cache = cache_in(&api, &dir, config);
    let start = api.image_calls();

    cache.load(&session, &a).await.unwrap();
    pause().await;
    cache.load(&session, &b).await.unwrap();
    pause().await;
    cache.load(&session, &a).await.unwrap();
    pause().await;
    cache.load(&session, &c).await.unwrap();
    assert_eq!(api.image_calls() - start, 3, "a was served from disk");

    cache.load(&session, &a).await.unwrap();
    cache.load(&session, &c).await.unwrap();
    assert_eq!(api.image_calls() - start, 3, "a and c are still cached");
    cache.load(&session, &b).await.unwrap();
    assert_eq!(api.image_calls() - start, 4, "b was evicted");
    assert!(dir_size(dir.path()) <= one * 2 + one / 2);
}

#[tokio::test]
async fn a_corrupt_original_is_deleted_and_fetched_again() {
    let api = server(1);
    let session = log_in(&api).await;
    let request = poster(&api, &session, 0).await;
    let dir = tempdir().unwrap();
    let config = ImageCacheConfig::default().with_memory_limit_bytes(0);
    let cache = cache_in(&api, &dir, config);
    cache.load(&session, &request).await.unwrap();
    for file in files_ending(dir.path(), ".orig") {
        fs::write(file, b"garbage").unwrap();
    }

    let again = cache.load(&session, &request).await.unwrap();

    assert!(matches!(again, ImageLoad::Ready(_)));
    assert_eq!(api.image_calls(), 2);
    let repaired = cache.load(&session, &request).await.unwrap();
    assert!(matches!(repaired, ImageLoad::Ready(_)));
    assert_eq!(api.image_calls(), 2, "the refetched file is good");
}

#[tokio::test]
async fn concurrent_requests_for_one_image_share_one_fetch() {
    let api = server(1);
    api.delay_images(Duration::from_millis(100));
    let session = log_in(&api).await;
    let request = poster(&api, &session, 0).await;
    let dir = tempdir().unwrap();
    let cache = Arc::new(cache_in(&api, &dir, ImageCacheConfig::default()));

    let tasks: Vec<_> = (0..8)
        .map(|_| {
            let (cache, session, request) = (Arc::clone(&cache), session.clone(), request.clone());
            tokio::spawn(async move { cache.load(&session, &request).await })
        })
        .collect();
    for task in tasks {
        assert!(matches!(task.await.unwrap(), Ok(ImageLoad::Ready(_))));
    }

    assert_eq!(api.image_calls(), 1);
}

#[tokio::test]
async fn no_more_fetches_run_at_once_than_the_limit() {
    let api = server(12);
    api.delay_images(Duration::from_millis(40));
    let session = log_in(&api).await;
    let dir = tempdir().unwrap();
    let cache = Arc::new(cache_in(
        &api,
        &dir,
        ImageCacheConfig::default().with_max_in_flight(3),
    ));

    let mut tasks = Vec::new();
    for n in 0..12 {
        let request = poster(&api, &session, n).await;
        let (cache, session) = (Arc::clone(&cache), session.clone());
        tasks.push(tokio::spawn(
            async move { cache.load(&session, &request).await },
        ));
    }
    for task in tasks {
        assert!(matches!(task.await.unwrap(), Ok(ImageLoad::Ready(_))));
    }

    assert_eq!(api.image_calls(), 12);
    assert_eq!(api.peak_in_flight(), 3);
}

#[tokio::test]
async fn a_dropped_caller_stops_its_fetch_and_the_next_caller_starts_a_new_one() {
    let api = server(1);
    api.delay_images(Duration::from_millis(300));
    let session = log_in(&api).await;
    let request = poster(&api, &session, 0).await;
    let dir = tempdir().unwrap();
    let cache = Arc::new(cache_in(&api, &dir, ImageCacheConfig::default()));
    let task = {
        let (cache, session, request) = (Arc::clone(&cache), session.clone(), request.clone());
        tokio::spawn(async move { cache.load(&session, &request).await })
    };
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(api.in_flight(), 1);

    task.abort();
    let _ = task.await;

    assert_eq!(api.in_flight(), 0, "nothing keeps running for nobody");
    api.delay_images(Duration::ZERO);
    let next = cache.load(&session, &request).await.unwrap();
    assert!(matches!(next, ImageLoad::Ready(_)));
    assert_eq!(api.image_calls(), 2);
}

#[tokio::test]
async fn the_memory_cap_evicts_old_images_but_disk_still_has_them() {
    let api = server(2);
    let session = log_in(&api).await;
    let (a, b) = (
        poster(&api, &session, 0).await,
        poster(&api, &session, 1).await,
    );
    let dir = tempdir().unwrap();
    // One 1x1 image is 4 bytes: room for one.
    let cache = cache_in(
        &api,
        &dir,
        ImageCacheConfig::default().with_memory_limit_bytes(4),
    );

    cache.load(&session, &a).await.unwrap();
    cache.load(&session, &b).await.unwrap();
    cache.load(&session, &a).await.unwrap();
    cache.load(&session, &a).await.unwrap();

    assert_eq!(api.image_calls(), 2, "the re-read of a came from disk");
}
