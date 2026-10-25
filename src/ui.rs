mod lyrics;
mod utils;

use crate::config::Config;
use crate::lyrics_download::{FetchResult, LyricsQuery, fetch_lyrics};
use crate::song::Song;
use bytes::Bytes;
use iced::{
    Background, Border, Color, Element, Font, Length, Radians, Rotation, Shadow, Subscription,
    Task, Theme, Vector,
    alignment::{Horizontal, Vertical},
    widget::{
        button, column, container, mouse_area, responsive, row, scrollable, slider, space, span,
        stack, text, tooltip,
    },
};
use image::DynamicImage;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Default)]
enum Page {
    #[default]
    PlayLists,
    CoverAndLyrics,
}

#[derive(Default)]
enum PlayStatus {
    #[default]
    Pause,
    Play,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum RepeatMode {
    #[default]
    None,
    All,
    One,
}

#[derive(Default, Clone, PartialEq, Eq)]
enum LyricsStatus {
    #[default]
    NotPresent,
    Local,
    Downloading,
    Downloaded,
    NotFound,
    Failed(String),
}

pub struct MusicPlayer {
    // Page
    page: Page,

    // Play status
    play_status: PlayStatus,
    playback_time: f32,
    volume: f32,
    cover_angle: f32,
    rotation_speed: f32,
    bass_slow: f32,
    bass_pulse: f32,
    accent_color: Color,
    last_animation_tick: Instant,

    // theme and background image
    cover: DynamicImage,
    cover_handle: iced::widget::image::Handle,
    background_image_handle: iced::widget::image::Handle,

    default_cover: DynamicImage,

    // songs
    songs: Vec<Song>,
    song_index: usize,
    shuffle: bool,
    repeat_mode: RepeatMode,

    // lyrics
    lrc: Option<lyrics::Lrc>,
    current_lyrics: String,
    lyrics_status: LyricsStatus,
    lyrics_request_id: u64,
    lyrics_initialized: bool,

    // config
    config: Config,
    config_path: PathBuf,
}

#[derive(Clone)]
pub enum Message {
    ChangePage,
    ChangePlayStatus,
    PlaybackSliderChanged(f32),
    VolumeChanged(f32),
    NextSong,
    LastSong,
    ChangeSong(usize),
    UpdatePlayBackTime,
    OpenMusicFolder,
    ChangeLyricsFolder,
    ToggleShuffle,
    ToggleRepeatMode,
    CloseWindow,
    InitializeLyrics,
    LyricsFetched {
        request_id: u64,
        result: FetchResult,
    },
}

impl MusicPlayer {
    pub fn boot() -> MusicPlayer {
        let default_cover =
            image::load_from_memory(include_bytes!("../images/record.png")).unwrap();

        let config_path = dirs::config_dir()
            .unwrap()
            .join("icemp")
            .join("config.toml");
        if !config_path.exists() {
            if !config_path.parent().unwrap().exists() {
                std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
            }
            std::fs::File::create(config_path.as_path()).unwrap();
        };

        let mut music_player = MusicPlayer {
            cover: default_cover.clone(),
            cover_handle: iced::widget::image::Handle::from_bytes(Bytes::new()),
            background_image_handle: iced::widget::image::Handle::from_bytes(Bytes::new()),
            default_cover,
            page: Default::default(),
            play_status: Default::default(),
            playback_time: 0.0,
            cover_angle: 0.0,
            rotation_speed: 0.0,
            bass_slow: 0.0,
            bass_pulse: 0.0,
            accent_color: Color::from_rgb(0.35, 0.45, 0.65),
            last_animation_tick: Instant::now(),
            songs: Vec::new(),
            song_index: 0,
            shuffle: false,
            repeat_mode: RepeatMode::default(),
            lrc: None,
            current_lyrics: "No lyrics available".into(),
            lyrics_status: LyricsStatus::default(),
            lyrics_request_id: 0,
            lyrics_initialized: false,
            volume: 1.0,
            config: Default::default(),
            config_path,
        };

        let mut config = Config::load(music_player.config_path.to_str().unwrap());

        if config.music_path().is_empty() {
            config.set_music_path(dirs::audio_dir().unwrap().to_str().unwrap());
        }
        if config.lyrics_path().is_empty() {
            config.set_lyrics_path(
                PathBuf::from(config.music_path())
                    .join("lyrics")
                    .to_str()
                    .unwrap(),
            );
        }

        config.save(music_player.config_path.to_str().unwrap());

        let music_path = PathBuf::from(config.music_path());
        music_player.config = config;
        music_player.update_songs(music_path);

        music_player.volume = music_player.config.volume();
        music_player.songs[music_player.song_index].set_volume(music_player.volume);
        music_player.play_status = PlayStatus::Pause;
        music_player.songs[music_player.song_index].pause();

        music_player
    }

    pub fn theme(&self) -> Option<Theme> {
        let res = utils::get_theme_from_image_color(&self.cover);
        Some(res)
    }

