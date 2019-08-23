use std::collections::HashMap;

use futures::sync::mpsc;
use futures::{Future, Stream};
use lazy_static::lazy_static;
use parking_lot::RwLock;
use std::sync::atomic::{AtomicUsize, Ordering};
use warp::{
    self,
    ws::{Message, WebSocket},
};

type Listeners = RwLock<HashMap<usize, mpsc::UnboundedSender<Message>>>;

// Next id for use by a websocket listener
static NEXT_LISTENER_ID: AtomicUsize = AtomicUsize::new(1);
lazy_static! {
    // Channels for each of the websocket listeners
    static ref LISTENERS: Listeners = RwLock::new(HashMap::new());
}

pub fn send_update(logo_png: &Vec<u8>) {
    for tx in LISTENERS.read().values() {
        if let Err(err) = tx.unbounded_send(Message::binary(logo_png.clone())) {
            eprintln!("Error sending: {:?}", err);
        }
    }
}

pub fn listener_connected(ws: WebSocket) -> impl Future<Item = (), Error = ()> {
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
