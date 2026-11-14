use crate::lyrics_download::LyricsSource;
use serde::Deserialize;
use serde::Serialize;
use std::io::{Read, Write};

#[derive(Deserialize, Serialize)]
pub struct Config {
    music_path: String,
    lyrics_path: String,
    volume: f32,
    #[serde(default = "default_auto_download_lyrics")]
    auto_download_lyrics: bool,
    #[serde(default)]
    lyrics_source_priority: LyricsSource,
}

impl Config {
    pub fn load(path: &str) -> Config {
        match std::fs::File::open(path) {
            Ok(mut file) => {
                let mut contents = String::new();
                match file.read_to_string(&mut contents) {
                    Ok(_) => toml::from_str(&contents).unwrap_or_else(|_e| {
                        println!("Failed to load config file: {}", _e);
                        Self::default()
                    }),
                    Err(_e) => {
                        println!("Failed to load config file: {}", _e);
                        Self::default()
                    }
                }
            }
            Err(_e) => {
                println!("Failed to load config file: {}", _e);
                Self::default()
            }
        }
    }

    pub fn save(&self, path: &str) {
        match toml::to_string(&self) {
            Ok(contents) => {
                match std::fs::OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .open(path)
                {
                    Ok(mut file) => {
                        file.write_all(contents.as_bytes()).unwrap_or_else(|_e| {
                            println!("Failed to save config file: {}", _e);
                        });
                    }
                    Err(_e) => {
                        println!("Failed to save config file: {}", _e);
                    }
                }
            }
            Err(_e) => {
                println!("Failed to save config file: {}", _e);
            }
        }
    }

    pub fn volume(&self) -> f32 {
        self.volume
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume;
    }

    pub fn lyrics_path(&self) -> String {
        self.lyrics_path.clone()
    }

    pub fn set_lyrics_path(&mut self, lyrics_path: &str) {
        self.lyrics_path = lyrics_path.to_string();
    }

    pub fn music_path(&self) -> String {
        self.music_path.clone()
    }
    pub fn set_music_path(&mut self, music_path: &str) {
        self.music_path = music_path.to_string();
    }

    pub fn auto_download_lyrics(&self) -> bool {
        self.auto_download_lyrics
    }

    pub fn lyrics_source_priority(&self) -> LyricsSource {
        self.lyrics_source_priority
    }

    pub fn set_lyrics_source_priority(&mut self, source: LyricsSource) {
        self.lyrics_source_priority = source;
    }
}

fn default_auto_download_lyrics() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Config {
            music_path: "".into(),
            lyrics_path: "".into(),
            volume: 1.0,
            auto_download_lyrics: default_auto_download_lyrics(),
            lyrics_source_priority: LyricsSource::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_config_uses_default_lyrics_source_priority() {
        let config: Config = toml::from_str(
            r#"
music_path = "/music"
lyrics_path = "/lyrics"
volume = 0.8
auto_download_lyrics = true
"#,
        )
        .unwrap();

        assert_eq!(config.lyrics_source_priority(), LyricsSource::Lrclib);
    }

    #[test]
    fn lyrics_source_priority_is_serialized() {
        let config = Config {
            lyrics_source_priority: LyricsSource::Netease,
            ..Config::default()
        };
        let serialized = toml::to_string(&config).unwrap();

        assert!(serialized.contains("lyrics_source_priority = \"netease\""));
    }
}