    pub fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::ChangePage => match self.page {
                Page::PlayLists => self.page = Page::CoverAndLyrics,
                Page::CoverAndLyrics => self.page = Page::PlayLists,
            },
            Message::ChangePlayStatus => match self.play_status {
                PlayStatus::Play => {
                    self.play_status = PlayStatus::Pause;
                    self.songs[self.song_index].pause();
                }
                PlayStatus::Pause => {
                    if !self.songs[self.song_index].prepared() {
                        self.songs[self.song_index].prepare_play()
                    }
                    self.play_status = PlayStatus::Play;
                    self.songs[self.song_index].play();
                }
            },
            Message::PlaybackSliderChanged(val) => {
                self.playback_time = val;
                let pos = (val / 100.0 * self.songs[self.song_index].duration() as f32) as u64;
                if pos >= self.songs[self.song_index].get_pos() {
                    self.songs[self.song_index].set_pos(pos);
                } else {
                    self.songs[self.song_index].end_play();
                    self.songs[self.song_index].prepare_play();
                    self.songs[self.song_index].play();
                    self.songs[self.song_index].set_pos(pos);
                }
            }
            Message::VolumeChanged(val) => {
                self.volume += if val > 0.0 { 0.05 } else { -0.05 };
                if self.volume > 1.0 {
                    self.volume = 1.0;
                }
                if self.volume < 0.0 {
                    self.volume = 0.0;
                }
                self.songs[self.song_index].set_volume(self.volume);
                self.config.set_volume(self.volume);
            }
            Message::NextSong => {
                self.change_song(self.next_song_id());
                return self.start_lyrics_download();
            }
            Message::LastSong => {
                self.change_song(self.prev_song_id());
                return self.start_lyrics_download();
            }
            Message::ChangeSong(id) => {
                self.change_song(id);
                return self.start_lyrics_download();
            }
            Message::UpdatePlayBackTime => {
                let now = Instant::now();
                let delta = now
                    .saturating_duration_since(self.last_animation_tick)
                    .as_secs_f32()
                    .min(0.1);
                self.last_animation_tick = now;
                self.update_animation(delta);

                let pos = self.songs[self.song_index].get_pos();
                let duration = self.songs[self.song_index].duration();
                self.playback_time = pos as f32 / duration as f32 * 100.0;
                if self.songs[self.song_index].playback_ended() {
                    match self.repeat_mode {
                        RepeatMode::One => {
                            self.songs[self.song_index].end_play();
                            self.songs[self.song_index].prepare_play();
                            self.songs[self.song_index].play();
                            self.playback_time = 0.0;
                        }
                        RepeatMode::None
                            if !self.shuffle && self.song_index + 1 >= self.songs.len() =>
                        {
                            self.play_status = PlayStatus::Pause;
                        }
                        _ => return Task::done(Message::NextSong),
                    }
                }
                if let Some(lrc) = self.lrc.as_ref() {
                    self.current_lyrics = lrc.get_lyrics(pos);
                }
            }
            Message::OpenMusicFolder => {
                let folder = rfd::FileDialog::new()
                    .set_title("Select music folder")
                    .pick_folder();
                match folder {
                    Some(path) => {
                        self.update_songs(path.clone());
                        self.config.set_music_path(path.to_str().unwrap());
                        self.config.save(self.config_path.to_str().unwrap());
                        return self.start_lyrics_download();
                    }
                    None => {}
                }
            }
            Message::ChangeLyricsFolder => {
                let folder = rfd::FileDialog::new()
                    .set_title("Select lyrics folder")
                    .pick_folder();
                match folder {
                    Some(path) => {
                        self.config.set_lyrics_path(path.to_str().unwrap());
                        self.config.save(self.config_path.to_str().unwrap());
                        self.reload_lyrics_for_current_song();
                        return self.start_lyrics_download();
                    }
                    None => {}
                }
            }
            Message::CloseWindow => {
                println!("exit");
                self.config.save(self.config_path.to_str().unwrap());
            }
            Message::ToggleShuffle => {
                self.shuffle = !self.shuffle;
            }
            Message::ToggleRepeatMode => {
                self.repeat_mode = match self.repeat_mode {
                    RepeatMode::None => RepeatMode::All,
                    RepeatMode::All => RepeatMode::One,
                    RepeatMode::One => RepeatMode::None,
                };
            }
            Message::InitializeLyrics => {
                if !self.lyrics_initialized {
                    self.lyrics_initialized = true;
                    return self.start_lyrics_download();
                }
            }
            Message::LyricsFetched { request_id, result } => {
                return self.handle_lyrics_fetched(request_id, result);
            }
        }
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let content_up = match self.page {
            Page::PlayLists => ui_element_playlists(self),
            Page::CoverAndLyrics => ui_element_panel(self),
        };
        column![content_up, ui_element_player(self)].into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let exit_listen = iced::event::listen_with(|event, _, _| match event {
            iced::Event::Window(iced::window::Event::CloseRequested) => Some(Message::CloseWindow),
            _ => None,
        });

