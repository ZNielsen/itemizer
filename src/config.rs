// © Zach Nielsen 2024

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub image_dir: PathBuf,
    pub upscaled_image_dir: PathBuf,
    pub done_file: PathBuf,
    pub rules_file: PathBuf,
    pub purchases_file: PathBuf,
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = config_file_path()?;

        let mut config = if config_path.exists() {
            let text = std::fs::read_to_string(&config_path)
                .with_context(|| format!("Failed to read config file: {}", config_path.display()))?;
            toml::from_str(&text)
                .with_context(|| format!("Failed to parse config file: {}", config_path.display()))?
        } else {
            Self::default_config()?
        };

        // Env var overrides
        if let Ok(v) = std::env::var("ITEMIZER_IMAGE_DIR") { config.image_dir = PathBuf::from(v); }
        if let Ok(v) = std::env::var("ITEMIZER_UPSCALED_IMAGE_DIR") { config.upscaled_image_dir = PathBuf::from(v); }
        if let Ok(v) = std::env::var("ITEMIZER_IMAGE_DONE_FILE") { config.done_file = PathBuf::from(v); }
        if let Ok(v) = std::env::var("ITEMIZER_RULES_FILE") { config.rules_file = PathBuf::from(v); }
        if let Ok(v) = std::env::var("ITEMIZER_PURCHASES_FILE") { config.purchases_file = PathBuf::from(v); }

        Ok(config)
    }

    fn default_config() -> Result<Self> {
        let data_dir = data_dir_path()?;
        Ok(Self {
            image_dir: data_dir.join("images"),
            upscaled_image_dir: data_dir.join("upscaled"),
            done_file: data_dir.join("done"),
            rules_file: data_dir.join("rules"),
            purchases_file: data_dir.join("purchases"),
        })
    }

    pub fn init() -> Result<()> {
        let config_path = config_file_path()?;
        if config_path.exists() {
            println!("Config already exists: {}", config_path.display());
            return Ok(());
        }

        let config = Self::default_config()?;

        // Create config directory
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config directory: {}", parent.display()))?;
        }

        // Create data directories
        std::fs::create_dir_all(&config.image_dir)
            .with_context(|| format!("Failed to create image directory: {}", config.image_dir.display()))?;
        std::fs::create_dir_all(&config.upscaled_image_dir)
            .with_context(|| format!("Failed to create upscaled directory: {}", config.upscaled_image_dir.display()))?;

        // Write config file
        let text = toml::to_string_pretty(&config)
            .context("Failed to serialize default config")?;
        std::fs::write(&config_path, &text)
            .with_context(|| format!("Failed to write config file: {}", config_path.display()))?;

        println!("Created config at: {}", config_path.display());
        println!("Edit it to set your image directory and other paths.");
        println!("\nData directories created:");
        println!("  Images:   {}", config.image_dir.display());
        println!("  Upscaled: {}", config.upscaled_image_dir.display());

        Ok(())
    }
}

fn config_dir() -> Result<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        Ok(PathBuf::from(xdg).join("itemizer"))
    } else {
        let home = std::env::var("HOME").context("HOME environment variable not set")?;
        Ok(PathBuf::from(home).join(".config").join("itemizer"))
    }
}

fn config_file_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.toml"))
}

fn data_dir_path() -> Result<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        Ok(PathBuf::from(xdg).join("itemizer"))
    } else {
        let home = std::env::var("HOME").context("HOME environment variable not set")?;
        Ok(PathBuf::from(home).join(".local").join("share").join("itemizer"))
    }
}
