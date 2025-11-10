use anyhow::{Context, Result};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    TrayIcon, TrayIconBuilder,
};
use image::{Rgba, RgbaImage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayMenuEvent {
    StartTranscription,
    StopTranscription,
    Settings,
    Quit,
}

pub struct TrayApp {
    #[allow(dead_code)]
    tray_icon: TrayIcon,
    start_item: MenuItem,
    stop_item: MenuItem,
    settings_item: MenuItem,
}

impl TrayApp {
    pub fn new() -> Result<Self> {
        // Create icon programmatically - waveform bars (white on transparent)
        // White is the standard color for macOS menu bar icons
        let size = 32u32;
        let mut img = RgbaImage::from_pixel(size, size, Rgba([0, 0, 0, 0]));

        // White color for the icon (matches other menu bar icons)
        let black = Rgba([255, 255, 255, 255]);

        // Draw 4 vertical waveform bars
        // Scale from 22x22 design to 32x32
        let scale = 32.0 / 22.0;

        // Short bar (left)
        Self::draw_rect(&mut img, (2.0*scale) as u32, (10.0*scale) as u32,
                       (3.0*scale) as u32, (6.0*scale) as u32, black);

        // Long bar (middle-left)
        Self::draw_rect(&mut img, (7.0*scale) as u32, (4.0*scale) as u32,
                       (3.0*scale) as u32, (14.0*scale) as u32, black);

        // Medium bar (middle-right)
        Self::draw_rect(&mut img, (12.0*scale) as u32, (7.0*scale) as u32,
                       (3.0*scale) as u32, (10.0*scale) as u32, black);

        // Short bar (right)
        Self::draw_rect(&mut img, (17.0*scale) as u32, (9.0*scale) as u32,
                       (3.0*scale) as u32, (7.0*scale) as u32, black);

        let icon = tray_icon::Icon::from_rgba(
            img.into_raw(),
            size,
            size,
        )
        .context("Failed to create icon")?;

        // Create menu
        let menu = Menu::new();

        let start_item = MenuItem::new("Start Transcription", true, None);
        let stop_item = MenuItem::new("Stop Transcription", false, None);
        let settings_item = MenuItem::new("Settings", true, None);

        menu.append(&start_item)?;
        menu.append(&stop_item)?;
        menu.append(&PredefinedMenuItem::separator())?;
        menu.append(&settings_item)?;
        menu.append(&PredefinedMenuItem::separator())?;
        menu.append(&PredefinedMenuItem::quit(Some("Quit")))?;

        // Create tray icon
        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Live Transcribe")
            .with_icon(icon)
            .build()
            .context("Failed to create tray icon")?;

        Ok(TrayApp {
            tray_icon,
            start_item,
            stop_item,
            settings_item,
        })
    }

    fn draw_rect(img: &mut RgbaImage, x: u32, y: u32, w: u32, h: u32, color: Rgba<u8>) {
        let width = img.width();
        let height = img.height();

        for py in y..(y + h).min(height) {
            for px in x..(x + w).min(width) {
                img.put_pixel(px, py, color);
            }
        }
    }

    pub fn set_transcribing(&self, is_transcribing: bool) {
        self.start_item.set_enabled(!is_transcribing);
        self.stop_item.set_enabled(is_transcribing);
    }

    pub fn poll_event(&self) -> Option<TrayMenuEvent> {
        if let Ok(event) = MenuEvent::receiver().try_recv() {
            let id = event.id();

            if id == self.start_item.id() {
                return Some(TrayMenuEvent::StartTranscription);
            } else if id == self.stop_item.id() {
                return Some(TrayMenuEvent::StopTranscription);
            } else if id == self.settings_item.id() {
                return Some(TrayMenuEvent::Settings);
            } else if id.0 == "quit" {
                return Some(TrayMenuEvent::Quit);
            }
        }
        None
    }
}
