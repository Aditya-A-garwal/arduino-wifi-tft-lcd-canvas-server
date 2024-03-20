#![feature(iter_array_chunks)]

#![doc(html_favicon_url = "https://i0.wp.com/dumblebots.com/wp-content/uploads/2023/12/dumblebots-logo-round.png")]
#![doc(html_logo_url = "https://i0.wp.com/dumblebots.com/wp-content/uploads/2023/12/dumblebots-logo-round.png")]

//! # Arduino WiFI TFT LCD Canvas Server
//! Server for the [Arduino WiFi TFT LCD Canvas App](https://github.com/Aditya-A-garwal/Arduino-WiFi-TFT-LCD-Canvas-App).

mod image;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread::{self};

use clap::Parser;
use pbr::ProgressBar;

use image::*;

/// Width of the progress bar in characters
const PROGRESS_BAR_WIDTH: usize = 96;
/// Period of time to wait for the client's request for the next chunk, before the communication is terminated (considered failed)
const SOCKET_TIMEOUT: Option<std::time::Duration> = Some(std::time::Duration::from_secs(8));
/// Whether to display the progress bar or not
const SHOW_PROGRESS_BAR: bool = true;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Port on which to list for incoming requests
    #[arg(short, long, default_value_t = 5005)]
    port: u16,

    /// Path to directory where images are stored
    #[arg(short, long, default_value_t = String::from("images-dir"))]
    image_dir: String,
}

fn main() {
    let args = Args::parse();

    let host = "0.0.0.0";
    let port = args.port;

    let image_dir = args.image_dir;

    println!();
    println!("Starting Dumblebots Arduino Canvas Server...");
    println!();

    match std::fs::create_dir(&image_dir) {
        Ok(()) => println!("Successfully created images directory"),
        Err(err) => {
            if err.kind() == std::io::ErrorKind::AlreadyExists {
                println!("Found image directory")
            } else {
                eprintln!("Failed to create image directory");
                return;
            }
        }
    };

    let listener = match TcpListener::bind((host, port)) {
        Ok(listener) => listener,
        Err(err) => {
            if err.kind() == std::io::ErrorKind::PermissionDenied {
                eprintln!("Permission denied while binding server to port {}", port);
                eprintln!("hint: use sudo on linux");
            } else {
                eprintln!("Failed to bind server to port {}", port);
            }
            return;
        }
    };

    if let Ok(local_ip_addr) = local_ip_address::local_ip() {
        println!("Waiting for request on \"{:?}:{}\"", local_ip_addr, port)
    } else {
        println!("Waiting for requests on port \"{}\"", port);
    }

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let dir = image_dir.clone();
                thread::spawn(move || {
                    serve_client(stream, &dir);
                });
            }
            Err(e) => {
                eprintln!("Failed to accept connection: {}", e);
            }
        }
    }
}

/// Serves a single request from a single client
///
/// # Arguments
///
/// * `stream` - TCP connection with the client
///
fn serve_client(mut stream: TcpStream, dir: &str) {
    let mut buffer = [0; 6];

    // try to set the timeout for this connection
    let Ok(()) = stream.set_read_timeout(SOCKET_TIMEOUT) else {
        eprintln!("Failed to set timeout for socket");
        return;
    };

    // try to get the address of the client
    let Ok(peer) = stream.peer_addr() else {
        eprintln!("Failed to read peer for request");
        return;
    };

    let Ok(()) = stream.read_exact(&mut buffer) else {
        eprintln!("Failed Request");
        return;
    };

    let rw = buffer[0];
    let name = buffer[1];
    let height = u16::from_le_bytes([buffer[2], buffer[3]]) as usize;
    let width = u16::from_le_bytes([buffer[4], buffer[5]]) as usize;

    if rw == 1 {
        println!(
            r#"
            Saving new image from "{}" with
            Dimensions: {} x {}
            name: image_{}.bmp
            "#,
            peer, height, width, name
        );
        save_image(height, width, name, stream, dir);
    } else if rw == 2 {
        println!(
            r#"
            Loading new image to "{}" with
            Dimensions: {} x {}
            name: image_{}.bmp
            "#,
            peer, height, width, name
        );
        load_image(height, width, name, stream, dir);
    }
}

