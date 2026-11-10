mod lyrics;
mod utils;

use crate::config::Config;
use crate::lyrics_download::{FetchResult, LyricsQuery, fetch_lyrics};
use crate::song::Song;
use bytes::Bytes;
use iced::{
    Background, Border, Color, Element, Font, Length, Padding, Radians, Rotation, Shadow,
    Subscription, Task, Theme, Vector,
    alignment::{Horizontal, Vertical},
    widget::{
        button, column, container, mouse_area, responsive, row, scrollable, slider, space, stack,
        text, tooltip,
    },
    window,
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
    current_lyric_index: Option<usize>,
    lyrics_status: LyricsStatus,
    lyrics_request_id: u64,
    lyrics_initialized: bool,

    // window
    window_id: Option<window::Id>,
    is_maximized: bool,
    is_focused: bool,

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
    WindowClose,
    WindowEvent(window::Id, window::Event),
    InitializeWindow,
    WindowIdResolved(Option<window::Id>),
    WindowMaximizedState(bool),
    WindowDrag,
    WindowMinimize,
    WindowToggleMaximize,
    WindowResize(window::Direction),
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
            current_lyric_index: None,
            lyrics_status: LyricsStatus::default(),
            lyrics_request_id: 0,
            lyrics_initialized: false,
            window_id: None,
            is_maximized: false,
            is_focused: true,
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
                    self.current_lyric_index = lrc.current_index(pos);
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
            Message::WindowClose => {
                self.config.save(self.config_path.to_str().unwrap());
                if let Some(id) = self.window_id {
                    return window::close(id);
                }
            }
            Message::WindowEvent(id, event) => {
                self.window_id = Some(id);
                match event {
                    window::Event::Focused => self.is_focused = true,
                    window::Event::Unfocused => self.is_focused = false,
                    window::Event::Resized(_) => {
                        return window::is_maximized(id).map(Message::WindowMaximizedState);
                    }
                    window::Event::CloseRequested => {
                        self.config.save(self.config_path.to_str().unwrap());
                        return window::close(id);
                    }
                    _ => {}
                }
            }
            Message::InitializeWindow => {
                if self.window_id.is_none() {
                    return window::latest().map(Message::WindowIdResolved);
                }
            }
            Message::WindowIdResolved(id) => {
                if self.window_id.is_none() {
                    self.window_id = id;
                }
            }
            Message::WindowMaximizedState(maximized) => {
                self.is_maximized = maximized;
            }
            Message::WindowDrag => {
                if let Some(id) = self.window_id {
                    return window::drag(id);
                }
            }
            Message::WindowMinimize => {
                if let Some(id) = self.window_id {
                    return window::minimize(id, true);
                }
            }
            Message::WindowToggleMaximize => {
                if let Some(id) = self.window_id {
                    self.is_maximized = !self.is_maximized;
                    return window::toggle_maximize(id);
                }
            }
            Message::WindowResize(direction) => {
                if let Some(id) = self.window_id {
                    return window::drag_resize(id, direction);
                }
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
        let page = match self.page {
            Page::PlayLists => ui_element_playlists(self),
            Page::CoverAndLyrics => ui_element_panel(self),
        };
        let player_overlay = container(ui_element_player(self))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Horizontal::Center)
            .align_y(Vertical::Bottom)
            .padding(Padding {
                top: 0.0,
                right: PAGE_HORIZONTAL_PADDING,
                bottom: PLAYER_BOTTOM_MARGIN,
                left: PAGE_HORIZONTAL_PADDING,
            });

        let body = stack![page, player_overlay]
            .width(Length::Fill)
            .height(Length::Fill);

        let shell = container(
            column![ui_element_titlebar(self), body]
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .clip(true)
        .style(move |theme| ui_style_window_shell(theme, self.is_maximized));

        stack![shell, ui_element_resize_handles(self)]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let window_events = window::events().map(|(id, event)| Message::WindowEvent(id, event));

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

        let mut listens = vec![window_events, keyboard_listen];
        if self.window_id.is_none() {
            listens.push(
                iced::time::every(iced::time::Duration::from_millis(50))
                    .map(|_| Message::InitializeWindow),
            );
        }
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
            self.current_lyric_index = self.lrc.as_ref().and_then(|lrc| lrc.current_index(pos));
            self.current_lyrics = self
                .lrc
                .as_ref()
                .map(|lrc| lrc.get_lyrics(pos))
                .unwrap_or_default();
        } else {
            self.lrc = None;
            self.current_lyric_index = None;
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
        self.current_lyric_index = None;
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
                self.current_lyric_index = self.lrc.as_ref().and_then(|lrc| lrc.current_index(pos));
                self.current_lyrics = self
                    .lrc
                    .as_ref()
                    .map(|lrc| lrc.get_lyrics(pos))
                    .unwrap_or_default();
            }
            FetchResult::NotFound => {
                self.lyrics_status = LyricsStatus::NotFound;
                self.current_lyric_index = None;
                self.current_lyrics = "No lyrics found online".into();
            }
            FetchResult::Error(error) => {
                println!("Failed to download lyrics: {error}");
                self.lyrics_status = LyricsStatus::Failed(error.clone());
                self.current_lyric_index = None;
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

const PAGE_HORIZONTAL_PADDING: f32 = 12.0;
const PLAYER_BOTTOM_MARGIN: f32 = 12.0;
const PANEL_RADIUS: f32 = 18.0;
const PANEL_GAP: f32 = 10.0;
const PANEL_PADDING: Padding = Padding {
    top: 14.0,
    right: 16.0,
    bottom: 14.0,
    left: 16.0,
};

// elements helper
fn ui_element_panel(status: &MusicPlayer) -> Element<'_, Message> {
    let corner_radius = if status.is_maximized {
        iced::border::radius(0.0)
    } else {
        iced::border::Radius {
            top_left: 0.0,
            top_right: 0.0,
            bottom_right: 18.0,
            bottom_left: 18.0,
        }
    };

    let back_image = iced::widget::image(&status.background_image_handle)
        .width(Length::Fill)
        .height(Length::Fill)
        .content_fit(iced::ContentFit::Cover)
        .scale(1.03_f32)
        .opacity(0.72_f32)
        .border_radius(corner_radius);

    let accent = status.accent_color;
    let slow = status.bass_slow.clamp(0.0, 1.0);
    let pulse = visual_pulse(status);
    let record_panel = container(ui_element_record(status))
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(PANEL_PADDING)
        .style(move |_| ui_style_glass_panel(accent, slow, pulse));
    let cover = column![record_panel, ui_element_song_info(status)]
        .spacing(PANEL_GAP)
        .width(Length::FillPortion(5))
        .height(Length::Fill);
    let lyrics = ui_element_lyrics(status);

    let content = row![cover, lyrics]
        .spacing(PAGE_HORIZONTAL_PADDING)
        .align_y(Vertical::Top)
        .padding(Padding {
            top: 14.0,
            right: PAGE_HORIZONTAL_PADDING,
            bottom: 124.0,
            left: PAGE_HORIZONTAL_PADDING,
        });

    stack![back_image, content]
        .height(Length::Fill)
        .width(Length::Fill)
        .into()
}

fn ui_element_song_info(status: &MusicPlayer) -> Element<'_, Message> {
    let title = status.songs[status.song_index].title();
    let artist = status.songs[status.song_index].artist();
    let album = status.songs[status.song_index].album();

    let mut info = column![
        text(title)
            .size(22)
            .center()
            .width(Length::Fill)
            .color(Color::WHITE),
        text(artist)
            .size(14)
            .center()
            .width(Length::Fill)
            .color(with_alpha(Color::WHITE, 0.72)),
    ]
    .spacing(3)
    .width(Length::Fill);

    if !album.trim().is_empty() {
        info = info.push(
            text(album)
                .size(12)
                .center()
                .width(Length::Fill)
                .color(with_alpha(Color::WHITE, 0.48)),
        );
    }

    let accent = status.accent_color;
    let slow = status.bass_slow.clamp(0.0, 1.0);
    let pulse = visual_pulse(status);

    container(info)
        .width(Length::Fill)
        .align_x(Horizontal::Center)
        .padding(PANEL_PADDING)
        .style(move |_| ui_style_glass_panel(accent, slow, pulse))
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
    .width(Length::Fill)
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

fn ui_element_titlebar(status: &MusicPlayer) -> Element<'_, Message> {
    let title_area = mouse_area(
        container(
            row![
                text("◆").size(13).color(status.accent_color),
                text("icemp").size(14).color(Color::WHITE),
                space(),
            ]
            .spacing(7)
            .align_y(Vertical::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(Padding {
            top: 0.0,
            right: 8.0,
            bottom: 0.0,
            left: 14.0,
        })
        .center_y(Length::Fill),
    )
    .on_press(Message::WindowDrag)
    .on_double_click(Message::WindowToggleMaximize)
    .interaction(iced::mouse::Interaction::Grab);

    let maximize_icon = if status.is_maximized {
        "\u{2750}"
    } else {
        "\u{25A1}"
    };

    let controls = row![
        ui_element_window_control("\u{2014}", Message::WindowMinimize, false),
        ui_element_window_control(maximize_icon, Message::WindowToggleMaximize, false),
        ui_element_window_control("\u{2715}", Message::WindowClose, true),
    ]
    .spacing(3);

    container(row![title_area, controls].align_y(Vertical::Center))
        .width(Length::Fill)
        .height(Length::Fixed(42.0))
        .padding(Padding {
            top: 4.0,
            right: 6.0,
            bottom: 4.0,
            left: 0.0,
        })
        .style(move |theme| ui_style_titlebar(theme, status.is_maximized, status.is_focused))
        .into()
}

fn ui_element_window_control(
    label: &'static str,
    message: Message,
    danger: bool,
) -> Element<'static, Message> {
    button(text(label).size(15).center())
        .width(Length::Fixed(38.0))
        .height(Length::Fixed(28.0))
        .padding(0)
        .on_press(message)
        .style(move |theme, status| ui_style_window_control(theme, status, danger))
        .into()
}

fn ui_element_resize_handles(status: &MusicPlayer) -> Element<'_, Message> {
    if status.is_maximized {
        return space().into();
    }

    stack![
        ui_element_resize_handle(
            Message::WindowResize(window::Direction::North),
            Length::Fill,
            Length::Fixed(6.0),
            Horizontal::Center,
            Vertical::Top,
            iced::mouse::Interaction::ResizingVertically,
        ),
        ui_element_resize_handle(
            Message::WindowResize(window::Direction::South),
            Length::Fill,
            Length::Fixed(6.0),
            Horizontal::Center,
            Vertical::Bottom,
            iced::mouse::Interaction::ResizingVertically,
        ),
        ui_element_resize_handle(
            Message::WindowResize(window::Direction::West),
            Length::Fixed(6.0),
            Length::Fill,
            Horizontal::Left,
            Vertical::Center,
            iced::mouse::Interaction::ResizingHorizontally,
        ),
        ui_element_resize_handle(
            Message::WindowResize(window::Direction::East),
            Length::Fixed(6.0),
            Length::Fill,
            Horizontal::Right,
            Vertical::Center,
            iced::mouse::Interaction::ResizingHorizontally,
        ),
        ui_element_resize_handle(
            Message::WindowResize(window::Direction::NorthWest),
            Length::Fixed(12.0),
            Length::Fixed(12.0),
            Horizontal::Left,
            Vertical::Top,
            iced::mouse::Interaction::ResizingDiagonallyUp,
        ),
        ui_element_resize_handle(
            Message::WindowResize(window::Direction::NorthEast),
            Length::Fixed(12.0),
            Length::Fixed(12.0),
            Horizontal::Right,
            Vertical::Top,
            iced::mouse::Interaction::ResizingDiagonallyUp,
        ),
        ui_element_resize_handle(
            Message::WindowResize(window::Direction::SouthWest),
            Length::Fixed(12.0),
            Length::Fixed(12.0),
            Horizontal::Left,
            Vertical::Bottom,
            iced::mouse::Interaction::ResizingDiagonallyDown,
        ),
        ui_element_resize_handle(
            Message::WindowResize(window::Direction::SouthEast),
            Length::Fixed(12.0),
            Length::Fixed(12.0),
            Horizontal::Right,
            Vertical::Bottom,
            iced::mouse::Interaction::ResizingDiagonallyDown,
        ),
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn ui_element_resize_handle(
    message: Message,
    width: Length,
    height: Length,
    align_x: Horizontal,
    align_y: Vertical,
    interaction: iced::mouse::Interaction,
) -> Element<'static, Message> {
    let handle = mouse_area(container(space()).width(width).height(height))
        .on_press(message)
        .interaction(interaction);

    container(handle)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(align_x)
        .align_y(align_y)
        .into()
}

fn ui_element_lyrics(status: &MusicPlayer) -> Element<'_, Message> {
    let accent = status.accent_color;
    let slow = status.bass_slow.clamp(0.0, 1.0);
    let pulse = visual_pulse(status);

    let lyrics_content: Element<'_, Message> = match status.lrc.as_ref() {
        Some(lrc) if !lrc.lines().is_empty() => {
            let current = status.current_lyric_index.unwrap_or_else(|| {
                lrc.current_index(status.songs[status.song_index].get_pos())
                    .unwrap_or(0)
            });
            let range = lrc.visible_range(current, 3);
            let mut lines = column![].spacing(9).width(Length::Fill);

            for index in range {
                let distance = index.abs_diff(current);
                lines = lines.push(ui_element_lyric_line(
                    lrc.lines()[index].1.clone(),
                    distance,
                    accent,
                    slow,
                    pulse,
                ));
            }

            container(lines)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_y(Length::Fill)
                .into()
        }
        _ => container(
            text(status.current_lyrics.clone())
                .size(17)
                .center()
                .width(Length::Fill)
                .color(with_alpha(Color::WHITE, 0.72)),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_y(Length::Fill)
        .into(),
    };

    container(lyrics_content)
        .width(Length::FillPortion(7))
        .height(Length::Fill)
        .padding(PANEL_PADDING)
        .style(move |_| ui_style_glass_panel(accent, slow, pulse))
        .into()
}

