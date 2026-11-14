use std::time::Duration;

use serde::{Deserialize, de::DeserializeOwned};

const LRCLIB_GET_URL: &str = "https://lrclib.net/api/get";
const LRCLIB_SEARCH_URL: &str = "https://lrclib.net/api/search";
const NETEASE_SEARCH_URL: &str = "https://music.163.com/api/search/get/web";
const NETEASE_LYRIC_URL: &str = "https://music.163.com/api/song/lyric";
const QQ_SEARCH_URL: &str = "https://u.y.qq.com/cgi-bin/musicu.fcg";
const QQ_LYRIC_URL: &str = "https://c.y.qq.com/lyric/fcgi-bin/fcg_query_lyric_new.fcg";
const APP_USER_AGENT: &str = "icemp/0.1.0 (io.github.bks34.icemp)";
const BROWSER_USER_AGENT: &str =
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Chrome/124.0 Safari/537.36";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LyricsSource {
    #[default]
    Lrclib,
    Netease,
    QqMusic,
}

impl LyricsSource {
    pub fn next(self) -> Self {
        match self {
            Self::Lrclib => Self::Netease,
            Self::Netease => Self::QqMusic,
            Self::QqMusic => Self::Lrclib,
        }
    }

    pub fn previous(self) -> Self {
        match self {
            Self::Lrclib => Self::QqMusic,
            Self::Netease => Self::Lrclib,
            Self::QqMusic => Self::Netease,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Lrclib => "LRCLIB",
            Self::Netease => "网易云",
            Self::QqMusic => "QQ 音乐",
        }
    }

    pub fn download_order(self) -> [Self; 3] {
        match self {
            Self::Lrclib => [Self::Lrclib, Self::Netease, Self::QqMusic],
            Self::Netease => [Self::Netease, Self::Lrclib, Self::QqMusic],
            Self::QqMusic => [Self::QqMusic, Self::Lrclib, Self::Netease],
        }
    }
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
enum ProviderResult {
    Found(String),
    NotFound,
    Error(String),
}

#[derive(Debug, Clone)]
struct TrackCandidate {
    title: String,
    artist: String,
    album: String,
    duration_secs: Option<f64>,
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

#[derive(Debug, Deserialize)]
struct NeteaseSearchResponse {
    result: Option<NeteaseSearchResult>,
}

#[derive(Debug, Deserialize)]
struct NeteaseSearchResult {
    #[serde(default)]
    songs: Vec<NeteaseSong>,
}

#[derive(Debug, Deserialize)]
struct NeteaseSong {
    id: u64,
    name: String,
    #[serde(default)]
    artists: Vec<NeteaseArtist>,
    duration: Option<u64>,
    album: Option<NeteaseAlbum>,
}

#[derive(Debug, Deserialize)]
struct NeteaseArtist {
    name: String,
}

#[derive(Debug, Deserialize)]
struct NeteaseAlbum {
    #[serde(default)]
    name: String,
}

#[derive(Debug, Deserialize)]
struct NeteaseLyricResponse {
    lrc: Option<NeteaseLyrics>,
}

#[derive(Debug, Deserialize)]
struct NeteaseLyrics {
    lyric: Option<String>,
}

#[derive(Debug, Deserialize)]
struct QqSearchResponse {
    req_1: Option<QqRequestResponse>,
}

#[derive(Debug, Deserialize)]
struct QqRequestResponse {
    data: Option<QqSearchData>,
}

#[derive(Debug, Deserialize)]
struct QqSearchData {
    body: Option<QqSearchBody>,
}

#[derive(Debug, Deserialize)]
struct QqSearchBody {
    song: Option<QqSongData>,
}

#[derive(Debug, Deserialize)]
struct QqSongData {
    #[serde(default)]
    list: Vec<QqSong>,
}

#[derive(Debug, Deserialize)]
struct QqSong {
    #[serde(rename = "mid")]
    songmid: String,
    #[serde(rename = "name", alias = "title")]
    songname: String,
    #[serde(default)]
    singer: Vec<QqSinger>,
    interval: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct QqSinger {
    name: String,
}

#[derive(Debug, Deserialize)]
struct QqLyricResponse {
    lyric: Option<String>,
}

pub async fn fetch_lyrics(query: LyricsQuery, priority: LyricsSource) -> FetchResult {
    let client = match reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .user_agent(APP_USER_AGENT)
        .build()
    {
        Ok(client) => client,
        Err(error) => return FetchResult::Error(format!("无法创建网络客户端：{error}")),
    };

    let mut errors = Vec::new();

    for source in priority.download_order() {
        let result = match source {
            LyricsSource::Lrclib => fetch_from_lrclib(&client, &query).await,
            LyricsSource::Netease => fetch_from_netease(&client, &query).await,
            LyricsSource::QqMusic => fetch_from_qq_music(&client, &query).await,
        };

        match result {
            ProviderResult::Found(lyrics) => return FetchResult::Found(lyrics),
            ProviderResult::NotFound => {}
            ProviderResult::Error(error) => errors.push(format!("{}：{error}", source.label())),
        }
    }

    if errors.is_empty() {
        FetchResult::NotFound
    } else {
        FetchResult::Error(errors.join("；"))
    }
}

async fn fetch_from_lrclib(client: &reqwest::Client, query: &LyricsQuery) -> ProviderResult {
    let mut params = vec![
        ("artist_name", query.artist.clone()),
        ("track_name", query.title.clone()),
    ];
    if query.duration_secs > 0 {
        params.push(("duration", query.duration_secs.to_string()));
    }

    let response = match client.get(LRCLIB_GET_URL).query(&params).send().await {
        Ok(response) => response,
        Err(error) => return ProviderResult::Error(format!("请求失败：{error}")),
    };

    if response.status().is_success() {
        match response.json::<LrclibTrack>().await {
            Ok(track) => {
                if let Some(lyrics) = usable_synced_lyrics(&track) {
                    return ProviderResult::Found(lyrics);
                }
            }
            Err(error) => return ProviderResult::Error(format!("响应解析失败：{error}")),
        }
    } else if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return ProviderResult::Error("请求过于频繁".into());
    } else if response.status() != reqwest::StatusCode::NOT_FOUND {
        return ProviderResult::Error(format!("返回状态码 {}", response.status()));
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
        Err(error) => return ProviderResult::Error(format!("搜索请求失败：{error}")),
    };

