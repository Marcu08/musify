//! Manual latency harness. It uses a local fake Jamendo server with a fixed
//! 40 ms delay, so the numbers are comparable between runs and machines.
//!
//! ```powershell
//! cargo test -p musify-jamendo --release --test latency -- --nocapture
//! ```
#![allow(clippy::expect_used)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use musify_core::SourceQuery;
use musify_jamendo::{JamendoOptions, JamendoSource, JamendoStore};
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const NETWORK_DELAY: Duration = Duration::from_millis(40);

fn tracks(count: usize) -> serde_json::Value {
    let results: Vec<_> = (0..count)
        .map(|index| {
            json!({
                "id": format!("{index}"),
                "name": format!("Track {index}"),
                "duration": 180,
                "artist_name": "Artist",
                "album_name": "Album",
                "album_image": format!("https://img.example/{index}.jpg"),
                "audio": format!("https://audio.example/{index}.mp3"),
                "license_ccurl": "http://creativecommons.org/licenses/by-sa/3.0/",
                "shareurl": format!("https://www.jamendo.com/track/{index}")
            })
        })
        .collect();
    json!({
        "headers": {"status": "success", "code": 0, "error_message": ""},
        "results": results
    })
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
}

fn source(server: &MockServer, store: Arc<JamendoStore>) -> JamendoSource {
    JamendoSource::with_options(
        store,
        tempfile::tempdir().expect("artwork dir").keep(),
        "synthetic-client".into(),
        JamendoOptions {
            base_url: server.uri(),
            min_interval: Duration::ZERO,
            retry_base: Duration::from_millis(1),
            cache_ttl_secs: 3600,
        },
    )
}

fn query(order: &str, offset: usize, limit: usize) -> SourceQuery {
    let mut query = SourceQuery {
        text: String::new(),
        filters: vec![
            ("tag".into(), "jazz".into()),
            ("order".into(), order.into()),
            ("offset".into(), offset.to_string()),
            ("limit".into(), limit.to_string()),
        ],
    };
    if order.is_empty() {
        query.filters.retain(|(key, _)| key != "order");
    }
    query
}

fn measure(source: &JamendoSource, query: &SourceQuery) -> Duration {
    let started = Instant::now();
    let outcome = source.search(query).expect("search");
    let elapsed = started.elapsed();
    assert!(!outcome.items.is_empty());
    elapsed
}

#[test]
fn measure_cold_warm_and_prefetched_first_page() {
    let runtime = runtime();
    let server = runtime.block_on(MockServer::start());
    runtime.block_on(
        Mock::given(method("GET"))
            .and(path("/tracks/"))
            .and(query_param("limit", "100"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(NETWORK_DELAY)
                    .set_body_json(tracks(100)),
            )
            .mount(&server),
    );
    runtime.block_on(
        Mock::given(method("GET"))
            .and(path("/tracks/"))
            .and(query_param("limit", "60"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(NETWORK_DELAY)
                    .set_body_json(tracks(60)),
            )
            .mount(&server),
    );
    let dir = tempfile::tempdir().expect("dir");

    // Same shape as the M7.2 baseline: 100 results, tag search, no order.
    let store = Arc::new(JamendoStore::open(&dir.path().join("cold100.sqlite3")).expect("store"));
    let cold100 = source(&server, store);
    let cold = measure(&cold100, &query("", 0, 100));
    let warm = measure(&cold100, &query("", 0, 100));

    // New UI page: 60 results, popularity week.
    let store = Arc::new(JamendoStore::open(&dir.path().join("cold60.sqlite3")).expect("store"));
    let cold60_source = source(&server, store);
    let cold60 = measure(&cold60_source, &query("popular_week", 0, 60));
    let warm60 = measure(&cold60_source, &query("popular_week", 0, 60));

    // Prefetch then a first click: the click is a cache read.
    let store = Arc::new(JamendoStore::open(&dir.path().join("prefetch.sqlite3")).expect("store"));
    store.record_genre_use("jazz").expect("usage");
    let prefetched = source(&server, store);
    let prefetch_started = Instant::now();
    prefetched.prefetch();
    let prefetch = prefetch_started.elapsed();
    let first_click = measure(&prefetched, &query("popular_week", 0, 60));

    println!(
        "latency cold100={cold:?} warm100={warm:?} cold60={cold60:?} warm60={warm60:?} prefetch={prefetch:?} prefetched_click={first_click:?}"
    );
}
