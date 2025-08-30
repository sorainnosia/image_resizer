use clap::{Arg, Command};
use image::{DynamicImage, ImageFormat, ImageError};
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
struct Config {
    input_path: PathBuf,
    target_size_kb: Option<u64>,
    dimensions: Option<(u32, u32)>,
    output_dir: Option<PathBuf>,
    maintain_aspect_ratio: bool,
    parallel: bool,
    verbose: bool,
	auto_scale: bool
}

#[derive(Debug)]
struct ProcessResult {
    input_path: PathBuf,
    output_path: PathBuf,
    original_size: u64,
    final_size: u64,
    success: bool,
    message: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = Command::new("Image Resizer Pro")
        .version("1.1")
        .author("Your Name")
        .about("Advanced image resizing by file size and/or dimensions")
        .arg(
            Arg::new("input")
                .short('i')
                .long("input")
                .value_name("PATH")
                .help("Input image file or directory")
                .required(true),
        )
        .arg(
            Arg::new("size")
                .short('s')
                .long("size")
                .value_name("KB")
                .help("Target file size in KB")
                .value_parser(clap::value_parser!(u64)),
        )
        .arg(
            Arg::new("dimensions")
                .short('d')
                .long("dimensions")
                .value_name("WIDTHxHEIGHT")
                .help("Target dimensions (e.g., 800x600)"),
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .value_name("DIR")
                .help("Output directory (default: creates 'resized' subdirectory)"),
        )
		.arg(
            Arg::new("auto-scale")
                .short('c')
                .long("auto-scale")
                .help("Auto scale image to fit target size, default: false")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("maintain-ratio")
                .short('r')
                .long("maintain-ratio")
                .help("Maintain aspect ratio when resizing")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("parallel")
                .short('p')
                .long("parallel")
                .help("Process images in parallel")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .help("Show detailed processing information")
                .action(clap::ArgAction::SetTrue),
        )
        .get_matches();

    let config = Config {
        input_path: PathBuf::from(matches.get_one::<String>("input").unwrap()),
        target_size_kb: matches.get_one::<u64>("size").copied(),
        dimensions: parse_dimensions(matches.get_one::<String>("dimensions")),
        output_dir: matches.get_one::<String>("output").map(PathBuf::from),
        maintain_aspect_ratio: matches.get_flag("maintain-ratio"),
		auto_scale: matches.get_flag("auto-scale"),
        parallel: matches.get_flag("parallel"),
		verbose: matches.get_flag("verbose"),
    };

    process_images(&config)?;
    Ok(())
}

fn parse_dimensions(dim_str: Option<&String>) -> Option<(u32, u32)> {
    dim_str.and_then(|s| {
        let parts: Vec<&str> = s.split('x').collect();
        if parts.len() == 2 {
            if let (Ok(width), Ok(height)) = (parts[0].parse(), parts[1].parse()) {
                return Some((width, height));
            }
        }
        None
    })
}

fn process_images(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let images = collect_images(&config.input_path)?;
    
    if images.is_empty() {
        println!("❌ No image files found!");
        return Ok(());
    }

    println!("📸 Found {} image(s) to process", images.len());
    
    let pb = ProgressBar::new(images.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}")?
            .progress_chars("#>-"),
    );

    let results = Arc::new(Mutex::new(Vec::new()));
    
    if config.parallel {
        let config = Arc::new(config.clone());
        let pb = Arc::new(pb);
        
        images.par_iter().for_each(|image_path| {
            let result = process_single_image_with_result(image_path, &config);
            pb.inc(1);
            
            if let Some(file_name) = image_path.file_name() {
                pb.set_message(format!("Processing: {}", file_name.to_string_lossy()));
            }
            
            results.lock().unwrap().push(result);
        });
        
        pb.finish_with_message("✨ Processing complete!");
    } else {
        for image_path in &images {
            if let Some(file_name) = image_path.file_name() {
                pb.set_message(format!("Processing: {}", file_name.to_string_lossy()));
            }
            
            let result = process_single_image_with_result(image_path, config);
            results.lock().unwrap().push(result);
            pb.inc(1);
        }
        pb.finish_with_message("✨ Processing complete!");
    }

    // Print summary
    println!("\n📊 Processing Summary:");
    println!("{}", "─".repeat(60));
    
    let results = results.lock().unwrap();
    let successful = results.iter().filter(|r| r.success).count();
    let failed = results.len() - successful;
    
    let total_original: u64 = results.iter().map(|r| r.original_size).sum();
    let total_final: u64 = results.iter().filter(|r| r.success).map(|r| r.final_size).sum();
    let total_saved = total_original.saturating_sub(total_final);
    
    println!("✅ Successful: {}", successful);
    println!("❌ Failed: {}", failed);
	if successful > 0 {
		println!("💾 Total saved: {} KB ({:.1}% reduction)", 
			total_saved / 1024, 
			(total_saved as f64 / total_original as f64) * 100.0
		);
	} else if failed > 0 {
		println!("❌ Couldn't reach target file size, specify -c to auto scale image");
	}
    
    if config.verbose {
        println!("\n📋 Detailed Results:");
        for result in results.iter() {
            if result.success {
                println!("  ✓ {} → {} ({} KB → {} KB) {}",
                    result.input_path.file_name().unwrap().to_string_lossy(),
                    result.output_path.file_name().unwrap().to_string_lossy(),
                    result.original_size / 1024,
                    result.final_size / 1024,
                    result.message
                );
            } else {
                println!("  ✗ {} - {}",
                    result.input_path.file_name().unwrap().to_string_lossy(),
                    result.message
                );
            }
        }
    }

    Ok(())
}

