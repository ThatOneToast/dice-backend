use dice_shared::packets::DicePacket;
use dice_shared::prelude::{DiceSession, GameMode};
use std::io::Write;
use std::net::{Shutdown, TcpStream};
use tnet::packet::{NetWrapperPacket, Packet};
use tnet::prelude::Session;

use crate::GAMES;
use dice_shared::prelude::*;
use tnet::prelude::tlog::prelude::*;

pub fn handle_in_game(session: &mut DiceSession, packet: DicePacket, stream: &mut TcpStream) {
    let game_id_res = session.game_id.clone();
    let game_id = match game_id_res {
        Some(game_id) => game_id,
        None => {
            warn_box!(
                format!("Session: {} - In match w/ No ID?", session.id).as_str(),
                "Here is the packet: {:?}",
                packet
            );
            stream.shutdown(Shutdown::Both).unwrap();
            return;
        }
    };

    warn!(
        "Game Packets Not Implemented",
        "No game Packets can be sent right now."
    );
}

pub fn handle_match_making(session: &mut DiceSession, packet: DicePacket, stream: &mut TcpStream) {
    if packet.action != 0 {
        warn_box!(
            format!("Session: {} - Valid Session w/ Invalid Action", session.id).as_str(),
            "This handle is only for matchmaking requests:\n{}",
            format!("Here is the packet: {:?}", packet).as_str()
        );
        stream.shutdown(Shutdown::Both).unwrap();
        return;
    }

    debug!("Handler Matchmaking", "Entered into the matchmaking state");

    // The user is trying to join match making state while
    // Currently being in a match.
    // That is a no no.
    if session.in_match {
        let def_net_packet = NetWrapperPacket::respond(
            DicePacket {
                action: 255,
                is_err: (
                    true,
                    Some(DiceError::JoinMatchMakingWhileInMatch(session.id.clone())),
                ),
                ..Default::default()
            }
            .encode(),
            session.id.clone(),
            session.encode(),
        );

        warn_box!(
            "Matchmaking",
            "Got a matchmaking packet but they are already in a match"
        );

        stream.write(def_net_packet.encode().as_slice()).unwrap();
        stream.shutdown(Shutdown::Both).unwrap();
        return;
    }

    let game_mode = packet.game_mode.clone();

    // So you want to join a game,
    // But you don't tell me what game to join?
    // Crazy man, you crazy dawg.
    if game_mode.is_none() {
        warn_box!(
            "Matchmaking",
            "Got a matchmaking packet but it didn't specify a GameMode to queue into."
        );
        stream
            .write(
                NetWrapperPacket::respond(
                    DicePacket {
                        action: 255,
                        is_err: (true, Some(DiceError::MatchmakingNoGamemode)),
                        ..Default::default()
                    }
                    .encode(),
                    session.id.clone(),
                    session.encode(),
                )
                .encode()
                .as_slice(),
            )
            .unwrap();
        stream.shutdown(Shutdown::Both).unwrap();
        return;
    }

    session.in_match_making = true;
    let gamemode = packet.game_mode.clone().unwrap();

    // Oh yeah give me that unsafe action
    // You gave me game mode,
    // ill give you game
    unsafe {
        match gamemode {
            GameMode::OneVOneNormal => {
                GAMES
                    .get_mut()
                    .unwrap()
                    .send_1v1_matchmaking(session.id.clone(), stream.try_clone().unwrap());

                stream
                    .write(
                        NetWrapperPacket::respond(
                            DicePacket {
                                action: 255,
                                is_err: (false, None),
                                ..Default::default()
                            }
                            .encode(),
                            session.id.clone(),
                            session.clone().encode(),
                        )
                        .encode()
                        .as_slice(),
                    )
                    .unwrap();
            }
        }
    }
}