    if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return ProviderResult::Error("搜索请求过于频繁".into());
    }
    if !response.status().is_success() {
        return ProviderResult::Error(format!("搜索返回状态码 {}", response.status()));
    }

    let tracks = match response.json::<Vec<LrclibTrack>>().await {
        Ok(tracks) => tracks,
        Err(error) => return ProviderResult::Error(format!("搜索结果解析失败：{error}")),
    };

    let candidates: Vec<TrackCandidate> = tracks
        .iter()
        .map(|track| TrackCandidate {
            title: track.track_name.clone(),
            artist: track.artist_name.clone(),
            album: track.album_name.clone(),
            duration_secs: track.duration,
        })
        .collect();

    match select_best_candidate(query, &candidates) {
        Some(index) => match usable_synced_lyrics(&tracks[index]) {
            Some(lyrics) => ProviderResult::Found(lyrics),
            None => ProviderResult::NotFound,
        },
        None => ProviderResult::NotFound,
    }
}

async fn fetch_from_netease(client: &reqwest::Client, query: &LyricsQuery) -> ProviderResult {
    let search_text = format!("{} {}", query.artist, query.title);
    let search_params = [
        ("csrf_token", ""),
        ("s", search_text.as_str()),
        ("type", "1"),
        ("offset", "0"),
        ("total", "true"),
        ("limit", "10"),
    ];

    let response = match client
        .get(NETEASE_SEARCH_URL)
        .query(&search_params)
        .header(reqwest::header::REFERER, "https://music.163.com/")
        .header(reqwest::header::USER_AGENT, BROWSER_USER_AGENT)
        .send()
        .await
    {
        Ok(response) => response,
        Err(error) => return ProviderResult::Error(format!("搜索请求失败：{error}")),
    };

    if !response.status().is_success() {
        return ProviderResult::Error(format!("搜索返回状态码 {}", response.status()));
    }

    let search = match response.json::<NeteaseSearchResponse>().await {
        Ok(search) => search,
        Err(error) => return ProviderResult::Error(format!("搜索结果解析失败：{error}")),
    };
    let songs = search.result.map(|result| result.songs).unwrap_or_default();
    if songs.is_empty() {
        return ProviderResult::NotFound;
    }

    let candidates: Vec<TrackCandidate> = songs
        .iter()
        .map(|song| TrackCandidate {
            title: song.name.clone(),
            artist: song
                .artists
                .iter()
                .map(|artist| artist.name.as_str())
                .collect::<Vec<_>>()
                .join(" / "),
            album: song
                .album
                .as_ref()
                .map(|album| album.name.clone())
                .unwrap_or_default(),
            duration_secs: song.duration.map(|duration| duration as f64 / 1000.0),
        })
        .collect();

    let Some(index) = select_best_candidate(query, &candidates) else {
        return ProviderResult::NotFound;
    };
    let song_id = songs[index].id;
    let lyric_params = [
        ("id", song_id.to_string()),
        ("lv", "1".to_string()),
        ("kv", "1".to_string()),
        ("tv", "-1".to_string()),
    ];

    let response = match client
        .get(NETEASE_LYRIC_URL)
        .query(&lyric_params)
        .header(reqwest::header::REFERER, "https://music.163.com/")
        .header(reqwest::header::USER_AGENT, BROWSER_USER_AGENT)
        .send()
        .await
    {
        Ok(response) => response,
        Err(error) => return ProviderResult::Error(format!("歌词请求失败：{error}")),
    };

    if !response.status().is_success() {
        return ProviderResult::Error(format!("歌词返回状态码 {}", response.status()));
    }

    let response = match response.json::<NeteaseLyricResponse>().await {
        Ok(response) => response,
        Err(error) => return ProviderResult::Error(format!("歌词响应解析失败：{error}")),
    };

    match response
        .lrc
        .and_then(|lrc| lrc.lyric)
        .map(|lyrics| lyrics.trim().to_string())
        .filter(|lyrics| !lyrics.is_empty())
    {
        Some(lyrics) => ProviderResult::Found(lyrics),
        None => ProviderResult::NotFound,
    }
}

async fn fetch_from_qq_music(client: &reqwest::Client, query: &LyricsQuery) -> ProviderResult {
    let search_text = format!("{} {}", query.artist, query.title);
    let payload = serde_json::json!({
        "req_1": {
            "method": "DoSearchForQQMusicDesktop",
            "module": "music.search.SearchCgiService",
            "param": {
                "num_per_page": 10,
                "page_num": 1,
                "query": search_text,
                "search_type": 0
            }
        }
    });

    let response = match client
        .post(QQ_SEARCH_URL)
        .header(reqwest::header::REFERER, "https://y.qq.com/")
        .header(reqwest::header::USER_AGENT, BROWSER_USER_AGENT)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(payload.to_string())
        .send()
        .await
    {
        Ok(response) => response,
        Err(error) => return ProviderResult::Error(format!("搜索请求失败：{error}")),
    };

    if !response.status().is_success() {
        return ProviderResult::Error(format!("搜索返回状态码 {}", response.status()));
    }

    let search = match parse_json_or_jsonp::<QqSearchResponse>(response).await {
        Ok(search) => search,
        Err(error) => return ProviderResult::Error(format!("搜索结果解析失败：{error}")),
    };
    let songs = search
        .req_1
        .and_then(|request| request.data)
        .and_then(|data| data.body)
        .and_then(|body| body.song)
        .map(|song| song.list)
        .unwrap_or_default();
    if songs.is_empty() {
        return ProviderResult::NotFound;
    }

    let candidates: Vec<TrackCandidate> = songs
        .iter()
        .map(|song| TrackCandidate {
            title: song.songname.clone(),
            artist: song
                .singer
                .iter()
                .map(|singer| singer.name.as_str())
                .collect::<Vec<_>>()
                .join(" / "),
            album: String::new(),
            duration_secs: song.interval.map(|duration| duration as f64),
        })
        .collect();

    let Some(index) = select_best_candidate(query, &candidates) else {
        return ProviderResult::NotFound;
    };
    let song_mid = songs[index].songmid.clone();
    let lyric_params = [
        ("songmid", song_mid),
        ("format", "json".into()),
        ("nobase64", "1".into()),
    ];

    let response = match client
        .get(QQ_LYRIC_URL)
        .query(&lyric_params)
        .header(reqwest::header::REFERER, "https://y.qq.com/")
        .header(reqwest::header::USER_AGENT, BROWSER_USER_AGENT)
        .send()
        .await
    {
        Ok(response) => response,
        Err(error) => return ProviderResult::Error(format!("歌词请求失败：{error}")),
    };

    if !response.status().is_success() {
        return ProviderResult::Error(format!("歌词返回状态码 {}", response.status()));
    }

    let response = match parse_json_or_jsonp::<QqLyricResponse>(response).await {
        Ok(response) => response,
        Err(error) => return ProviderResult::Error(format!("歌词响应解析失败：{error}")),
    };

    match response
        .lyric
        .map(|lyrics| decode_html_entities(&lyrics).trim().to_string())
        .filter(|lyrics| !lyrics.is_empty())
    {
        Some(lyrics) => ProviderResult::Found(lyrics),
        None => ProviderResult::NotFound,
    }
}

async fn parse_json_or_jsonp<T: DeserializeOwned>(
    response: reqwest::Response,
) -> Result<T, String> {
    let text = response.text().await.map_err(|error| error.to_string())?;
    let trimmed = text.trim();
    let json = if trimmed.starts_with('{') || trimmed.starts_with('[') {
        trimmed
    } else {
        let start = trimmed
            .find('{')
            .ok_or_else(|| "响应不是 JSON".to_string())?;
        let end = trimmed
            .rfind('}')
            .ok_or_else(|| "响应不是 JSON".to_string())?;
        &trimmed[start..=end]
    };

    serde_json::from_str(json).map_err(|error| error.to_string())
}

fn usable_synced_lyrics(track: &LrclibTrack) -> Option<String> {
    track
        .synced_lyrics
        .as_ref()
        .map(|lyrics| lyrics.trim().to_string())
        .filter(|lyrics| !lyrics.is_empty())
}

fn select_best_candidate(query: &LyricsQuery, candidates: &[TrackCandidate]) -> Option<usize> {
    let query_title = normalize(&query.title);
    let query_artist = normalize(&query.artist);

    candidates
        .iter()
        .enumerate()
        .filter_map(|(index, candidate)| {
            let title_matches = normalize(&candidate.title) == query_title;
            let artist_matches = normalize(&candidate.artist) == query_artist;
            let album_matches = !query.album.trim().is_empty()
                && normalize(&candidate.album) == normalize(&query.album);
            let duration_difference = match (candidate.duration_secs, query.duration_secs) {
                (Some(track_duration), query_duration) if query_duration > 0 => {
                    Some((track_duration - query_duration as f64).abs())
                }
                _ => None,
            };

            if !title_matches && !artist_matches {
                return None;
            }
            if duration_difference.is_some_and(|difference| difference > 10.0) {
                return None;
            }

            Some((
                (
                    !title_matches,
                    !artist_matches,
                    duration_difference.unwrap_or(f64::MAX),
                    !album_matches,
                ),
                index,
            ))
        })
        .min_by(|(left_score, left_index), (right_score, right_index)| {
            left_score
                .0
                .cmp(&right_score.0)
                .then_with(|| left_score.1.cmp(&right_score.1))
                .then_with(|| left_score.2.total_cmp(&right_score.2))
                .then_with(|| left_score.3.cmp(&right_score.3))
                .then_with(|| left_index.cmp(right_index))
        })
        .map(|(_, index)| index)
}

fn decode_html_entities(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut rest = input;

    while let Some(start) = rest.find('&') {
        output.push_str(&rest[..start]);
        let after_ampersand = &rest[start + 1..];

        let Some(end) = after_ampersand.find(';') else {
            output.push('&');
            rest = after_ampersand;
            continue;
        };

        if end > 10 {
            output.push('&');
            rest = after_ampersand;
            continue;
        }

        let entity = &after_ampersand[..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "nbsp" => Some(' '),
            _ if entity.starts_with("#x") || entity.starts_with("#X") => {
                u32::from_str_radix(&entity[2..], 16)
                    .ok()
                    .and_then(char::from_u32)
            }
            _ if entity.starts_with('#') => {
                entity[1..].parse::<u32>().ok().and_then(char::from_u32)
            }
            _ => None,
        };

        match decoded {
            Some(character) => {
                output.push(character);
                rest = &after_ampersand[end + 1..];
            }
            None => {
                output.push('&');
                rest = after_ampersand;
            }
        }
    }

    output.push_str(rest);
    output
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

    fn candidate(title: &str, artist: &str, duration: f64) -> TrackCandidate {
        TrackCandidate {
            title: title.into(),
            artist: artist.into(),
            album: String::new(),
            duration_secs: Some(duration),
        }
    }

    #[test]
    fn lyrics_source_cycles_and_builds_priority_order() {
        assert_eq!(LyricsSource::default(), LyricsSource::Lrclib);
        assert_eq!(LyricsSource::Lrclib.next(), LyricsSource::Netease);
        assert_eq!(LyricsSource::Netease.next(), LyricsSource::QqMusic);
        assert_eq!(LyricsSource::QqMusic.next(), LyricsSource::Lrclib);
        assert_eq!(LyricsSource::Lrclib.previous(), LyricsSource::QqMusic);
        assert_eq!(LyricsSource::Netease.previous(), LyricsSource::Lrclib);
        assert_eq!(
            LyricsSource::Netease.download_order(),
            [
                LyricsSource::Netease,
                LyricsSource::Lrclib,
                LyricsSource::QqMusic
            ]
        );
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
            candidate("Song", "Other", 180.0),
            candidate("Song", "Artist", 220.0),
            candidate("Song", "Artist", 181.0),
        ];

        assert_eq!(select_best_candidate(&query, &tracks), Some(2));
    }

