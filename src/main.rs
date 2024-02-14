#![feature(iter_array_chunks)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

use pbr::ProgressBar;

use clap::Parser;

use crate::image::*;

mod image;

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

fn handle_client(mut stream: TcpStream, dir: &str) {
    let loading_bar_width = 96;

    let Ok(peer) = stream.peer_addr() else {
        println!("Failed to read peer");
        return;
    };

    // get the first give bytes, which are the image ID and dimensions
    let mut buffer = [0; 6];
    let Ok(_) = stream.read_exact(&mut buffer) else {
        println!("Failed Request");
        return;
    };

    let rw = buffer[0];
    let name = buffer[1];
    let height = (buffer[2] as usize) + ((buffer[3] as usize) << 8);
    let width = (buffer[4] as usize) + ((buffer[5] as usize) << 8);

    if rw == 1 {
        println!(
            r#"
            Receiving new image from "{peer}" with
            Dimensions: {height} x {width}
            name: image_{name}.bmp
            "#
        );

        let mut img = Vec::with_capacity(height);

        let mut pb = ProgressBar::new(height as u64);
        pb.set_width(Some(loading_bar_width));

        for row in 0..height {
            let mut mode = [0u8];
            let mut codes = vec![0; width];

            let Ok(_) = stream.read_exact(&mut mode) else {
                println!("Error reading mode");
                return;
            };

            if mode[0] == 0 {
                // normal mode

                let Ok(_) = stream.read_exact(&mut codes) else {
                    println!("Error reading row {row}");
                    return;
                };
            } else {
                // compressed mode

                let mut segments_bytes = vec![0u8; 2 * (mode[0] as usize)];
                let mut segments = vec![0u16; mode[0] as usize];

                let Ok(_) = stream.read_exact(&mut segments_bytes) else {
                    println!("Error reading compressed row {row}");
                    return;
                };

                segments
                    .iter_mut()
                    .zip(segments_bytes.into_iter().array_chunks::<2>())
                    .for_each(|(seg, pair)| *seg = u16::from_le_bytes(pair));

                let mut idx = 0;
                for &segment in segments.iter() {
                    let code = (segment & 0xF) as u8;
                    let count = ((segment >> 4) & 0x1FF) as usize;

                    codes
                        .iter_mut()
                        .skip(idx)
                        .take(count)
                        .for_each(|v| *v = code);
                    idx += count;
                }
            }
            img.push(codes.iter().map(|&v| code_2_color(v).unwrap()).collect());

            pb.inc();
        }
        pb.finish_println("");

        save_bmp_image(&img, &format!("{dir}/image_{name}"));
    } else if rw == 2 {
        println!(
            r#"
            Sending new image from "{peer}" with
            Dimensions: {height} x {width}
            name: image_{name}.bmp
            "#
        );

        let img = load_bmp_image(&format!("{dir}/image_{name}"));

        let mut pb = ProgressBar::new(height as u64);
        pb.set_width(Some(loading_bar_width));

        for (i, row) in img.iter().enumerate() {
            let val: Vec<u8> = (*row).iter().map(|&v| color_2_code(v).unwrap()).collect();

            let Ok(_) = stream.write_all(&val) else {
                println!("Not able to send row {i}");
                return;
            };
            pb.inc();
        }
        pb.finish_println("");
    }
}

fn main() {
    let args = Args::parse();

    // Define the host and port to listen on
    let host = "0.0.0.0";
    let port = args.port;

    // Name of images directory
    let image_dir = args.image_dir;

    // create the folder where images are stored
    match std::fs::create_dir(&image_dir) {
        Ok(()) => println!("Successfully created images directory"),
        Err(err) => {
            if err.kind() == std::io::ErrorKind::AlreadyExists {
                println!("Found image directory")
            } else {
                panic!("Failed to create image directory");
            }
        }
    };

    // Bind to the host and port
    let listener = match TcpListener::bind((host, port)) {
        Ok(listener) => listener,
        Err(err) => {
            if err.kind() == std::io::ErrorKind::PermissionDenied {
                println!("Permission denied while binding server to port {port}");
                println!("hint: use sudo on linux");
            } else {
                println!("Failed to bind server to port {port}");
            }
            return;
        }
    };

    // println!("TCP server is listening on {}:{}", host, port);
    println!("Waiting for requests on port {}", port);

    // Accept incoming connections
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let dir = image_dir.clone();

                // Spawn a new thread to handle each client connection
                thread::spawn(move || {
                    handle_client(stream, &dir);
                });
            }
            Err(e) => {
                eprintln!("Failed to accept connection: {}", e);
            }
        }
    }
}
