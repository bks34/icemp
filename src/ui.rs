use iced::{widget::{button, column, row, text, space}, Background, Color, Element, Length, Task};


pub struct MusicPlayer {}

#[derive(Clone)]
pub enum Message {}

impl MusicPlayer {
    pub fn boot() -> MusicPlayer {
        MusicPlayer {}
    }

    pub fn update(&mut self, msg: Message) -> Task<Message> {
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        column![
            ui_element_panel(self),
            ui_element_player(self)
        ].into()
    }
}

fn ui_element_panel(status: &MusicPlayer) -> Element<'_, Message> {
    text("Cover and Lyrics")
        .height(Length::FillPortion(4))
        .width(Length::FillPortion(1))
        .into()
}

fn ui_element_player(status: &MusicPlayer) -> Element<'_, Message> {
    row![
        button("play list").height(Length::FillPortion(1)).width(Length::FillPortion(1))
            .style(ui_style_player_button),
        button("<").height(Length::FillPortion(1)).width(Length::FillPortion(1))
            .style(ui_style_player_button),
        button("play").height(Length::FillPortion(1)).width(Length::FillPortion(1))
            .style(ui_style_player_button),
        button(">").height(Length::FillPortion(1)).width(Length::FillPortion(1))
            .style(ui_style_player_button),
        space().height(Length::FillPortion(1)).width(Length::FillPortion(1))
    ]
    .padding(10)
    .width(Length::FillPortion(1))
    .height(Length::FillPortion(1))
    .into()
}

fn ui_style_player_button(theme: &iced::Theme, status: button::Status) -> button::Style {
    button::Style {
        background : Some(Background::Color(Color::TRANSPARENT)),
        text_color: Color::WHITE,
        ..Default::default()
    }
}