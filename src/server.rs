// -*- coding: utf-8 -*-
//
// Copyright 2021 Michael Buesch <m@bues.ch>
//
// This program is free software; you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation; either version 2 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with this program; if not, write to the Free Software Foundation, Inc.,
// 51 Franklin Street, Fifth Floor, Boston, MA 02110-1301 USA.
//

use anyhow as ah;
use std::io::{
    Read,
    Write,
};
use std::net::{
    Shutdown,
    SocketAddr,
    TcpListener,
    TcpStream,
    ToSocketAddrs,
};
use std::sync::{Mutex, MutexGuard, Arc};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use crate::game_state::GameState;
use crate::player::{
    Player,
    PlayerList,
    PlayerMode,
    num_to_player_mode,
};
use crate::protocol::{
    MSG_BUFFER_SIZE,
    MSG_RESULT_OK,
    MSG_RESULT_NOK,
    Message,
    MsgPong,
    MsgResult,
    MsgType,
    buffer_skip,
    message_from_bytes,
    net_sync,
};

const DEBUG_RAW: bool = false;

/// Server instance thread corresponding to one connected client.
struct ServerInstance<'a> {
    stream:         &'a mut TcpStream,
    peer_addr:      SocketAddr,
    rooms:          Arc<Mutex<Vec<ServerRoom>>>,
    joined_room:    Option<String>,
    player_mode:    PlayerMode,
}

fn find_room<'a>(rooms: &'a mut MutexGuard<'_, Vec<ServerRoom>>,
                 name: &'_ str)
                 -> Option<&'a mut ServerRoom> {
    for room in &mut **rooms {
        if room.get_name() == name {
            return Some(room);
        }
    }
    None
}

