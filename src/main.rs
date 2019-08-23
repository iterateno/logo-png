use std::error::Error;
use std::mem;
use std::thread;
use std::time::Duration;

use futures::future::poll_fn;
use futures::sync::mpsc;
use futures::{Future, Stream};
use lazy_static::lazy_static;
use parking_lot::RwLock;
use png::{self};
use reqwest;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio_threadpool::blocking;
use warp::{
    self,
    http::{self, Response},
    path,
    ws::{Message, WebSocket},
    Filter,
};

type Listeners = RwLock<HashMap<usize, mpsc::UnboundedSender<Message>>>;

lazy_static! {
    // Last logo fetched from the api
    static ref LOGO_CACHE: RwLock<LogoResponse> = RwLock::new(LogoResponse { logo: vec![] });
    // Channels for each of the websocket listeners
    static ref LISTENERS: Listeners = RwLock::new(HashMap::new());
}

// Next id for use by a websocket listener
static NEXT_LISTENER_ID: AtomicUsize = AtomicUsize::new(1);

#[derive(Debug, Deserialize, Eq, PartialEq)]
struct LogoResponse {
    logo: Vec<Vec<Vec<String>>>,
}

fn main() {
    let logo_options = warp::query::<LogoOptions>();

    thread::spawn(|| loop {
        if let Err(err) = update_logo() {
            println!("Error updating logo: {}", err);
        }
        thread::sleep(Duration::from_secs(1));
    });

    // GET /hello/warp => 200 OK with body "Hello, warp!"
    let logo = path!("logo.png").and(logo_options).and_then(|options| {
        poll_fn(move || blocking(|| logo(options)).map_err(|err| warp::reject::custom(err)))
    });
    let index = path::end().and(warp::fs::file("src/index.html"));
    let health = path!("health").map(|| "OK");

    let live = warp::path("live")
        // The `ws2()` filter will prepare Websocket handshake...
        .and(warp::ws2())
        .map(|ws: warp::ws::Ws2| {
            // This will call our function if the handshake succeeds.
            ws.on_upgrade(move |socket| listener_connected(socket))
        });

    let routes = index.or(logo).or(health).or(live);

    warp::serve(routes).run(([0, 0, 0, 0], 3000));
}

fn update_logo() -> Result<(), Box<dyn Error>> {
    let live_logo: LogoResponse = reqwest::get("https://logo-api.g2.iterate.no/logo")?.json()?;
    let old_logo = LOGO_CACHE.read();

    if live_logo != *old_logo {
        // Avoid deadlock
        drop(old_logo);

        let mut logo_cache = LOGO_CACHE.write();
        mem::replace(&mut *logo_cache, live_logo);

        // Avoid deadlock
        drop(logo_cache);

        let logo_png = get_logo_png(LogoOptions::default()).expect("Could not get logo data");

        for tx in LISTENERS.read().values() {
            if let Err(err) = tx.unbounded_send(Message::binary(logo_png.clone())) {
                eprintln!("Error sending: {:?}", err);
            }
        }
    }

    Ok(())
}

fn listener_connected(ws: WebSocket) -> impl Future<Item = (), Error = ()> {
    // Use a counter to assign a new unique ID for this user.
    let my_id = NEXT_LISTENER_ID.fetch_add(1, Ordering::Relaxed);

    eprintln!("new listener: {}", my_id);

    // Split the socket into a sender and receive of messages.
    let (listener_ws_tx, listener_ws_rx) = ws.split();

    // Use an unbounded channel to handle buffering and flushing of messages
    // to the websocket...
    let (tx, rx) = mpsc::unbounded();
    warp::spawn(
        rx.map_err(|()| -> warp::Error { unreachable!("unbounded rx never errors") })
            .forward(listener_ws_tx)
            .map(|_tx_rx| ())
            .map_err(|ws_err| eprintln!("websocket send error: {}", ws_err)),
    );

    // Save the sender in our list of connected users.
    LISTENERS.write().insert(my_id, tx);

    // Return a `Future` that is basically a state machine managing
    // this specific user's connection.

    listener_ws_rx
        // Every time the user sends a message, broadcast it to
        // all other users...
        .for_each(move |msg| {
            println!("Got message from listener: {:?}", msg);
            Ok(())
        })
        // for_each will keep processing as long as the user stays
        // connected. Once they disconnect, then...
        .then(move |result| {
            eprintln!("good bye listener: {}", my_id);

            // Stream closed up, so remove from the user list
            LISTENERS.write().remove(&my_id);
            result
        })
        // If at any time, there was a websocket error, log here...
        .map_err(move |e| {
            eprintln!("websocket error(uid={}): {}", my_id, e);
        })
}

#[derive(Debug, Deserialize, Copy, Clone, Default)]
struct LogoOptions {
    size: Option<u32>,
}

struct Logo {
    width: usize,
    height: usize,
    data: Vec<u8>,
}

fn logo(options: LogoOptions) -> Result<Response<Vec<u8>>, http::Error> {
    let logo_png = get_logo_png(options).expect("Could not get logo data");
    Response::builder().body(logo_png)
}

fn get_logo_png(options: LogoOptions) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut result = Vec::new();
    let logo = get_logo_data(options)?; // An array containing a RGBA sequence. First pixel is red and second pixel is black.

    {
        let mut encoder = png::Encoder::new(&mut result, logo.width as u32, logo.height as u32); // Width is 2 pixels and height is 1.
        encoder.set_color(png::ColorType::RGBA);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();

        writer.write_image_data(&logo.data).unwrap(); // Save
    }

    Ok(result)
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

    let live_logo = LOGO_CACHE.read();

    for (char_index, chr) in live_logo.logo.iter().enumerate() {
        for (panel_index, panel) in chr.iter().enumerate() {
            for (pixel_index, pixel) in panel.iter().enumerate() {
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

    Ok(Logo {
        width,
        height,
        data: image,
    })
}