fn ui_element_lyric_line(
    line: String,
    distance: usize,
    accent: Color,
    slow: f32,
    pulse: f32,
) -> Element<'static, Message> {
    if distance == 0 {
        let background = with_alpha(
            mix_color(Color::from_rgb(0.04, 0.05, 0.07), accent, 0.42),
            0.74,
        );
        let border = with_alpha(
            mix_color(accent, Color::WHITE, 0.42),
            0.35 + slow * 0.20 + pulse * 0.35,
        );

        container(
            text(line)
                .size(24)
                .center()
                .width(Length::Fill)
                .color(Color::WHITE),
        )
        .width(Length::Fill)
        .padding(Padding {
            top: 9.0,
            right: 14.0,
            bottom: 9.0,
            left: 14.0,
        })
        .style(move |_| container::Style {
            background: Some(Background::Color(background)),
            border: Border {
                color: border,
                width: 1.0 + pulse,
                radius: iced::border::radius(14.0),
            },
            text_color: Some(Color::WHITE),
            shadow: Shadow {
                color: with_alpha(accent, 0.12 + slow * 0.10 + pulse * 0.24),
                offset: Vector::new(0.0, 2.0),
                blur_radius: 6.0 + pulse * 10.0,
            },
            ..Default::default()
        })
        .into()
    } else {
        let alpha = if distance == 1 { 0.58 } else { 0.30 };
        let size = if distance == 1 { 18 } else { 16 };

        text(line)
            .size(size)
            .center()
            .width(Length::Fill)
            .color(with_alpha(Color::WHITE, alpha))
            .into()
    }
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
    }))
    .spacing(2)
    .padding(Padding {
        top: 8.0,
        right: 8.0,
        bottom: 8.0,
        left: 8.0,
    });

    let toolbar = container(row![
        tooltip(
            button(ui_element_playlists_open_music_folder_button())
                .on_press(Message::OpenMusicFolder)
                .width(Length::FillPortion(1))
                .height(Length::FillPortion(1))
                .style(ui_style_playlist_icon_button),
            "open music folder",
            tooltip::Position::Bottom
        )
        .style(ui_style_tooltip)
        .delay(iced::time::Duration::from_millis(500)),
        tooltip(
            button(ui_element_playlists_change_lyrics_folder_button())
                .on_press(Message::ChangeLyricsFolder)
                .width(Length::FillPortion(1))
                .height(Length::FillPortion(1))
                .style(ui_style_playlist_icon_button),
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
    .padding(Padding {
        top: 4.0,
        right: 8.0,
        bottom: 4.0,
        left: 8.0,
    })
    .style(move |_| ui_style_glass_panel(accent, slow, pulse));

    let toolbar = stack![
        toolbar,
        ui_element_pulse_line(accent, slow, pulse, Vertical::Bottom)
    ]
    .width(Length::Fill)
    .height(Length::Fixed(50.0));

    let corner_radius = if status.is_maximized {
        iced::border::radius(0.0)
    } else {
        iced::border::Radius {
            top_left: 0.0,
            top_right: 0.0,
            bottom_right: 18.0,
            bottom_left: 18.0,
        }
    };
    let background = stack![
        iced::widget::image(&status.background_image_handle)
            .width(Length::Fill)
            .height(Length::Fill)
            .content_fit(iced::ContentFit::Cover)
            .scale(1.03_f32)
            .opacity(0.34_f32)
            .border_radius(corner_radius),
        container(space())
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| container::Style {
                background: Some(Background::Color(Color::from_rgba(0.01, 0.012, 0.02, 0.34))),
                border: Border {
                    radius: corner_radius,
                    ..Default::default()
                },
                ..Default::default()
            }),
    ]
    .width(Length::Fill)
    .height(Length::Fill);

    let page = column![
        toolbar,
        scrollable::Scrollable::new(content)
            .style(move |theme, scrollable_status| {
                ui_style_playlists_animated(theme, scrollable_status, accent, slow, pulse)
            })
            .width(Length::Fill)
            .height(Length::Fill),
        space().height(Length::Fixed(112.0)),
    ]
    .spacing(PANEL_GAP)
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(Padding {
        top: 10.0,
        right: PAGE_HORIZONTAL_PADDING,
        bottom: 0.0,
        left: PAGE_HORIZONTAL_PADDING,
    });

    stack![background, page]
        .width(Length::Fill)
        .height(Length::Fill)
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
                        .width(Length::FillPortion(1))
                        .style(ui_style_player_control_button),
                    tooltip(
                        button(ui_element_player_shuffle_button(status.shuffle))
                            .on_press(Message::ToggleShuffle)
                            .height(Length::FillPortion(1))
                            .width(Length::FillPortion(1))
                            .style(ui_style_player_control_button),
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
                        .width(Length::FillPortion(1))
                        .style(ui_style_player_control_button),
                    button(play_button_content)
                        .on_press(Message::ChangePlayStatus)
                        .height(Length::FillPortion(1))
                        .width(Length::FillPortion(1))
                        .style(ui_style_player_control_button),
                    button(ui_element_player_next_button())
                        .on_press(Message::NextSong)
                        .height(Length::FillPortion(1))
                        .width(Length::FillPortion(1))
                        .style(ui_style_player_control_button),
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
                            .width(Length::FillPortion(1))
                            .style(ui_style_player_control_button),
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
                            iced::mouse::ScrollDelta::Lines { y, .. } => Message::VolumeChanged(y),
                            iced::mouse::ScrollDelta::Pixels { y, .. } => Message::VolumeChanged(y),
                        }
                    }),
                    space().width(Length::FillPortion(1)),
                ]
                .height(Length::Fill)
            )
            .width(Length::FillPortion(2))
            .height(Length::Fill),
        ]
        .padding(Padding {
            top: 8.0,
            right: 12.0,
            bottom: 4.0,
            left: 12.0,
        })
        .width(Length::FillPortion(1))
        .height(Length::Fixed(58.0)),
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
        .height(Length::Fixed(38.0)),
    ))
    .width(Length::Fill)
    .style(move |theme| ui_style_player_bar(theme, accent, slow, pulse));

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
fn ui_style_glass_panel(accent: Color, slow: f32, pulse: f32) -> container::Style {
    let glow = mix_color(accent, Color::WHITE, 0.42);

    container::Style {
        text_color: Some(Color::WHITE),
        background: Some(Background::Color(Color::from_rgba(
            0.025, 0.03, 0.045, 0.50,
        ))),
        border: Border {
            color: with_alpha(glow, 0.14 + slow * 0.10 + pulse * 0.28),
            width: 1.0 + pulse * 0.6,
            radius: iced::border::radius(PANEL_RADIUS),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.30),
            offset: Vector::new(0.0, 6.0),
            blur_radius: 18.0 + pulse * 8.0,
        },
        snap: false,
    }
}

