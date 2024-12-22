use dice_shared::prelude::*;
use once_cell::sync::Lazy;
use sqlite::{Connection, State};
use std::net::{TcpStream};
use std::sync::OnceLock;
use std::time::Duration;
use tnet::prelude::*;
use tnet::standard::listener::Listener;

pub mod games;
mod handles;

use crate::games::Games;
use crate::handles::{handle_in_game, handle_match_making};
use tlog::prelude::*;
use dice_shared::general::GameMode::OneVOneNormal;

#[macro_export]
macro_rules! new_1v1_game {
    () => {
        unsafe {
            let games: &mut Games = GAMES.get_mut().unwrap();
            games.new_1v1_game()
        }
    };
}

#[macro_export]
macro_rules! get_1v1_game {
    ($id:expr) => {
        unsafe {
            let games: &mut Games = GAMES.get_mut().unwrap();
            games.get_1v1_game($id)
        }
    };
}

static mut GAMES: OnceLock<Games> = OnceLock::new();
static mut LISTENER: Lazy<Listener<DiceSession, DicePacket>> = Lazy::new(|| {
    init_database().unwrap();
    unsafe {
        GAMES.set(Games::new()).unwrap();
    }
    text_styling_off();
    let port = 25560;
    let mut server = Listener::port_w_handler(port, Box::new(ok));

    server.allow_passthrough = false;
    server.set_auth_handler(Box::new(auth));

    info!("Dice", "Starting server on 127.0.0.1:{port}");
    info!("Dice-Games", "Creating empty game; ID: {}", new_1v1_game!());
    server
});

fn ok(session: &mut DiceSession, packet: DicePacket, stream: &mut TcpStream) {
    debug!("Handler", "Starting to do handling things");

    match packet.action {
        0 => {
            if session.in_match {
                handle_in_game(session, packet, stream);
                return;
            }
            handle_match_making(session, packet, stream);
        }
        _ => {
            warn_box!(
                "Undocumented Action",
                "Some action tried to pass your borders."
            );
        }
    }
}

#[macro_export]
macro_rules! send_resp {
    ($stream:expr, $packet:expr, $ses:expr, $ses_data:expr ) => {
        $stream
            .write(
                NetWrapperPacket::respond($packet.encode(), $ses, $ses_data.encode())
                    .encode()
                    .as_slice(),
            )
            .unwrap()
    };
}

fn main() {
    unsafe {
        LISTENER.listen();
    }
}

fn auth(user: &str, pass: &str) -> bool {
    let db = Connection::open("./dice.db").unwrap();
    let mut statement = db
        .prepare("SELECT COUNT(*) FROM users WHERE username = ? AND password = ?")
        .unwrap();

    statement.bind((1, user)).unwrap();
    statement.bind((2, pass)).unwrap();

    if let Ok(State::Row) = statement.next() {
        statement.read::<i64, _>(0).unwrap() > 0
    } else {
        false
    }
}

fn init_database() -> Result<(), Box<dyn std::error::Error>> {
    let conn = Connection::open("./dice.db")?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS users (
            username TEXT NOT NULL UNIQUE,
            password TEXT NOT NULL
        )",
    )?;

    Ok(())
}
