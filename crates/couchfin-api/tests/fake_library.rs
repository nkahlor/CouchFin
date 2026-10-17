//! The in-memory fake serves the library endpoints the way the client's
//! server does, so couchfin-core and the UI can test against it.

use std::time::Duration;

use couchfin_api::fake::FakeJellyfin;
use couchfin_api::{
    ApiError, ImageKind, ImageRequest, ItemKind, ItemsQuery, JellyfinApi, LibraryKind, Password,
    ServerUrl, Session,
};

fn url() -> ServerUrl {
    "http://fake.lan:8096".parse().expect("URL is valid")
}

fn fake() -> FakeJellyfin {
    FakeJellyfin::new(url())
        .with_user("alice", "hunter2")
        .with_library("Movies", LibraryKind::Movies)
        .with_movie("Movies", "Brazil")
        .with_movie("Movies", "arrival")
        .with_movie("Movies", "Casablanca")
        .with_library("Shows", LibraryKind::TvShows)
        .with_series("Shows", "Severance", 2, 3)
}

async fn log_in(fake: &FakeJellyfin) -> Session {
    let result = fake
        .authenticate_by_name(&url(), "alice", &Password::new("hunter2"))
        .await
        .expect("alice can log in");
    Session::new(url(), result.access_token)
}

#[tokio::test]
async fn lists_its_libraries_in_the_order_they_were_added() {
    let fake = fake();
    let session = log_in(&fake).await;

    let libraries = fake.libraries(&session).await.unwrap();

    let names: Vec<_> = libraries.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["Movies", "Shows"]);
    assert_eq!(libraries[0].kind, LibraryKind::Movies);
    assert_eq!(libraries[1].kind, LibraryKind::TvShows);
    assert_eq!(fake.library_named("Shows"), Some(libraries[1].clone()));
    assert_eq!(fake.library_named("Nope"), None);
}

#[tokio::test]
async fn pages_sorted_by_name_ignoring_case() {
    let fake = fake();
    let session = log_in(&fake).await;
    let movies = fake.library_named("Movies").expect("library exists");
    let query = ItemsQuery::default().in_parent(movies.id).page(1, 5);

    let page = fake.items(&session, &query).await.unwrap();

    let names: Vec<_> = page.items.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, ["Brazil", "Casablanca"]);
    assert_eq!(page.total, 3);
    assert_eq!(page.start_index, 1);
}

#[tokio::test]
async fn a_page_past_the_end_is_empty_but_keeps_the_total() {
    let fake = fake();
    let session = log_in(&fake).await;
    let movies = fake.library_named("Movies").expect("library exists");
    let query = ItemsQuery::default().in_parent(movies.id).page(10, 5);

    let page = fake.items(&session, &query).await.unwrap();

    assert_eq!(page.items, []);
    assert_eq!(page.total, 3);
}

#[tokio::test]
async fn recursive_and_kind_filters_work_like_the_server() {
    let fake = fake();
    let session = log_in(&fake).await;
    let shows = fake.library_named("Shows").expect("library exists");
    let direct = ItemsQuery::default().in_parent(shows.id.clone());
    let episodes = direct.clone().of_kinds([ItemKind::Episode]).recursive();
    let everything = direct.clone().recursive();

    let children = fake.items(&session, &direct).await.unwrap();
    let deep = fake.items(&session, &episodes).await.unwrap();
    let all = fake.items(&session, &everything).await.unwrap();

    assert_eq!(children.total, 1);
    assert_eq!(children.items[0].kind, ItemKind::Series);
    assert_eq!(deep.total, 6);
    assert!(deep.items.iter().all(|i| i.kind == ItemKind::Episode));
    assert_eq!(all.total, 1 + 2 + 6);
}

#[tokio::test]
async fn generates_a_large_library_with_stable_names() {
    let fake = FakeJellyfin::new(url())
        .with_user("alice", "hunter2")
        .with_generated_library("Big", 2_000);
    let session = log_in(&fake).await;
    let library = fake.library_named("Big").expect("library exists");
    let query = ItemsQuery::default()
        .in_parent(library.id)
        .of_kinds([ItemKind::Movie])
        .recursive();

    let first = fake
        .items(&session, &query.clone().page(0, 50))
        .await
        .unwrap();
    let last = fake.items(&session, &query.page(1_990, 50)).await.unwrap();

    assert_eq!(first.total, 2_000);
    assert_eq!(first.items.len(), 50);
    assert_eq!(first.items[0].name, "Big 00000");
    assert_eq!(last.items.len(), 10);
    assert_eq!(last.items[9].name, "Big 01999");
    let image = ImageRequest::new(first.items[0].id.clone(), ImageKind::Primary, 300);
    assert_ne!(
        fake.image(&session, &image).await.unwrap().bytes,
        Vec::<u8>::new()
    );
}

