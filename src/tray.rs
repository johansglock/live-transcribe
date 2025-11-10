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
    tray_icon: TrayIcon,
    start_item: MenuItem,
    stop_item: MenuItem,
    settings_item: MenuItem,
    base_icon: tray_icon::Icon,
    recording_icon: tray_icon::Icon,
    is_recording_visible: bool,
}

impl TrayApp {
    pub fn new() -> Result<Self> {
        // Create base icon - waveform bars (white on transparent)
        let size = 32u32;
        let base_icon = Self::create_base_icon(size)?;

        // Create recording icon (with red dot)
        let recording_icon = Self::create_recording_icon(size)?;

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

        // Create tray icon with base icon initially
        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Live Transcribe")
            .with_icon(base_icon.clone())
            .build()
            .context("Failed to create tray icon")?;

        Ok(TrayApp {
            tray_icon,
            start_item,
            stop_item,
            settings_item,
            base_icon,
            recording_icon,
            is_recording_visible: false,
        })
    }

    fn create_base_icon(size: u32) -> Result<tray_icon::Icon> {
        let mut img = RgbaImage::from_pixel(size, size, Rgba([0, 0, 0, 0]));

        // White color for the icon (matches other menu bar icons)
        let white = Rgba([255, 255, 255, 255]);

        // Draw 4 vertical waveform bars
        // Scale from 22x22 design to 32x32
        let scale = 32.0 / 22.0;

        // Short bar (left)
        Self::draw_rect(&mut img, (2.0*scale) as u32, (10.0*scale) as u32,
                       (3.0*scale) as u32, (6.0*scale) as u32, white);

        // Long bar (middle-left)
        Self::draw_rect(&mut img, (7.0*scale) as u32, (4.0*scale) as u32,
                       (3.0*scale) as u32, (14.0*scale) as u32, white);

        // Medium bar (middle-right)
        Self::draw_rect(&mut img, (12.0*scale) as u32, (7.0*scale) as u32,
                       (3.0*scale) as u32, (10.0*scale) as u32, white);

        // Short bar (right)
        Self::draw_rect(&mut img, (17.0*scale) as u32, (9.0*scale) as u32,
                       (3.0*scale) as u32, (7.0*scale) as u32, white);

        tray_icon::Icon::from_rgba(img.into_raw(), size, size)
            .context("Failed to create base icon")
    }

    fn create_recording_icon(size: u32) -> Result<tray_icon::Icon> {
        let mut img = RgbaImage::from_pixel(size, size, Rgba([0, 0, 0, 0]));

        // White color for the icon
        let white = Rgba([255, 255, 255, 255]);

        // Scale from 22x22 design to 32x32
        let scale = 32.0 / 22.0;

        // Draw waveform bars (same as base icon)
        Self::draw_rect(&mut img, (2.0*scale) as u32, (10.0*scale) as u32,
                       (3.0*scale) as u32, (6.0*scale) as u32, white);
        Self::draw_rect(&mut img, (7.0*scale) as u32, (4.0*scale) as u32,
                       (3.0*scale) as u32, (14.0*scale) as u32, white);
        Self::draw_rect(&mut img, (12.0*scale) as u32, (7.0*scale) as u32,
                       (3.0*scale) as u32, (10.0*scale) as u32, white);
        Self::draw_rect(&mut img, (17.0*scale) as u32, (9.0*scale) as u32,
                       (3.0*scale) as u32, (7.0*scale) as u32, white);

        // Add red dot in top-right corner
        let red = Rgba([255, 59, 48, 255]); // macOS red color
        Self::draw_circle(&mut img, (24.0*scale) as u32, (4.0*scale) as u32,
                         (3.0*scale) as u32, red);

        tray_icon::Icon::from_rgba(img.into_raw(), size, size)
            .context("Failed to create recording icon")
    }

    fn draw_circle(img: &mut RgbaImage, cx: u32, cy: u32, radius: u32, color: Rgba<u8>) {
        let width = img.width();
        let height = img.height();
        let r_sq = (radius * radius) as i32;

        for dy in -(radius as i32)..=(radius as i32) {
            for dx in -(radius as i32)..=(radius as i32) {
                if dx * dx + dy * dy <= r_sq {
                    let px = (cx as i32 + dx) as u32;
                    let py = (cy as i32 + dy) as u32;
                    if px < width && py < height {
                        img.put_pixel(px, py, color);
                    }
                }
            }
        }
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

    pub fn set_transcribing(&mut self, is_transcribing: bool) {
        self.start_item.set_enabled(!is_transcribing);
        self.stop_item.set_enabled(is_transcribing);

        // If stopping transcription, reset to base icon
        if !is_transcribing {
            let _ = self.tray_icon.set_icon(Some(self.base_icon.clone()));
            self.is_recording_visible = false;
        }
    }

    /// Toggle the recording indicator (call this periodically for blinking effect)
    pub fn blink_recording_indicator(&mut self) {
        self.is_recording_visible = !self.is_recording_visible;
        let icon = if self.is_recording_visible {
            &self.recording_icon
        } else {
            &self.base_icon
        };
        let _ = self.tray_icon.set_icon(Some(icon.clone()));
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