fn ui_style_window_shell(theme: &Theme, is_maximized: bool) -> container::Style {
    let base = mix_color(theme.palette().background, Color::BLACK, 0.22);
    let radius = if is_maximized { 0.0 } else { 18.0 };

    container::Style {
        text_color: Some(theme.palette().text),
        background: Some(Background::Color(with_alpha(base, 0.96))),
        border: Border {
            color: Color::from_rgba(1.0, 1.0, 1.0, 0.10),
            width: 1.0,
            radius: iced::border::radius(radius),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.35),
            offset: Vector::new(0.0, 8.0),
            blur_radius: 24.0,
        },
        snap: false,
    }
}

fn ui_style_titlebar(_theme: &Theme, is_maximized: bool, is_focused: bool) -> container::Style {
    let alpha = if is_focused { 0.72 } else { 0.54 };
    let radius = if is_maximized { 0.0 } else { 18.0 };

    container::Style {
        text_color: Some(Color::WHITE),
        background: Some(Background::Color(Color::from_rgba(
            0.015, 0.02, 0.03, alpha,
        ))),
        border: Border {
            color: Color::from_rgba(1.0, 1.0, 1.0, 0.08),
            width: 1.0,
            radius: iced::border::Radius {
                top_left: radius,
                top_right: radius,
                bottom_right: 0.0,
                bottom_left: 0.0,
            },
        },
        shadow: Default::default(),
        snap: false,
    }
}

