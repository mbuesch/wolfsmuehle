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
    TcpStream,
    ToSocketAddrs,
};
use crate::player::{
    PlayerMode,
    player_mode_to_num,
};
use crate::print::Print;
use crate::net::protocol::{
    MSG_BUFFER_SIZE,
    Message,
    MsgJoin,
    MsgLeave,
    MsgMove,
    MsgNop,
    MsgPing,
    MsgReqGameState,
    MsgReqPlayerList,
    MsgReqRoomList,
    MsgReset,
    MsgSay,
    MsgType,
    buffer_skip,
    message_from_bytes,
    net_sync,
};
use std::time::Instant;

const DEBUG_RAW: bool = false;

pub struct Client {
    stream:         TcpStream,
    sequence:       u32,
    rx_queue:       Option<Vec<u8>>,
    sync:           bool,
}

impl Client {
    pub fn new(addr: impl ToSocketAddrs) -> ah::Result<Client> {
        let stream = TcpStream::connect(addr)?;
        stream.set_nonblocking(true)?;
        stream.set_nodelay(true)?;
        Ok(Client {
            stream,
            sequence:       0,
            rx_queue:       None,
            sync:           false,
        })
    }

    /// Send a data blob to the server.
    fn send(&mut self, data: &[u8]) -> ah::Result<()> {
        if DEBUG_RAW {
            Print::debug(&format!("Client TX: {:?}", data));
        }
        self.stream.write(data)?;
        Ok(())
    }

    /// Send a message to the server.
    pub fn send_msg(&mut self, msg: &mut impl Message) -> ah::Result<()> {
        msg.get_header_mut().set_sequence(self.sequence);
        self.send(&msg.to_bytes())?;
        self.sequence = self.sequence.wrapping_add(1);
        Ok(())
    }

    /// Wait for a reply from the server.
    fn wait_for_reply<F>(&mut self,
                         name: &str,
                         timeout: f32,
                         check_match: F) -> ah::Result<()>
        where F: Fn(&Box<dyn Message>) -> Option<ah::Result<()>>
    {
        let begin = Instant::now();
        let timeout_ms = (timeout * 1000.0).ceil() as u128;

        let mut backlog = vec![];
        let mut ret = Err(ah::format_err!("Timeout waiting for {} reply.", name));
        let mut exit = false;

        while Instant::now().duration_since(begin).as_millis() < timeout_ms &&
              !exit {
            if let Some(messages) = self.poll() {
                for msg in messages {
                    match check_match(&msg) {
                        Some(r) => {
                            // We got it!
                            ret = r;
                            exit = true;
                            break;
                        },
                        None => {
                            backlog.append(&mut msg.to_bytes());
                        },
                    }
                }
            }
        }
        Print::debug(&format!("net/client: Wait blocked {} ms.",
                              Instant::now().duration_since(begin).as_millis()));

        if !backlog.is_empty() {
            if let Some(mut q) = self.rx_queue.take() {
                backlog.append(&mut q);
            }
            self.rx_queue = Some(backlog);
        }
        ret
    }