        let keyboard_listen = iced::event::listen_with(|event, _, _| match event {
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. }) => match key {
                iced::keyboard::Key::Named(iced::keyboard::key::Named::Space) => {
                    Some(Message::ChangePlayStatus)
                }
                iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowLeft) => {
                    Some(Message::LastSong)
                }
                iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowRight) => {
                    Some(Message::NextSong)
                }
                iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowUp) => {
                    Some(Message::VolumeChanged(0.05))
                }
                iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowDown) => {
                    Some(Message::VolumeChanged(-0.05))
                }
                iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter) => {
                    Some(Message::ChangePage)
                }
                _ => None,
            },
            _ => None,
        });

        let mut listens = vec![exit_listen, keyboard_listen];
        if !self.lyrics_initialized {
            listens.push(
                iced::time::every(iced::time::Duration::from_millis(50))
                    .map(|_| Message::InitializeLyrics),
            );
        }
        if self.songs[self.song_index].prepared() {
            listens.push(
                iced::time::every(iced::time::Duration::from_millis(16))
                    .map(|_| Message::UpdatePlayBackTime),
            );
        }
        iced::Subscription::batch(listens)
    }

    fn update_songs(&mut self, path: PathBuf) {
        if self.songs.len() > 0 {
            self.songs[self.song_index].end_play();
        }
        self.songs.clear();
        for entry in std::fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_file() {
                let song = Song::from_path(path.to_str().unwrap().to_string());
                if song.duration() > 0 {
                    self.songs.push(song);
                }
            }
        }
        if self.songs.len() == 0 {
            self.songs.push(Song::from_path("unknown".into()));
        }
        self.songs.sort_by(|s1, s2| s1.artist().cmp(&s2.artist()));
        self.change_song(0);
    }

    fn change_song(&mut self, id: usize) {
        self.songs[self.song_index].end_play();
        self.song_index = id;
        self.songs[self.song_index].prepare_play();
        self.change_cover(self.song_index);
        self.songs[self.song_index].play();
        self.play_status = PlayStatus::Play;
        self.bass_slow = 0.0;
        self.bass_pulse = 0.0;

        self.reload_lyrics_for_current_song();
    }

    fn update_animation(&mut self, delta_seconds: f32) {
        let playing = matches!(self.play_status, PlayStatus::Play);
        let target_speed = if playing {
            std::f32::consts::TAU / 12.0
        } else {
            0.0
        };
        let speed_response = 1.0 - (-delta_seconds * 4.0).exp();
        self.rotation_speed += (target_speed - self.rotation_speed) * speed_response;
        self.cover_angle = (self.cover_angle + self.rotation_speed * delta_seconds)
            .rem_euclid(std::f32::consts::TAU);

        let features = self.songs[self.song_index].audio_features();
        let (target_slow, target_pulse) = if playing {
            (features.bass_slow, features.bass_pulse)
        } else {
            (0.0, 0.0)
        };

        let slow_response = if target_slow > self.bass_slow {
            1.0 - (-delta_seconds * 18.0).exp()
        } else {
            1.0 - (-delta_seconds * 5.0).exp()
        };
        self.bass_slow += (target_slow - self.bass_slow) * slow_response;
        self.bass_slow = self.bass_slow.clamp(0.0, 1.0);

        let pulse_response = if target_pulse > self.bass_pulse {
            1.0 - (-delta_seconds * 36.0).exp()
        } else {
            1.0 - (-delta_seconds * 4.5).exp()
        };
        self.bass_pulse += (target_pulse - self.bass_pulse) * pulse_response;
        self.bass_pulse = self.bass_pulse.clamp(0.0, 1.0);
    }

    fn lyrics_file_path(&self) -> PathBuf {
        let lyrics_file_name = format!(
            "{} - {}.lrc",
            self.songs[self.song_index].artist(),
            self.songs[self.song_index].title()
        );
        PathBuf::from(self.config.lyrics_path()).join(lyrics_file_name)
    }

    fn reload_lyrics_for_current_song(&mut self) {
        let lyrics_path = self.lyrics_file_path();

        if lyrics_path.is_file() {
            let lyrics_path = lyrics_path.to_string_lossy().into_owned();
            self.lrc = Some(lyrics::Lrc::from_path(lyrics_path));
            self.lyrics_status = LyricsStatus::Local;
            let pos = self.songs[self.song_index].get_pos();
            self.current_lyrics = self
                .lrc
                .as_ref()
                .map(|lrc| lrc.get_lyrics(pos))
                .unwrap_or_default();
        } else {
            self.lrc = None;
            self.lyrics_status = LyricsStatus::NotPresent;
            self.current_lyrics = "No lyrics available".into();
        }
    }

    fn start_lyrics_download(&mut self) -> Task<Message> {
        if !self.config.auto_download_lyrics()
            || self.lrc.is_some()
            || matches!(self.lyrics_status, LyricsStatus::Downloading)
        {
            return Task::none();
        }

        let song = &self.songs[self.song_index];
        let query = LyricsQuery {
            artist: song.artist(),
            title: song.title(),
            album: song.album(),
            duration_secs: song.duration() / 1000,
        };

        self.lyrics_request_id = self.lyrics_request_id.wrapping_add(1);
        let request_id = self.lyrics_request_id;
        self.lyrics_status = LyricsStatus::Downloading;
        self.current_lyrics = "Searching for lyrics...".into();

        Task::perform(fetch_lyrics(query), move |result| Message::LyricsFetched {
            request_id,
            result,
        })
    }

    fn handle_lyrics_fetched(&mut self, request_id: u64, result: FetchResult) -> Task<Message> {
        if request_id != self.lyrics_request_id
            || !matches!(self.lyrics_status, LyricsStatus::Downloading)
        {
            return Task::none();
        }

        match result {
            FetchResult::Found(lyrics_text) => {
                let lyrics_path = self.lyrics_file_path();
                if let Err(error) = save_lyrics_file(&lyrics_path, &lyrics_text) {
                    let message = format!("Failed to save lyrics: {error}");
                    println!("{message}");
                    self.lyrics_status = LyricsStatus::Failed(message.clone());
                    self.current_lyrics = message;
                    return Task::none();
                }

                let lyrics_path = lyrics_path.to_string_lossy().into_owned();
                self.lrc = Some(lyrics::Lrc::from_path(lyrics_path));
                self.lyrics_status = LyricsStatus::Downloaded;
                let pos = self.songs[self.song_index].get_pos();
                self.current_lyrics = self
                    .lrc
                    .as_ref()
                    .map(|lrc| lrc.get_lyrics(pos))
                    .unwrap_or_default();
            }
            FetchResult::NotFound => {
                self.lyrics_status = LyricsStatus::NotFound;
                self.current_lyrics = "No lyrics found online".into();
            }
            FetchResult::Error(error) => {
                println!("Failed to download lyrics: {error}");
                self.lyrics_status = LyricsStatus::Failed(error.clone());
                self.current_lyrics = format!("Lyrics download failed: {error}");
            }
        }

        Task::none()
    }

    fn change_cover(&mut self, id: usize) {
        let cover = match self.songs[id].get_cover() {
            Some(cover) => cover,
            None => self.default_cover.clone(),
        };

        // The background is heavily blurred and stretched, so blurring a
        // downscaled copy gives the same visual result with much less CPU work.
        const MAX_BLUR_DIMENSION: u32 = 1024;
        let original_max_dimension = cover.width().max(cover.height()).max(1) as f32;
        let blur_input =
            if cover.width() > MAX_BLUR_DIMENSION || cover.height() > MAX_BLUR_DIMENSION {
                cover.resize(
                    MAX_BLUR_DIMENSION,
                    MAX_BLUR_DIMENSION,
                    image::imageops::FilterType::Lanczos3,
                )
            } else {
                cover.clone()
            };
        let blur_scale =
            blur_input.width().max(blur_input.height()) as f32 / original_max_dimension;
        let blur_radius = (40.0 * blur_scale).round().max(12.0) as u32;

        let background_image = libblur::fast_gaussian_blur_image(
            blur_input,
            libblur::AnisotropicRadius::new(blur_radius),
            libblur::EdgeMode2D::default(),
            libblur::ThreadingPolicy::Adaptive,
        )
        .unwrap();

        self.cover = cover;
        self.accent_color = utils::get_accent_color_from_image(&self.cover);
        self.cover_handle = dynamic_image_handle(&self.cover);
        self.background_image_handle = dynamic_image_handle(&background_image);
    }

    fn next_song_id(&self) -> usize {
        if self.shuffle {
            self.random_id()
        } else {
            (self.song_index + 1) % self.songs.len()
        }
    }

    fn prev_song_id(&self) -> usize {
        if self.shuffle {
            self.random_id()
        } else {
            ((self.song_index as i32 + self.songs.len() as i32 - 1) % self.songs.len() as i32)
                as usize
        }
    }

    fn random_id(&self) -> usize {
        if self.songs.len() <= 1 {
            return 0;
        }
        use rand::RngExt;
        let mut rng = rand::rng();
        loop {
            let id = rng.random_range(0..self.songs.len());
            if id != self.song_index {
                return id;
            }
        }
    }
}

