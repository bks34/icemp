mod lyrics;
mod utils;

use crate::config;
use crate::config::Config;
use crate::song::Song;
use bytes::Bytes;
use iced::widget::span;
use iced::{
    widget::{
        button, column, container, mouse_area, row, scrollable, slider, space, stack, text, tooltip,
    },
    Background, Border, Color, Element, Font, Length, Subscription, Task, Theme,
};
use image::DynamicImage;
use std::path::PathBuf;

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

pub struct MusicPlayer {
    // Page
    page: Page,

    // Play status
    play_status: PlayStatus,
    playback_time: f32,
    volume: f32,

    // theme and background image
    cover: DynamicImage,
    cover_handle: iced::widget::image::Handle,
    background_image_handle: iced::widget::image::Handle,

    default_cover: DynamicImage,

    // songs
    songs: Vec<Song>,
    song_index: usize,

    // lyrics
    lrc: Option<lyrics::Lrc>,
    current_lyrics: String,

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
    OpenFolder,
    CloseWindow,
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
            songs: Vec::new(),
            song_index: 0,
            lrc: None,
            current_lyrics: "No lyrics available".into(),
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

        music_player.update_songs(PathBuf::from(config.music_path()));
        music_player.change_song(0);

        music_player.volume = config.volume();
        music_player.play_status = PlayStatus::Pause;
        music_player.songs[music_player.song_index].pause();

        music_player.config = config;

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
                let id = (self.song_index + 1) % self.songs.len();
                self.change_song(id);
            }
            Message::LastSong => {
                let id = ((self.song_index as i32 + self.songs.len() as i32 - 1)
                    % self.songs.len() as i32) as usize;
                self.change_song(id);
            }
            Message::ChangeSong(id) => {
                self.change_song(id);
            }
            Message::UpdatePlayBackTime => {
                let pos = self.songs[self.song_index].get_pos();
                let duration = self.songs[self.song_index].duration();
                self.playback_time = pos as f32 / duration as f32 * 100.0;
                if self.songs[self.song_index].playback_ended() {
                    return Task::done(Message::NextSong);
                }
                self.current_lyrics = self.lrc.as_ref().unwrap().get_lyrics(pos);
            }
            Message::OpenFolder => {
                let folder = rfd::FileDialog::new()
                    .set_title("Please select music folder")
                    .pick_folder();
                match folder {
                    Some(path) => {
                        self.update_songs(path.clone());
                        self.config.set_music_path(path.to_str().unwrap());
                    }
                    None => {}
                }
            }
            Message::CloseWindow => {
                println!("exit");
                self.config.save(self.config_path.to_str().unwrap());
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
        let mut listens = vec![exit_listen];
        if self.songs[self.song_index].prepared() {
            listens.push(
                iced::time::every(iced::time::Duration::from_millis(10))
                    .map(|_| Message::UpdatePlayBackTime),
            );
            iced::Subscription::batch(listens)
        } else {
            iced::Subscription::batch(listens)
        }
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
    }

    fn change_song(&mut self, id: usize) {
        self.songs[self.song_index].end_play();
        self.song_index = id;
        self.songs[self.song_index].prepare_play();
        self.change_cover(self.song_index);
        self.songs[self.song_index].play();
        self.play_status = PlayStatus::Play;

        //
        let lyrics_path = PathBuf::from(self.config.lyrics_path());
        let lyrics_file_name = format!(
            "{} - {}.lrc",
            self.songs[self.song_index].artist(),
            self.songs[self.song_index].title()
        );

        let ans = lyrics_path.join(lyrics_file_name.as_str());

        if !ans.exists() {
            println!("there is no file {}", lyrics_file_name)
        }

        self.lrc = Some(lyrics::Lrc::from_path(ans.to_str().unwrap().to_string()));
    }

    fn change_cover(&mut self, id: usize) {
        let _cover_ = match self.songs[id].get_cover() {
            Some(cover) => cover,
            None => self.default_cover.clone(),
        };
        let _background_image_ = libblur::fast_gaussian_blur_image(
            _cover_.clone(),
            libblur::AnisotropicRadius::new(40),
            libblur::EdgeMode2D::default(),
            libblur::ThreadingPolicy::Adaptive,
        )
        .unwrap();

        let mut t1: Vec<u8> = Vec::new();
        _cover_
            .write_to(std::io::Cursor::new(&mut t1), image::ImageFormat::Png)
            .expect("write image error");

        let mut t2: Vec<u8> = Vec::new();
        _background_image_
            .write_to(std::io::Cursor::new(&mut t2), image::ImageFormat::Png)
            .expect("write image error");

        let _cover = iced::widget::image::Handle::from_bytes(t1);
        let _background_image = iced::widget::image::Handle::from_bytes(t2);
        self.cover = _cover_;
        self.cover_handle = _cover;
        self.background_image_handle = _background_image;
    }
}

