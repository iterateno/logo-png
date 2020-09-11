#![recursion_limit = "256"]

use std::error::Error;
use std::thread;
use std::time::Duration;

use dotenv::dotenv;
use futures::future::poll_fn;
use tokio_threadpool::blocking;
use warp::{
    self,
    http::{self, Response},
    path, reply, Filter,
};

mod db;
mod live;
mod logo;

fn main() -> Result<(), Box<dyn Error>> {
    dotenv().ok();

    db::init_db()?;

    let logo_options = warp::query::<logo::LogoOptions>();
    let get_history_options = warp::query::<db::GetHistoryOptions>();

    thread::spawn(|| loop {
        if let Err(err) = logo::update_logo() {
            println!("Error updating logo: {}", err);
        }
        thread::sleep(Duration::from_secs(1));
    });

    // Note: Warp also applies cors-filter on websockets
    let cors = warp::cors()
        .allow_origin("http://localhost:8000")
        .allow_methods(vec!["GET"]);

    // GET /logo.png
    let logo = path!("logo.png").and(logo_options).and_then(|options| {
        poll_fn(move || blocking(|| logo_route(options)).map_err(|err| warp::reject::custom(err)))
    });
    // GET /
    let index = path::end().and(warp::fs::file("src/index.html"));
    // GET /history
    let history = path!("history").and(warp::fs::file("history-frontend/history.html"));
    // GET /history/elm.js
    let history_elm = path!("history.js").and(warp::fs::file("history-frontend/history.js"));
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
    let history_api = path!("api" / "v1" / "history")
        .and(get_history_options)
        .and_then(|options| {
            poll_fn(move || {
                blocking(|| db::get_history(options).expect("Could not get history"))
                    .map_err(|err| warp::reject::custom(err))
            })
        });
    let history_api_by_date =
        path!("api" / "v1" / "history" / String).and_then(|entry_date: String| {
            poll_fn(move || {
                blocking(|| {
                    db::get_history_from_date(entry_date.clone())
                        .expect("Could not get history at index")
                })
                .map_err(|err| warp::reject::custom(err))
            })
        });
    let history_api_index = path!("api" / "v1" / "history" / "index").and_then(|| {
        poll_fn(move || {
            blocking(|| db::get_history_index().expect("Could not get history index"))
                .map_err(|err| warp::reject::custom(err))
        })
    });

    let routes = index
        .or(logo)
        .or(health)
        .or(live)
        .or(history_api_index)
        .or(history_api_by_date)
        .or(history_api.with(cors).boxed())
        .or(history)
        .or(history_elm);

    let main = routes;

    warp::serve(main).run(([0, 0, 0, 0], 3000));

    Ok(())
}

fn logo_route(options: logo::LogoOptions) -> Result<reply::Response, http::Error> {
    let logo_png = match logo::get_logo_png(options) {
        Ok(logo) => logo,
        Err(err) => {
            eprintln!("Error generating png: {}", err);
            include_bytes!("error.png").to_vec()
        }
    };
    Ok(Response::builder().body(logo_png.into())?)
}

// This function receives a `Rejection` and tries to return a custom
// value, othewise simply passes the rejection along.
// fn customize_error(err: Rejection) -> Result<String, http::Error> {
//     Ok(err.to_string())
// }
