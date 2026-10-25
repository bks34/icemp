use std::time::Duration;

use serde::Deserialize;

const LRCLIB_GET_URL: &str = "https://lrclib.net/api/get";
const LRCLIB_SEARCH_URL: &str = "https://lrclib.net/api/search";
const USER_AGENT: &str = "icemp/0.1.0 (io.github.bks34.icemp)";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone)]
pub struct LyricsQuery {
    pub artist: String,
    pub title: String,
    pub album: String,
    pub duration_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchResult {
    Found(String),
    NotFound,
    Error(String),
}

#[derive(Debug, Deserialize)]
struct LrclibTrack {
    #[serde(rename = "trackName")]
    track_name: String,
    #[serde(rename = "artistName")]
    artist_name: String,
    #[serde(rename = "albumName", default)]
    album_name: String,
    duration: Option<f64>,
    #[serde(rename = "syncedLyrics")]
    synced_lyrics: Option<String>,
}

pub async fn fetch_lyrics(query: LyricsQuery) -> FetchResult {
    let client = match reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .user_agent(USER_AGENT)
        .build()
    {
        Ok(client) => client,
        Err(error) => return FetchResult::Error(format!("无法创建网络客户端：{error}")),
    };

    let mut params = vec![
        ("artist_name", query.artist.clone()),
        ("track_name", query.title.clone()),
    ];
    if query.duration_secs > 0 {
        params.push(("duration", query.duration_secs.to_string()));
    }

    let response = match client.get(LRCLIB_GET_URL).query(&params).send().await {
        Ok(response) => response,
        Err(error) => {
            return FetchResult::Error(format!("歌词请求失败：{error}"));
        }
    };

    if response.status().is_success() {
        match response.json::<LrclibTrack>().await {
            Ok(track) => {
                if let Some(lyrics) = usable_synced_lyrics(&track) {
                    return FetchResult::Found(lyrics);
                }
                // 精确查询成功但只有纯文本歌词时继续尝试搜索，搜索接口可能返回同步歌词。
            }
            Err(error) => {
                return FetchResult::Error(format!("歌词响应解析失败：{error}"));
            }
        }
    } else if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return FetchResult::Error("歌词服务请求过于频繁，请稍后重试".into());
    } else if response.status() != reqwest::StatusCode::NOT_FOUND {
        return FetchResult::Error(format!("歌词服务返回状态码 {}", response.status()));
    }

    let search_params = vec![
        ("artist_name", query.artist.clone()),
        ("track_name", query.title.clone()),
    ];

    let response = match client
        .get(LRCLIB_SEARCH_URL)
        .query(&search_params)
        .send()
        .await
    {
        Ok(response) => response,
        Err(error) => {
            return FetchResult::Error(format!("歌词搜索请求失败：{error}"));
        }
    };

    if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return FetchResult::Error("歌词服务请求过于频繁，请稍后重试".into());
    }
    if !response.status().is_success() {
        return FetchResult::Error(format!("歌词搜索返回状态码 {}", response.status()));
    }

    let tracks = match response.json::<Vec<LrclibTrack>>().await {
        Ok(tracks) => tracks,
        Err(error) => {
            return FetchResult::Error(format!("歌词搜索结果解析失败：{error}"));
        }
    };

    match choose_best_match(&query, tracks) {
        Some(lyrics) => FetchResult::Found(lyrics),
        None => FetchResult::NotFound,
    }
}

fn usable_synced_lyrics(track: &LrclibTrack) -> Option<String> {
    track
        .synced_lyrics
        .as_ref()
        .map(|lyrics| lyrics.trim().to_string())
        .filter(|lyrics| !lyrics.is_empty())
}

fn choose_best_match(query: &LyricsQuery, tracks: Vec<LrclibTrack>) -> Option<String> {
    let query_title = normalize(&query.title);
    let query_artist = normalize(&query.artist);

    tracks
        .into_iter()
        .filter_map(|track| {
            let lyrics = usable_synced_lyrics(&track)?;
            let title_matches = normalize(&track.track_name) == query_title;
            let artist_matches = normalize(&track.artist_name) == query_artist;
            let album_matches = !query.album.trim().is_empty()
                && normalize(&track.album_name) == normalize(&query.album);
            let duration_difference = match (track.duration, query.duration_secs) {
                (Some(track_duration), query_duration) if query_duration > 0 => {
                    (track_duration - query_duration as f64).abs()
                }
                _ => f64::MAX,
            };

            if !title_matches && !artist_matches {
                return None;
            }
            if query.duration_secs > 0
                && duration_difference.is_finite()
                && duration_difference > 10.0
            {
                return None;
            }

            Some((
                !title_matches,
                !artist_matches,
                duration_difference,
                !album_matches,
                lyrics,
            ))
        })
        .min_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then_with(|| left.1.cmp(&right.1))
                .then_with(|| left.2.total_cmp(&right.2))
                .then_with(|| left.3.cmp(&right.3))
        })
        .map(|(_, _, _, _, lyrics)| lyrics)
}

fn normalize(value: &str) -> String {
    value
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(title: &str, artist: &str, duration: f64, lyrics: Option<&str>) -> LrclibTrack {
        LrclibTrack {
            track_name: title.into(),
            artist_name: artist.into(),
            album_name: String::new(),
            duration: Some(duration),
            synced_lyrics: lyrics.map(str::to_string),
        }
    }

    #[test]
    fn normalizes_case_and_whitespace() {
        assert_eq!(normalize("  Hello   World "), "hello world");
    }

    #[test]
    fn chooses_exact_title_and_closest_duration() {
        let query = LyricsQuery {
            artist: "Artist".into(),
            title: "Song".into(),
            album: String::new(),
            duration_secs: 180,
        };
        let tracks = vec![
            track("Song", "Other", 180.0, Some("[00:01.00]wrong artist")),
            track("Song", "Artist", 220.0, Some("[00:01.00]far away")),
            track("Song", "Artist", 181.0, Some("[00:01.00]best")),
        ];

        assert_eq!(
            choose_best_match(&query, tracks),
            Some("[00:01.00]best".into())
        );
    }

    #[test]
    fn ignores_tracks_without_synced_lyrics() {
        let query = LyricsQuery {
            artist: "Artist".into(),
            title: "Song".into(),
            album: String::new(),
            duration_secs: 180,
        };
        let tracks = vec![track("Song", "Artist", 180.0, None)];

        assert_eq!(choose_best_match(&query, tracks), None);
    }

    #[test]
    fn rejects_unrelated_results() {
        let query = LyricsQuery {
            artist: "Artist".into(),
            title: "Song".into(),
            album: String::new(),
            duration_secs: 180,
        };
        let tracks = vec![track(
            "Different",
            "Someone",
            180.0,
            Some("[00:01.00]not this song"),
        )];

        assert_eq!(choose_best_match(&query, tracks), None);
    }
}