fn dynamic_image_handle(image: &DynamicImage) -> iced::widget::image::Handle {
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    iced::widget::image::Handle::from_rgba(width, height, rgba.into_raw())
}

fn save_lyrics_file(path: &Path, lyrics: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(mut file) => {
            if let Err(error) = file.write_all(lyrics.as_bytes()) {
                drop(file);
                let _ = std::fs::remove_file(path);
                return Err(error);
            }
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(error),
    }
}

fn ui_element_pulse_line(
    accent: Color,
    slow: f32,
    pulse: f32,
    vertical: Vertical,
) -> Element<'static, Message> {
    responsive(move |size| {
        let width = (size.width * (0.06 + slow * 0.12 + pulse * 0.52)).max(10.0);
        let alpha = (0.14 + slow * 0.18 + pulse * 0.78).clamp(0.0, 1.0);

        let line = container(space())
            .width(Length::Fixed(width))
            .height(Length::Fixed(4.0))
            .style(move |_| container::Style {
                background: Some(Background::Color(with_alpha(accent, alpha))),
                border: Border {
                    radius: iced::border::radius(1.5),
                    ..Default::default()
                },
                ..Default::default()
            });

        container(line)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Horizontal::Center)
            .align_y(vertical)
            .into()
    })
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

// elements helper
fn ui_element_panel(status: &MusicPlayer) -> Element<'_, Message> {
    let back_image = iced::widget::image(&status.background_image_handle)
        .width(Length::Fill)
        .height(Length::Fill)
        .content_fit(iced::ContentFit::Cover)
        .scale(1.03_f32)
        .opacity(0.76_f32);

    let cover = ui_element_record(status);
    let lyrics = ui_element_lyrics(status);

    stack!(
        back_image,
        row![
            space().width(Length::FillPortion(1)),
            cover,
            lyrics,
            space().width(Length::FillPortion(1))
        ]
    )
    .height(Length::FillPortion(4))
    .width(Length::FillPortion(1))
    .into()
}

