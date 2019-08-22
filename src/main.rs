use std::error::Error;
use std::time::{Duration, Instant};

use futures::future::poll_fn;
use png::{self};
use reqwest;
use serde::Deserialize;
use tokio_threadpool::blocking;
use warp::{
    self,
    http::{self, Response},
    path, Filter,
};

fn main() {
    let logo_options = warp::query::<LogoOptions>();

    // GET /hello/warp => 200 OK with body "Hello, warp!"
    let logo = path!("logo.png").and(logo_options).and_then(|options| {
        poll_fn(move || blocking(|| logo(options)).map_err(|err| warp::reject::custom(err)))
    });
    let index = path::end().map(|| "Logo PNG");
    let health = path!("health").map(|| "OK");

    let routes = index.or(logo).or(health);

    warp::serve(routes).run(([0, 0, 0, 0], 3000));
}

#[derive(Debug, Deserialize, Copy, Clone)]
struct LogoOptions {
    size: Option<u32>,
}

struct Logo {
    width: usize,
    height: usize,
    data: Vec<u8>,
}

fn logo(options: LogoOptions) -> Result<Response<Vec<u8>>, http::Error> {
    let mut result = Vec::new();
    let logo = get_logo_data(options).expect("Could not get logo data"); // An array containing a RGBA sequence. First pixel is red and second pixel is black.

    {
        let mut encoder = png::Encoder::new(&mut result, logo.width as u32, logo.height as u32); // Width is 2 pixels and height is 1.
        encoder.set_color(png::ColorType::RGBA);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();

        writer.write_image_data(&logo.data).unwrap(); // Save
    }

    Response::builder().body(result)
}

#[derive(Debug, Deserialize)]
struct LogoResponse {
    logo: Vec<Vec<Vec<String>>>,
}

fn get_logo_data(options: LogoOptions) -> Result<Logo, Box<dyn Error>> {
    let coords = vec![
        vec![[0, 0], [0, 16], [0, 24], [0, 32]],
        vec![
            [8, 0],
            [8, 8],
            [16, 8],
            [8, 16],
            [8, 24],
            [16, 24],
            [24, 24],
        ],
        vec![
            [32, 8],
            [40, 8],
            [48, 8],
            [32, 16],
            [48, 16],
            [32, 24],
            [40, 24],
            [48, 24],
        ],
        vec![[56, 8], [64, 8], [72, 8], [56, 16], [56, 24]],
        vec![
            [88, 8],
            [96, 8],
            [80, 16],
            [96, 16],
            [80, 24],
            [88, 24],
            [96, 24],
        ],
        vec![
            [104, 0],
            [104, 8],
            [112, 8],
            [104, 16],
            [104, 24],
            [112, 24],
            [120, 24],
        ],
        vec![
            [128, 8],
            [136, 8],
            [144, 8],
            [128, 16],
            [144, 16],
            [128, 24],
            [136, 24],
        ],
    ];

    let pixel_size = options.size.unwrap_or(1) as usize;
    let width = 152 * pixel_size as usize;
    let height = 32 * pixel_size as usize;

    let mut image = vec![0; width * height * 4];

    let live_logo: LogoResponse = reqwest::get("https://logo-api.g2.iterate.no/logo")?.json()?;

    let now = Instant::now();

    for (char_index, chr) in live_logo.logo.into_iter().enumerate() {
        for (panel_index, panel) in chr.into_iter().enumerate() {
            for (pixel_index, pixel) in panel.into_iter().enumerate() {
                let panel_x = pixel_index % 8;
                let panel_y = pixel_index / 8;
                let x = coords[char_index][panel_index][0] + panel_x;
                let y = coords[char_index][panel_index][1] + panel_y;

                for extra_x in 0..pixel_size {
                    for extra_y in 0..pixel_size {
                        let x = extra_x + (x * pixel_size);
                        let y = extra_y + (y * pixel_size);

                        let r = u8::from_str_radix(&pixel[1..3], 16)?;
                        let g = u8::from_str_radix(&pixel[3..5], 16)?;
                        let b = u8::from_str_radix(&pixel[5..7], 16)?;

                        let image_idx = (x + y * width) * 4;

                        image[image_idx] = r;
                        image[image_idx + 1] = g;
                        image[image_idx + 2] = b;
                        image[image_idx + 3] = 255;
                    }
                }
            }
        }
    }

    let elapsed = now.elapsed();
    println!(
        "{}",
        elapsed.subsec_millis() as u64 + elapsed.as_secs() * 1000
    );

    Ok(Logo {
        width,
        height,
        data: image,
    })
}
