use anyhow::{Context, Result};
use image::{Rgba, RgbaImage};
use std::fs;
use std::path::Path;
use std::process::Command;

fn draw_rect(img: &mut RgbaImage, x: u32, y: u32, w: u32, h: u32, color: Rgba<u8>) {
    for dy in 0..h {
        for dx in 0..w {
            let px = x + dx;
            let py = y + dy;
            if px < img.width() && py < img.height() {
                img.put_pixel(px, py, color);
            }
        }
    }
}

fn create_app_icon(size: u32) -> Result<RgbaImage> {
    let mut img = RgbaImage::from_pixel(size, size, Rgba([0, 0, 0, 0]));

    // Blue color for app icon (matches macOS style)
    let blue = Rgba([59, 130, 246, 255]); // #3B82F6

    // Draw 4 vertical waveform bars
    // Scale from 22x22 design
    let scale = size as f32 / 22.0;

    // Short bar (left)
    draw_rect(&mut img, (2.0*scale) as u32, (10.0*scale) as u32,
             (3.0*scale) as u32, (6.0*scale) as u32, blue);

    // Long bar (middle-left)
    draw_rect(&mut img, (7.0*scale) as u32, (4.0*scale) as u32,
             (3.0*scale) as u32, (14.0*scale) as u32, blue);

    // Medium bar (middle-right)
    draw_rect(&mut img, (12.0*scale) as u32, (7.0*scale) as u32,
             (3.0*scale) as u32, (10.0*scale) as u32, blue);

    // Short bar (right)
    draw_rect(&mut img, (17.0*scale) as u32, (9.0*scale) as u32,
             (3.0*scale) as u32, (7.0*scale) as u32, blue);

    Ok(img)
}

fn main() -> Result<()> {
    println!("Generating app icon...");

    // Create iconset directory
    let iconset_dir = "AppIcon.iconset";
    if Path::new(iconset_dir).exists() {
        fs::remove_dir_all(iconset_dir)?;
    }
    fs::create_dir(iconset_dir)?;

    // Required sizes for macOS icons
    let sizes = vec![16, 32, 128, 256, 512];

    for size in sizes {
        println!("  Creating {}x{} icon...", size, size);

        // Create standard resolution
        let img = create_app_icon(size)?;
        img.save(format!("{}/icon_{}x{}.png", iconset_dir, size, size))?;

        // Create @2x (retina) version if applicable
        let size_2x = size * 2;
        if size_2x <= 1024 {
            println!("  Creating {}x{}@2x icon...", size, size);
            let img_2x = create_app_icon(size_2x)?;
            img_2x.save(format!("{}/icon_{}x{}@2x.png", iconset_dir, size, size))?;
        }
    }

    // Convert iconset to .icns using iconutil
    println!("  Converting to .icns...");
    let output = Command::new("iconutil")
        .args(&["-c", "icns", iconset_dir, "-o", "AppIcon.icns"])
        .output()
        .context("Failed to run iconutil (required on macOS)")?;

    if !output.status.success() {
        anyhow::bail!("iconutil failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    println!("✅ Icon created: AppIcon.icns");

    // Clean up iconset directory
    fs::remove_dir_all(iconset_dir)?;

    Ok(())
}
