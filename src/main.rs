// © Zach Nielsen 2024

mod config;
mod data;

use crate::config::Config;
use crate::data::*;

use anyhow::{Context, Result};
use tesseract::Tesseract;
use chrono::{NaiveDate, Local, Datelike, Months};
use image::imageops::FilterType;
use image::GenericImageView;
use regex::Regex;
use clap::{Parser, Subcommand};

use std::collections::HashMap;
use std::cmp::max;
use std::fs::{DirEntry, OpenOptions};
use std::io::Write;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}
#[derive(Subcommand, Debug)]
enum Commands {
    /// Scan receipt images and record purchases
    Scan,
    /// Display totals for a month
    Display {
        #[arg(short, long, default_value_t = 0)]
        offset: i8,
    },
    /// Initialize config with default values
    Init,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Init) => Config::init(),
        Some(Commands::Display { offset }) => {
            let config = Config::load()?;
            let itemizer = FileItemizer::new(config)?;
            display_month(&itemizer, offset)
        }
        Some(Commands::Scan) | None => {
            let config = Config::load()?;
            let itemizer = FileItemizer::new(config)?;
            parse_files(itemizer)
        }
    }
}

fn process_single_image(entry: &DirEntry, itemizer: &mut FileItemizer) -> Result<()> {
    let entry_path = entry.path();
    let entry_path_str = entry_path.to_str()
        .context("Image path is not valid UTF-8")?;

    if image_done(entry_path_str, &itemizer.config.done_file)? {
        println!("Receipt already done, skipping: {}", entry_path_str);
        return Ok(());
    }

    // Upscale image
    let entry_name = entry.file_name().into_string()
        .map_err(|_| anyhow::anyhow!("Filename is not valid UTF-8"))?;
    let resized_path = resize_image(entry_path_str, &entry_name, &itemizer.config)?;

    // OCR image
    let tess = Tesseract::new(None, Some("eng"))
        .map_err(|e| anyhow::anyhow!("Failed to initialize Tesseract: {}", e))?;
    let mut tess = tess.set_image(&resized_path)
        .map_err(|e| anyhow::anyhow!("Failed to set image for OCR: {}", e))?;
    let text = tess.get_text()
        .map_err(|e| anyhow::anyhow!("OCR failed: {}", e))?;

    // Clean up upscaled image
    if let Err(e) = std::fs::remove_file(&resized_path) {
        eprintln!("Warning: could not clean up upscaled image {}: {}", resized_path, e);
    }

    // Get the date from the file name
    let date_re = Regex::new(r"\d{4}-\d{2}-\d{2}").unwrap();
    let file_name = entry.file_name();
    let file_name_str = file_name.to_str()
        .context("Filename is not valid UTF-8")?;
    let date_str = date_re.find(file_name_str)
        .with_context(|| format!("No date found in filename: {}", file_name_str))?
        .as_str();
    let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .with_context(|| format!("Invalid date in filename: {}", date_str))?;
    itemizer.set_date(date);

    // Parse Receipt
    let receipt = Receipt::new(text)?;
    for line in receipt.text.lines() {
        let Some((code, desc, price)) = receipt.get_fields(line) else {
            continue;
        };
        itemizer.process_purchase(code, desc, price);
    }

    // Mark as done
    let mut done_fp = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&itemizer.config.done_file)
        .context("Failed to open done file for writing")?;
    writeln!(done_fp, "{}", entry_path_str)?;

    Ok(())
}

fn parse_files(mut itemizer: FileItemizer) -> Result<()> {
    // Collect and sort entries by filename for deterministic date-ordered processing
    let mut entries: Vec<DirEntry> = std::fs::read_dir(&itemizer.config.image_dir)
        .with_context(|| format!("Failed to read image directory: {}", itemizer.config.image_dir.display()))?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|ft| ft.is_file()).unwrap_or(false))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in &entries {
        if let Err(e) = process_single_image(entry, &mut itemizer) {
            eprintln!("Error processing {:?}: {:?}", entry.path(), e);
            continue;
        }
    }

    print_totals(itemizer.purchases());
    itemizer.save_to_disk()?;
    Ok(())
}

