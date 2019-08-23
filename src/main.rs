use std::error::Error;
use std::thread;
use std::time::Duration;

use dotenv::dotenv;
use futures::future::poll_fn;
use tokio_threadpool::blocking;
use warp::{
    self,
    http::{self, Response},
    path, Filter,
};

mod db;
mod live;
mod logo;

fn main() -> Result<(), Box<dyn Error>> {
    dotenv().ok();

    db::init_db()?;

    let logo_options = warp::query::<logo::LogoOptions>();

    thread::spawn(|| loop {
        if let Err(err) = logo::update_logo() {
            println!("Error updating logo: {}", err);
        }
        thread::sleep(Duration::from_secs(1));
    });

    // GET /logo.png
    let logo = path!("logo.png").and(logo_options).and_then(|options| {
        poll_fn(move || blocking(|| logo(options)).map_err(|err| warp::reject::custom(err)))
    });
    // GET /
    let index = path::end().and(warp::fs::file("src/index.html"));
    // GET /history
    let history = path!("history").and(warp::fs::file("src/history.html"));
    // GET /health
    let health = path!("health").map(|| "OK");
    // GET /live (websocket)
    let live = warp::path("live")
        // The `ws2()` filter will prepare Websocket handshake...
        .and(warp::ws2())
        .map(|ws: warp::ws::Ws2| {
            // This will call our function if the handshake succeeds.
            ws.on_upgrade(move |socket| live::listener_connected(socket))
        });
    // GET /api/v1/history
    let history_api = path!("api" / "v1" / "history").and_then(|| {
        poll_fn(move || {
            blocking(|| warp::reply::json(&db::get_history().expect("Could not get history")))
                .map_err(|err| warp::reject::custom(err))
        })
    });

    let routes = index
        .or(logo)
        .or(health)
        .or(live)
        .or(history)
        .or(history_api);

    warp::serve(routes).run(([0, 0, 0, 0], 3000));

    Ok(())
}

fn logo(options: logo::LogoOptions) -> Result<Response<Vec<u8>>, http::Error> {
    let logo_png = logo::get_logo_png(options).expect("Could not get logo data");
    Response::builder().body(logo_png)
}
