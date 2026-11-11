use std::time::Duration;

use feed_rs::model::{Entry, Text};
use url::Url;

use crate::error::PodcastError;

#[derive(Debug, Default)]
pub(crate) struct ParsedFeed {
    pub(crate) title: Option<String>,
    pub(crate) author: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) website: Option<String>,
    pub(crate) artwork_url: Option<String>,
    pub(crate) entries: Vec<ParsedEpisode>,
}

#[derive(Debug)]
pub(crate) struct ParsedEpisode {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) description: String,
    pub(crate) published_at: Option<i64>,
    pub(crate) duration: Option<Duration>,
    pub(crate) audio_url: String,
    pub(crate) audio_type: Option<String>,
}

pub(crate) fn parse(
    podcast_id: &str,
    bytes: &[u8],
    feed_url: &str,
) -> Result<ParsedFeed, PodcastError> {
    let feed = feed_rs::parser::Builder::new()
        .base_uri(Some(feed_url))
        .build()
        .parse(bytes)
        .map_err(|error| PodcastError::Feed(format!("cannot parse feed: {error}")))?;
    let fallback_durations = itunes_durations(bytes);
    let title = feed
        .title
        .as_ref()
        .map(text)
        .filter(|value| !value.is_empty());
    let author = feed
        .authors
        .first()
        .and_then(|person| person.name.clone())
        .filter(|value| !value.is_empty());
    let description = feed
        .description
        .as_ref()
        .map(text)
        .map(|value| clean(&value))
        .filter(|value| !value.is_empty());
    let website = feed.links.first().map(|link| link.href.clone());
    let artwork_url = feed
        .logo
        .as_ref()
        .or(feed.icon.as_ref())
        .and_then(|image| absolute(feed_url, &image.uri));
    let entries = feed
        .entries
        .iter()
        .filter_map(|entry| episode(podcast_id, entry, feed_url, &fallback_durations))
        .collect();
    Ok(ParsedFeed {
        title,
        author,
        description,
        website,
        artwork_url,
        entries,
    })
}

fn episode(
    podcast_id: &str,
    entry: &Entry,
    feed_url: &str,
    fallback_durations: &[(String, Duration)],
) -> Option<ParsedEpisode> {
    let (raw_url, audio_type) = enclosure(entry)?;
    let audio_url = absolute(feed_url, &raw_url)?;
    let raw_id = if entry.id.trim().is_empty() {
        audio_url.clone()
    } else {
        entry.id.clone()
    };
    let id = blake3::hash(format!("{podcast_id}\0{raw_id}").as_bytes())
        .to_hex()
        .to_string();
    let title = entry
        .title
        .as_ref()
        .map(text)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "Episodio".to_string());
    let description = entry
        .summary
        .as_ref()
        .map(text)
        .or_else(|| {
            entry
                .content
                .as_ref()
                .and_then(|content| content.body.clone())
        })
        .map(|value| clean(&value))
        .unwrap_or_default();
    let published_at = entry
        .published
        .or(entry.updated)
        .map(|date| date.timestamp());
    let duration = entry
        .media
        .iter()
        .find_map(|media| media.duration)
        .or_else(|| {
            fallback_durations
                .iter()
                .find(|(key, _)| key == &entry.id || key == &title)
                .map(|(_, duration)| *duration)
        })
        .filter(|duration| !duration.is_zero());
    Some(ParsedEpisode {
        id,
        title,
        description: truncate(description, 600),
        published_at,
        duration,
        audio_url,
        audio_type,
    })
}

fn enclosure(entry: &Entry) -> Option<(String, Option<String>)> {
    // RSS `<enclosure>` becomes a media object with an audio content type.
    // Prefer those over `content.src`: feed-rs sets `content.src` to the feed
    // URL when it parses `<content:encoded>`, so using it first would play the
    // feed XML as audio.
    let mut fallback = None;
    for content in entry
        .media
        .iter()
        .flat_map(|media| &media.content)
        .filter(|content| content.url.is_some())
    {
        let media_type = content
            .content_type
            .as_ref()
            .map(std::string::ToString::to_string);
        let url = content.url.as_ref()?.to_string();
        if is_audio(&url, media_type.as_deref()) {
            return Some((url, media_type));
        }
        fallback.get_or_insert((url, media_type));
    }
    if let Some(link) = entry
        .links
        .iter()
        .find(|link| link.rel.as_deref() == Some("enclosure"))
    {
        return Some((link.href.clone(), link.media_type.clone()));
    }
    if let Some(link) = entry
        .content
        .as_ref()
        .and_then(|content| content.src.as_ref())
        && is_audio(&link.href, link.media_type.as_deref())
    {
        return Some((link.href.clone(), link.media_type.clone()));
    }
    fallback
}

fn is_audio(url: &str, media_type: Option<&str>) -> bool {
    if media_type.is_some_and(|value| value.to_ascii_lowercase().starts_with("audio/")) {
        return true;
    }
    let path = url.split('?').next().unwrap_or(url).to_ascii_lowercase();
    [".mp3", ".m4a", ".aac", ".ogg", ".opus", ".flac", ".wav"]
        .iter()
        .any(|extension| path.ends_with(extension))
}

fn text(text: &Text) -> String {
    text.content.trim().to_string()
}

