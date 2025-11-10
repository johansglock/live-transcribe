use image::{Rgba, RgbaImage};

fn main() {
    // Create 64x64 image with transparent background (for @2x retina)
    let mut img = RgbaImage::from_pixel(64, 64, Rgba([0, 0, 0, 0]));

    // Black color
    let black = Rgba([0, 0, 0, 255]);

    // Scale factor from 22x22 design to 64x64
    let scale = 2.9;

    // Draw waveform bars with rounded corners (simple approximation)
    // Short bar (left)
    draw_rounded_rect(&mut img, (2.0*scale) as u32, (10.0*scale) as u32,
                     (3.0*scale) as u32, (6.0*scale) as u32, black);

    // Long bar (middle)
    draw_rounded_rect(&mut img, (7.0*scale) as u32, (4.0*scale) as u32,
                     (3.0*scale) as u32, (14.0*scale) as u32, black);

    // Medium bar (right)
    draw_rounded_rect(&mut img, (12.0*scale) as u32, (7.0*scale) as u32,
                     (3.0*scale) as u32, (10.0*scale) as u32, black);

    // Short bar (right)
    draw_rounded_rect(&mut img, (17.0*scale) as u32, (9.0*scale) as u32,
                     (3.0*scale) as u32, (7.0*scale) as u32, black);

    // Save @2x version
    img.save("assets/icon@2x.png").expect("Failed to save @2x icon");

    // Create 32x32 version
    let img32 = image::imageops::resize(&img, 32, 32, image::imageops::FilterType::Lanczos3);
    img32.save("assets/icon.png").expect("Failed to save icon");

    println!("Icons generated successfully!");
}

fn draw_rounded_rect(img: &mut RgbaImage, x: u32, y: u32, w: u32, h: u32, color: Rgba<u8>) {
    // Simple rounded rectangle (just fill for now)
    for py in y..(y + h).min(img.height()) {
        for px in x..(x + w).min(img.width()) {
            img.put_pixel(px, py, color);
        }
    }
}
