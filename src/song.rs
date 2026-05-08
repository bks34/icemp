use image::DynamicImage;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::probe::Probe;
use lofty::tag::Accessor;
use rodio::{MixerDeviceSink, Player};
use std::fs::File;
use std::io::BufReader;

// for database
#[derive(Default)]
pub struct SongMetadata {
    path: String,
    title: String,
    artist: String,
}

// for player
pub struct Song {
    song_metadata: SongMetadata,
    player: Option<Player>,
    sink_handle: Option<MixerDeviceSink>,
    duration: u64, //the unit is ms
    prepared: bool,
}

impl Song {
    pub fn from_database(db: SongMetadata) -> Self {
        Self::from_path(db.path)
    }

    pub fn from_path(path: String) -> Self {
        match Probe::open(path.clone()) {
            Ok(probe) => match probe.read() {
                Ok(tagged_fie) => match tagged_fie.primary_tag() {
                    Some(tag) => {
                        let title = match tag.title() {
                            Some(t) => String::from(t),
                            None => String::from("unknown"),
                        };
                        let artist = match tag.artist() {
                            Some(a) => String::from(a),
                            None => String::from("unknown"),
                        };
                        let duration = tagged_fie.properties().duration().as_millis() as u64;
                        return Song {
                            song_metadata: SongMetadata {
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
                    None => {}
                },
                Err(_e) => {
                    #[cfg(debug_assertions)]
                    println!("from_path error: {}", _e);
                }
            },
            Err(_e) => {
                #[cfg(debug_assertions)]
                println!("from_path error: {}", _e);
            }
        }
        Song {
            song_metadata: SongMetadata {
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
        match Probe::open(self.song_metadata.path.clone()) {
            Ok(probe) => match probe.read() {
                Ok(tagged_fie) => match tagged_fie.primary_tag() {
                    Some(tag) => {
                        if tag.picture_count() > 0 {
                            let pic = tag.pictures().first().unwrap().clone();
                            match image::load_from_memory(pic.data()) {
                                Ok(img) => Some(img),
                                Err(_e) => {
                                    #[cfg(debug_assertions)]
                                    println!("get_cover error: {}", _e);
                                    None
                                }
                            }
                        } else {
                            None
                        }
                    }
                    None => None,
                },
                Err(_e) => {
                    #[cfg(debug_assertions)]
                    println!("get_cover error: {}", _e);
                    None
                }
            },
            Err(_e) => {
                #[cfg(debug_assertions)]
                println!("get_cover error: {}", _e);
                None
            }
        }
    }

    pub fn prepare_play(&mut self) {
        match rodio::DeviceSinkBuilder::open_default_sink() {
            Ok(mut sink_handle) => {
                sink_handle.log_on_drop(false);
                match File::open(self.song_metadata.path.clone()) {
                    Ok(fd) => {
                        let file = BufReader::new(fd);
                        match rodio::play(&sink_handle.mixer(), file) {
                            Ok(player) => {
                                player.pause();
                                self.sink_handle = Some(sink_handle);
                                self.player = Some(player);
                                self.prepared = true;
                            }
                            Err(_e) => {
                                #[cfg(debug_assertions)]
                                println!("prepare_play error: {}", _e);
                                self.prepared = false;
                            }
                        }
                    }
                    Err(_e) => {
                        #[cfg(debug_assertions)]
                        println!("prepare_play error: {}", _e);
                        self.prepared = false;
                    }
                }
            }
            Err(_e) => {
                #[cfg(debug_assertions)]
                println!("prepare_play error: {}", _e);
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
        // file name in Linux OS can't contain '/'
        #[cfg(target_os = "linux")]
        return self.song_metadata.title.replace('/', " ");
        #[cfg(target_os = "windows")]
        return self.song_database.title.replace('/', ",");
    }

    pub fn artist(&self) -> String {
        // file name in Linux OS can't contain '/'
        #[cfg(target_os = "linux")]
        return self.song_metadata.artist.replace('/', " ");
        #[cfg(target_os = "windows")]
        return self.song_database.artist.replace('/', ",");
    }

    pub fn path(&self) -> String {
        self.song_metadata.path.clone()
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
            Some(player) => player.get_pos().as_millis() as u64,
            None => 0,
        }
    }

    pub fn set_pos(&mut self, pos: u64) {
        println!("try seek to {}s", pos / 1000);
        match self
            .player
            .as_ref()
            .unwrap()
            .try_seek(std::time::Duration::from_millis(pos))
        {
            Ok(_) => {}
            Err(_e) => {
                #[cfg(debug_assertions)]
                println!("Error when trying to seek playback: {}", _e);
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
