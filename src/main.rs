// © Zach Nielsen 2024

mod data;
use crate::data::*;

use tesseract::Tesseract;
use chrono::{NaiveDate, Local, Datelike};
use image::imageops::FilterType;
use image::GenericImageView;
use regex::Regex;
use clap::{Parser, Subcommand};

use std::collections::HashMap;
use std::cmp::max;
use std::fs::OpenOptions;
use std::io::Write;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}
#[derive(Subcommand, Debug)]
enum Commands {
    Display {
        #[arg(short, long, default_value_t = 0)]
        offset: i8,
    },
}

fn main() {
    let cli = Cli::parse();
    let itemizer = FileItemizer::new();

    match &cli.command {
        Some(Commands::Display { offset }) => display_month(itemizer, offset),
        None => parse_files(itemizer),
    }
}

fn parse_files(mut itemizer: impl Itemizer) {
    let image_dir = get_env("ITEMIZER_IMAGE_DIR");
    let done_file = get_env("ITEMIZER_IMAGE_DONE_FILE");

    for entry in std::fs::read_dir(image_dir).unwrap() {
        let entry = entry.unwrap();
        let entry_path = entry.path();
        if entry.file_type().unwrap().is_dir() {
            // TODO: Recurse into directory?
            continue;
        }
        let entry_path = entry_path.as_os_str().to_str().unwrap();
        if image_done(entry_path) {
            println!("Receipt already done, skipping: {}", entry_path);
            continue;
        }

        // Upscale image
        let entry_name = entry.file_name().into_string().unwrap();
        let resized_path = resize_image(entry_path, entry_name);

        // OCR image
        let tess = Tesseract::new(None, Some("eng")).unwrap();
        let mut tess = tess.set_image(&resized_path).unwrap();
        // let mut tess = tess.set_image(entry_str).unwrap();
        let text = tess.get_text().unwrap();
        // println!("Recognized text:\n{}", text);
        // TODO: Delete upscaled image


        // Get the date in the file name
        let date_re = Regex::new(r"\d{4}-\d{2}-\d{2}").unwrap();
        let file_name = entry.file_name();
        let date_str = date_re.find(file_name.to_str().unwrap()).unwrap().as_str();
        itemizer.set_date(NaiveDate::parse_from_str(date_str, "%Y-%m-%d").unwrap());

        // Parse Receipt
        let receipt = Receipt::new(text);
        for line in receipt.text.lines() {
            let Some((code, desc, price)) = receipt.get_fields(line) else {
                continue;
            };

            // println!("Checking for [{}], [{}]", code, desc);
            itemizer.process_purchase(code, desc, price)
        }

        let mut done_fp = OpenOptions::new()
            .append(true)
            .open(&done_file).unwrap();
        writeln!(done_fp, "{}", entry_path).unwrap();
    }

    print_totals(itemizer.purchases());
    itemizer.save_to_disk();
}

fn resize_image(path: &str, name: String) -> String {
    println!("About to open: {:?}", path);
    let resized_path =
        [get_env("ITEMIZER_UPSCALED_IMAGE_DIR"), name].join("/");
    if std::fs::metadata(&resized_path).is_ok() {
        println!("Already upscaled [{}], moving on", resized_path);
    } else {
        let img = image::open(&path).unwrap();
        let (width, height) = img.dimensions();
        println!("About to upscale: {:?}", &path);
        let upscale = 1.5;
        let new_width = (width as f32 * upscale) as u32;
        let new_height = (height as f32 * upscale) as u32;
        // Resize the image using the Lanczos3 filter
        let resized_img = image::imageops::resize(&img, new_width, new_height, FilterType::Lanczos3);
        println!("About to save upscaled to: {}", &resized_path);
        resized_img.save(&resized_path).unwrap();
        println!("Image has been upscaled and saved successfully.");
    }
    resized_path
}

fn display_month(itemizer: impl Itemizer, offset: &i8) {
    let month = Local::now().month() as i8 + offset;
    let mut keep_list: Purchases = data::Purchases(Vec::new());
    for purchase in itemizer.purchases_iter() {
        if purchase.date.month0() == month as u32 - 1 {
            keep_list.push(purchase.clone());
        }
    }

    print_totals(&keep_list);
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

    let mut vec: Vec<(&str, f64)> = tot.iter().map(|(&k, &v)| (k,v)).collect();
    vec.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

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
            if tag == "" { continue; }
            tot.entry(&tag).and_modify(|val| *val += p.price).or_insert(p.price);
            total += p.price;
        }
    }

    let mut vec: Vec<(&str, f64)> = tot.iter().map(|(&k, &v)| (k,v)).collect();
    vec.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

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