/// Saves an image sent from the client to the filesystem
///
/// # Arguments
///
/// * `height` - Number of rows in the image
/// * `width` - Number of columns in the image
/// * `stream` - TCP connection with the client
/// * `name` - The slot number of the image
/// * `dir` - Directory to save image to
///
fn save_image(height: usize, width: usize, name: u8, mut stream: TcpStream, dir: &str) {
    let mut img = Vec::with_capacity(height);

    let mut pb = match SHOW_PROGRESS_BAR {
        false => None,
        true => {
            let mut pb = ProgressBar::new(height as u64);
            pb.set_width(Some(PROGRESS_BAR_WIDTH));
            Some(pb)
        }
    };

    for row in 0..height {
        let mut mode = [0u8];
        let mut codes = vec![0; width];

        let Ok(_) = stream.read_exact(&mut mode) else {
            eprintln!("Error reading mode");
            return;
        };

        if mode[0] == 0 {
            let Ok(_) = stream.read_exact(&mut codes) else {
                eprintln!("Error reading row {}", row);
                return;
            };
        } else {
            let mut segments_bytes = vec![0u8; 2 * (mode[0] as usize)];
            let mut segments = vec![0u16; mode[0] as usize];

            let Ok(_) = stream.read_exact(&mut segments_bytes) else {
                eprintln!("Error reading compressed row {}", row);
                return;
            };

            segments
                .iter_mut()
                .zip(segments_bytes.into_iter().array_chunks::<2>())
                .for_each(|(seg, pair)| *seg = u16::from_le_bytes(pair));

            uncompress(&segments, &mut codes);
        }
        img.push(codes.iter().map(|&v| code_2_color(v).unwrap()).collect());

        match &mut pb {
            Some(pb) => pb.inc(),
            None => 0,
        };
    }
    match &mut pb {
        Some(pb) => pb.finish_println(""),
        None => (),
    };

    save_bmp_image(&img, &format!("{dir}/image_{name}"));
}

/// Loads an image from the filesystem to the client
///
/// # Arguments
///
/// * `expected_height` - Number of rows in the image as expected by the client
/// * `expected_width` - Number of columns in the image as expected by the client
/// * `stream` - TCP connection with the client
/// * `name` - The slot number of the image
/// * `dir` - Directory to retrieve the image from
///
fn load_image(
    expected_height: usize,
    expected_width: usize,
    name: u8,
    mut stream: TcpStream,
    dir: &str,
) {
    let img = load_bmp_image(
        &format!("{dir}/image_{name}"),
        expected_width,
        expected_height,
    );

    let mut pb = match SHOW_PROGRESS_BAR {
        false => None,
        true => {
            let mut pb = ProgressBar::new(expected_height as u64);
            pb.set_width(Some(PROGRESS_BAR_WIDTH));
            Some(pb)
        }
    };

    for (i, row) in img.iter().enumerate() {
        let codes: Vec<u8> = (*row).iter().map(|&v| color_2_code(v).unwrap()).collect();

        let Ok(()) = stream.write_all(&codes) else {
            eprintln!("Error while sending row {}", i);
            return;
        };
        let Ok(()) = stream.flush() else {
            eprintln!("Error while flushing row {}", i);
            return;
        };

        if (i % 10) == 0 {
            let Ok(()) = stream.read_exact(&mut [0u8]) else {
                eprintln!("Not received confirmation after row {}", i);
                return;
            };
        }
        match &mut pb {
            Some(pb) => pb.inc(),
            None => 0,
        };
    }

    let Ok(()) = stream.read_exact(&mut [0u8]) else {
        println!("Not recieved final confirmation");
        return;
    };
    match &mut pb {
        Some(pb) => pb.finish_println(""),
        None => (),
    };
}

/// Uncompress a row from segment-representation into its pixel-representation and get the number of pixels
///
/// # Arguments
///
/// * `segments` - Slice of 16-bit integers, each representing a valid segment with a code and size
/// * `codes` - Mutable slice of 8-bit integers, where the uncompressed data must be stored
///
pub fn uncompress(segments: &[u16], codes: &mut [u8]) -> usize {
    let mut idx = 0;

    for &segment in segments.iter() {
        let code = (segment & 0xF) as u8;
        let count = ((segment >> 4) & 0x1FF) as usize;

        if codes.len() < (idx + count) {
            break;
        }

        codes
            .iter_mut()
            .skip(idx)
            .take(count)
            .for_each(|v| *v = code);
        idx += count;
    }

    idx
}

/// Compresse a row from pixel-representation into its segment-representation and get the number of segments, pixels
///
/// # Arguments
///
/// * `segments` - Mutable slice of 16-bit integers, where the compressed data must be stored
/// * `codes` - Slice of 8-bit integers, each representing a valid code
///
pub fn compress(segments: &mut [u16], codes: &[u8]) -> (usize, usize) {
    let mut num_segments = 0usize;
    let mut num_pixels = 0usize;

    let mut code_it = codes.iter().enumerate();
    let mut segment_it = segments.iter_mut();

    while let Some((l, &lo)) = code_it.next() {
        let r = codes
            .iter()
            .skip(l + 1)
            .position(|&hi| hi != lo)
            .unwrap_or(codes.len());

        let code = (lo & 0xF) as u16;
        let count = ((r - l) & 0x1FF) as u16;

        let Some(segment) = segment_it.next() else {
            break;
        };

        *segment = (count << 4) | code;
        num_segments += 1;
        num_pixels += r - l;

        code_it.nth(r - 1);
    }

    (num_segments, num_pixels)
}
