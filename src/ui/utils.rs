use iced::{Color, Theme};
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

/// Extracts a saturated accent color from a cover image.
///
/// Pixels with more chroma contribute more heavily, which avoids producing a
/// muddy gray when the cover contains only a small colorful element.
pub fn get_accent_color_from_image(img: &image::DynamicImage) -> Color {
    let sample = img.thumbnail(64, 64);
    let mut weighted_color = [0.0_f32; 3];
    let mut total_weight = 0.0_f32;

    for (_, _, pixel) in sample.pixels() {
        let [red, green, blue, _] = pixel.0;
        let red = red as f32 / 255.0;
        let green = green as f32 / 255.0;
        let blue = blue as f32 / 255.0;
        let max_channel = red.max(green).max(blue);
        let min_channel = red.min(green).min(blue);
        let chroma = max_channel - min_channel;
        let luminance = 0.2126 * red + 0.7152 * green + 0.0722 * blue;

        if luminance < 0.04 || luminance > 0.97 {
            continue;
        }

        let saturation = if max_channel > f32::EPSILON {
            chroma / max_channel
        } else {
            0.0
        };
        let weight = 0.15 + saturation * saturation * 3.0;

        weighted_color[0] += red * weight;
        weighted_color[1] += green * weight;
        weighted_color[2] += blue * weight;
        total_weight += weight;
    }

    if total_weight <= f32::EPSILON {
        return Color::from_rgb(0.35, 0.45, 0.65);
    }

    let mut red = weighted_color[0] / total_weight;
    let mut green = weighted_color[1] / total_weight;
    let mut blue = weighted_color[2] / total_weight;
    let luminance = 0.2126 * red + 0.7152 * green + 0.0722 * blue;

    // Increase chroma a little so the glow reads as color instead of gray.
    const SATURATION_BOOST: f32 = 1.45;
    red = (luminance + (red - luminance) * SATURATION_BOOST).clamp(0.0, 1.0);
    green = (luminance + (green - luminance) * SATURATION_BOOST).clamp(0.0, 1.0);
    blue = (luminance + (blue - luminance) * SATURATION_BOOST).clamp(0.0, 1.0);

    // Keep the accent visible on dark covers without turning it into white.
    let max_channel = red.max(green).max(blue);
    if max_channel < 0.65 {
        let scale = 0.65 / max_channel.max(f32::EPSILON);
        red = (red * scale).clamp(0.0, 1.0);
        green = (green * scale).clamp(0.0, 1.0);
        blue = (blue * scale).clamp(0.0, 1.0);
    }

    Color::from_rgb(red, green, blue)
}

#[cfg(test)]
mod tests {
    use super::get_accent_color_from_image;
    use image::{DynamicImage, Rgba, RgbaImage};

    #[test]
    fn extracts_color_from_red_cover() {
        let image = DynamicImage::ImageRgba8(RgbaImage::from_pixel(4, 4, Rgba([220, 30, 30, 255])));
        let accent = get_accent_color_from_image(&image);

        assert!(accent.r > accent.g);
        assert!(accent.r > accent.b);
    }

    #[test]
    fn falls_back_for_fully_transparent_image() {
        let image = DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 2, Rgba([255, 255, 255, 0])));
        let accent = get_accent_color_from_image(&image);

        assert_eq!(accent.a, 1.0);
    }
}
