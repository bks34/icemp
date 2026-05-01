mod utils;

use crate::song::Song;
use iced::{
    widget::{button, column, row, scrollable, slider, space, stack, text},
    Background, Border, Color, Element, Font, Length, Subscription, Task, Theme,
};
use image::DynamicImage;

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

    // theme and background image
    cover: DynamicImage,
    cover_handle: iced::widget::image::Handle,
    background_image_handle: iced::widget::image::Handle,

    default_cover: DynamicImage,

    // songs
    songs: Vec<Song>,
    song_index: usize,
}

#[derive(Clone)]
pub enum Message {
    ChangePage,
    ChangePlayStatus,
    PlaybackSliderChanged(f32),
    NextSong,
    LastSong,
    ChangeSong(usize),
    UpdatePlayBackTime,
    OpenFolder,
}

impl MusicPlayer {
    pub fn boot() -> MusicPlayer {
        let default_cover =
            image::load_from_memory(include_bytes!("../images/record.png")).unwrap();
        let _cover_ = default_cover.clone();
        let mut t1: Vec<u8> = Vec::new();
        _cover_
            .write_to(std::io::Cursor::new(&mut t1), image::ImageFormat::Png)
            .expect("write image error");

        let _background_image_ = libblur::fast_gaussian_blur_image(
            _cover_.clone(),
            libblur::AnisotropicRadius::new(50),
            libblur::EdgeMode2D::default(),
            libblur::ThreadingPolicy::Adaptive,
        )
        .unwrap();
        let mut t2: Vec<u8> = Vec::new();
        _background_image_
            .write_to(std::io::Cursor::new(&mut t2), image::ImageFormat::Png)
            .expect("write image error");

        let _cover_handle = iced::widget::image::Handle::from_bytes(t1);
        let _background_image_handle = iced::widget::image::Handle::from_bytes(t2);
        let mut _songs: Vec<Song> = Vec::new();
        _songs.push(Song::from_path(
           "unknown".into()
        ));
        MusicPlayer {
            cover: _cover_,
            cover_handle: _cover_handle,
            background_image_handle: _background_image_handle,
            default_cover,
            page: Default::default(),
            play_status: Default::default(),
            playback_time: 0.0,
            songs: _songs,
            song_index: 0,
        }
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
            Message::NextSong => {
                self.songs[self.song_index].end_play();
                self.song_index = (self.song_index + 1) % self.songs.len();
                self.songs[self.song_index].prepare_play();
                self.change_cover(self.song_index);
                self.songs[self.song_index].play();
                self.play_status = PlayStatus::Play;
            }
            Message::LastSong => {
                self.songs[self.song_index].end_play();
                self.song_index = ((self.song_index as i32 + self.songs.len() as i32 - 1) % self.songs.len() as i32) as usize;
                self.songs[self.song_index].prepare_play();
                self.change_cover(self.song_index);
                self.songs[self.song_index].play();
                self.play_status = PlayStatus::Play;
            }
            Message::ChangeSong(id) => {
                self.songs[self.song_index].end_play();
                self.song_index = id;
                self.songs[self.song_index].prepare_play();
                self.change_cover(self.song_index);
                self.songs[self.song_index].play();
                self.play_status = PlayStatus::Play;
            }
            Message::UpdatePlayBackTime => {
                let pos = self.songs[self.song_index].get_pos();
                let duration = self.songs[self.song_index].duration();
                self.playback_time = pos as f32 / duration as f32 * 100.0;
                if self.songs[self.song_index].playback_ended() {
                    return Task::done(Message::NextSong);
                }
            }
            Message::OpenFolder => {
                let folder = rfd::FileDialog::new()
                    .set_title("Please select music folder")
                    .pick_folder();
                match folder {
                    Some(path) => {
                        self.songs[self.song_index].end_play();
                        self.songs.clear();
                        for entry in std::fs::read_dir(path).unwrap() {
                            let path = entry.unwrap().path();
                            if path.is_file() {
                                self.songs
                                    .push(Song::from_path(path.to_str().unwrap().to_string()));
                            }
                        }
                    }
                    None => {}
                }
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
        if self.songs[self.song_index].prepared() {
            iced::time::every(iced::time::Duration::from_secs(1))
                .map(|_| Message::UpdatePlayBackTime)
        } else {
            iced::Subscription::none()
        }
    }

    fn change_cover(&mut self, id: usize) {
        let _cover_ = match self.songs[self.song_index].get_cover() {
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
    let lyrics = text(format!(
        "{}\n  {}",
        status.songs[status.song_index].title(),
        status.songs[status.song_index].artist()
    ))
    .center()
    .width(Length::FillPortion(6))
    .height(Length::FillPortion(1));
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

fn ui_element_playlists(status: &MusicPlayer) -> Element<'_, Message> {
    let playlists: Vec<String> = status
        .songs
        .iter()
        .enumerate()
        .map(|item| {
            if item.0 == status.song_index {
                format!("{} {} - {}", '*', item.1.artist(), item.1.title())
            } else {
                format!("{:02} {} - {}", item.0 + 1, item.1.artist(), item.1.title())
            }
        })
        .collect();
    let content = column(playlists.iter().enumerate().map(|item| {
        row![
            space().width(Length::FillPortion(1)),
            button(text(item.1.clone()).line_height(1.3))
                .style(ui_style_player_button)
                .on_press(Message::ChangeSong(item.0))
                .width(Length::FillPortion(18)),
            space().width(Length::FillPortion(1)),
        ]
        .into()
    }));
    column![
        button(
            text("Open folder")
                .line_height(2.0)
                .center()
                .width(Length::Fill)
        )
        .on_press(Message::OpenFolder)
        .style(ui_style_player_button)
        .width(Length::Fill)
        .width(Length::FillPortion(1)),
        scrollable::Scrollable::new(content)
            .style(ui_style_playlists)
            .width(Length::Fill)
            .height(Length::FillPortion(6))

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
    column!(
        row![
            button(playlists_button_content)
                .on_press(Message::ChangePage)
                .height(Length::FillPortion(1))
                .width(Length::FillPortion(1))
                .style(ui_style_player_button),
            button(ui_element_player_last_button())
                .on_press(Message::LastSong)
                .height(Length::FillPortion(1))
                .width(Length::FillPortion(1))
                .style(ui_style_player_button),
            button(play_button_content)
                .on_press(Message::ChangePlayStatus)
                .height(Length::FillPortion(1))
                .width(Length::FillPortion(1))
                .style(ui_style_player_button),
            button(ui_element_player_next_button())
                .on_press(Message::NextSong)
                .height(Length::FillPortion(1))
                .width(Length::FillPortion(1))
                .style(ui_style_player_button),
            space()
                .height(Length::FillPortion(1))
                .width(Length::FillPortion(1))
        ]
        .padding(10)
        .width(Length::FillPortion(1))
        .height(Length::Fixed(80.0)),
        row![
            space().width(Length::FillPortion(1)),
            text("00:00").width(Length::Fixed(42.0)),
            slider(
                0.0..=100.0,
                status.playback_time,
                Message::PlaybackSliderChanged
            )
            .width(Length::FillPortion(3))
            .style(ui_style_playback_slider),
            text(format!(
                "{:02}:{:02}",
                status.songs[status.song_index].duration() / 60,
                status.songs[status.song_index].duration() % 60
            ))
            .width(Length::Fixed(42.0)),
            space().width(Length::FillPortion(1))
        ]
        .width(Length::FillPortion(1))
        .height(Length::Fixed(60.0)),
    )
    .into()
}

fn ui_element_player_playlists_open_button<'a>() -> Element<'a, Message> {
    text('\u{E807}')
        .font(Font::with_name("player_button"))
        .center()
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn ui_element_player_playlists_close_button<'a>() -> Element<'a, Message> {
    text('\u{E806}')
        .font(Font::with_name("player_button"))
        .center()
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
fn ui_element_player_play_button<'a>() -> Element<'a, Message> {
    text('\u{E800}')
        .font(Font::with_name("player_button"))
        .center()
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn ui_element_player_pause_button<'a>() -> Element<'a, Message> {
    text('\u{E802}')
        .font(Font::with_name("player_button"))
        .center()
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn ui_element_player_last_button<'a>() -> Element<'a, Message> {
    text('\u{E804}')
        .font(Font::with_name("player_button"))
        .center()
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn ui_element_player_next_button<'a>() -> Element<'a, Message> {
    text('\u{E803}')
        .font(Font::with_name("player_button"))
        .center()
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

// styles helper
fn ui_style_player_button(theme: &Theme, status: button::Status) -> button::Style {
    button::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        text_color: theme.palette().text,
        ..Default::default()
    }
}

fn ui_style_playlists(theme: &Theme, status: scrollable::Status) -> scrollable::Style {
    scrollable::Style {
        container: iced::widget::container::Style{
            text_color: Some(theme.palette().background),
            background: Some(Background::Color(theme.palette().background)),
            border: Border{
                color: theme.palette().text,
                width: 2.0,
                radius: iced::border::radius(0.0),
            },
            shadow: Default::default(),
            snap: false,
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