impl<'a> ServerInstance<'a> {
    fn new(stream: &'a mut TcpStream,
           rooms: Arc<Mutex<Vec<ServerRoom>>>) -> ah::Result<ServerInstance<'a>> {
        let peer_addr = stream.peer_addr()?;
        stream.set_nodelay(true)?;
        Ok(ServerInstance {
            stream,
            peer_addr,
            rooms,
            joined_room: None,
            player_mode: PlayerMode::Spectator,
        })
    }

    fn send(&mut self, data: &[u8]) -> ah::Result<()> {
        if DEBUG_RAW {
            println!("Server TX: {:?}", data);
        }
        self.stream.write(data)?;
        Ok(())
    }

    fn handle_rx_room_message(&mut self,
                              msg_type: &mut MsgType) -> ah::Result<()> {
        let mut rooms = self.rooms.lock().unwrap();

        let room = if let Some(room_name) = self.joined_room.as_ref() {
            find_room(&mut rooms, &room_name)
        } else {
            return Err(ah::format_err!("Not in a room."));
        };
        let room = if let Some(room) = room {
            room
        } else {
            return Err(ah::format_err!("Room '{}' not found.",
                                       self.joined_room.as_ref().unwrap()))
        };

        match msg_type {
            MsgType::MsgTypeReset(msg) => {
                room.get_game_state(self.player_mode).reset_game(false);
                drop(rooms);
                self.send(&MsgResult::new(*msg, MSG_RESULT_OK)?.to_bytes())?;
            },
            MsgType::MsgTypeReqGameState(_msg) => {
                let game_state = room.get_game_state(self.player_mode).make_state_message();
                drop(rooms);
                self.send(&game_state.to_bytes())?;
            },
            MsgType::MsgTypeGameState(msg) => {
                //TODO
                drop(rooms);
                self.send(&MsgResult::new(*msg, MSG_RESULT_OK)?.to_bytes())?;
            },
            MsgType::MsgTypeMove(msg) => {
                match room.get_game_state(self.player_mode).server_handle_rx_msg_move(&msg) {
                    Ok(_) => {
                        drop(rooms);
                        self.send(&MsgResult::new(*msg, MSG_RESULT_OK)?.to_bytes())?;
                    },
                    Err(e) => {
                        drop(rooms);
                        self.send(&MsgResult::new(*msg, MSG_RESULT_NOK)?.to_bytes())?;
                        return Err(ah::format_err!("token move error: {}", e));
                    },
                }
            },
            _ => {
                return Err(ah::format_err!("handle_rx_room_message: Received invalid message."));
            }
        }
        Ok(())
    }

    /// Handle received message.
    fn handle_rx_message(&mut self, mut msg_type: MsgType) -> ah::Result<()> {
        match msg_type {
            MsgType::MsgTypeNop(_) |
            MsgType::MsgTypePong(_) |
            MsgType::MsgTypeResult(_) => {
                // Nothing to do.
            },
            MsgType::MsgTypePing(_msg) => {
                self.send(&MsgPong::new().to_bytes())?;
            },
            MsgType::MsgTypeJoin(msg) => {
                let mut result = Ok(());
                if self.joined_room.is_none() {
                    if let Ok(room_name) = msg.get_room_name() {
                        let mut rooms = self.rooms.lock().unwrap();
                        match find_room(&mut rooms, &room_name) {
                            Some(room) => {
                                self.player_mode = num_to_player_mode(msg.get_player_mode())?;
                                //TODO restrict player modes.
                                self.joined_room = Some(room.get_name().to_string());
                                println!("{} joined '{}'",
                                         self.peer_addr, room.get_name());
                            }
                            None => {
                                result = Err(ah::format_err!("join: Room '{}' not found.",
                                                             room_name));
                            },
                        }
                    } else {
                        result = Err(ah::format_err!("join: Received invalid room name."));
                    }
                } else {
                    result = Err(ah::format_err!("join: Already in room."));
                }
                match result {
                    Ok(_) => {
                        self.send(&MsgResult::new(msg, MSG_RESULT_OK)?.to_bytes())?;
                    },
                    Err(e) => {
                        self.send(&MsgResult::new(msg, MSG_RESULT_NOK)?.to_bytes())?;
                        return Err(e);
                    },
                }
            },
            MsgType::MsgTypeLeave(msg) => {
                self.joined_room = None;
                self.player_mode = PlayerMode::Spectator;
                self.send(&MsgResult::new(msg, MSG_RESULT_OK)?.to_bytes())?;
                println!("{} left", self.peer_addr);
            },
            MsgType::MsgTypeReset(_) |
            MsgType::MsgTypeReqGameState(_) |
            MsgType::MsgTypeGameState(_) |
            MsgType::MsgTypeMove(_) => {
                self.handle_rx_room_message(&mut msg_type)?;
            },
        }
        Ok(())
    }

    /// Handle received data.
    fn handle_rx_data(&mut self, data: &[u8]) -> ah::Result<Option<usize>> {
        if DEBUG_RAW {
            println!("Server RX: {:?}", data);
        }
        match message_from_bytes(data) {
            Ok((msg_len, Some(msg))) => {
                let message = msg.get_message();
                match self.handle_rx_message(message) {
                    Ok(()) => (),
                    Err(e) => {
                        eprintln!("Failed to handle received message: {}", e);
                        // We don't forward this error to our caller.
                    },
                }
                Ok(Some(msg_len))
            },
            Ok((_msg_len, None)) => {
                // Not enough data for this message, yet.
                Ok(None)
            },
            Err(e) => {
                Err(e)
            },
        }
    }

    /// Main server loop.
    fn run_loop(&mut self) {
        println!("Client connected: {}", self.peer_addr);

        let mut sync = false;
        let mut buffer = Vec::with_capacity(MSG_BUFFER_SIZE);

        loop {
            let mut tail_len = buffer.len();
            if tail_len >= MSG_BUFFER_SIZE {
                eprintln!("Tail buffer overrun.");
                buffer.clear();
                tail_len = 0;
                sync = false;
            }

            // Calculate next RX length.
            let read_len = MSG_BUFFER_SIZE - buffer.len();
            buffer.resize(tail_len + read_len, 0);

            // Try to receive more data.
            assert!(read_len > 0);
            match self.stream.read(&mut buffer[tail_len..(tail_len + read_len)]) {
                Ok(actual_len) => {
                    if actual_len == 0 {
                        println!("Client disconnected: {}", self.peer_addr);
                        break;
                    }
                    buffer.truncate(tail_len + actual_len);

                    // Synchronize to the data stream.
                    if !sync {
                        match net_sync(&buffer) {
                            Some(skip_len) => {
                                // Success. Skip the garbage bytes.
                                buffer = buffer_skip(buffer, skip_len);
                            },
                            None => {
                                // No sync. Discard everything.
                                buffer.clear();
                            },
                        }
                    }

                    // Process all received data.
                    while buffer.len() > 0 {
                        match self.handle_rx_data(&buffer) {
                            Ok(Some(consumed_len)) => {
                                buffer = buffer_skip(buffer, consumed_len);
                                sync = true;
                            },
                            Ok(None) => {
                                // Not enough data, yet.
                                break;
                            },
                            Err(e) => {
                                eprintln!("Server message error: {}", e);
                                sync = false;
                                buffer.clear();
                                break;
                            },
                        }
                    }
                },
                Err(e) => {
                    eprintln!("Server thread error: {}", e);
                    break;
                },
            }
        }
    }
}

