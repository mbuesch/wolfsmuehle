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
use crate::protocol::{
    MSG_BUFFER_SIZE,
    Message,
    MsgJoin,
    MsgLeave,
    MsgMove,
    MsgNop,
    MsgPing,
    MsgReqGameState,
    MsgReset,
    MsgType,
    buffer_skip,
    message_from_bytes,
    net_sync,
};
use std::time::Instant;

pub struct Client {
    stream:         TcpStream,
    tail_buffer:    Option<Vec<u8>>,
    sync:           bool,
}

impl Client {
    pub fn new(addr: impl ToSocketAddrs) -> ah::Result<Client> {
        let stream = TcpStream::connect(addr)?;
        stream.set_nonblocking(true)?;
        stream.set_nodelay(true)?;
        Ok(Client {
            stream,
            tail_buffer:    None,
            sync:           false,
        })
    }

    fn wait_for_reply<F>(&mut self, name: &str, check_match: F) -> ah::Result<()>
        where F: Fn(Box<dyn Message>) -> bool
    {
        let begin = Instant::now();
        while Instant::now().duration_since(begin).as_millis() < 3000 {
            match self.poll() {
                Some(messages) => {
                    for m in messages {
                        if check_match(m) {
                            return Ok(());
                        }
                    }
                },
                None => (),
            }
        }
        Err(ah::format_err!("Timeout waiting for {} reply.", name))
    }

    /// Send a NOP message to the server.
    pub fn send_nop(&mut self) -> ah::Result<()> {
        self.stream.write(&MsgNop::new().to_bytes())?;
        Ok(())
    }

    /// Send a ping message to the server and wait for the pong response.
    pub fn send_ping(&mut self) -> ah::Result<()> {
        self.stream.write(&MsgPing::new().to_bytes())?;
        self.wait_for_reply("ping", |m| { match m.get_message() {
            MsgType::MsgTypePong(_) => true,
            _ => false,
        }})?;
        Ok(())
    }

    pub fn send_join(&mut self, room_name: &str) -> ah::Result<()> {
        let join = MsgJoin::new(room_name)?;
        self.stream.write(&join.to_bytes())?;
        self.wait_for_reply("join", |m| { match m.get_message() {
            MsgType::MsgTypeResult(result) => result.is_in_reply_to(&join) && result.is_ok(),
            _ => false,
        }})?;
        Ok(())
    }

    pub fn send_leave(&mut self) -> ah::Result<()> {
        let leave = MsgLeave::new();
        self.stream.write(&leave.to_bytes())?;
        self.wait_for_reply("leave", |m| { match m.get_message() {
            MsgType::MsgTypeResult(result) => result.is_in_reply_to(&leave) && result.is_ok(),
            _ => false,
        }})?;
        Ok(())
    }

    pub fn send_reset(&mut self) -> ah::Result<()> {
        let reset = MsgReset::new();
        self.stream.write(&reset.to_bytes())?;
        self.wait_for_reply("reset", |m| { match m.get_message() {
            MsgType::MsgTypeResult(result) => result.is_in_reply_to(&reset) && result.is_ok(),
            _ => false,
        }})?;
        Ok(())
    }

    pub fn send_request_gamestate(&mut self) -> ah::Result<()> {
        self.stream.write(&MsgReqGameState::new().to_bytes())?;
        Ok(())
    }

    pub fn send_move_token(&mut self,
                           action: u32,
                           token: u32,
                           coord_x: u32,
                           coord_y: u32) -> ah::Result<()> {
        self.stream.write(&MsgMove::new(action,
                                        token,
                                        coord_x,
                                        coord_y).to_bytes())?;
        Ok(())
    }

    pub fn poll(&mut self) -> Option<Vec<Box<dyn Message>>> {
        let mut buffer = vec![0u8; MSG_BUFFER_SIZE];
        let offset = match self.tail_buffer.as_ref() {
            None => 0,
            Some(tail_buffer) => {
                let tlen = tail_buffer.len();
                buffer[0..tlen].copy_from_slice(&tail_buffer[0..tlen]);
                self.tail_buffer = None;
                tlen
            },
        };

        match self.stream.read(&mut buffer[offset..]) {
            Ok(0) | Err(_) =>
                return None,
            Ok(len) =>
                buffer.truncate(offset + len),
        }

        if !self.sync {
            match net_sync(&buffer[..]) {
                Some(skip) => {
                    buffer = buffer_skip(buffer, skip);
                    self.sync = true;
                },
                None => {
                    self.sync = false;
                    return None;
                },
            }
        }

        let mut messages: Vec<Box<dyn Message>> = vec![];
        loop {
            match message_from_bytes(&buffer) {
                Ok((len, Some(message))) => {
                    messages.push(message);
                    buffer = buffer_skip(buffer, len);
                },
                Ok((_len, None)) => {
                    // Not enough data for this message, yet.
                    break;
                },
                Err(e) => {
                    eprintln!("Received invalid message: {}", e);
                    self.sync = false;
                    buffer.clear();
                    break;
                },
            }
        }

        if !buffer.is_empty() {
            self.tail_buffer = Some(buffer);
        }

        Some(messages)
    }

    pub fn disconnect(&mut self) {
        self.send_leave().ok();
        self.stream.shutdown(Shutdown::Both).ok();
    }
}

// vim: ts=4 sw=4 expandtab
