use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

const BASE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

pub struct ModelDownloader {
    models_dir: PathBuf,
}

impl ModelDownloader {
    pub fn new(models_dir: PathBuf) -> Self {
        ModelDownloader { models_dir }
    }

    pub fn ensure_model_exists(&self, model_name: &str) -> Result<()> {
        let model_path = self.models_dir.join(format!("ggml-{}.bin", model_name));

        if model_path.exists() {
            println!("✓ Model found: {}", model_path.display());

            // Check for CoreML encoder if using .en model
            if model_name.ends_with(".en") {
                self.ensure_coreml_encoder(model_name)?;
            }

            return Ok(());
        }

        println!("Model not found, downloading...");
        self.download_model(model_name)?;

        Ok(())
    }

    fn download_model(&self, model_name: &str) -> Result<()> {
        // Create models directory
        fs::create_dir_all(&self.models_dir)
            .context("Failed to create models directory")?;

        let model_filename = format!("ggml-{}.bin", model_name);
        let model_path = self.models_dir.join(&model_filename);
        let url = format!("{}/{}", BASE_URL, model_filename);

        println!("Downloading {} model...", model_name);
        println!("URL: {}", url);
        println!("This may take a few minutes depending on your connection...");

        self.download_file(&url, &model_path)?;

        println!("✓ Model downloaded successfully!");

        // Download CoreML encoder for .en models
        if model_name.ends_with(".en") {
            println!("\nDownloading CoreML encoder for Neural Engine acceleration...");
            self.download_coreml_encoder(model_name)?;
        }

        Ok(())
    }

    fn ensure_coreml_encoder(&self, model_name: &str) -> Result<()> {
        let encoder_dir = self.models_dir.join(format!("ggml-{}-encoder.mlmodelc", model_name));

        if encoder_dir.exists() {
            println!("✓ CoreML encoder found");
            return Ok(());
        }

        println!("CoreML encoder not found, downloading...");
        self.download_coreml_encoder(model_name)?;

        Ok(())
    }

    fn download_coreml_encoder(&self, model_name: &str) -> Result<()> {
        let encoder_filename = format!("ggml-{}-encoder.mlmodelc.zip", model_name);
        let encoder_zip_path = self.models_dir.join(&encoder_filename);
        let url = format!("{}/{}", BASE_URL, encoder_filename);

        println!("Downloading CoreML encoder...");
        self.download_file(&url, &encoder_zip_path)?;

        // Unzip the encoder
        println!("Extracting CoreML encoder...");
        self.unzip_file(&encoder_zip_path)?;

        // Clean up zip file
        fs::remove_file(&encoder_zip_path)
            .context("Failed to remove zip file")?;

        println!("✓ CoreML encoder installed successfully!");
        println!("  Neural Engine acceleration enabled for faster transcription!");

        Ok(())
    }

    fn download_file(&self, url: &str, dest: &Path) -> Result<()> {
        // Use curl on macOS (always available)
        let output = std::process::Command::new("curl")
            .arg("-L") // Follow redirects
            .arg("-#") // Show progress bar
            .arg("-o")
            .arg(dest)
            .arg(url)
            .status()
            .context("Failed to execute curl")?;

        if !output.success() {
            anyhow::bail!("Failed to download file from {}", url);
        }

        Ok(())
    }

    fn unzip_file(&self, zip_path: &Path) -> Result<()> {
        let output = std::process::Command::new("unzip")
            .arg("-q") // Quiet mode
            .arg("-o") // Overwrite files
            .arg(zip_path)
            .arg("-d")
            .arg(&self.models_dir)
            .status()
            .context("Failed to execute unzip")?;

        if !output.success() {
            anyhow::bail!("Failed to unzip file: {}", zip_path.display());
        }

        Ok(())
    }

    pub fn list_available_models() -> Vec<(&'static str, &'static str, &'static str)> {
        vec![
            ("tiny.en", "~75MB", "Fastest, good quality"),
            ("base.en", "~142MB", "Recommended - best balance"),
            ("small.en", "~466MB", "Better quality, slower"),
            ("medium.en", "~1.5GB", "Highest quality, slowest"),
            ("tiny", "~75MB", "Multilingual, fastest"),
            ("base", "~142MB", "Multilingual, balanced"),
            ("small", "~466MB", "Multilingual, better quality"),
            ("medium", "~1.5GB", "Multilingual, high quality"),
        ]
    }

}