    #[test]
    fn rejects_unrelated_results() {
        let query = LyricsQuery {
            artist: "Artist".into(),
            title: "Song".into(),
            album: String::new(),
            duration_secs: 180,
        };
        let tracks = vec![candidate("Different", "Someone", 180.0)];

        assert_eq!(select_best_candidate(&query, &tracks), None);
    }

    #[test]
    fn decodes_common_lyric_entities() {
        let input = "[00:01.00]line 1&#10;[00:02.00]Tom &amp; Jerry &quot;test&quot;";
        let decoded = decode_html_entities(input);

        assert_eq!(decoded, "[00:01.00]line 1\n[00:02.00]Tom & Jerry \"test\"");
    }

    #[test]
    fn parses_netease_response() {
        let json = r#"{"result":{"songs":[{"id":123,"name":"Song","artists":[{"name":"Artist"}],"duration":180000,"album":{"name":"Album"}}]}}"#;
        let response: NeteaseSearchResponse = serde_json::from_str(json).unwrap();
        let songs = response.result.unwrap().songs;

        assert_eq!(songs[0].id, 123);
        assert_eq!(songs[0].artists[0].name, "Artist");
    }

    #[test]
    fn parses_qq_response() {
        let json = r#"{"req_1":{"data":{"body":{"song":{"list":[{"mid":"abc","name":"Song","singer":[{"name":"Artist"}],"interval":180}]}}}}}"#;
        let response: QqSearchResponse = serde_json::from_str(json).unwrap();
        let songs = response
            .req_1
            .unwrap()
            .data
            .unwrap()
            .body
            .unwrap()
            .song
            .unwrap()
            .list;

        assert_eq!(songs[0].songmid, "abc");
        assert_eq!(songs[0].singer[0].name, "Artist");
    }
}
