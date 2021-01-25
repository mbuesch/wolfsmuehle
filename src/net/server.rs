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
    player_mode_to_num,
};
use crate::net::{
    consts::{
        MAX_PLAYERS,
        MAX_ROOMS,
    },
    protocol::{
        MSG_BUFFER_SIZE,
        MSG_RESULT_NOK,
        MSG_RESULT_OK,
        Message,
        MsgPlayerList,
        MsgPong,
        MsgResult,
        MsgRoomList,
        MsgType,
        buffer_skip,
        message_from_bytes,
        net_sync,
    },
};

const DEBUG_RAW: bool = false;

/// Server instance thread corresponding to one connected client.
struct ServerInstance<'a> {
    stream:         &'a mut TcpStream,
    sequence:       u32,
    peer_addr:      SocketAddr,
    rooms:          Arc<Mutex<Vec<ServerRoom>>>,
    joined_room:    Option<String>,
    player_name:    Option<String>,
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

macro_rules! do_leave {
    ($self:expr, $rooms:expr) => {
        if let Some(player_name) = $self.player_name.take() {
            if let Some(room_name) = $self.joined_room.take() {
                match find_room(&mut $rooms, &room_name) {
                    Some(room) => room.remove_player(&player_name),
                    None => (),
                }
                println!("{} / '{}' / '{}' has left the room '{}'",
                         $self.peer_addr,
                         player_name,
                         $self.player_mode,
                         room_name);
            }
            $self.player_mode = PlayerMode::Spectator;
        }
    }
}

