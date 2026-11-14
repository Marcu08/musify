use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use crossbeam_channel::Sender;
use musify_core::{
    Event, ResolvedSource, SourceEntry, SourceItem, SourcePlayback, SourceProvider, SourceQuery,
    Track,
};

use super::Waker;

struct ProviderEntry {
    provider: Arc<dyn SourceProvider>,
    request: Arc<AtomicU64>,
}

/// Registry of the online sources, keyed by their URI scheme. Source-specific
/// work (network, cache, parsing) stays inside each `SourceProvider`; this
/// registry only routes UI requests and emits generic events.
pub(crate) struct SourceRegistry {
    entries: HashMap<&'static str, ProviderEntry>,
    events: Sender<Event>,
    waker: Arc<Mutex<Option<Waker>>>,
    artwork_inflight: Arc<Mutex<HashSet<(String, String)>>>,
}

impl SourceRegistry {
    pub(crate) fn new(events: Sender<Event>, waker: Arc<Mutex<Option<Waker>>>) -> Self {
        Self {
            entries: HashMap::new(),
            events,
            waker,
            artwork_inflight: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub(crate) fn register<P>(&mut self, provider: Arc<P>)
    where
        P: SourceProvider + 'static,
    {
        let scheme = provider.scheme();
        self.entries.insert(
            scheme,
            ProviderEntry {
                provider,
                request: Arc::new(AtomicU64::new(0)),
            },
        );
    }

    pub(crate) fn contains(&self, source: &str) -> bool {
        self.entries.contains_key(source)
    }

    pub(crate) fn search(&self, source: &str, query: SourceQuery) {
        let Some(entry) = self.entries.get(source) else {
            let _ = self.events.send(Event::SourceSearchFailed {
                source: source.to_string(),
                request_id: 0,
                message: format!("source '{source}' is not registered"),
            });
            wake(&self.waker);
            return;
        };
        let request_id = entry.request.fetch_add(1, Ordering::AcqRel) + 1;
        let provider = Arc::clone(&entry.provider);
        let requests = Arc::clone(&entry.request);
        let source_name = provider.scheme().to_string();
        let events = self.events.clone();
        let waker = Arc::clone(&self.waker);
        let inflight = Arc::clone(&self.artwork_inflight);
        let _ = thread::Builder::new()
            .name(format!("musify-{source}-search"))
            .spawn(move || {
                let (event, artwork_ids) = match provider.search(&query) {
                    Ok(outcome) => {
                        let artwork_ids = search_artwork_ids(&outcome.items);
                        (
                            Event::SourceResults {
                                source: source_name.clone(),
                                request_id,
                                items: outcome.items,
                                offline: outcome.offline,
                                cached: outcome.cached,
                                has_more: outcome.has_more,
                            },
                            artwork_ids,
                        )
                    }
                    Err(error) => (
                        Event::SourceSearchFailed {
                            source: source_name.clone(),
                            request_id,
                            message: error.to_string(),
                        },
                        Vec::new(),
                    ),
                };
                // A newer search supersedes this one; drop the stale result.
                if requests.load(Ordering::Acquire) != request_id {
                    return;
                }
                let _ = events.send(event);
                wake(&waker);
                prefetch_artwork(
                    &provider,
                    &source_name,
                    &events,
                    &waker,
                    &inflight,
                    artwork_ids,
                );
            });
    }

    pub(crate) fn cancel(&self, source: &str) {
        if let Some(entry) = self.entries.get(source) {
            // A newer request id makes the worker drop its event even if the
            // provider was already inside a blocking HTTP read.
            entry.request.fetch_add(1, Ordering::AcqRel);
            entry.provider.cancel();
        }
    }

    /// One-shot cache warm-up, run on a named background thread. The provider
    /// itself is idempotent; calling this twice does not start a second pass.
    pub(crate) fn prefetch(&self, source: &str) {
        let Some(entry) = self.entries.get(source) else {
            return;
        };
        let provider = Arc::clone(&entry.provider);
        let _ = thread::Builder::new()
            .name(format!("musify-{source}-prefetch"))
            .spawn(move || provider.prefetch());
    }

    /// Curated selections are a background operation because a cold cache may
    /// need two network requests. A failure or cancellation simply hides the
    /// section; it is not an error banner.
    pub(crate) fn selections(&self, source: &str) {
        let Some(entry) = self.entries.get(source) else {
            return;
        };
        let provider = Arc::clone(&entry.provider);
        let source_name = provider.scheme().to_string();
        let events = self.events.clone();
        let waker = Arc::clone(&self.waker);
        let _ = thread::Builder::new()
            .name(format!("musify-{source}-selections"))
            .spawn(move || {
                let items = provider.selections().unwrap_or_default();
                let _ = events.send(Event::SourceSelections {
                    source: source_name,
                    request_id: 0,
                    items,
                });
                wake(&waker);
            });
    }

    pub(crate) fn favorites(&self, source: &str) -> Result<Vec<SourceItem>, String> {
        let entry = self.entry(source)?;
        let items = entry
            .provider
            .favorites()
            .map_err(|error| error.to_string())?;
        let ids = search_artwork_ids(&items);
        self.prefetch_in_background(entry.provider.scheme(), Arc::clone(&entry.provider), ids);
        Ok(items)
    }

    /// Resolves a list of source items for play-all/enqueue/save. Items that
    /// disappeared from the source cache are skipped, like missing local files
    /// in a playlist; live streams are not finite queue items.
    pub(crate) fn resolve_tracks(
        &self,
        source: &str,
        ids: &[String],
    ) -> Result<Vec<Track>, String> {
        let entry = self.entry(source)?;
        let mut tracks = Vec::new();
        for id in ids {
            match entry.provider.resolve(id) {
                Ok(resolved) if !matches!(resolved.playback, SourcePlayback::Live) => {
                    tracks.push(resolved.track);
                }
                Ok(_) | Err(_) => {}
            }
        }
        if tracks.is_empty() {
            return Err("no playable source items".into());
        }
        Ok(tracks)
    }

    pub(crate) fn toggle_favorite(&self, source: &str, id: &str) -> Result<bool, String> {
        self.entry(source)?
            .provider
            .toggle_favorite(id)
            .map_err(|error| error.to_string())
    }

    pub(crate) fn resolve(&self, source: &str, id: &str) -> Result<ResolvedSource, String> {
        self.entry(source)?
            .provider
            .resolve(id)
            .map_err(|error| error.to_string())
    }

    pub(crate) fn entries(&self, source: &str, id: &str) -> Result<Vec<SourceEntry>, String> {
        self.entry(source)?
            .provider
            .entries(id)
            .map_err(|error| error.to_string())
    }

    pub(crate) fn refresh(&self, source: &str, id: &str) {
        let Ok(entry) = self.entry(source) else {
            let _ = self.events.send(Event::SourceEntries {
                source: source.to_string(),
                id: id.to_string(),
                result: Err(format!("source '{source}' is not registered")),
            });
            wake(&self.waker);
            return;
        };
        let provider = Arc::clone(&entry.provider);
        let source_name = provider.scheme().to_string();
        let id = id.to_string();
        let events = self.events.clone();
        let waker = Arc::clone(&self.waker);
        let _ = thread::Builder::new()
            .name(format!("musify-{source}-refresh"))
            .spawn(move || {
                let result = provider.refresh(&id).map_err(|error| error.to_string());
                let _ = events.send(Event::SourceEntries {
                    source: source_name,
                    id,
                    result,
                });
                wake(&waker);
            });
    }

    pub(crate) fn resume_position(
        &self,
        source: &str,
        id: &str,
    ) -> Result<Option<std::time::Duration>, String> {
        self.entry(source)?
            .provider
            .resume_position(id)
            .map_err(|error| error.to_string())
    }

    pub(crate) fn save_progress(
        &self,
        source: &str,
        id: &str,
        position: std::time::Duration,
        watched: bool,
    ) -> Result<(), String> {
        self.entry(source)?
            .provider
            .save_progress(id, position, watched)
            .map_err(|error| error.to_string())
    }

    pub(crate) fn set_watched(&self, source: &str, id: &str, watched: bool) -> Result<(), String> {
        self.entry(source)?
            .provider
            .set_watched(id, watched)
            .map_err(|error| error.to_string())
    }

    fn entry(&self, source: &str) -> Result<&ProviderEntry, String> {
        self.entries
            .get(source)
            .ok_or_else(|| format!("source '{source}' is not registered"))
    }

    fn prefetch_in_background(
        &self,
        source: &str,
        provider: Arc<dyn SourceProvider>,
        ids: Vec<String>,
    ) {
        if ids.is_empty() {
            return;
        }
        let source_name = source.to_string();
        let events = self.events.clone();
        let waker = Arc::clone(&self.waker);
        let inflight = Arc::clone(&self.artwork_inflight);
        let _ = thread::Builder::new()
            .name(format!("musify-{source}-artwork"))
            .spawn(move || {
                prefetch_artwork(&provider, &source_name, &events, &waker, &inflight, ids);
            });
    }
}

fn search_artwork_ids(items: &[SourceItem]) -> Vec<String> {
    items
        .iter()
        .take(8)
        .filter(|item| item.image_path.is_none())
        .map(|item| item.id.clone())
        .collect()
}

fn prefetch_artwork(
    provider: &Arc<dyn SourceProvider>,
    source: &str,
    events: &Sender<Event>,
    waker: &Arc<Mutex<Option<Waker>>>,
    inflight: &Mutex<HashSet<(String, String)>>,
    ids: Vec<String>,
) {
    for id in ids {
        let key = (source.to_string(), id.clone());
        if let Ok(mut inflight) = inflight.lock()
            && !inflight.insert(key.clone())
        {
            continue;
        }
        let cached = provider.artwork(&id).ok().flatten();
        if let Some(path) = cached {
            let _ = events.send(Event::SourceArtwork {
                source: source.to_string(),
                id: id.clone(),
                path,
            });
            wake(waker);
        }
        if let Ok(mut inflight) = inflight.lock() {
            inflight.remove(&key);
        }
    }
}

fn wake(waker: &Arc<Mutex<Option<Waker>>>) {
    let task = waker.lock().ok().and_then(|guard| guard.as_ref().cloned());
    if let Some(task) = task {
        task();
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Duration;

    use crossbeam_channel::unbounded;
    use musify_core::{
        Event, ResolvedSource, SourceError, SourceId, SourceItem, SourcePlayback, SourceProvider,
        SourceQuery, SourceSearch, Track, TrackId,
    };

    use super::SourceRegistry;

    struct FakeProvider {
        scheme: &'static str,
    }

    impl SourceProvider for FakeProvider {
        fn scheme(&self) -> &'static str {
            self.scheme
        }

        fn search(&self, query: &SourceQuery) -> Result<SourceSearch, SourceError> {
            Ok(SourceSearch {
                items: vec![self.item(&query.text)],
                offline: false,
                cached: false,
                has_more: true,
            })
        }

        fn selections(&self) -> Result<Vec<SourceItem>, SourceError> {
            Ok(vec![self.item("selection")])
        }

        fn favorites(&self) -> Result<Vec<SourceItem>, SourceError> {
            Ok(vec![self.item("favorite")])
        }

        fn toggle_favorite(&self, _id: &str) -> Result<bool, SourceError> {
            Ok(true)
        }

        fn resolve(&self, id: &str) -> Result<ResolvedSource, SourceError> {
            Ok(ResolvedSource {
                track: Track {
                    id: TrackId::new(7),
                    source: SourceId::new(self.scheme, id),
                    path: PathBuf::new(),
                    title: "resolved".into(),
                    artist: "source".into(),
                    album: "source".into(),
                    album_artist: None,
                    track_number: None,
                    disc_number: None,
                    duration: Duration::from_secs(30),
                    year: None,
                    genre: None,
                    cover_path: None,
                    added_at: 0,
                },
                playback: SourcePlayback::OnDemand {
                    duration: Duration::from_secs(30),
                    seekable: true,
                },
            })
        }
    }

    impl FakeProvider {
        fn item(&self, title: &str) -> SourceItem {
            SourceItem {
                id: title.to_string(),
                source: SourceId::new(self.scheme, title),
                title: title.to_string(),
                subtitle: String::new(),
                detail: String::new(),
                uri: title.to_string(),
                favorite: false,
                image_path: None,
                duration: None,
                license_url: None,
            }
        }
    }

    fn registry() -> (
        SourceRegistry,
        crossbeam_channel::Receiver<Event>,
        Arc<std::sync::Mutex<Option<super::Waker>>>,
    ) {
        let (tx, rx) = unbounded();
        let waker = Arc::new(std::sync::Mutex::new(None));
        let mut registry = SourceRegistry::new(tx, Arc::clone(&waker));
        registry.register(Arc::new(FakeProvider { scheme: "fake" }));
        (registry, rx, waker)
    }

    #[test]
    fn registry_routes_searches_and_rejects_unknown_sources() {
        let (registry, events, _waker) = registry();
        registry.search(
            "fake",
            SourceQuery {
                text: "jazz".into(),
                ..SourceQuery::default()
            },
        );
        let event = events
            .recv_timeout(Duration::from_secs(2))
            .expect("search event");
        match event {
            Event::SourceResults {
                source,
                items,
                has_more,
                ..
            } => {
                assert_eq!(source, "fake");
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].title, "jazz");
                assert!(has_more);
            }
            other => panic!("unexpected event: {other:?}"),
        }
        assert!(registry.favorites("missing").is_err());
        assert!(registry.resolve("missing", "id").is_err());
    }

