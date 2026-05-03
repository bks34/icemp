mod song;
mod ui;

fn main() -> iced::Result {
    let settings = iced::Settings {
        antialiasing: true,
        vsync: true,
        fonts: vec![include_bytes!("../fonts/music_player_buttons.ttf").into()],
        ..Default::default()
    };

    let window_settings = iced::window::Settings {
        size: iced::Size::new(800.0, 600.0),
        min_size: Some(iced::Size::new(600.0, 450.0)),
        icon: Some(
            iced::window::icon::from_file_data(
                include_bytes!("../images/icon.png"),
                Some(image::ImageFormat::Png),
            )
            .unwrap(),
        ),
        ..Default::default()
    };
    iced::application(
        ui::MusicPlayer::boot,
        ui::MusicPlayer::update,
        ui::MusicPlayer::view,
    )
    .settings(settings)
    .window(window_settings)
    .title("icemp - A simple music player")
    .theme(ui::MusicPlayer::theme)
    .subscription(ui::MusicPlayer::subscription)
    .run()
}