fn ui_style_window_control(_theme: &Theme, status: button::Status, danger: bool) -> button::Style {
    let background = match status {
        button::Status::Hovered | button::Status::Pressed if danger => {
            Some(Background::Color(Color::from_rgba(0.82, 0.18, 0.22, 0.88)))
        }
        button::Status::Hovered => Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.12))),
        button::Status::Pressed => Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.18))),
        _ => None,
    };

    button::Style {
        background,
        text_color: Color::WHITE,
        border: Border {
            radius: iced::border::radius(8.0),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn ui_style_player_bar(theme: &Theme, accent: Color, slow: f32, pulse: f32) -> container::Style {
    let base = mix_color(theme.palette().background, Color::BLACK, 0.72);
    let border = mix_color(accent, Color::WHITE, 0.34);

    container::Style {
        text_color: Some(Color::WHITE),
        background: Some(Background::Color(with_alpha(base, 0.64))),
        border: Border {
            color: with_alpha(border, 0.24 + slow * 0.16 + pulse * 0.36),
            width: 1.0 + pulse * 0.8,
            radius: iced::border::radius(18.0),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.42),
            offset: Vector::new(0.0, 8.0),
            blur_radius: 22.0 + pulse * 8.0,
        },
        snap: false,
    }
}

fn ui_style_player_control_button(_theme: &Theme, status: button::Status) -> button::Style {
    let background = match status {
        button::Status::Hovered => Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.12))),
        button::Status::Pressed => Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.18))),
        _ => None,
    };

    button::Style {
        background,
        text_color: Color::WHITE,
        border: Border {
            radius: iced::border::radius(10.0),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn ui_style_playlist_icon_button(_theme: &Theme, status: button::Status) -> button::Style {
    let background = match status {
        button::Status::Hovered => Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.10))),
        button::Status::Pressed => Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.16))),
        _ => None,
    };

    button::Style {
        background,
        text_color: Color::WHITE,
        border: Border {
            radius: iced::border::radius(10.0),
            ..Default::default()
        },
        ..Default::default()
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
            radius: iced::border::radius(8.0),
        },
        shadow: Default::default(),
        snap: false,
    }
}