/// feed-rs keeps `itunes:duration` on a media object that is only emitted when
/// it also contains content, so duration-only items would lose it. Extract the
/// value from the raw item block and key it by GUID (or title as a fallback).
fn itunes_durations(xml: &[u8]) -> Vec<(String, Duration)> {
    let text = String::from_utf8_lossy(xml);
    let mut durations = Vec::new();
    let mut rest = text.as_ref();
    while let Some(start) = rest.find("<item") {
        let item = &rest[start..];
        let Some(end) = item.find("</item>") else {
            break;
        };
        let block = &item[..end];
        let key = extract_tag(block, "guid").or_else(|| extract_tag(block, "title"));
        let duration = extract_tag(block, "itunes:duration")
            .as_deref()
            .and_then(parse_duration);
        if let (Some(key), Some(duration)) = (key, duration) {
            durations.push((key, duration));
        }
        rest = &item[end + "</item>".len()..];
    }
    durations
}

fn extract_tag(block: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    let start = block.find(&open)?;
    let content_start = block[start..].find('>')? + start + 1;
    let close = format!("</{tag}>");
    let end = block[content_start..].find(&close)? + content_start;
    let value = block[content_start..end]
        .trim()
        .trim_start_matches("<![CDATA[")
        .trim_end_matches("]]>")
        .trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn parse_duration(value: &str) -> Option<Duration> {
    let parts: Vec<u64> = value
        .split(':')
        .map(str::trim)
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    let seconds = match parts.as_slice() {
        [hours, minutes, seconds] => hours * 3600 + minutes * 60 + seconds,
        [minutes, seconds] => minutes * 60 + seconds,
        [seconds] => *seconds,
        _ => return None,
    };
    Some(Duration::from_secs(seconds))
}

fn absolute(base: &str, href: &str) -> Option<String> {
    if href.starts_with("http://") || href.starts_with("https://") {
        return Some(href.to_string());
    }
    Url::parse(base)
        .ok()?
        .join(href)
        .ok()
        .map(|url| url.to_string())
}

fn clean(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut in_tag = false;
    for character in input.chars() {
        match character {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                output.push(' ');
            }
            _ if !in_tag => output.push(character),
            _ => {}
        }
    }
    let decoded = output
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");
    decoded.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate(mut value: String, max: usize) -> String {
    if value.chars().count() <= max {
        return value;
    }
    let end = value
        .char_indices()
        .nth(max)
        .map_or(value.len(), |(index, _)| index);
    value.truncate(end);
    value.push('…');
    value
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    const FEED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd">
  <channel>
    <title>Test Podcast</title>
    <description>Un feed di prova</description>
    <link>https://example.com/podcast</link>
    <itunes:author>Test Author</itunes:author>
    <image><url>https://example.com/cover.jpg</url></image>
    <item>
      <title>Episode One</title>
      <guid>episode-1</guid>
      <description>&lt;p&gt;Prima &amp;amp; unica&lt;/p&gt;</description>
      <pubDate>Tue, 10 Nov 2026 10:00:00 GMT</pubDate>
      <itunes:duration>01:02:03</itunes:duration>
      <enclosure url="audio/one.mp3" type="audio/mpeg" length="1000"/>
    </item>
    <item>
      <title>No Enclosure</title>
      <guid>episode-2</guid>
    </item>
  </channel>
</rss>"#;

    #[test]
    fn parses_metadata_relative_enclosures_and_duration() {
        let feed =
            parse("podcast-1", FEED.as_bytes(), "https://example.com/feed.xml").expect("feed");
        assert_eq!(feed.title.as_deref(), Some("Test Podcast"));
        assert_eq!(feed.author.as_deref(), Some("Test Author"));
        assert_eq!(feed.website.as_deref(), Some("https://example.com/podcast"));
        assert_eq!(
            feed.artwork_url.as_deref(),
            Some("https://example.com/cover.jpg")
        );
        assert_eq!(feed.entries.len(), 1, "missing enclosure must be skipped");
        let entry = &feed.entries[0];
        assert_eq!(entry.title, "Episode One");
        assert_eq!(entry.audio_url, "https://example.com/audio/one.mp3");
        assert_eq!(entry.duration, Some(Duration::from_secs(3723)));
        assert_eq!(entry.description, "Prima & unica");
        assert!(entry.published_at.is_some());
    }

    #[test]
    fn content_encoded_does_not_replace_the_enclosure() {
        let feed = r#"<?xml version="1.0"?>
<rss version="2.0" xmlns:content="http://purl.org/rss/1.0/modules/content/">
  <channel>
    <title>Feed URL trap</title>
    <link>https://example.com/</link>
    <item>
      <title>Episode</title>
      <guid>episode-1</guid>
      <content:encoded><![CDATA[<p>Show notes</p>]]></content:encoded>
      <enclosure url="https://cdn.example.com/audio/episode.mp3" type="audio/mpeg"/>
    </item>
  </channel>
</rss>"#;
        let parsed = parse(
            "podcast-1",
            feed.as_bytes(),
            "https://feeds.example.com/podcast.xml",
        )
        .expect("feed");
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(
            parsed.entries[0].audio_url,
            "https://cdn.example.com/audio/episode.mp3"
        );
    }

    #[test]
    fn malformed_feed_is_reported() {
        let error = parse(
            "podcast-1",
            b"<rss><channel>",
            "https://example.com/feed.xml",
        )
        .expect_err("malformed feed");
        assert!(matches!(error, PodcastError::Feed(_)));
    }
}
