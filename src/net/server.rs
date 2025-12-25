// -*- coding: utf-8 -*-
//
// Copyright 2021 Michael Buesch <m@bues.ch>
//
// SPDX-License-Identifier: MIT OR Apache-2.0
//

mod multicast;
mod room;

use crate::net::{
    consts::MAX_ROOMS,
    protocol::{
        MSG_BUFFER_SIZE, MSG_RESULT_NOK, MSG_RESULT_OK, Message, MsgPlayerList, MsgPong, MsgRecord,
        MsgResult, MsgRoomList, MsgType, buffer_skip, message_from_bytes, net_sync,
    },
    server::{
        multicast::{MulticastPacket, MulticastRouter, MulticastSubscriber, MulticastSync},
        room::ServerRoom,
    },
};
use crate::player::{PlayerMode, num_to_player_mode, player_mode_to_num};
use crate::print::Print;
use anyhow as ah;
use itertools::Itertools;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const DEBUG_RAW: bool = false;

type ServerRoomMap = HashMap<String, ServerRoom>;

/// Server instance thread corresponding to one connected client.
struct ServerInstance<'a> {
    stream: &'a mut TcpStream,
    mc_sub: MulticastSubscriber,
    sequence: u32,
    peer_addr: SocketAddr,
    rooms: Arc<Mutex<ServerRoomMap>>,
    joined_room: Option<String>,
    player_name: Option<String>,
    player_mode: PlayerMode,
}

macro_rules! do_leave {
    ($self:expr, $rooms:expr) => {
        if let Some(player_name) = $self.player_name.take() {
            if let Some(room_name) = $self.joined_room.take() {
                match $rooms.get_mut(&room_name) {
                    Some(mut room) => {
                        room.remove_player(&player_name);
                        if let Err(e) = $self.broadcast_player_list(&mut room, false) {
                            Print::error(&format!("Failed to broadcast player list: {}", e));
                        }
                    }
                    None => (),
                }
                Print::info(&format!(
                    "{} / '{}' / '{}' has left the room '{}'",
                    $self.peer_addr, player_name, $self.player_mode, room_name
                ));
            }
            $self.player_mode = PlayerMode::Spectator;
        }
    };
}

impl<'a> ServerInstance<'a> {
    fn new(
        stream: &'a mut TcpStream,
        mc_sub: MulticastSubscriber,
        rooms: Arc<Mutex<ServerRoomMap>>,
    ) -> ah::Result<ServerInstance<'a>> {
        let peer_addr = stream.peer_addr()?;

        stream.set_nodelay(true)?;
        stream.set_read_timeout(Some(Duration::from_millis(10)))?;
        stream.set_write_timeout(Some(Duration::from_millis(5000)))?;

        let mut self_ = ServerInstance {
            stream,
            mc_sub,
            sequence: 0,
            peer_addr,
            rooms,
            joined_room: None,
            player_name: None,
            player_mode: PlayerMode::Spectator,
        };

        for reply in &mut self_.gen_room_list_msgs()? {
            self_.send_msg(reply)?;
        }