fn ui_element_record(status: &MusicPlayer) -> Element<'_, Message> {
    let slow = status.bass_slow.clamp(0.0, 1.0);
    let pulse = visual_pulse(status);

    responsive(move |size| {
        let pulse_scale = 1.0 + pulse * 0.012;
        let disc_diameter = (size.width.min(size.height) * 0.80 * pulse_scale).max(80.0);
        let disc_radius = disc_diameter / 2.0;
        let artwork_diameter = (disc_diameter - 12.0).max(1.0);
        let artwork_radius = artwork_diameter / 2.0;
        let hole_diameter = (disc_diameter * 0.055).max(6.0);
        let hole_radius = hole_diameter / 2.0;

        let artwork = iced::widget::image(&status.cover_handle)
            .width(Length::Fixed(artwork_diameter))
            .height(Length::Fixed(artwork_diameter))
            .content_fit(iced::ContentFit::Cover)
            .rotation(Rotation::Floating(Radians(status.cover_angle)))
            .border_radius(artwork_radius);

        let glow_color = mix_color(status.accent_color, Color::WHITE, 0.42);
        let glow_core = mix_color(status.accent_color, Color::WHITE, 0.72);
        let ring_alpha = 0.18 + slow * 0.28 + pulse * 0.70;
        let disc = container(artwork)
            .center(Length::Fixed(disc_diameter))
            .style(move |_| container::Style {
                background: Some(Background::Color(Color::from_rgb(0.025, 0.025, 0.03))),
                border: Border {
                    color: with_alpha(glow_core, ring_alpha),
                    width: 1.5 + pulse * 3.0,
                    radius: iced::border::radius(disc_radius),
                },
                ..Default::default()
            });
        let disc_overlay = container(disc)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill);

        let center_hole = container(space())
            .width(Length::Fixed(hole_diameter))
            .height(Length::Fixed(hole_diameter))
            .style(move |_| container::Style {
                background: Some(Background::Color(Color::from_rgb(0.015, 0.015, 0.02))),
                border: Border {
                    color: with_alpha(status.accent_color, 0.20 + pulse * 0.40),
                    width: 1.0,
                    radius: iced::border::radius(hole_radius),
                },
                ..Default::default()
            });
        let hole_overlay = container(center_hole)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill);

        let glow_alpha = 0.04 + slow * 0.22 + pulse * 0.58;
        let outer_glow = ui_element_glow(
            disc_diameter * 1.30,
            with_alpha(glow_color, glow_alpha * 0.26),
        );
        let middle_glow = ui_element_glow(
            disc_diameter * 1.18,
            with_alpha(glow_color, glow_alpha * 0.48),
        );
        let inner_glow = ui_element_glow(
            disc_diameter * 1.08,
            with_alpha(glow_core, glow_alpha * 0.76),
        );
        let glow_ring = ui_element_glow_ring(
            disc_diameter * 1.16,
            with_alpha(glow_core, 0.16 + slow * 0.24 + pulse * 0.60),
            1.5 + pulse * 2.5,
        );

        container(
            stack![
                outer_glow,
                middle_glow,
                glow_ring,
                inner_glow,
                disc_overlay,
                hole_overlay
            ]
            .width(Length::Fill)
            .height(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
    })
    .width(Length::FillPortion(4))
    .height(Length::Fill)
    .into()
}

fn ui_element_glow(diameter: f32, color: Color) -> Element<'static, Message> {
    container(
        container(space())
            .center(Length::Fixed(diameter))
            .style(move |_| container::Style {
                background: Some(Background::Color(color)),
                border: Border {
                    radius: iced::border::radius(diameter / 2.0),
                    ..Default::default()
                },
                ..Default::default()
            }),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .into()
}

fn ui_element_glow_ring(diameter: f32, color: Color, width: f32) -> Element<'static, Message> {
    container(
        container(space())
            .center(Length::Fixed(diameter))
            .style(move |_| container::Style {
                background: None,
                border: Border {
                    color,
                    width,
                    radius: iced::border::radius(diameter / 2.0),
                },
                ..Default::default()
            }),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .into()
}

fn with_alpha(mut color: Color, alpha: f32) -> Color {
    color.a = alpha.clamp(0.0, 1.0);
    color
}

fn visual_pulse(status: &MusicPlayer) -> f32 {
    (status.bass_pulse * 1.8).clamp(0.0, 1.0).powf(0.75)
}

fn mix_color(from: Color, to: Color, amount: f32) -> Color {
    let amount = amount.clamp(0.0, 1.0);
    Color::from_rgba(
        from.r + (to.r - from.r) * amount,
        from.g + (to.g - from.g) * amount,
        from.b + (to.b - from.b) * amount,
        from.a + (to.a - from.a) * amount,
    )
}

fn ui_element_lyrics(status: &MusicPlayer) -> Element<'_, Message> {
    iced::widget::rich_text([
        span(format!("{}\n", status.songs[status.song_index].title()))
            .size(38)
            .padding(5),
        span(format!("{}\n", status.songs[status.song_index].artist())).size(18),
        span("\n"),
        span("\n"),
        span(format!("{}\n", status.current_lyrics)).size(20),
    ])
    .on_link_click(iced::never)
    .center()
    .width(Length::FillPortion(6))
    .height(Length::Fill)
    .into()
}