    #[test]
    fn registry_resolves_lists_and_emits_selections() {
        let (registry, events, _waker) = registry();
        let ids = vec!["first".to_string(), "second".to_string()];
        let tracks = registry.resolve_tracks("fake", &ids).expect("tracks");
        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[0].source.scheme(), "fake");
        registry.selections("fake");
        let event = events
            .recv_timeout(Duration::from_secs(2))
            .expect("selections event");
        match event {
            Event::SourceSelections { source, items, .. } => {
                assert_eq!(source, "fake");
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].title, "selection");
            }
            other => panic!("unexpected event: {other:?}"),
        }
        assert!(registry.resolve_tracks("missing", &ids).is_err());
    }

    #[test]
    fn registry_routes_favorites_toggle_and_resolve() {
        let (registry, _events, _waker) = registry();
        let favorites = registry.favorites("fake").expect("favorites");
        assert_eq!(favorites.len(), 1);
        assert_eq!(favorites[0].title, "favorite");
        assert!(
            registry
                .toggle_favorite("fake", "favorite")
                .expect("toggle")
        );
        let resolved = registry.resolve("fake", "item").expect("resolve");
        assert_eq!(resolved.track.source.scheme(), "fake");
        assert_eq!(
            resolved.playback,
            SourcePlayback::OnDemand {
                duration: Duration::from_secs(30),
                seekable: true,
            }
        );
    }
}