// elements helper
fn ui_element_panel(status: &MusicPlayer) -> Element<'_, Message> {
    let back_image = iced::widget::image(&status.background_image_handle)
        .width(Length::Fill)
        .height(Length::Fill)
        .content_fit(iced::ContentFit::Cover)
        .opacity(0.8);
    let cover = iced::widget::image(&status.cover_handle)
        .width(Length::FillPortion(4))
        .height(Length::FillPortion(1))
        .content_fit(iced::ContentFit::Contain);
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
        row![
            space().width(Length::FillPortion(1)),
            button(text(item.1.clone()).line_height(1.3))
                .padding(5.0)
                .style(ui_style_playlists_item_button)
                .on_press(Message::ChangeSong(item.0))
                .width(Length::FillPortion(18)),
            space().width(Length::FillPortion(1)),
        ]
        .into()
    }));
    column![
        container(row![
            tooltip(
                button(ui_element_playlists_open_folder_button())
                    .on_press(Message::OpenFolder)
                    .width(Length::FillPortion(1))
                    .height(Length::FillPortion(1)),
                "open folder",
                tooltip::Position::Right
            )
            .delay(iced::time::Duration::from_millis(500)),
            space().width(Length::FillPortion(4)),
            tooltip(
                text(format!(
                    "{:04}|{:04}",
                    status.song_index + 1,
                    status.songs.len()
                ))
                .center()
                .width(Length::FillPortion(1))
                .height(Length::FillPortion(1)),
                "the index of music which is playing\n and the total number of music",
                tooltip::Position::Bottom
            )
            .delay(iced::time::Duration::from_millis(500)),
        ])
        .height(Length::Fixed(50.0))
        .style(ui_style_outer_container),
        scrollable::Scrollable::new(content)
            .style(ui_style_playlists)
            .width(Length::Fill)
            .height(Length::FillPortion(8))
    ]
    .into()
}

fn ui_element_player(status: &MusicPlayer) -> Element<'_, Message> {
    let playlists_button_content = match status.page {
        Page::PlayLists => ui_element_player_playlists_close_button(),
        Page::CoverAndLyrics => ui_element_player_playlists_open_button(),
    };
    let play_button_content = match status.play_status {
        PlayStatus::Play => ui_element_player_pause_button(),
        PlayStatus::Pause => ui_element_player_play_button(),
    };

    container(column!(
        row![
            button(playlists_button_content)
                .on_press(Message::ChangePage)
                .height(Length::FillPortion(1))
                .width(Length::FillPortion(1)),
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
            })
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
            .style(ui_style_playback_slider),
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
    .style(ui_style_outer_container)
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

fn ui_element_playlists_open_folder_button<'a>() -> Element<'a, Message> {
    text('\u{E801}')
        .font(Font::with_name("music_player_buttons"))
        .center()
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

// styles helper
fn ui_style_playlists_item_button(theme: &Theme, status: button::Status) -> button::Style {
    let border = match status {
        button::Status::Hovered => iced::Border::default()
            .width(2.0)
            .color(theme.palette().text),
        _ => iced::Border::default(),
    };
    button::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        text_color: theme.palette().text,
        border,
        ..Default::default()
    }
}

fn ui_style_outer_container(theme: &Theme) -> container::Style {
    let bc = Color::from_rgb(
        theme.palette().background.r / 2.0,
        theme.palette().background.g / 2.0,
        theme.palette().background.b / 2.0,
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
            color: bc,
            width: 1.0,
            radius: iced::border::radius(1.0),
        },
        shadow: Default::default(),
        snap: false,
    }
}

fn ui_style_playlists(theme: &Theme, status: scrollable::Status) -> scrollable::Style {
    scrollable::Style {
        container: iced::widget::container::Style {
            text_color: Some(theme.palette().background),
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
            background: Some(Background::Color(theme.palette().background)),
            border: Default::default(),
            scroller: scrollable::Scroller {
                background: Background::Color(theme.palette().text),
                border: Default::default(),
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