#[tokio::test]
async fn item_returns_details_and_not_found_for_unknown_ids() {
    let fake = fake();
    let session = log_in(&fake).await;
    let movie = fake.item_named("Brazil").expect("movie exists");

    let item = fake.item(&session, &movie.id).await.unwrap();
    let missing = "nope".parse().expect("valid ID");
    let error = fake.item(&session, &missing).await;

    assert_eq!(item, movie);
    assert_eq!(item.kind, ItemKind::Movie);
    assert_eq!(error, Err(ApiError::ItemNotFound));
}

#[tokio::test]
async fn series_have_seasons_and_episodes() {
    let fake = fake();
    let session = log_in(&fake).await;
    let series = fake.item_named("Severance").expect("series exists");

    let seasons = fake.seasons(&session, &series.id).await.unwrap();
    let second = &seasons[1];
    let episodes = fake
        .episodes(&session, &series.id, &second.id)
        .await
        .unwrap();

    assert_eq!(seasons.len(), 2);
    assert_eq!(second.season_number, Some(2));
    assert_eq!(second.series_id.as_ref(), Some(&series.id));
    assert_eq!(episodes.len(), 3);
    assert_eq!(episodes[2].episode_number, Some(3));
    assert_eq!(episodes[2].season_number, Some(2));
    assert_eq!(episodes[2].series_name.as_deref(), Some("Severance"));
}

#[tokio::test]
async fn seasons_of_an_unknown_series_are_not_found() {
    let fake = fake();
    let session = log_in(&fake).await;
    let movie = fake.item_named("Brazil").expect("movie exists");

    let error = fake.seasons(&session, &movie.id).await;

    assert_eq!(error, Err(ApiError::ItemNotFound));
}

#[tokio::test]
async fn resume_and_next_up_return_what_was_scripted() {
    let fake = fake()
        .with_resume("Brazil", Duration::from_secs(600))
        .with_next_up("Severance S1E2");
    let session = log_in(&fake).await;

    let resume = fake.resume(&session, 10).await.unwrap();
    let next_up = fake.next_up(&session, 10).await.unwrap();

    assert_eq!(resume.len(), 1);
    assert_eq!(resume[0].name, "Brazil");
    let data = resume[0].user_data.as_ref().expect("has user data");
    assert_eq!(data.position, Duration::from_secs(600));
    assert!(data.played_percentage.is_some_and(|p| p > 0.0));
    assert_eq!(next_up.len(), 1);
    assert_eq!(next_up[0].kind, ItemKind::Episode);
    assert_eq!(fake.resume(&session, 0).await.unwrap().len(), 0);
}

#[tokio::test]
async fn latest_returns_the_newest_movies_and_series_first() {
    let fake = fake();
    let session = log_in(&fake).await;
    let movies = fake.library_named("Movies").expect("library exists");
    let shows = fake.library_named("Shows").expect("library exists");

    let latest = fake.latest(&session, &movies.id, 2).await.unwrap();
    let shows_latest = fake.latest(&session, &shows.id, 16).await.unwrap();

    let names: Vec<_> = latest.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, ["Casablanca", "arrival"]);
    assert_eq!(shows_latest.len(), 1);
    assert_eq!(shows_latest[0].kind, ItemKind::Series);
}

#[tokio::test]
async fn serves_placeholder_image_bytes_with_a_content_type() {
    let fake = fake();
    let session = log_in(&fake).await;
    let movie = fake.item_named("Brazil").expect("movie exists");
    let tag = movie.image_tags.primary.clone();

    let request = ImageRequest::new(movie.id.clone(), ImageKind::Primary, 300);
    let image = fake.image(&session, &request).await.unwrap();
    let again = fake.image(&session, &request).await.unwrap();

    assert!(tag.is_some());
    assert_eq!(image.content_type, "image/png");
    assert!(image.bytes.starts_with(b"\x89PNG"));
    assert_eq!(image, again);
}

#[tokio::test]
async fn an_image_the_item_lacks_is_not_found() {
    let fake = fake();
    let session = log_in(&fake).await;
    let season = fake.item_named("Severance S1").expect("season exists");
    let unknown = "nope".parse().expect("valid ID");

    let backdrop = ImageRequest::new(season.id, ImageKind::Backdrop, 300);
    let missing = ImageRequest::new(unknown, ImageKind::Primary, 300);

    assert_eq!(
        fake.image(&session, &backdrop).await,
        Err(ApiError::ItemNotFound)
    );
    assert_eq!(
        fake.image(&session, &missing).await,
        Err(ApiError::ItemNotFound)
    );
}

#[tokio::test]
async fn revoked_tokens_and_scripted_failures_apply_to_library_calls() {
    let fake = fake();
    let session = log_in(&fake).await;

    fake.fail_with(ApiError::Timeout);
    assert_eq!(fake.libraries(&session).await, Err(ApiError::Timeout));
    fake.clear_failure();
    fake.revoke_tokens();

    let query = ItemsQuery::default();
    assert_eq!(fake.libraries(&session).await, Err(ApiError::TokenRejected));
    assert_eq!(
        fake.items(&session, &query).await,
        Err(ApiError::TokenRejected)
    );
    assert_eq!(fake.resume(&session, 5).await, Err(ApiError::TokenRejected));
}
