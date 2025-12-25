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

use super::GameState;
use crate::net::protocol::{Message, MsgType, message_from_bytes};
use anyhow as ah;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::Path;

impl GameState {
    pub fn save_game(&self, filename: &Path) -> ah::Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(filename)?;
        file.write_all(&self.serialize()?)?;
        file.sync_all()?;
        Ok(())
    }

    pub fn load_game(&mut self, filename: &Path) -> ah::Result<()> {
        let mut file = OpenOptions::new().read(true).open(filename)?;
        let mut buf = vec![];
        file.read_to_end(&mut buf)?;
        self.deserialize(&buf)?;
        Ok(())
    }

    fn serialize(&self) -> ah::Result<Vec<u8>> {
        let game_state_msg = self.make_state_message();
        Ok(game_state_msg.to_bytes())
    }

    fn deserialize(&mut self, data: &[u8]) -> ah::Result<()> {
        let mut offset = 0;
        let mut messages = vec![];
        loop {
            let (size, msg) = message_from_bytes(&data[offset..])?;
            if size == 0 {
                break;
            }
            if let Some(msg) = msg {
                offset += size;

                // Check if this is a supported message.
                match msg.get_message() {
                    MsgType::GameState(_) => (),
                    invalid => {
                        return Err(ah::format_err!(
                            "File data contains unsupported packet {:?}",
                            invalid
                        ));
                    }
                }

                messages.push(msg);
            } else {
                break;
            }
        }
        if messages.is_empty() {
            return Err(ah::format_err!(
                "File data does not contain valid game state."
            ));
        }

        // Set the local game state to the message contents.
        self.reset_game(true);
        for msg in messages {
            if let MsgType::GameState(msg) = msg.get_message() {
                self.read_state_message(msg, true)?;
            }
        }
        // Send the local game state to the server (if any).
        self.client_send_full_gamestate()?;

        Ok(())
    }
}

// vim: ts=4 sw=4 expandtab