impl<'a> ServerInstance<'a> {
    fn new(stream: &'a mut TcpStream,
           rooms: Arc<Mutex<Vec<ServerRoom>>>) -> ah::Result<ServerInstance<'a>> {
        let peer_addr = stream.peer_addr()?;
        stream.set_nodelay(true)?;
        Ok(ServerInstance {
            stream,
            sequence:       0,
            peer_addr,
            rooms,
            joined_room:    None,
            player_name:    None,
            player_mode:    PlayerMode::Spectator,
        })
    }

    fn send(&mut self, data: &[u8]) -> ah::Result<()> {
        if DEBUG_RAW {
            println!("Server TX: {:?}", data);
        }
        self.stream.write(data)?;
        Ok(())
    }

    fn send_msg(&mut self, msg: &mut impl Message) -> ah::Result<()> {
        msg.get_header_mut().set_sequence(self.sequence);
        self.send(&msg.to_bytes())?;
        self.sequence = self.sequence.wrapping_add(1);
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
            MsgType::Reset(msg) => {
                room.get_game_state(self.player_mode).reset_game(false);
                drop(rooms);
                self.send_msg(&mut MsgResult::new(*msg, MSG_RESULT_OK, "")?)?;
            },
            MsgType::ReqGameState(_msg) => {
                let mut game_state = room.get_game_state(self.player_mode).make_state_message();
                drop(rooms);
                self.send_msg(&mut game_state)?;
            },
            MsgType::GameState(msg) => {
                let err = match room.get_game_state(self.player_mode).read_state_message(msg, false) {
                    Ok(_) => None,
                    Err(e) => Some(format!("{}", e)),
                };
                drop(rooms);
                if let Some(e) = err {
                    self.send_msg(&mut MsgResult::new(*msg, MSG_RESULT_NOK, &e)?)?;
                } else {
                    self.send_msg(&mut MsgResult::new(*msg, MSG_RESULT_OK, "")?)?;
                }
            },
            MsgType::ReqPlayerList(_msg) => {
                let mut replies = vec![];
                let player_list = room.get_player_list_ref();
                for (index, player) in player_list.iter().enumerate() {
                    replies.push(MsgPlayerList::new(
                        player_list.count() as u32,
                        index as u32,
                        &player.name,
                        player_mode_to_num(player.mode))?);
                }
                drop(rooms);
                for reply in &mut replies {
                    self.send_msg(reply)?;
                }
            },
            MsgType::PlayerList(msg) => {
                drop(rooms);
                self.send_msg(&mut MsgResult::new(*msg, MSG_RESULT_NOK,
                                                  "MsgPlayerList not supported.")?)?;
            },
            MsgType::Move(msg) => {
                match room.get_game_state(self.player_mode).server_handle_rx_msg_move(&msg) {
                    Ok(_) => {
                        drop(rooms);
                        self.send_msg(&mut MsgResult::new(*msg, MSG_RESULT_OK, "")?)?;
                    },
                    Err(e) => {
                        drop(rooms);
                        let text = format!("token move error: {}", e);
                        self.send_msg(&mut MsgResult::new(*msg, MSG_RESULT_NOK, &text)?)?;
                        return Err(ah::format_err!("{}", text));
                    },
                }
            },
            _ => {
                return Err(ah::format_err!("handle_rx_room_message: Received invalid message."));
            }
        }
        Ok(())
    }

    fn do_join(&mut self,
               room_name: &str,
               player_name: &str,
               player_mode: PlayerMode) -> ah::Result<()> {
        let mut rooms = self.rooms.lock().unwrap();

        // Check if join is possible.
        match find_room(&mut rooms, &room_name) {
            Some(room) => {
                let mut ignore_player = None;
                if let Some(joined_room) = self.joined_room.as_ref() {
                    if joined_room == room_name {
                        // We're about to re-join this room with a different
                        // name or mode. Ignore the old name during checks.
                        if let Some(old_player_name) = self.player_name.as_ref() {
                            ignore_player = Some(&old_player_name[..]);
                        }
                    }
                }

                match room.can_add_player(player_name, player_mode, ignore_player) {
                    Ok(_) => (),
                    Err(e) => {
                        // Room already has a player by that name,
                        // or the mode is in conflict.
                        return Err(e);
                    },
                }
            }
            None => {
                return Err(ah::format_err!("join: Room '{}' not found.",
                                           room_name));
            },
        }

        // Remove old player, if this player already joined a room.
        do_leave!(self, rooms);

        // Join the new room.
        match find_room(&mut rooms, &room_name) {
            Some(room) => {
                // Add player to room.
                match room.add_player(player_name, player_mode) {
                    Ok(_) => {
                        self.player_mode = player_mode;
                        self.player_name = Some(player_name.to_string());
                        self.joined_room = Some(room.get_name().to_string());
                        println!("{} / '{}' / '{}' has joined the room '{}'",
                                 self.peer_addr,
                                 player_name,
                                 self.player_mode,
                                 room.get_name());
                    },
                    Err(e) => {
                        // Adding player to room failed.
                        // This should actually never happen,
                        // because it should be caught by the check above.
                        return Err(e);
                    }
                }
            }
            None => {
                return Err(ah::format_err!("join: Room '{}' not found.",
                                           room_name));
            },
        }
        Ok(())
    }

    fn do_leave(&mut self) {
        let mut rooms = self.rooms.lock().unwrap();
        do_leave!(self, rooms);
    }

    /// Handle received message.
    fn handle_rx_message(&mut self, mut msg_type: MsgType) -> ah::Result<()> {
        match msg_type {
            MsgType::Nop(_) |
            MsgType::Pong(_) |
            MsgType::Result(_) => {
                // Nothing to do.
            },
            MsgType::Ping(_msg) => {
                self.send_msg(&mut MsgPong::new())?;
            },
            MsgType::Join(msg) => {
                let result;
                if let Ok(room_name) = msg.get_room_name() {
                    if let Ok(player_name) = msg.get_player_name() {
                        if let Ok(player_mode) = num_to_player_mode(msg.get_player_mode()) {
                            result = self.do_join(&room_name,
                                                  &player_name,
                                                  player_mode);
                        } else {
                            result = Err(ah::format_err!("Received invalid player mode."));
                        }
                    } else {
                        result = Err(ah::format_err!("Received invalid player name."));
                    }
                } else {
                    result = Err(ah::format_err!("Received invalid room name."));
                }
                match result {
                    Ok(_) => {
                        self.send_msg(&mut MsgResult::new(msg, MSG_RESULT_OK, "")?)?;
                    },
                    Err(e) => {
                        let text = format!("Join failed: {}", e);
                        self.send_msg(&mut MsgResult::new(msg, MSG_RESULT_NOK, &text)?)?;
                        return Err(ah::format_err!("{}", text));
                    },
                }
            },
            MsgType::Leave(msg) => {
                self.do_leave();
                self.send_msg(&mut MsgResult::new(msg, MSG_RESULT_OK, "")?)?;
            },
            MsgType::ReqRoomList(_msg) => {
                let mut room_names = vec![];
                {
                    let rooms = self.rooms.lock().unwrap();
                    for room in rooms.iter() {
                        room_names.push(room.get_name().to_string());
                    }
                }
                for (i, room_name) in room_names.iter().enumerate() {
                    self.send_msg(&mut MsgRoomList::new(room_names.len() as u32,
                                                        i as u32,
                                                        &room_name)?)?;
                }
            },
            MsgType::RoomList(msg) => {
                self.send_msg(&mut MsgResult::new(msg, MSG_RESULT_NOK,
                                                  "Cannot change room list.")?)?;
            },
            MsgType::Reset(_) |
            MsgType::ReqGameState(_) |
            MsgType::GameState(_) |
            MsgType::ReqPlayerList(_) |
            MsgType::PlayerList(_) |
            MsgType::Move(_) => {
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
        self.do_leave();
    }
}

struct ServerRoom {
    name:                   String,
    game_state:             GameState,
    player_list:            PlayerList,
    restrict_player_modes:  bool,
}

impl ServerRoom {
    fn new(name: String,
           restrict_player_modes: bool) -> ah::Result<ServerRoom> {
        let mut game_state = GameState::new(PlayerMode::Both,
                                            None)?; /* no player name */
        let player_list = PlayerList::new(vec![]);
        game_state.set_room_player_list(player_list.clone());
        Ok(ServerRoom {
            name,
            game_state,
            player_list,
            restrict_player_modes,
        })
    }

    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_game_state(&mut self, player_mode: PlayerMode) -> &mut GameState {
        self.game_state.set_player_mode(player_mode)
            .expect("game_state.set_player_mode failed.");
        &mut self.game_state
    }

    fn can_add_player(&self,
                      player_name: &str,
                      player_mode: PlayerMode,
                      ignore_name: Option<&str>) -> ah::Result<()> {
        let mut player_list = self.player_list.clone();
        if let Some(ignore_name) = ignore_name {
            player_list.remove_player_by_name(ignore_name);
        }

        if self.restrict_player_modes {
            match player_mode {
                PlayerMode::Spectator => (),
                PlayerMode::Both => {
                    return Err(ah::format_err!("PlayerMode::Both not supported in restricted mode."));
                },
                PlayerMode::Wolf => {
                    if !player_list.find_players_by_mode(PlayerMode::Wolf).is_empty() {
                        return Err(ah::format_err!("The game already has a Wolf player."));
                    }
                },
                PlayerMode::Sheep => {
                    if !player_list.find_players_by_mode(PlayerMode::Sheep).is_empty() {
                        return Err(ah::format_err!("The game already has a Sheep player."));
                    }
                },
            }
        }

        if player_list.count() < MAX_PLAYERS {
            if player_list.find_player_by_name(player_name).is_some() {
                Err(ah::format_err!("Player name '{}' is already occupied.",
                                    player_name))
            } else {
                Ok(())
            }
        } else {
            Err(ah::format_err!("Player '{}' can't join. Too many players in room.",
                                player_name))
        }
    }

    fn add_player(&mut self,
                  player_name: &str,
                  player_mode: PlayerMode) -> ah::Result<()> {

        self.can_add_player(player_name, player_mode, None)?;

        self.player_list.add_player(Player::new(player_name.to_string(),
                                                player_mode,
                                                false));
        self.game_state.set_room_player_list(self.player_list.clone());
        Ok(())
    }

    fn remove_player(&mut self, player_name: &str) {
        self.player_list.remove_player_by_name(player_name);
        self.game_state.set_room_player_list(self.player_list.clone());
    }

    fn get_player_list_ref(&self) -> &PlayerList {
        &self.player_list
    }
}

pub struct Server {
    listener:               TcpListener,
    max_conns:              usize,
    restrict_player_modes:  bool,
    active_conns:           Arc<AtomicUsize>,
    rooms:                  Arc<Mutex<Vec<ServerRoom>>>
}

impl Server {
    pub fn new(addr: impl ToSocketAddrs,
               max_conns: u16,
               restrict_player_modes: bool) -> ah::Result<Server> {
        let listener = TcpListener::bind(addr)?;
        Ok(Server {
            listener,
            max_conns:      max_conns as usize,
            restrict_player_modes,
            active_conns:   Arc::new(AtomicUsize::new(0)),
            rooms:          Arc::new(Mutex::new(vec![])),
        })
    }

    pub fn run(&mut self, room_names: &Vec<String>) -> ah::Result<()> {
        {
            if room_names.len() > MAX_ROOMS {
                return Err(ah::format_err!("Maximum number of rooms ({}) exceeded.",
                                           MAX_ROOMS));
            }
            let mut rooms = self.rooms.lock().unwrap();
            rooms.clear();
            for name in room_names {
                println!("Opening room: {}", name);
                let room = ServerRoom::new(name.to_string(),
                                           self.restrict_player_modes)?;
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
                                    drop(instance);
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
