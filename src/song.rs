use image::DynamicImage;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::probe::Probe;
use lofty::tag::Accessor;
use rodio::{MixerDeviceSink, Player};
use std::fs::File;
use std::io::BufReader;

// for database
#[derive(Default)]
pub struct SongDataBase {
    path: String,
    title: String,
    artist: String,
}

// for player
pub struct Song {
    in_database: bool,
    song_database: SongDataBase,
    player: Option<Player>,
    sink_handle: Option<MixerDeviceSink>,
    duration: u64,
    prepared: bool,
}

impl Song {
    pub fn from_database(db: SongDataBase) -> Self {
        Self::from_path(db.path)
    }

    pub fn from_path(path: String) -> Self {
        match Probe::open(path.clone()) {
            Ok(probe) => match probe.read() {
                Ok(tagged_fie) => match tagged_fie.primary_tag() {
                    Some(tag) => {
                        let title = String::from(tag.title().unwrap());
                        let artist = String::from(tag.artist().unwrap());
                        let duration = tagged_fie.properties().duration().as_secs();
                        return Song {
                            in_database: false,
                            song_database: SongDataBase {
                                title,
                                artist,
                                path,
                            },
                            player: None,
                            sink_handle: None,
                            duration,
                            prepared: false,
                        };
                    }
                    None => {},
                },
                Err(e) => {
                    println!("from_path error: {}", e);
                }
            },
            Err(e) => {
                println!("from_path error: {}", e);
            }
        }
        Song {
            in_database: false,
            song_database: SongDataBase {
                title: "unknown".into(),
                artist: "unknown".into(),
                path,
            },
            player: None,
            sink_handle: None,
            duration: 0,
            prepared: false,
        }
    }

    pub fn get_cover(&self) -> Option<DynamicImage> {
        match Probe::open(self.song_database.path.clone()) {
            Ok(probe) => match probe.read() {
                Ok(tagged_fie) => match tagged_fie.primary_tag() {
                    Some(tag) => {
                        if tag.picture_count() > 0 {
                            let pic = tag.pictures().first().unwrap().clone();
                            let img = image::load_from_memory(pic.data()).unwrap();
                            Some(img)
                        } else {
                            None
                        }
                    }
                    None => None,
                },
                Err(e) => {
                    println!("get_cover error: {}", e);
                    None
                }
            },
            Err(e) => {
                println!("get_cover error: {}", e);
                None
            }
        }
    }

    pub fn prepare_play(&mut self) {
        match rodio::DeviceSinkBuilder::open_default_sink() {
            Ok(mut sink_handle) => {
                sink_handle.log_on_drop(false);
                match File::open(self.song_database.path.clone()) {
                    Ok(fd) => {
                        let file = BufReader::new(fd);
                        match rodio::play(&sink_handle.mixer(), file) {
                            Ok(player) => {
                                player.pause();
                                self.sink_handle = Some(sink_handle);
                                self.player = Some(player);
                                self.prepared = true;
                            }
                            Err(e) => {
                                println!("prepare_play error: {}", e);
                                self.prepared = false;
                            }
                        }
                    }
                    Err(e) => {
                        println!("prepare_play error: {}", e);
                        self.prepared = false;
                    }
                }
            }
            Err(e) => {
                println!("prepare_play error: {}", e);
                self.prepared = false;
            }
        }
    }

    pub fn end_play(&mut self) {
        self.player = None;
        self.sink_handle = None;
        self.prepared = false;
    }

    pub fn prepared(&self) -> bool {
        self.prepared
    }

    pub fn title(&self) -> String {
        self.song_database.title.clone()
    }

    pub fn artist(&self) -> String {
        self.song_database.artist.clone()
    }

    pub fn duration(&self) -> u64 {
        self.duration
    }
    pub fn play(&self) {
        match self.player.as_ref() {
            Some(player) => player.play(),
            None => {}
        }
    }

    pub fn pause(&self) {
        match self.player.as_ref() {
            Some(player) => player.pause(),
            None => {}
        }
    }

    pub fn get_pos(&self) -> u64 {
        match self.player.as_ref() {
            Some(player) => player.get_pos().as_secs(),
            None => 0,
        }
    }

    pub fn set_pos(&mut self, pos: u64) {
        println!("try seek to {}s", pos);
        match self
            .player
            .as_ref()
            .unwrap()
            .try_seek(std::time::Duration::from_secs(pos))
        {
            Ok(_) => {}
            Err(e) => {
                println!("Error when trying to seek playback: {}", e);
            }
        }
    }

    pub fn playback_ended(&self) -> bool {
        match self.player.as_ref() {
            Some(player) => player.empty(),
            None => false,
        }
    }

    pub fn set_volume(&self, volume: f32) {
        match self.player.as_ref() {
            Some(player) => player.set_volume(volume),
            None => {}
        }
    }
}
