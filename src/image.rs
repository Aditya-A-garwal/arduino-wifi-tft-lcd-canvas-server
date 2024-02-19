use std::fs::File;
use std::io::prelude::*;
use std::io::{Seek, SeekFrom};

use byteorder::*;

pub fn save_bmp_image(data: &[Vec<u16>], filename: &str) {
    let height = data.len();
    let width = data.first().unwrap().len();

    // Calculate the size of the image data in bytes
    let row_size = width * 2; // Each pixel is 16 bits (2 bytes)
    let padding_size = (4 - (row_size % 4)) % 4; // Calculate padding needed per row
    let image_size = (row_size + padding_size) * height;

    let mut bmp_header = Vec::with_capacity(14);
    let mut dib_header = Vec::with_capacity(40);

    let padding = vec![0; padding_size];

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

pub fn load_bmp_image(filename: &str) -> Vec<Vec<u16>> {
    // Open the BMP file
    let mut bmp_file = File::open(format!("{}.bmp", filename)).expect("Failed to open BMP file");

    // Read the BMP header
    let mut bmp_header = [0; 54];
    bmp_file
        .read_exact(&mut bmp_header)
        .expect("Failed to read BMP header");

    // Move to the start of pixel data
    bmp_file
        .seek(SeekFrom::Start(54))
        .expect("Failed to seek to pixel data");

    // Extract image width and height from the header
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

pub fn color_2_code(clr: u16) -> Result<u8, ()> {
    match clr {
        0xF800u16 => Ok(0),
        0x07E0u16 => Ok(1),
        0x001Fu16 => Ok(2),
        0x07FFu16 => Ok(3),
        0xF81Fu16 => Ok(4),
        0xFFE0u16 => Ok(5),
        0xFFFFu16 => Ok(6),
        0x520Au16 => Ok(7),
        0x0000u16 => Ok(8),
        _ => Err(()),
    }
}

pub fn code_2_color(code: u8) -> Result<u16, ()> {
    match code {
        0 => Ok(0xF800u16),
        1 => Ok(0x07E0u16),
        2 => Ok(0x001Fu16),
        3 => Ok(0x07FFu16),
        4 => Ok(0xF81Fu16),
        5 => Ok(0xFFE0u16),
        6 => Ok(0xFFFFu16),
        7 => Ok(0x520Au16),
        8 => Ok(0x0000u16),
        _ => Err(()),
    }
}
