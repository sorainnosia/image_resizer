# Image Resizer

A powerful command-line tool for resizing images by file size and/or dimensions, written in Rust.

## Features

- **Target File Size**: Automatically compress images to stay under a specific file size (in KB)
- **Dimension Resizing**: Resize images to specific dimensions with optional aspect ratio maintenance
- **Batch Processing**: Process individual files or entire directories
- **Smart Compression**: Uses iterative compression and scaling to achieve target sizes
- **Multiple Format Support**: JPEG, PNG, GIF, BMP, WebP

## Installation

1. Make sure you have Rust installed (https://rustup.rs/)
2. Clone the repository or create a new Rust project
3. Add the code to `src/main.rs`
4. Update `Cargo.toml` with the dependencies
5. Build the project:

```bash
cargo build --release
```

The executable will be in `target/release/image-resizer`

## Usage

### Basic Commands

**Resize a single image to under 100KB:**
```bash
image-resizer -i photo.jpg -s 100
```

**Resize all images in a directory to under 500KB:**
```bash
image-resizer -i /path/to/images -s 500
```

**Resize to specific dimensions:**
```bash
image-resizer -i photo.jpg -d 800x600
```

**Combine size and dimension constraints:**
```bash
image-resizer -i photo.jpg -s 200 -d 1024x768
```

**Maintain aspect ratio when resizing:**
```bash
image-resizer -i photo.jpg -d 800x600 -r
```

**Specify output directory:**
```bash
image-resizer -i input_dir -s 100 -o output_dir
```

### Command Line Options

- `-i, --input <PATH>` - Input image file or directory (required)
- `-s, --size <KB>` - Target file size in kilobytes
- `-d, --dimensions <WIDTHxHEIGHT>` - Target dimensions (e.g., 800x600)
- `-o, --output <DIR>` - Output directory (default: creates 'resized' subdirectory)
- `-r, --maintain-ratio` - Maintain aspect ratio when resizing
- `-h, --help` - Print help information
- `-V, --version` - Print version information

## How It Works

### File Size Reduction Algorithm

1. **Quality Reduction**: First tries to reduce file size by lowering JPEG quality (95% → 20%)
2. **Image Scaling**: If quality reduction isn't enough, starts scaling down the image by 10% increments
3. **Iterative Process**: Continues until the target size is achieved or maximum iterations reached
4. **Best Fit**: Saves the best result that meets the size requirements

### Supported Formats

- JPEG/JPG - Uses quality-based compression
- PNG - Uses compression level optimization
- GIF, BMP - Basic support
- WebP - Converted to JPEG for compression (native WebP support can be added)

## Examples

**Batch resize photos for web upload (max 2MB):**
```bash
image-resizer -i vacation_photos/ -s 2048 -o web_ready/
```

**Prepare thumbnails (200x200, max 50KB):**
```bash
image-resizer -i products/ -d 200x200 -s 50 -r -o thumbnails/
```

**Process a single large photo for email:**
```bash
image-resizer -i DSC_0001.jpg -s 500 -d 1920x1080 -r
```

## Output

The tool provides feedback for each processed image:
```
Found 3 image(s) to process
✓ Processed: photo1.jpg (3456 KB → 98 KB)
  → Compressed with quality 75 to meet size target
✓ Processed: photo2.png (2100 KB → 95 KB)
  → Scaled to 85% with quality 80 to meet size target
✓ Processed: photo3.jpg (5200 KB → 99 KB)
  → Scaled to 70% with quality 85 to meet size target
```

## Tips

1. **PNG Files**: PNG compression is less flexible than JPEG. For strict size requirements, consider converting to JPEG
2. **Quality vs Size**: The tool prioritizes meeting size requirements over maintaining quality
3. **Aspect Ratio**: Use `-r` flag to prevent image distortion when resizing
4. **Large Reductions**: For very large size reductions (e.g., 10MB → 100KB), expect significant quality loss

## Future Enhancements

Consider adding:
- Progress bars for batch processing (using `indicatif` crate)
- Better WebP support (using `webp` crate)
- Configuration file support
- Parallel processing for faster batch operations
- Custom quality ranges
- EXIF data preservation options