fn process_single_image_with_result(input_path: &Path, config: &Config) -> ProcessResult {
    let original_size = match fs::metadata(input_path) {
        Ok(metadata) => metadata.len(),
        Err(e) => {
            return ProcessResult {
                input_path: input_path.to_path_buf(),
                output_path: PathBuf::new(),
                original_size: 0,
                final_size: 0,
                success: false,
                message: format!("Failed to read file metadata: {}", e),
            };
        }
    };

    match process_single_image(input_path, config) {
        Ok(output_path) => {
            match fs::metadata(&output_path) {
                Ok(metadata) => ProcessResult {
                    input_path: input_path.to_path_buf(),
                    output_path: output_path.clone(),
                    original_size,
                    final_size: metadata.len(),
                    success: true,
                    message: String::new(),
                },
                Err(e) => ProcessResult {
                    input_path: input_path.to_path_buf(),
                    output_path,
                    original_size,
                    final_size: 0,
                    success: false,
                    message: format!("Failed to read output file: {}", e),
                },
            }
        }
        Err(e) => ProcessResult {
            input_path: input_path.to_path_buf(),
            output_path: PathBuf::new(),
            original_size,
            final_size: 0,
            success: false,
            message: e.to_string(),
        },
    }
}

fn collect_images(path: &Path) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut images = Vec::new();

    if path.is_file() {
        if is_image_file(path) {
            images.push(path.to_path_buf());
        }
    } else if path.is_dir() {
        for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file() && is_image_file(path) {
                images.push(path.to_path_buf());
            }
        }
    }

    Ok(images)
}

fn is_image_file(path: &Path) -> bool {
    match path.extension() {
        Some(ext) => {
            let ext = ext.to_string_lossy().to_lowercase();
            matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "tiff" | "tif")
        }
        None => false,
    }
}

fn process_single_image(
    input_path: &Path,
    config: &Config,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut img = image::open(input_path)?;
    
    // Apply dimension resize if specified
    if let Some((width, height)) = config.dimensions {
        img = resize_image(img, width, height, config.maintain_aspect_ratio);
    }

    // Determine output path
    let output_path = get_output_path(input_path, config)?;
    
    // If no target size specified, just save with default quality
    if config.target_size_kb.is_none() {
        save_image(&img, &output_path, 90)?;
        return Ok(output_path);
    }

    // Apply file size reduction using smart algorithm
    let target_bytes = config.target_size_kb.unwrap() * 1024;
    let format = get_image_format(input_path)?;
    
    // Smart compression algorithm
    let result = smart_compress(img, target_bytes, format, config.verbose, config.auto_scale)?;
    
    // Save the result
    fs::write(&output_path, result.data)?;
    
    if config.verbose {
        println!("  → Final quality: {}, Scale: {:.0}%", 
            result.quality, 
            result.scale * 100.0
        );
    }

    Ok(output_path)
}

struct CompressionResult {
    data: Vec<u8>,
    quality: u8,
    scale: f32,
}

