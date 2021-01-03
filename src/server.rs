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
use std::thread;
use crate::game_state::GameState;
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

/// Server instance thread corresponding to one connected client.
struct ServerInstance {
    stream:         TcpStream,
    peer_addr:      SocketAddr,
    rooms:          Arc<Mutex<Vec<ServerRoom>>>,
    joined_room:    Option<String>,
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

impl ServerInstance {
    fn new(stream: TcpStream,
           rooms: Arc<Mutex<Vec<ServerRoom>>>) -> ah::Result<ServerInstance> {
        let peer_addr = stream.peer_addr()?;
        stream.set_nodelay(true)?;
        Ok(ServerInstance {
            stream,
            peer_addr,
            rooms,
            joined_room: None,
        })
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
            MsgType::MsgTypeReset(_msg) => {
                room.get_game_state_mut().reset_game();
            },
            MsgType::MsgTypeReqGameState(_msg) => {
                self.stream.write(&room.get_game_state().make_state_message().to_bytes())?;
            },
            MsgType::MsgTypeGameState(msg) => {
                //TODO
                self.stream.write(&MsgResult::new(*msg, MSG_RESULT_OK)?.to_bytes())?;
            },
            MsgType::MsgTypeMove(msg) => {
                match room.get_game_state_mut().server_handle_rx_msg_move(&msg) {
                    Ok(_) => {
                        self.stream.write(&MsgResult::new(*msg, MSG_RESULT_OK)?.to_bytes())?;
                    },
                    Err(e) => {
                        self.stream.write(&MsgResult::new(*msg, MSG_RESULT_NOK)?.to_bytes())?;
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
                self.stream.write(&MsgPong::new().to_bytes())?;
            },
            MsgType::MsgTypeJoin(msg) => {
                if self.joined_room.is_none() {
                    if let Ok(room_name) = msg.get_room_name() {
                        let mut rooms = self.rooms.lock().unwrap();
                        self.joined_room = match find_room(&mut rooms, &room_name) {
                            Some(r) => {
                                self.stream.write(&MsgResult::new(msg, MSG_RESULT_OK)?.to_bytes())?;
                                println!("{} joined '{}'", self.peer_addr, r.get_name());
                                Some(r.get_name().to_string())
                            }
                            None => {
                                self.stream.write(&MsgResult::new(msg, MSG_RESULT_NOK)?.to_bytes())?;
                                return Err(ah::format_err!("join: Room '{}' not found.", room_name));
                            },
                        }
                    } else {
                        self.stream.write(&MsgResult::new(msg, MSG_RESULT_NOK)?.to_bytes())?;
                        return Err(ah::format_err!("join: Received invalid room name."));
                    }
                } else {
                    self.stream.write(&MsgResult::new(msg, MSG_RESULT_NOK)?.to_bytes())?;
                    return Err(ah::format_err!("join: Already in room."));
                }
            },
            MsgType::MsgTypeLeave(msg) => {
                self.joined_room = None;
                self.stream.write(&MsgResult::new(msg, MSG_RESULT_OK)?.to_bytes())?;
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
                    self.stream.shutdown(Shutdown::Both).ok();
                    break;
                },
            }
        }
    }
}

struct ServerRoom {
    name:           String,
    game_state:     GameState,
}

impl ServerRoom {
    fn new(name: String) -> ah::Result<ServerRoom> {
        let game_state = GameState::new(None, name.to_string())?;
        Ok(ServerRoom {
            name,
            game_state,
        })
    }

    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_game_state(&self) -> &GameState {
        &self.game_state
    }

    fn get_game_state_mut(&mut self) -> &mut GameState {
        &mut self.game_state
    }
}

pub struct Server {
    listener:   TcpListener,
    rooms:      Arc<Mutex<Vec<ServerRoom>>>
}

impl Server {
    pub fn new(addr: impl ToSocketAddrs) -> ah::Result<Server> {
        let listener = TcpListener::bind(addr)?;
        Ok(Server {
            listener,
            rooms:      Arc::new(Mutex::new(vec![])),
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
                Ok(stream) => {
                    let thread_rooms = Arc::clone(&self.rooms);
                    thread::spawn(move || {
                        match ServerInstance::new(stream, thread_rooms) {
                            Ok(mut instance) => {
                                instance.run_loop();
                                println!("Server thread exiting.");
                            },
                            Err(e) => {
                                eprintln!("Could not construct server instance: {}", e);
                            },
                        };
                    });
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