struct ServerRoom {
    name:           String,
    game_state:     GameState,
    player_list:    PlayerList,
}

impl ServerRoom {
    fn new(name: String) -> ah::Result<ServerRoom> {
        let game_state = GameState::new(PlayerMode::Both,
                                        None, /* no player name */
                                        None, /* no connect */
                                        name.to_string())?;
        Ok(ServerRoom {
            name,
            game_state,
            player_list:    PlayerList::new(vec![]),
        })
    }

    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_game_state(&mut self, player_mode: PlayerMode) -> &mut GameState {
        self.game_state.set_player_mode(player_mode);
        &mut self.game_state
    }
}

pub struct Server {
    listener:       TcpListener,
    max_conns:      usize,
    active_conns:   Arc<AtomicUsize>,
    rooms:          Arc<Mutex<Vec<ServerRoom>>>
}

impl Server {
    pub fn new(addr: impl ToSocketAddrs,
               max_conns: u16) -> ah::Result<Server> {
        let listener = TcpListener::bind(addr)?;
        Ok(Server {
            listener,
            max_conns:      max_conns as usize,
            active_conns:   Arc::new(AtomicUsize::new(0)),
            rooms:          Arc::new(Mutex::new(vec![])),
        })
    }

    pub fn run(&mut self, room_names: &Vec<String>) -> ah::Result<()> {
        {
            let mut rooms = self.rooms.lock().unwrap();
            rooms.clear();
            for name in room_names {
                println!("Opening room: {}", name);
                let room = ServerRoom::new(name.to_string())?;
                rooms.push(room);
            }
        }

        for stream in self.listener.incoming() {
            match stream {
                Ok(mut stream) => {
                    if self.active_conns.fetch_add(1, Ordering::Acquire) < self.max_conns {

                        let thread_rooms = Arc::clone(&self.rooms);
                        let thread_active_conns = Arc::clone(&self.active_conns);
                        thread::spawn(move || {
                            match ServerInstance::new(&mut stream, thread_rooms) {
                                Ok(mut instance) => {
                                    instance.run_loop();
                                    println!("Server thread exiting.");
                                },
                                Err(e) => {
                                    eprintln!("Could not construct server instance: {}", e);
                                },
                            };
                            stream.shutdown(Shutdown::Both).ok();
                            thread_active_conns.fetch_sub(1, Ordering::Release);
                        });

                    } else {
                        let peer_addr = match stream.peer_addr() {
                            Ok(peer_addr) => peer_addr.to_string(),
                            Err(_) => "unknown".to_string(),
                        };
                        stream.shutdown(Shutdown::Both).ok();
                        eprintln!("Rejected connection from '{}': Too many connections.",
                                  peer_addr);
                        self.active_conns.fetch_sub(1, Ordering::Release);
                    }
                },
                Err(e) => {
                    return Err(ah::format_err!("Connection failed: {}", e));
                },
            }
        }
        Ok(())
    }
}

// vim: ts=4 sw=4 expandtab