fn ui_element_playlists(status: &MusicPlayer) -> Element<'_, Message> {
    let slow = status.bass_slow.clamp(0.0, 1.0);
    let pulse = visual_pulse(status);
    let accent = status.accent_color;

    let playlists: Vec<String> = status
        .songs
        .iter()
        .enumerate()
        .map(|item| {
            if item.0 == status.song_index {
                format!("{} {} - {}", '*', item.1.artist(), item.1.title())
            } else {
                format!("{:04} {} - {}", item.0 + 1, item.1.artist(), item.1.title())
            }
        })
        .collect();
    let content = column(playlists.iter().enumerate().map(|item| {
        let is_current = item.0 == status.song_index;
        let indicator_alpha = if is_current {
            0.30 + slow * 0.20 + pulse * 0.45
        } else {
            0.0
        };
        let indicator_color = mix_color(accent, Color::WHITE, 0.42);
        let indicator = container(space())
            .width(Length::Fixed(4.0))
            .height(Length::Fixed(24.0))
            .style(move |_| container::Style {
                background: Some(Background::Color(with_alpha(
                    indicator_color,
                    indicator_alpha,
                ))),
                border: Border {
                    radius: iced::border::radius(2.0),
                    ..Default::default()
                },
                shadow: Shadow {
                    color: with_alpha(indicator_color, indicator_alpha * 0.55),
                    offset: Vector::new(0.0, 0.0),
                    blur_radius: 3.0 + pulse * 6.0,
                },
                ..Default::default()
            });

        row![
            space().width(Length::FillPortion(1)),
            indicator,
            button(text(item.1.clone()).line_height(1.3))
                .padding(5.0)
                .style(move |theme, button_status| {
                    ui_style_playlists_item_button_animated(
                        theme,
                        button_status,
                        is_current,
                        accent,
                        slow,
                        pulse,
                    )
                })
                .on_press(Message::ChangeSong(item.0))
                .width(Length::FillPortion(17)),
            space().width(Length::FillPortion(1)),
        ]
        .into()
    }));

    let toolbar = container(row![
        tooltip(
            button(ui_element_playlists_open_music_folder_button())
                .on_press(Message::OpenMusicFolder)
                .width(Length::FillPortion(1))
                .height(Length::FillPortion(1)),
            "open music folder",
            tooltip::Position::Bottom
        )
        .style(ui_style_tooltip)
        .delay(iced::time::Duration::from_millis(500)),
        tooltip(
            button(ui_element_playlists_change_lyrics_folder_button())
                .on_press(Message::ChangeLyricsFolder)
                .width(Length::FillPortion(1))
                .height(Length::FillPortion(1)),
            "change lyrics folder",
            tooltip::Position::Bottom
        )
        .style(ui_style_tooltip)
        .delay(iced::time::Duration::from_millis(500)),
        space().width(Length::FillPortion(7)),
        tooltip(
            text(format!(
                "{:04}|{:04}",
                status.song_index + 1,
                status.songs.len()
            ))
            .center()
            .width(Length::Fixed(90.0))
            .height(Length::FillPortion(1)),
            "the index of music which is playing\n and the total number of music",
            tooltip::Position::Bottom
        )
        .style(ui_style_tooltip)
        .delay(iced::time::Duration::from_millis(500)),
    ])
    .width(Length::Fill)
    .height(Length::Fixed(50.0))
    .style(move |theme| ui_style_animated_bar(theme, accent, slow, pulse, 0.7));

    let toolbar = stack![
        toolbar,
        ui_element_pulse_line(accent, slow, pulse, Vertical::Bottom)
    ]
    .width(Length::Fill)
    .height(Length::Fixed(50.0));

    column![
        toolbar,
        scrollable::Scrollable::new(content)
            .style(move |theme, scrollable_status| {
                ui_style_playlists_animated(theme, scrollable_status, accent, slow, pulse)
            })
            .width(Length::Fill)
            .height(Length::FillPortion(8))
    ]
    .into()
}

fn ui_element_player(status: &MusicPlayer) -> Element<'_, Message> {
    let slow = status.bass_slow.clamp(0.0, 1.0);
    let pulse = visual_pulse(status);
    let accent = status.accent_color;

    let playlists_button_content = match status.page {
        Page::PlayLists => ui_element_player_playlists_close_button(),
        Page::CoverAndLyrics => ui_element_player_playlists_open_button(),
    };
    let play_button_content = match status.play_status {
        PlayStatus::Play => ui_element_player_pause_button(),
        PlayStatus::Pause => ui_element_player_play_button(),
    };

    let player_bar = container(column!(
        row![
            container(
                row![
                    space().width(Length::FillPortion(1)),
                    button(playlists_button_content)
                        .on_press(Message::ChangePage)
                        .height(Length::FillPortion(1))
                        .width(Length::FillPortion(1)),
                    tooltip(
                        button(ui_element_player_shuffle_button(status.shuffle))
                            .on_press(Message::ToggleShuffle)
                            .height(Length::FillPortion(1))
                            .width(Length::FillPortion(1)),
                        if status.shuffle {
                            "shuffle: on"
                        } else {
                            "shuffle: off"
                        },
                        tooltip::Position::Bottom
                    )
                    .style(ui_style_tooltip)
                    .delay(iced::time::Duration::from_millis(500)),
                    space().width(Length::FillPortion(1)),
                ]
                .height(Length::Fill)
            )
            .width(Length::FillPortion(2))
            .height(Length::Fill),
            container(
                row![
                    space().width(Length::FillPortion(1)),
                    button(ui_element_player_last_button())
                        .on_press(Message::LastSong)
                        .height(Length::FillPortion(1))
                        .width(Length::FillPortion(1)),
                    button(play_button_content)
                        .on_press(Message::ChangePlayStatus)
                        .height(Length::FillPortion(1))
                        .width(Length::FillPortion(1)),
                    button(ui_element_player_next_button())
                        .on_press(Message::NextSong)
                        .height(Length::FillPortion(1))
                        .width(Length::FillPortion(1)),
                    space().width(Length::FillPortion(1)),
                ]
                .height(Length::Fill)
            )
            .width(Length::FillPortion(3))
            .height(Length::Fill),
            container(
                row![
                    space().width(Length::FillPortion(1)),
                    tooltip(
                        button(ui_element_player_repeat_button(status.repeat_mode))
                            .on_press(Message::ToggleRepeatMode)
                            .height(Length::FillPortion(1))
                            .width(Length::FillPortion(1)),
                        match status.repeat_mode {
                            RepeatMode::None => "repeat: off",
                            RepeatMode::All => "repeat: all",
                            RepeatMode::One => "repeat: one",
                        },
                        tooltip::Position::Bottom
                    )
                    .style(ui_style_tooltip)
                    .delay(iced::time::Duration::from_millis(500)),
                    mouse_area(
                        text(format!("\u{E809}\t{:.0}%", status.volume * 100.0))
                            .font(Font::with_name("music_player_buttons"))
                            .center()
                            .height(Length::FillPortion(1))
                            .width(Length::FillPortion(1))
                    )
                    .on_scroll(|delta| {
                        match delta {
                            iced::mouse::ScrollDelta::Lines { x, y } => Message::VolumeChanged(y),
                            iced::mouse::ScrollDelta::Pixels { x, y } => Message::VolumeChanged(y),
                        }
                    }),
                    space().width(Length::FillPortion(1)),
                ]
                .height(Length::Fill)
            )
            .width(Length::FillPortion(2))
            .height(Length::Fill),
        ]
        .padding(10)
        .width(Length::FillPortion(1))
        .height(Length::Fixed(80.0)),
        row![
            space().width(Length::FillPortion(1)),
            text(format!(
                "{:02}:{:02}",
                status.songs[status.song_index].get_pos() / 1000 / 60,
                status.songs[status.song_index].get_pos() / 1000 % 60
            ))
            .width(Length::Fixed(42.0)),
            slider(
                0.0..=100.0,
                status.playback_time,
                Message::PlaybackSliderChanged
            )
            .width(Length::FillPortion(3))
            .style(move |theme, slider_status| {
                ui_style_playback_slider_animated(theme, slider_status, accent, slow, pulse)
            }),
            text(format!(
                "{:02}:{:02}",
                status.songs[status.song_index].duration() / 1000 / 60,
                status.songs[status.song_index].duration() / 1000 % 60
            ))
            .width(Length::Fixed(42.0)),
            space().width(Length::FillPortion(1))
        ]
        .width(Length::FillPortion(1))
        .height(Length::Fixed(60.0)),
    ))
    .width(Length::Fill)
    .style(move |theme| ui_style_animated_bar(theme, accent, slow, pulse, 1.0));

    stack![
        player_bar,
        ui_element_pulse_line(accent, slow, pulse, Vertical::Top)
    ]
    .width(Length::Fill)
    .height(Length::Shrink)
    .into()
}

