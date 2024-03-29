//! Functions to save/load BMP image files and do color-code conversions

use std::fs::File;
use std::io::prelude::*;
use std::io::SeekFrom;

use byteorder::*;

/// Saves a 16-bit color (5-6-5) BMP Image to the filesystem
///
/// # Arguments
///
/// * `data` - A 16-bit color bitmap that must be saved
/// * `filename` - The name of the file (extensionless)
///
/// # Panics
///
/// * When the given image has 0 rows
/// * When the program does not have sufficient priviledges to create/modify the file at the given location
///
pub fn save_bmp_image(data: &[Vec<u16>], filename: &str) {
    let height = data.len();
    let width = data.first().unwrap().len();

    let row_size = width * 2;
    let padding_size = (4 - (row_size % 4)) % 4;
    let image_size = (row_size + padding_size) * height;

    let padding = vec![0; padding_size];

    let mut bmp_header = Vec::with_capacity(14);
    let mut dib_header = Vec::with_capacity(40);

    bmp_header.write_all(b"BM").unwrap(); // Write the 2-byte string "BM"
    bmp_header
        .write_u32::<LE>(54 + (image_size as u32))
        .unwrap(); // Write a 32-bit unsigned integer (image size + 54)
    bmp_header.write_u16::<LE>(0).unwrap(); // Write a 16-bit unsigned integer (0)
    bmp_header.write_u16::<LE>(0).unwrap(); // Write a 16-bit unsigned integer (0)
    bmp_header.write_u32::<LE>(54).unwrap(); // Write a 32-bit unsigned integer (54)

    dib_header.write_u32::<LE>(40).unwrap(); // Write a 32-bit unsigned integer (40)
    dib_header.write_i32::<LE>(width as i32).unwrap(); // Write a 32-bit signed integer (width)
    dib_header.write_i32::<LE>(height as i32).unwrap(); // Write a 32-bit signed integer (height)
    dib_header.write_u16::<LE>(1).unwrap(); // Write a 16-bit unsigned integer (1)
    dib_header.write_u16::<LE>(16).unwrap(); // Write a 16-bit unsigned integer (16)
    dib_header.write_u32::<LE>(0).unwrap(); // Write a 32-bit unsigned integer (0)
    dib_header.write_u32::<LE>(image_size as u32).unwrap(); // Write a 32-bit unsigned integer (image size)
    dib_header.write_u32::<LE>(0).unwrap(); // Write a 32-bit unsigned integer (0)
    dib_header.write_u32::<LE>(0).unwrap(); // Write a 32-bit unsigned integer (0)
    dib_header.write_u32::<LE>(0).unwrap(); // Write a 32-bit unsigned integer (0)
    dib_header.write_u32::<LE>(0).unwrap(); // Write a 32-bit unsigned integer (0)

    // Write to BMP file
    let mut bmp_file =
        File::create(format!("{}.bmp", filename)).expect("Failed to create BMP file");
    bmp_file
        .write_all(&bmp_header)
        .expect("Failed to write BMP header");
    bmp_file
        .write_all(&dib_header)
        .expect("Failed to write DIB header");

    // Write pixel data
    for row in data.iter().rev() {
        for &v in row.iter() {
            bmp_file
                .write_all(&v.to_le_bytes())
                .expect("Failed to write pixel data");
        }

        // Write padding bytes
        bmp_file
            .write_all(&padding)
            .expect("Failed to write padding");
    }
}

/// Loads a 16-bit color (5-6-5) BMP Image from the filesystem
///
/// If the image dimensions do not match the expected dimensions or the image does not exist, a blank image is returned
///
/// # Arguments
///
/// * `filename` - The name of the file (extensionless)
/// * `expected_width` - The expected width of the image
/// * `expected_height` - The expected height of the image
///
/// # Panics
///
/// * When the program does not have sufficient priviledges to open/read the file at the given location
///
pub fn load_bmp_image(
    filename: &str,
    expected_width: usize,
    expected_height: usize,
) -> Vec<Vec<u16>> {
    // Open the BMP file
    let Ok(mut bmp_file) = File::open(format!("{}.bmp", filename)) else {
        let result = vec![vec![0u16; expected_width]; expected_height];
        return result;
    };

    // Read the BMP Header
    let mut bmp_header = [0; 54];
    bmp_file
        .read_exact(&mut bmp_header)
        .expect("Failed to read BMP header");
    bmp_file
        .seek(SeekFrom::Start(54))
        .expect("Failed to seek to pixel data");

    // Extract image dimensions from the header
    let width = u32::from_le_bytes([
        bmp_header[18],
        bmp_header[19],
        bmp_header[20],
        bmp_header[21],
    ]) as usize;
    let height = u32::from_le_bytes([
        bmp_header[22],
        bmp_header[23],
        bmp_header[24],
        bmp_header[25],
    ]) as usize;

    // if the actual dimensions do not match the expected dimensions, return a blank image with the expected dimensions
    if width != expected_width || height != expected_height {
        let result = vec![vec![0u16; expected_width]; expected_height];
        return result;
    }

    // Calculate the size of each row, including padding if necessary
    let row_size = width * 2; // Each pixel is 16 bits (2 bytes)
    let padding_size = (4 - (row_size % 4)) % 4; // Calculate padding needed per row

    let mut padding = vec![0; padding_size];

    // Read the pixel data
    let mut pixels = vec![vec![0; width]; height];
    let mut color_data = [0, 0];

    for row in pixels.iter_mut().rev() {
        for element in row.iter_mut() {
            bmp_file
                .read_exact(&mut color_data)
                .expect("Failed to read color data");

            *element = u16::from_le_bytes(color_data);
        }

        bmp_file
            .read_exact(&mut padding)
            .expect("Failed to read padding data");
    }

    pixels
}

/// Converts a 16-bit color to a 4-bit code
///
/// The code is placed in the lower nibble of the returned byte
///
/// # Arguments
///
/// * `color` - The 16-bit color to convert to its code
///
/// # Errors
///
/// * When the supplied color does not map to any code
///
pub fn color_2_code(color: u16) -> Option<u8> {
    match color {
        0xF800u16 => Some(0),
        0x07E0u16 => Some(1),
        0x001Fu16 => Some(2),
        0x07FFu16 => Some(3),
        0xF81Fu16 => Some(4),
        0xFFE0u16 => Some(5),
        0xFFFFu16 => Some(6),
        0x520Au16 => Some(7),
        0x0000u16 => Some(8),
        _ => None,
    }
}

/// Converts a 4-bit code to a 16-bit color
///
/// The code must be placed in the lower nibble of the passed byte
///
/// # Arguments
///
/// * `code` - The 4-bit color to convert to its code
///
/// # Errors
///
/// * When the supplied code does not map to any color
///
pub fn code_2_color(code: u8) -> Option<u16> {
    match code {
        0 => Some(0xF800u16),
        1 => Some(0x07E0u16),
        2 => Some(0x001Fu16),
        3 => Some(0x07FFu16),
        4 => Some(0xF81Fu16),
        5 => Some(0xFFE0u16),
        6 => Some(0xFFFFu16),
        7 => Some(0x520Au16),
        8 => Some(0x0000u16),
        _ => None,
    }
}
