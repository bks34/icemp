use serde::Deserialize;
use serde::Serialize;
use std::io::{Read, Write};

#[derive(Deserialize, Serialize)]
pub struct Config {
    music_path: String,
    lyrics_path: String,
    volume: f32,
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
}

impl Default for Config {
    fn default() -> Self {
        Config {
            music_path: "".into(),
            lyrics_path: "".into(),
            volume: 1.0,
        }
    }
}