    pub fn send_msg_wait_for_ok(&mut self,
                                name: &str,
                                timeout: f32,
                                msg: &mut impl Message) -> ah::Result<()> {
        self.send_msg(msg)?;
        self.wait_for_reply(name, timeout,
            |m| {
                match m.get_message() {
                    MsgType::Result(result) => {
                        if result.is_in_reply_to(msg) {
                            if result.is_ok() {
                                Some(Ok(()))
                            } else {
                                Some(Err(ah::format_err!("Server replied not-Ok ({}): {}.",
                                                         result.get_result_code(),
                                                         result.get_text())))
                            }
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            }
        )?;
        Ok(())
    }

    /// Send a NOP message to the server.
    pub fn send_nop(&mut self) -> ah::Result<()> {
        self.send_msg(&mut MsgNop::new())?;
        Ok(())
    }

    /// Send a ping message to the server and wait for the pong response.
    pub fn send_ping(&mut self) -> ah::Result<()> {
        self.send_msg(&mut MsgPing::new())?;
        self.wait_for_reply("ping", 3.0,
            |m| {
                match m.get_message() {
                    MsgType::Pong(_) => Some(Ok(())),
                    _ => None,
                }
            }
        )?;
        Ok(())
    }

    /// Send a Join message to the server and wait for the result.
    pub fn send_join(&mut self,
                     room_name: &str,
                     player_name: &str,
                     player_mode: PlayerMode) -> ah::Result<()> {
        self.send_msg_wait_for_ok(
            "join",
            3.0,
            &mut MsgJoin::new(room_name,
                              player_name,
                              player_mode_to_num(player_mode))?)?;
        Ok(())
    }

    /// Send a Leave message to the server and wait for the result.
    pub fn send_leave(&mut self) -> ah::Result<()> {
        self.send_msg_wait_for_ok("leave", 1.0, &mut MsgLeave::new())?;
        Ok(())
    }

    /// Send a Reset message to the server and wait for the result.
    pub fn send_reset(&mut self) -> ah::Result<()> {
        self.send_msg_wait_for_ok("reset", 3.0, &mut MsgReset::new())?;
        Ok(())
    }

    /// Send a RequestGameState message to the server.
    pub fn send_request_gamestate(&mut self) -> ah::Result<()> {
        self.send_msg(&mut MsgReqGameState::new())?;
        Ok(())
    }

    /// Send a RequestPlayerList message to the server.
    pub fn send_request_playerlist(&mut self) -> ah::Result<()> {
        self.send_msg(&mut MsgReqPlayerList::new())?;
        Ok(())
    }

    /// Send a RequestRoomList message to the server.
    pub fn send_request_roomlist(&mut self) -> ah::Result<()> {
        self.send_msg(&mut MsgReqRoomList::new())?;
        Ok(())
    }

    /// Send a chat message to the server.
    pub fn send_chat_message(&mut self, text: &str) -> ah::Result<()> {
        self.send_msg_wait_for_ok("say", 1.0,
                                  &mut MsgSay::new("", text)?)?;
        Ok(())
    }

    /// Send a MoveToken message to the server and wait for the result.
    pub fn send_move_token(&mut self,
                           action: u32,
                           token: u32,
                           coord_x: u32,
                           coord_y: u32) -> ah::Result<()> {
        self.send_msg_wait_for_ok(
            "move",
            1.0,
            &mut MsgMove::new(action, token, coord_x, coord_y))?;
        Ok(())
    }

    /// Poll the received messages.
    pub fn poll(&mut self) -> Option<Vec<Box<dyn Message>>> {
        let mut rx_queue = match self.rx_queue.take() {
            Some(q) => q,
            None => Vec::with_capacity(MSG_BUFFER_SIZE),
        };

        {
            let data_len = rx_queue.len();
            rx_queue.resize(data_len + MSG_BUFFER_SIZE, 0);

            // Read data from the network.
            match self.stream.read(&mut rx_queue[data_len..]) {
                Ok(len) => {
                    rx_queue.truncate(data_len + len);
                    if DEBUG_RAW {
                        Print::debug(&format!("Client RX: {:?}", &rx_queue[data_len..]));
                    }
                },
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    rx_queue.truncate(data_len);
                },
                Err(e) => {
                    rx_queue.truncate(data_len);
                    Print::error(&format!("Receive error: {}", e));
                },
            }

            // Try to sync to the data stream, if necessary.
            if !self.sync {
                if !rx_queue.is_empty() {
                    Print::debug("net/client: Trying to synchronize to data stream...");
                }
                match net_sync(&rx_queue) {
                    Some(skip) => {
                        rx_queue = buffer_skip(rx_queue, skip);
                        self.sync = true;
                        Print::debug("net/client: Synchronized to data stream.");
                    },
                    None => {
                        self.sync = false;
                        rx_queue.clear();
                    },
                }
            }
        }

        // Parse all received messages.
        let mut messages: Vec<Box<dyn Message>> = vec![];
        loop {
            match message_from_bytes(&rx_queue) {
                Ok((len, Some(message))) => {
                    messages.push(message);
                    rx_queue = buffer_skip(rx_queue, len);
                },
                Ok((_len, None)) => {
                    // Not enough data for this message, yet.
                    break;
                },
                Err(e) => {
                    Print::error(&format!("Received invalid message: {}", e));
                    self.sync = false;
                    rx_queue.clear();
                    break;
                },
            }
        }

        // Put all left over bytes to the queue.
        self.rx_queue = Some(rx_queue);

        if messages.is_empty() { None } else { Some(messages) }
    }

    /// Disconnect from the server.
    pub fn disconnect(mut self) {
        self.send_leave().ok();
        self.stream.shutdown(Shutdown::Both).ok();
    }
}

// vim: ts=4 sw=4 expandtab
