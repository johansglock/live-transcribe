mod config;

use anyhow::Result;
use config::Config;

fn main() -> Result<()> {
    println!("Live Transcribe - System Tray Application");

    // Load configuration
    let config = Config::load_or_create()?;
    println!("Configuration loaded: {:?}", config);

    Ok(())
}