fn resize_image(path: &str, name: &str, config: &Config) -> Result<String> {
    println!("About to open: {:?}", path);
    let resized_path = config.upscaled_image_dir.join(name);
    let resized_path_str = resized_path.to_str()
        .context("Upscaled image path is not valid UTF-8")?
        .to_owned();

    if resized_path.exists() {
        println!("Already upscaled [{}], moving on", resized_path_str);
    } else {
        let img = image::open(path)
            .with_context(|| format!("Failed to open image: {}", path))?;
        let (width, height) = img.dimensions();
        println!("About to upscale: {:?}", path);
        let upscale = 1.5;
        let new_width = (width as f32 * upscale) as u32;
        let new_height = (height as f32 * upscale) as u32;
        let resized_img = image::imageops::resize(&img, new_width, new_height, FilterType::Lanczos3);
        println!("About to save upscaled to: {}", &resized_path_str);
        resized_img.save(&resized_path)
            .with_context(|| format!("Failed to save upscaled image: {}", resized_path_str))?;
        println!("Image has been upscaled and saved successfully.");
    }
    Ok(resized_path_str)
}

fn display_month(itemizer: &FileItemizer, offset: &i8) -> Result<()> {
    let now = Local::now().naive_local().date();
    let target = if *offset >= 0 {
        now.checked_add_months(Months::new(*offset as u32))
    } else {
        now.checked_sub_months(Months::new(offset.unsigned_abs() as u32))
    }.context("Date offset out of range")?;

    let target_year = target.year();
    let target_month = target.month();

    println!("Showing: {} {}", target.format("%B"), target_year);

    let mut keep_list = Purchases(Vec::new());
    for p in &itemizer.purchases().0 {
        if p.date.year() == target_year && p.date.month() == target_month {
            keep_list.push(p.clone());
        }
    }

    print_totals(&keep_list);
    Ok(())
}


fn print_totals(purchases: &Purchases) {
    println!("\n===========================================================\n");
    print_totals_by_name(purchases);
    println!("\n===========================================================\n");
    println!("\n===========================================================\n");
    print_totals_by_tag(purchases);
    println!("\n===========================================================\n");
}

pub fn print_totals_by_name(purchases: &Purchases) {
    let mut total: f64 = 0.0;
    let mut tot: HashMap<&str, f64> = HashMap::new();
    for p in &purchases.0 {
        if p.tags.contains(&"EXCLUDE".to_owned()) { continue; }
        tot.entry(&p.name).and_modify(|val| *val += p.price).or_insert(p.price);
        total += p.price;
    }

    let mut vec: Vec<(&str, f64)> = tot.iter().map(|(&k, &v)| (k, v)).collect();
    vec.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut price_max = 10;
    let mut name_max = 0;
    for item in &vec {
        price_max = max(price_max, item.1.to_string().len());
        name_max = max(name_max, item.0.len());
    }
    println!("Totals by name: {:.2}", total);
    for item in vec {
        println!("{:>price_max$.2} | {:<name_max$}", item.1, item.0);
    }
}

pub fn print_totals_by_tag(purchases: &Purchases) {
    let mut total: f64 = 0.0;
    let mut tot: HashMap<&str, f64> = HashMap::new();
    for p in &purchases.0 {
        if p.tags.is_empty() || p.tags.contains(&"EXCLUDE".to_owned()) {
            continue;
        }

        for tag in &p.tags {
            if tag.is_empty() { continue; }
            tot.entry(tag).and_modify(|val| *val += p.price).or_insert(p.price);
            total += p.price;
        }
    }

    let mut vec: Vec<(&str, f64)> = tot.iter().map(|(&k, &v)| (k, v)).collect();
    vec.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut price_max = 10;
    let mut name_max = 0;
    for item in &vec {
        price_max = max(price_max, item.1.to_string().len());
        name_max = max(name_max, item.0.len());
    }
    println!("Totals by tag: {:.2}", total);
    for item in vec {
        println!("{:>price_max$.2} | {:<name_max$}", item.1, item.0);
    }
}