fn ui_style_playlists_item_button_animated(
    _theme: &Theme,
    status: button::Status,
    is_current: bool,
    accent: Color,
    slow: f32,
    pulse: f32,
) -> button::Style {
    let glow_color = mix_color(accent, Color::WHITE, 0.42);
    let base_color = Color::from_rgb(0.035, 0.045, 0.065);
    let background_alpha = if is_current {
        0.30 + slow * 0.12 + pulse * 0.18
    } else if status == button::Status::Hovered {
        0.10
    } else {
        0.0
    };
    let background_color = if is_current {
        mix_color(base_color, accent, 0.55)
    } else {
        Color::WHITE
    };
    let border = if is_current {
        Border {
            color: with_alpha(glow_color, 0.34 + slow * 0.22 + pulse * 0.52),
            width: 1.0 + pulse * 2.0,
            radius: iced::border::radius(12.0),
        }
    } else if status == button::Status::Hovered {
        Border {
            color: Color::from_rgba(1.0, 1.0, 1.0, 0.14),
            width: 1.0,
            radius: iced::border::radius(12.0),
        }
    } else {
        Border {
            radius: iced::border::radius(12.0),
            ..Default::default()
        }
    };
    let shadow_alpha = if is_current {
        0.12 + slow * 0.10 + pulse * 0.26
    } else {
        0.0
    };

    button::Style {
        background: Some(Background::Color(with_alpha(
            background_color,
            background_alpha,
        ))),
        text_color: Color::WHITE,
        border,
        shadow: Shadow {
            color: with_alpha(glow_color, shadow_alpha),
            offset: Vector::new(0.0, 2.0),
            blur_radius: 6.0 + pulse * 10.0,
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
    let glow_color = mix_color(accent, Color::WHITE, 0.42);

    style.container.background = Some(Background::Color(Color::from_rgba(
        0.025, 0.03, 0.045, 0.54,
    )));
    style.container.text_color = Some(Color::WHITE);
    style.container.border.color = with_alpha(glow_color, 0.16 + slow * 0.14 + pulse * 0.36);
    style.container.border.width = 1.0 + pulse;
    style.container.shadow = Shadow {
        color: Color::from_rgba(0.0, 0.0, 0.0, 0.32),
        offset: Vector::new(0.0, 6.0),
        blur_radius: 18.0 + pulse * 8.0,
    };
    style.vertical_rail.background = Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.06)));
    style.vertical_rail.scroller.background =
        Background::Color(with_alpha(glow_color, 0.42 + pulse * 0.24));
    style.vertical_rail.scroller.border.color = Color::from_rgba(1.0, 1.0, 1.0, 0.16);
    style.auto_scroll.background = Background::Color(Color::from_rgba(0.025, 0.03, 0.045, 0.82));
    style
}

