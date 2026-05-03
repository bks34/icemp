use iced::Theme;
use image::GenericImageView;

pub fn get_theme_from_image_color(img: &image::DynamicImage) -> iced::Theme {
    let w = img.width();
    let h = img.height();
    let pixel_tl = img.get_pixel(0, 0);
    let pixel_tr = img.get_pixel(w - 1, 0);
    let pixel_bl = img.get_pixel(0, h - 1);
    let pixel_br = img.get_pixel(w - 1, h - 1);
    let pixel_mm = img.get_pixel(w / 2, h / 2);
    let b_color = iced::Color::from_rgba(
        (pixel_tl.0[0] as f32
            + pixel_tr.0[0] as f32
            + pixel_bl.0[0] as f32
            + pixel_br.0[0] as f32
            + pixel_mm.0[0] as f32)
            / (255.0 * 5.0),
        (pixel_tl.0[1] as f32
            + pixel_tr.0[1] as f32
            + pixel_bl.0[1] as f32
            + pixel_br.0[1] as f32
            + pixel_mm.0[1] as f32)
            / (255.0 * 5.0),
        (pixel_tl.0[2] as f32
            + pixel_tr.0[2] as f32
            + pixel_bl.0[2] as f32
            + pixel_br.0[2] as f32
            + pixel_mm.0[2] as f32)
            / (255.0 * 5.0),
        (pixel_tl.0[3] as f32
            + pixel_tr.0[3] as f32
            + pixel_bl.0[3] as f32
            + pixel_br.0[3] as f32
            + pixel_mm.0[3] as f32)
            / (255.0 * 5.0),
    );
    let luminance = b_color.relative_luminance();
    let text_color = if luminance > 0.179 {
        iced::Color::BLACK
    } else {
        iced::Color::WHITE
    };

    Theme::custom(
        "custom",
        iced::theme::Palette {
            background: b_color,
            text: text_color,
            primary: Default::default(),
            success: Default::default(),
            warning: Default::default(),
            danger: Default::default(),
        },
    )
}