fn ui_element_player_playlists_open_button<'a>() -> Element<'a, Message> {
    text('\u{E807}')
        .font(Font::with_name("music_player_buttons"))
        .center()
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn ui_element_player_playlists_close_button<'a>() -> Element<'a, Message> {
    text('\u{E806}')
        .font(Font::with_name("music_player_buttons"))
        .center()
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
fn ui_element_player_play_button<'a>() -> Element<'a, Message> {
    text('\u{E800}')
        .font(Font::with_name("music_player_buttons"))
        .center()
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn ui_element_player_pause_button<'a>() -> Element<'a, Message> {
    text('\u{E802}')
        .font(Font::with_name("music_player_buttons"))
        .center()
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn ui_element_player_last_button<'a>() -> Element<'a, Message> {
    text('\u{E804}')
        .font(Font::with_name("music_player_buttons"))
        .center()
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn ui_element_player_next_button<'a>() -> Element<'a, Message> {
    text('\u{E803}')
        .font(Font::with_name("music_player_buttons"))
        .center()
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn ui_element_player_shuffle_button<'a>(active: bool) -> Element<'a, Message> {
    let content = if active { " [S] " } else { "  S  " };
    text(content)
        .center()
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn ui_element_player_repeat_button<'a>(mode: RepeatMode) -> Element<'a, Message> {
    let content = match mode {
        RepeatMode::None => " \u{2014} ", // em dash
        RepeatMode::All => " \u{21BB} ",  // ↻
        RepeatMode::One => " \u{21BA} ",  // ↺
    };
    text(content)
        .center()
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn ui_element_playlists_open_music_folder_button<'a>() -> Element<'a, Message> {
    text('\u{E801}')
        .font(Font::with_name("music_player_buttons"))
        .center()
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn ui_element_playlists_change_lyrics_folder_button<'a>() -> Element<'a, Message> {
    text('\u{E80A}')
        .font(Font::with_name("music_player_buttons"))
        .center()
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

// styles helper
fn ui_style_animated_bar(
    theme: &Theme,
    accent: Color,
    slow: f32,
    pulse: f32,
    strength: f32,
) -> container::Style {
    let base = Color::from_rgb(
        theme.palette().background.r / 2.0,
        theme.palette().background.g / 2.0,
        theme.palette().background.b / 2.0,
    );
    let tint = ((0.03 + slow * 0.03 + pulse * 0.08) * strength).clamp(0.0, 0.14);
    let background = mix_color(base, accent, tint);
    let border_alpha = ((0.10 + slow * 0.14 + pulse * 0.62) * strength).clamp(0.0, 1.0);

    // Keep the text color stable for the whole animation. Deriving it from the
    // animated background would make text flip between black and white when
    // the pulsing tint crosses the luminance threshold.
    let text_color = if base.relative_luminance() > 0.179 {
        Color::BLACK
    } else {
        Color::WHITE
    };

    container::Style {
        text_color: Some(text_color),
        background: Some(Background::Color(background)),
        border: Border {
            color: with_alpha(accent, border_alpha),
            width: 1.0 + pulse * 1.5,
            radius: iced::border::radius(2.0),
        },
        shadow: Default::default(),
        snap: false,
    }
}

fn ui_style_tooltip(theme: &Theme) -> container::Style {
    let bc = Color::from_rgb(
        theme.palette().background.r / 3.0,
        theme.palette().background.g / 3.0,
        theme.palette().background.b / 3.0,
    );
    let tc = if bc.relative_luminance() > 0.179 {
        Color::BLACK
    } else {
        Color::WHITE
    };
    container::Style {
        text_color: Some(tc),
        background: Some(Background::Color(bc)),
        border: Border {
            color: tc,
            width: 1.0,
            radius: iced::border::radius(2.0),
        },
        shadow: Default::default(),
        snap: false,
    }
}

fn ui_style_playlists_item_button_animated(
    theme: &Theme,
    status: button::Status,
    is_current: bool,
    accent: Color,
    slow: f32,
    pulse: f32,
) -> button::Style {
    let glow_color = mix_color(accent, Color::WHITE, 0.42);
    let background_alpha = if is_current {
        0.10 + slow * 0.08 + pulse * 0.18
    } else if status == button::Status::Hovered {
        0.08
    } else {
        0.0
    };
    let background_color = if is_current || status == button::Status::Hovered {
        mix_color(theme.palette().background, accent, 0.68)
    } else {
        accent
    };
    let border = if is_current {
        Border {
            color: with_alpha(glow_color, 0.34 + slow * 0.22 + pulse * 0.52),
            width: 1.0 + pulse * 2.0,
            radius: iced::border::radius(2.0),
        }
    } else if status == button::Status::Hovered {
        Border {
            color: with_alpha(glow_color, 0.45),
            width: 1.0,
            radius: iced::border::radius(2.0),
        }
    } else {
        Border::default()
    };
    let shadow_alpha = if is_current {
        0.10 + slow * 0.08 + pulse * 0.24
    } else {
        0.0
    };

    button::Style {
        background: Some(Background::Color(with_alpha(
            background_color,
            background_alpha,
        ))),
        text_color: theme.palette().text,
        border,
        shadow: Shadow {
            color: with_alpha(glow_color, shadow_alpha),
            offset: Vector::new(0.0, 0.0),
            blur_radius: 4.0 + pulse * 8.0,
        },
        ..Default::default()
    }
}

fn ui_style_playlists_animated(
    theme: &Theme,
    status: scrollable::Status,
    accent: Color,
    slow: f32,
    pulse: f32,
) -> scrollable::Style {
    let mut style = ui_style_playlists(theme, status);
    let tint = (0.01 + slow * 0.015 + pulse * 0.04).clamp(0.0, 0.07);

    style.container.background = Some(Background::Color(mix_color(
        theme.palette().background,
        accent,
        tint,
    )));
    style.container.text_color = Some(theme.palette().text);
    let glow_color = mix_color(accent, Color::WHITE, 0.42);
    style.container.border.color = with_alpha(glow_color, 0.30 + slow * 0.22 + pulse * 0.48);
    style.container.border.width = 2.0 + pulse * 1.5;
    style.container.shadow = Shadow {
        color: with_alpha(glow_color, 0.08 + slow * 0.08 + pulse * 0.22),
        offset: Vector::new(0.0, 0.0),
        blur_radius: 5.0 + pulse * 10.0,
    };
    style
}

fn ui_style_playlists(theme: &Theme, status: scrollable::Status) -> scrollable::Style {
    scrollable::Style {
        container: iced::widget::container::Style {
            text_color: Some(theme.palette().text),
            background: Some(Background::Color(theme.palette().background)),
            border: Border {
                color: theme.palette().text,
                width: 2.0,
                radius: iced::border::radius(0.0),
            },
            shadow: Default::default(),
            snap: true,
        },
        vertical_rail: scrollable::Rail {
            background: Some(Background::Color(theme.palette().text)),
            border: Default::default(),
            scroller: scrollable::Scroller {
                background: Background::Color(theme.palette().background),
                border: Border {
                    color: theme.palette().text,
                    width: 2.0,
                    radius: iced::border::radius(3.0),
                },
            },
        },
        horizontal_rail: scrollable::Rail {
            background: None,
            border: Default::default(),
            scroller: scrollable::Scroller {
                background: Background::Color(theme.palette().background),
                border: Default::default(),
            },
        },
        gap: None,
        auto_scroll: scrollable::AutoScroll {
            background: Background::Color(theme.palette().background),
            border: Default::default(),
            shadow: Default::default(),
            icon: Default::default(),
        },
    }
}

fn ui_style_playback_slider_animated(
    theme: &Theme,
    status: slider::Status,
    accent: Color,
    slow: f32,
    pulse: f32,
) -> slider::Style {
    let mut style = ui_style_playback_slider(theme, status);
    let active = mix_color(
        theme.palette().text,
        accent,
        0.35 + slow * 0.25 + pulse * 0.35,
    );
    style.rail.backgrounds.0 = Background::Color(active);
    style.handle.border_width = 2.0 + pulse * 2.0;
    style.handle.border_color = with_alpha(accent, 0.65 + pulse * 0.35);
    style
}

fn ui_style_playback_slider(theme: &Theme, status: slider::Status) -> slider::Style {
    let back_color = theme.palette().background;
    slider::Style {
        rail: slider::Rail {
            backgrounds: (
                Background::Color(theme.palette().text),
                Background::Color(theme.palette().background),
            ),
            width: 8.0,
            border: Border {
                color: theme.palette().text,
                width: 1.0,
                radius: iced::border::radius(1.0),
            },
        },
        handle: slider::Handle {
            shape: slider::HandleShape::Circle { radius: 8.0 },
            background: Background::Color(back_color),
            border_width: 2.0,
            border_color: theme.palette().text,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{save_lyrics_file, ui_style_animated_bar};
    use iced::{Color, Theme};

    #[test]
    fn animated_bar_text_color_stays_stable() {
        let quiet = ui_style_animated_bar(&Theme::Dark, Color::WHITE, 0.0, 0.0, 1.0);
        let loud = ui_style_animated_bar(&Theme::Dark, Color::WHITE, 1.0, 1.0, 1.0);

        assert_eq!(quiet.text_color, loud.text_color);
    }

    #[test]
    fn downloaded_lyrics_do_not_overwrite_existing_file() {
        let directory = std::env::temp_dir().join(format!(
            "icemp-lyrics-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("Artist - Title.lrc");

        save_lyrics_file(&path, "first").unwrap();
        save_lyrics_file(&path, "second").unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "first");
        std::fs::remove_dir_all(directory).unwrap();
    }
}
