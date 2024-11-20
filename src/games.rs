use crate::{get_1v1_game, new_1v1_game, send_resp, GAMES, LISTENER};
use dice_shared::packets::GameOptions;
use dice_shared::prelude::*;
use rand::Rng;
use std::collections::HashMap;
use std::io::Read;
use std::io::Write;
use std::net::TcpStream;
use std::sync::{Arc, Mutex, RwLock};
use tnet::packet::NetWrapperPacket;
use tnet::prelude::tlog::prelude::*;
use tnet::prelude::*;
use tnet::read_packet;

pub type SessionID = String;
pub type GameID = String;

#[derive(Debug)]
pub struct Games {
    pub active_one_v_ones: Arc<RwLock<HashMap<GameID, OneVOneArena>>>,
    one_v_one_matchmaking: Arc<RwLock<HashMap<SessionID, TcpStream>>>,
    thread_rng: rand::rngs::ThreadRng,
}

impl Games {
    pub fn new() -> Self {
        let selff = Self {
            active_one_v_ones: Arc::new(RwLock::new(HashMap::new())),
            one_v_one_matchmaking: Arc::new(RwLock::new(HashMap::new())),
            thread_rng: rand::thread_rng(),
        };

        selff.matchmaking_1v1_loop();
        selff
    }

    /// Returns (player1, player2) Session IDs
    ///
    /// Returns None if there isn't enough players to start a game,
    pub fn return_next_1v1_pair(
        &mut self,
    ) -> Option<((SessionID, TcpStream), (SessionID, TcpStream))> {
        let mut one_v_one_matchmaking = self.one_v_one_matchmaking.write().unwrap();
        if one_v_one_matchmaking.len() < 2 {
            return None;
        }

        let p1 = one_v_one_matchmaking.keys().next().cloned()?;
        let p2 = one_v_one_matchmaking.keys().nth(1).cloned()?;

        let p1_stream = one_v_one_matchmaking.remove(&p1).unwrap();
        let p2_stream = one_v_one_matchmaking.remove(&p2).unwrap();

        Some(((p1.to_owned(), p1_stream), (p2.to_owned(), p2_stream)))
    }

    pub fn matchmaking_1v1_loop(&self) {
        let arena_1v1s_matchmaking = self.one_v_one_matchmaking.clone();

        std::thread::spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(5));

                // Acquire a write lock on the matchmaking map
                let mut one_v_one_matchmaking = arena_1v1s_matchmaking.write().unwrap();

                while one_v_one_matchmaking.len() >= 2 {
                    // Get the first two keys and their associated streams
                    let p1 = one_v_one_matchmaking.keys().next().cloned();
                    let p2 = one_v_one_matchmaking.keys().nth(1).cloned();

                    if let (Some(p1), Some(p2)) = (p1, p2) {
                        // Remove the matched players from the queue
                        let p1_stream = one_v_one_matchmaking.remove(&p1).unwrap();
                        let p2_stream = one_v_one_matchmaking.remove(&p2).unwrap();

                        // Handle the matched pair (e.g., spawn a game session)
                        success_box!(
                            "Matchmaking Matched!",
                            "Matched: Player 1: {:?},\nPlayer 2: {:?}",
                            p1,
                            p2
                        );

                        // Optionally start the 1v1 session in another thread
                        // Here you can pass the pair into your session handler
                        std::thread::spawn(move || unsafe {
                            // Example: Process the match
                            handle_1v1_match(p1, p1_stream, p2, p2_stream);
                        });
                    }
                }
            }
        });
    }

    pub fn send_1v1_matchmaking(&mut self, client_session_id: SessionID, stream: TcpStream) {
        self.one_v_one_matchmaking
            .write()
            .unwrap()
            .insert(client_session_id, stream);
    }

    pub fn remove_1v1_matchmaking(&mut self, client_session_id: SessionID) {
        self.one_v_one_matchmaking
            .write()
            .unwrap()
            .remove(&client_session_id);
    }

    pub fn new_1v1_game(&mut self) -> String {
        let id = self.random_game_id();

        let arena = OneVOneArena::new();
        self.active_one_v_ones
            .write()
            .unwrap()
            .insert(id.clone(), arena);

        id
    }

    pub fn get_1v1_game(&mut self, id: String) -> Option<OneVOneArena> {
        self.active_one_v_ones.write().unwrap().get(&id).cloned()
    }

    fn random_game_id(&mut self) -> String {
        const CHARSET: &[u8] = b"0123456789";
        (0..16)
            .map(|_| {
                let idx = self.thread_rng.gen_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect()
    }
}

// Example session handler
unsafe fn handle_1v1_match(
    p1_id: SessionID,
    p1_stream: TcpStream,
    p2_id: SessionID,
    p2_stream: TcpStream,
) {
    let global_sessions_clone = LISTENER.sessions.clone();
    let mut global_sessions = global_sessions_clone.write().unwrap();
    success_box!(
        "Starting 1v1 Match!",
        "{}",
        format!("Starting 1v1 match between:\n {:?} and {:?}", p1_id, p2_id).as_str()
    );

    let mut p1_stream = p1_stream;
    let mut p2_stream = p2_stream;

    let arena_id = new_1v1_game!();
    let mut arena = get_1v1_game!(arena_id).unwrap();

    let found_match_packet = DicePacket {
        action: 0,
        game_options: Some(GameOptions {
            receive_arena_state: (true, Some((GameMode::OneVOneNormal, arena.encode()))),
            ..Default::default()
        }),
        ..Default::default()
    };

    {
        let p1_ses_data = global_sessions.get_mut(&p1_id).unwrap();
        p1_ses_data.in_match_making = false;
        p1_ses_data.in_match = true;

        info!("Found Match!", "Sending the arena to player1");
        send_resp!(
            p1_stream,
            found_match_packet.clone(),
            p1_ses_data.id.clone(),
            p1_ses_data.clone()
        );
    }

    {
        let p2_ses_data = global_sessions.get_mut(&p2_id).unwrap();
        p2_ses_data.in_match_making = false;
        p2_ses_data.in_match = true;

        info!("Found Match!", "Sending the arena to player2");
        send_resp!(
            p2_stream,
            found_match_packet.clone(),
            p2_ses_data.id.clone(),
            p2_ses_data.clone()
        );
    }

    let p1_play: DicePacket = read_packet!(p1_stream, DicePacket).unwrap();
    debug_box!("Read_Packet P1", "Got {:?}", p1_play);

    let p2_player: DicePacket = read_packet!(p2_stream, DicePacket).unwrap();
    debug_box!("Read_Packet P2", "Got {:?}", p2_player);
}