        Ok(self_)
    }

    fn send(&mut self, data: &[u8]) -> ah::Result<()> {
        if DEBUG_RAW {
            Print::debug(&format!("Server TX: {:?}", data));
        }
        self.stream.write_all(data)?;
        Ok(())
    }

    fn send_msg(&mut self, msg: &mut impl Message) -> ah::Result<()> {
        msg.get_header_mut().set_sequence(self.sequence);
        self.send(&msg.to_bytes())?;
        self.sequence = self.sequence.wrapping_add(1);
        Ok(())
    }

    fn send_broadcast(
        &self,
        msg: &impl Message,
        room: Option<&ServerRoom>,
        include_self: bool,
        sync: MulticastSync,
    ) {
        self.mc_sub.send_broadcast(MulticastPacket {
            data: msg.to_bytes(),
            meta_data: if let Some(room) = room {
                room.get_name().as_bytes().to_vec()
            } else {
                vec![]
            },
            include_self,
            sync,
        });
    }

    fn broadcast_game_state(&self, room: &mut ServerRoom) {
        let game_state = room.get_game_state(self.player_mode).make_state_message();
        self.send_broadcast(&game_state, Some(room), true, MulticastSync::NoSync);
    }

    fn broadcast_player_list(&self, room: &mut ServerRoom, include_self: bool) -> ah::Result<()> {
        let messages = self.gen_player_list_msgs(room)?;
        for msg in messages {
            self.send_broadcast(
                &msg,
                Some(room),
                include_self,
                if include_self {
                    MulticastSync::NoSync
                } else {
                    MulticastSync::ToRouter
                },
            );
        }
        Ok(())
    }

    fn gen_player_list_msgs(&self, room: &ServerRoom) -> ah::Result<Vec<MsgPlayerList>> {
        let mut messages = vec![];
        let player_list = room.get_player_list_ref();
        for (index, player) in player_list.iter().sorted().enumerate() {
            let msg = MsgPlayerList::new(
                player_list.count() as u32,
                index as u32,
                &player.name,
                player_mode_to_num(player.mode),
            )?;
            messages.push(msg);
        }
        Ok(messages)
    }

    fn gen_room_list_msgs(&self) -> ah::Result<Vec<MsgRoomList>> {
        let mut room_names = vec![];
        {
            let rooms = self.rooms.lock().unwrap();
            for (_room_name, room) in rooms.iter().sorted() {
                room_names.push(room.get_name().to_string());
            }
        }
        let mut messages = vec![];
        for (i, room_name) in room_names.iter().enumerate() {
            messages.push(MsgRoomList::new(
                room_names.len() as u32,
                i as u32,
                room_name,
            )?);
        }
        Ok(messages)
    }

    fn handle_rx_room_message(&mut self, msg_type: &mut MsgType) -> ah::Result<()> {
        let mut rooms = self.rooms.lock().unwrap();

        let room = if let Some(room_name) = self.joined_room.as_ref() {
            rooms.get_mut(room_name)
        } else {
            return Err(ah::format_err!("Not in a room."));
        };
        let room = if let Some(room) = room {
            room
        } else {
            return Err(ah::format_err!(
                "Room '{}' not found.",
                self.joined_room.as_ref().unwrap()
            ));
        };

        match msg_type {
            MsgType::Reset(msg) => {
                room.get_game_state(self.player_mode).reset_game(false);
                self.broadcast_game_state(room);
                drop(rooms);
                self.send_msg(&mut MsgResult::new(*msg, MSG_RESULT_OK, "")?)?;
            }
            MsgType::ReqGameState(_msg) => {
                let mut game_state = room.get_game_state(self.player_mode).make_state_message();
                drop(rooms);
                self.send_msg(&mut game_state)?;
            }
            MsgType::GameState(msg) => {
                let err = match room
                    .get_game_state(self.player_mode)
                    .read_state_message(msg, false)
                {
                    Ok(_) => None,
                    Err(e) => Some(format!("{}", e)),
                };
                self.broadcast_game_state(room);
                drop(rooms);
                if let Some(e) = err {
                    self.send_msg(&mut MsgResult::new(*msg, MSG_RESULT_NOK, &e)?)?;
                } else {
                    self.send_msg(&mut MsgResult::new(*msg, MSG_RESULT_OK, "")?)?;
                }
            }
            MsgType::ReqPlayerList(_msg) => {
                let mut replies = self.gen_player_list_msgs(room)?;
                drop(rooms);
                for reply in &mut replies {
                    self.send_msg(reply)?;
                }
            }
            MsgType::PlayerList(msg) => {
                drop(rooms);
                self.send_msg(&mut MsgResult::new(
                    *msg,
                    MSG_RESULT_NOK,
                    "MsgPlayerList not supported.",
                )?)?;
            }
            MsgType::ReqRecord(msg) => {
                let record = room
                    .get_game_state(self.player_mode)
                    .get_recorder()
                    .get_moves_as_text();
                drop(rooms);
                let mut replies = MsgRecord::new(&record);
                for reply in &mut replies {
                    self.send_msg(reply)?;
                }
                self.send_msg(&mut MsgResult::new(*msg, MSG_RESULT_OK, "")?)?;
            }
            MsgType::Record(msg) => {
                drop(rooms);
                self.send_msg(&mut MsgResult::new(
                    *msg,
                    MSG_RESULT_NOK,
                    "MsgRecord not supported.",
                )?)?;
            }
            MsgType::Move(msg) => {
                match room
                    .get_game_state(self.player_mode)
                    .server_handle_rx_msg_move(msg)
                {
                    Ok(_) => {
                        self.broadcast_game_state(room);
                        drop(rooms);
                        self.send_msg(&mut MsgResult::new(*msg, MSG_RESULT_OK, "")?)?;
                    }
                    Err(e) => {
                        drop(rooms);
                        let text = format!("token move error: {}", e);
                        self.send_msg(&mut MsgResult::new(*msg, MSG_RESULT_NOK, &text)?)?;
                        return Err(ah::format_err!("{}", text));
                    }
                }
            }
            MsgType::Say(msg) => {
                let mut msg = msg.clone();

                // Override the player name. It might be forged.
                if let Some(player_name) = self.player_name.as_ref() {
                    msg.set_player_name(player_name)?;
                } else {
                    msg.set_player_name("")?;
                }

                // Forward the message to all other connected clients.
                self.send_broadcast(&msg, Some(room), true, MulticastSync::NoSync);

                drop(rooms);
                self.send_msg(&mut MsgResult::new(&msg, MSG_RESULT_OK, "")?)?;
            }
            _ => {
                return Err(ah::format_err!(
                    "handle_rx_room_message: Received invalid message."
                ));
            }
        }
        Ok(())
    }

    fn do_join(
        &mut self,
        room_name: &str,
        player_name: &str,
        player_mode: PlayerMode,
    ) -> ah::Result<()> {
        let mut rooms = self.rooms.lock().unwrap();

        // Check if join is possible.
        match rooms.get(room_name) {
            Some(room) => {
                let mut ignore_player = None;
                if let Some(joined_room) = self.joined_room.as_ref()
                    && joined_room == room_name
                {
                    // We're about to re-join this room with a different
                    // name or mode. Ignore the old name during checks.
                    if let Some(old_player_name) = self.player_name.as_ref() {
                        ignore_player = Some(&old_player_name[..]);
                    }
                }

                match room.can_add_player(player_name, player_mode, ignore_player) {
                    Ok(_) => (),
                    Err(e) => {
                        // Room already has a player by that name,
                        // or the mode is in conflict.
                        return Err(e);
                    }
                }
            }
            None => {
                return Err(ah::format_err!("join: Room '{}' not found.", room_name));
            }
        }

        // Remove old player, if this player already joined a room.
        do_leave!(self, rooms);

        // Join the new room.
        match rooms.get_mut(room_name) {
            Some(room) => {
                // Add player to room.
                match room.add_player(player_name, player_mode) {
                    Ok(_) => {
                        self.player_mode = player_mode;
                        self.player_name = Some(player_name.to_string());
                        self.joined_room = Some(room.get_name().to_string());
                        Print::info(&format!(
                            "{} / '{}' / '{}' has joined the room '{}'",
                            self.peer_addr,
                            player_name,
                            self.player_mode,
                            room.get_name()
                        ));
                        if let Err(e) = self.broadcast_player_list(room, true) {
                            Print::error(&format!("Failed to broadcast player list: {}", e));
                        }
                    }
                    Err(e) => {
                        // Adding player to room failed.
                        // This should actually never happen,
                        // because it should be caught by the check above.
                        return Err(e);
                    }
                }
            }
            None => {
                return Err(ah::format_err!("join: Room '{}' not found.", room_name));
            }
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
            MsgType::Nop(_) | MsgType::Pong(_) | MsgType::Result(_) => {
                // Nothing to do.
            }
            MsgType::Ping(_msg) => {
                self.send_msg(&mut MsgPong::new())?;
            }
            MsgType::Join(msg) => {
                let result;
                if let Ok(room_name) = msg.get_room_name() {
                    if let Ok(player_name) = msg.get_player_name() {
                        if let Ok(player_mode) = num_to_player_mode(msg.get_player_mode()) {
                            result = self.do_join(&room_name, &player_name, player_mode);
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
                    }
                    Err(e) => {
                        let text = format!("Join failed: {}", e);
                        self.send_msg(&mut MsgResult::new(msg, MSG_RESULT_NOK, &text)?)?;
                        return Err(ah::format_err!("{}", text));
                    }
                }
            }
            MsgType::Leave(msg) => {
                self.do_leave();
                self.send_msg(&mut MsgResult::new(msg, MSG_RESULT_OK, "")?)?;
            }
            MsgType::ReqRoomList(_msg) => {
                for reply in &mut self.gen_room_list_msgs()? {
                    self.send_msg(reply)?;
                }
            }
            MsgType::RoomList(msg) => {
                self.send_msg(&mut MsgResult::new(
                    msg,
                    MSG_RESULT_NOK,
                    "Cannot change room list.",
                )?)?;
            }
            MsgType::Reset(_)
            | MsgType::ReqGameState(_)
            | MsgType::GameState(_)
            | MsgType::ReqPlayerList(_)
            | MsgType::PlayerList(_)
            | MsgType::ReqRecord(_)
            | MsgType::Record(_)
            | MsgType::Move(_)
            | MsgType::Say(_) => {
                self.handle_rx_room_message(&mut msg_type)?;
            }
        }
        Ok(())
    }

    /// Handle received data.
    fn handle_rx_data(&mut self, data: &[u8]) -> ah::Result<Option<usize>> {
        if DEBUG_RAW {
            Print::debug(&format!("Server RX: {:?}", data));
        }
        match message_from_bytes(data) {
            Ok((msg_len, Some(msg))) => {
                let message = msg.get_message();
                match self.handle_rx_message(message) {
                    Ok(()) => (),
                    Err(e) => {
                        Print::error(&format!("Failed to handle received message: {}", e));
                        // We don't forward this error to our caller.
                    }
                }
                Ok(Some(msg_len))
            }
            Ok((_msg_len, None)) => {
                // Not enough data for this message, yet.
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }

    fn handle_rx_multicast_message(&mut self, msg_type: MsgType) -> ah::Result<()> {
        macro_rules! forward {
            ($msg:expr) => {
                self.send_msg(&mut $msg.clone())
            };
        }

        macro_rules! forward_if_joined_room {
            ($msg:expr) => {{
                if self.joined_room.is_some() {
                    forward!($msg)?;
                }
                Ok(())
            }};
        }

        match msg_type {
            MsgType::Say(msg) => forward_if_joined_room!(msg),
            MsgType::GameState(msg) => forward_if_joined_room!(msg),
            MsgType::PlayerList(msg) => forward!(msg),
            MsgType::RoomList(msg) => forward!(msg),
            other => Err(ah::format_err!(
                "Received unexpected multicast: {:?}",
                other
            )),
        }
    }

    fn handle_rx_multicast_data(&mut self, pack: &MulticastPacket) -> ah::Result<()> {
        if let Some(joined_room) = self.joined_room.as_ref() {
            if pack.meta_data != joined_room.as_bytes() {
                // We're not in the destination room. Discard it.
                return Ok(());
            }
        } else if !pack.meta_data.is_empty() {
            // We're not in the destination room. Discard it.
            return Ok(());
        }

        if DEBUG_RAW {
            Print::debug(&format!("Multicast RX: {:?}", pack.data));
        }
        match message_from_bytes(&pack.data) {
            Ok((_msg_len, Some(msg))) => {
                let message = msg.get_message();
                self.handle_rx_multicast_message(message)?;
                Ok(())
            }
            Ok((_msg_len, None)) => Err(ah::format_err!("Multicast: Received incomplete message.")),
            Err(e) => Err(e),
        }
    }

    /// Main server loop.
    fn run_loop(&mut self) {
        Print::info(&format!("Client connected: {}", self.peer_addr));

        let mut sync = false;
        let mut buffer = Vec::with_capacity(MSG_BUFFER_SIZE);

        loop {
            let mut tail_len = buffer.len();
            if tail_len >= MSG_BUFFER_SIZE {
                Print::error("Tail buffer overrun.");
                buffer.clear();
                tail_len = 0;
                sync = false;
            }

            // Calculate next RX length.
            let read_len = MSG_BUFFER_SIZE - buffer.len();
            buffer.resize(tail_len + read_len, 0);

            // Try to receive more data.
            assert!(read_len > 0);
            match self
                .stream
                .read(&mut buffer[tail_len..(tail_len + read_len)])
            {
                Ok(actual_len) => {
                    if actual_len == 0 {
                        Print::info(&format!("Client disconnected: {}", self.peer_addr));
                        break;
                    }
                    buffer.truncate(tail_len + actual_len);

                    // Synchronize to the data stream.
                    if !sync {
                        match net_sync(&buffer) {
                            Some(skip_len) => {
                                // Success. Skip the garbage bytes.
                                buffer = buffer_skip(buffer, skip_len);
                            }
                            None => {
                                // No sync. Discard everything.
                                buffer.clear();
                            }
                        }
                    }

                    // Process all received data.
                    while !buffer.is_empty() {
                        match self.handle_rx_data(&buffer) {
                            Ok(Some(consumed_len)) => {
                                buffer = buffer_skip(buffer, consumed_len);
                                sync = true;
                            }
                            Ok(None) => {
                                // Not enough data, yet.
                                break;
                            }
                            Err(e) => {
                                Print::error(&format!("Server message error: {}", e));
                                sync = false;
                                buffer.clear();
                                break;
                            }
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    buffer.truncate(tail_len);
                }
                Err(e) => {
                    Print::error(&format!("Server thread error: {}", e));
                    break;
                }
            }

            // Check if we received a multicast from other instances.
            while let Some(pack) = self.mc_sub.receive() {
                if let Err(e) = self.handle_rx_multicast_data(&pack) {
                    Print::error(&format!("Server multicast error: {}", e));
                }
            }
        }
        self.do_leave();
    }
}

pub struct Server {
    listener: TcpListener,
    max_conns: usize,
    restrict_player_modes: bool,
    active_conns: Arc<AtomicUsize>,
    rooms: Arc<Mutex<ServerRoomMap>>,
}

impl Server {
    pub fn new(
        addr: impl ToSocketAddrs,
        max_conns: u16,
        restrict_player_modes: bool,
    ) -> ah::Result<Server> {
        let listener = TcpListener::bind(addr)?;
        listener.set_nonblocking(true)?;

        Ok(Server {
            listener,
            max_conns: max_conns as usize,
            restrict_player_modes,
            active_conns: Arc::new(AtomicUsize::new(0)),
            rooms: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn run(&mut self, room_names: &Vec<String>) -> ah::Result<()> {
        {
            if room_names.len() > MAX_ROOMS {
                return Err(ah::format_err!(
                    "Maximum number of rooms ({}) exceeded.",
                    MAX_ROOMS
                ));
            }
            let mut rooms = self.rooms.lock().unwrap();
            rooms.clear();
            for name in room_names {
                Print::info(&format!("Opening room: {}", name));
                let room = ServerRoom::new(name.to_string(), self.restrict_player_modes)?;
                rooms.insert(name.to_string(), room);
            }
        }

        let mut mc_router = MulticastRouter::new();

        for stream in self.listener.incoming() {
            match stream {
                Ok(mut stream) => {
                    if self.active_conns.fetch_add(1, Ordering::Acquire) < self.max_conns {
                        let mc_sub = mc_router.new_subscriber();
                        let thread_rooms = Arc::clone(&self.rooms);
                        let thread_active_conns = Arc::clone(&self.active_conns);
                        thread::spawn(move || {
                            match ServerInstance::new(&mut stream, mc_sub, thread_rooms) {
                                Ok(mut instance) => {
                                    instance.run_loop();
                                    drop(instance);
                                    Print::debug("Server thread exiting.");
                                }
                                Err(e) => {
                                    Print::error(&format!(
                                        "Could not construct server instance: {}",
                                        e
                                    ));
                                }
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
                        Print::error(&format!(
                            "Rejected connection from '{}': Too many connections.",
                            peer_addr
                        ));
                        self.active_conns.fetch_sub(1, Ordering::Release);
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // Nothing to do.
                }
                Err(e) => {
                    return Err(ah::format_err!("Connection failed: {}", e));
                }
            }

            mc_router.run_router();

            thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    }
}

// vim: ts=4 sw=4 expandtab
