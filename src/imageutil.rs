use egui::ColorImage;
use itertools::iproduct;
use rand::{distributions::Alphanumeric, Rng};
use sciimg::prelude::Image;

pub fn sciimg_to_color_image(ser_frame: &Image) -> ColorImage {
    let mut copied = ser_frame.clone();
    let size: [usize; 2] = [copied.width as _, copied.height as _];
    copied.normalize_to_8bit();
    let mut rgb: Vec<u8> = Vec::with_capacity(copied.height * copied.width * 3);
    iproduct!(0..copied.height, 0..copied.width).for_each(|(y, x)| {
        let (r, g, b) = if copied.num_bands() == 1 {
            (
                copied.get_band(0).get(x, y),
                copied.get_band(0).get(x, y),
                copied.get_band(0).get(x, y),
            )
        } else {
            (
                copied.get_band(0).get(x, y),
                copied.get_band(1).get(x, y),
                copied.get_band(2).get(x, y),
            )
        };
        rgb.push(r as u8);
        rgb.push(g as u8);
        rgb.push(b as u8);
    });
    ColorImage::from_rgb(size, &rgb)
}

// https://stackoverflow.com/questions/54275459/how-do-i-create-a-random-string-by-sampling-from-alphanumeric-characters
pub fn gen_random_texture_name() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(7)
        .map(char::from)
        .collect()
}