fn ui_style_playlists(_theme: &Theme, _status: scrollable::Status) -> scrollable::Style {
    scrollable::Style {
        container: iced::widget::container::Style {
            text_color: Some(Color::WHITE),
            background: Some(Background::Color(Color::from_rgba(
                0.025, 0.03, 0.045, 0.54,
            ))),
            border: Border {
                color: Color::from_rgba(1.0, 1.0, 1.0, 0.10),
                width: 1.0,
                radius: iced::border::radius(PANEL_RADIUS),
            },
            shadow: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.32),
                offset: Vector::new(0.0, 6.0),
                blur_radius: 18.0,
            },
            snap: false,
        },
        vertical_rail: scrollable::Rail {
            background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.06))),
            border: Border::default(),
            scroller: scrollable::Scroller {
                background: Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.28)),
                border: Border {
                    color: Color::from_rgba(1.0, 1.0, 1.0, 0.16),
                    width: 1.0,
                    radius: iced::border::radius(6.0),
                },
            },
        },
        horizontal_rail: scrollable::Rail {
            background: None,
            border: Border::default(),
            scroller: scrollable::Scroller {
                background: Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.20)),
                border: Border::default(),
            },
        },
        gap: None,
        auto_scroll: scrollable::AutoScroll {
            background: Background::Color(Color::from_rgba(0.025, 0.03, 0.045, 0.82)),
            border: Border::default(),
            shadow: Shadow::default(),
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
    let active = mix_color(accent, Color::WHITE, 0.24 + slow * 0.12 + pulse * 0.18);
    style.rail.backgrounds.0 = Background::Color(active);
    style.rail.backgrounds.1 = Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.20));
    style.rail.border.color = Color::from_rgba(1.0, 1.0, 1.0, 0.16);
    style.handle.background = Background::Color(Color::from_rgb(0.08, 0.09, 0.12));
    style.handle.border_width = 2.0 + pulse * 2.0;
    style.handle.border_color =
        with_alpha(mix_color(accent, Color::WHITE, 0.45), 0.70 + pulse * 0.30);
    style
}

fn ui_style_playback_slider(theme: &Theme, _status: slider::Status) -> slider::Style {
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
    use super::{save_lyrics_file, ui_style_glass_panel};
    use iced::Color;

    #[test]
    fn glass_panel_text_color_stays_stable() {
        let quiet = ui_style_glass_panel(Color::WHITE, 0.0, 0.0);
        let loud = ui_style_glass_panel(Color::WHITE, 1.0, 1.0);

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