fn smart_compress(
    img: DynamicImage,
    target_bytes: u64,
    format: ImageFormat,
    verbose: bool,
	auto_scale: bool
) -> Result<CompressionResult, Box<dyn std::error::Error>> {
    // Binary search for optimal quality
    let mut low_quality = 10;
    let mut high_quality = 95;
    let mut best_result = None;
    let mut scale_factor = 1.0;
    
    // First, try to achieve target with quality adjustment only
    while low_quality <= high_quality {
        let quality = (low_quality + high_quality) / 2;
        let buffer = save_to_buffer(&img, format, quality)?;
        let size = buffer.len() as u64;
        
        if verbose {
            println!("  Testing quality {}: {} KB", quality, size / 1024);
        }
        
        if size <= target_bytes {
            best_result = Some(CompressionResult {
                data: buffer,
                quality,
                scale: 1.0,
            });
            low_quality = quality + 1;
        } else {
            high_quality = quality - 1;
        }
    }
    
    // If quality adjustment alone isn't enough, start scaling
    if best_result.is_none() && auto_scale {
        scale_factor = 0.95;
        
        while scale_factor > 0.3 {
            let scaled_img = scale_image(&img, scale_factor);
            
            // Binary search with scaled image
            low_quality = 60;
            high_quality = 95;
            
            while low_quality <= high_quality {
                let quality = (low_quality + high_quality) / 2;
                let buffer = save_to_buffer(&scaled_img, format, quality)?;
                let size = buffer.len() as u64;
                
                if verbose {
                    println!("  Testing scale {:.0}%, quality {}: {} KB", 
                        scale_factor * 100.0, quality, size / 1024);
                }
                
                if size <= target_bytes {
                    best_result = Some(CompressionResult {
                        data: buffer,
                        quality,
                        scale: scale_factor,
                    });
                    break;
                } else {
                    high_quality = quality - 1;
                }
            }
            
            if best_result.is_some() {
                break;
            }
            
            scale_factor *= 0.85;
        }
    }
    
    best_result.ok_or_else(|| "Could not achieve target file size".into())
}

fn scale_image(img: &DynamicImage, scale: f32) -> DynamicImage {
    let new_width = (img.width() as f32 * scale) as u32;
    let new_height = (img.height() as f32 * scale) as u32;
    img.resize(new_width, new_height, image::imageops::FilterType::Lanczos3)
}

fn resize_image(img: DynamicImage, width: u32, height: u32, maintain_ratio: bool) -> DynamicImage {
    if maintain_ratio {
        img.resize(width, height, image::imageops::FilterType::Lanczos3)
    } else {
        img.resize_exact(width, height, image::imageops::FilterType::Lanczos3)
    }
}

fn get_output_path(
    input_path: &Path,
    config: &Config,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let output_dir = match &config.output_dir {
        Some(dir) => dir.clone(),
        None => {
            let parent = input_path.parent().unwrap_or(Path::new("."));
            parent.join("resized")
        }
    };

    fs::create_dir_all(&output_dir)?;

    let file_stem = input_path.file_stem().unwrap();
    let extension = input_path.extension().unwrap_or_default();
    
    // Add suffix to avoid overwriting
    let file_name = format!("{}_resized.{}", 
        file_stem.to_string_lossy(), 
        extension.to_string_lossy()
    );
    
    Ok(output_dir.join(file_name))
}

fn save_image(
    img: &DynamicImage,
    path: &Path,
    quality: u8,
) -> Result<(), Box<dyn std::error::Error>> {
    let format = get_image_format(path)?;
    let buffer = save_to_buffer(img, format, quality)?;
    fs::write(path, buffer)?;
    Ok(())
}

fn save_to_buffer(
    img: &DynamicImage,
    format: ImageFormat,
    quality: u8,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut buffer = Cursor::new(Vec::new());
    
    match format {
        ImageFormat::Jpeg => {
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buffer, quality);
            img.write_with_encoder(encoder)?;
        }
        ImageFormat::Png => {
            // PNG uses compression level (0-9), map quality to compression
            let compression = image::codecs::png::CompressionType::Best;
            let encoder = image::codecs::png::PngEncoder::new_with_quality(
                &mut buffer,
                compression,
                image::codecs::png::FilterType::Adaptive,
            );
            img.write_with_encoder(encoder)?;
        }
        ImageFormat::WebP => {
            // For WebP, fall back to JPEG for now
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buffer, quality);
            img.write_with_encoder(encoder)?;
        }
        _ => {
            img.write_to(&mut buffer, format)?;
        }
    }
    
    Ok(buffer.into_inner())
}

fn get_image_format(path: &Path) -> Result<ImageFormat, Box<dyn std::error::Error>> {
    match path.extension().and_then(|e| e.to_str()) {
        Some("jpg") | Some("jpeg") => Ok(ImageFormat::Jpeg),
        Some("png") => Ok(ImageFormat::Png),
        Some("gif") => Ok(ImageFormat::Gif),
        Some("bmp") => Ok(ImageFormat::Bmp),
        Some("webp") => Ok(ImageFormat::WebP),
        Some("tiff") | Some("tif") => Ok(ImageFormat::Tiff),
        _ => Err("Unsupported image format".into()),
    }
}