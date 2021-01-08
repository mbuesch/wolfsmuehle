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
use crate::protocol::{
    MSG_JOIN_PLAYERMODE_SPECTATOR,
    MSG_JOIN_PLAYERMODE_WOLF,
    MSG_JOIN_PLAYERMODE_SHEEP,
    MSG_JOIN_PLAYERMODE_BOTH,
};

pub fn num_to_player_mode(player_mode: u32) -> ah::Result<PlayerMode> {
    match player_mode {
        MSG_JOIN_PLAYERMODE_SPECTATOR =>
            Ok(PlayerMode::Spectator),
        MSG_JOIN_PLAYERMODE_BOTH =>
            Ok(PlayerMode::Both),
        MSG_JOIN_PLAYERMODE_WOLF =>
            Ok(PlayerMode::Wolf),
        MSG_JOIN_PLAYERMODE_SHEEP =>
            Ok(PlayerMode::Sheep),
        _ =>
            Err(ah::format_err!("Received invalid player_mode: {}", player_mode)),
    }
}

pub const fn player_mode_to_num(player_mode: PlayerMode) -> u32 {
    match player_mode {
        PlayerMode::Spectator =>
            MSG_JOIN_PLAYERMODE_SPECTATOR,
        PlayerMode::Both =>
            MSG_JOIN_PLAYERMODE_BOTH,
        PlayerMode::Wolf =>
            MSG_JOIN_PLAYERMODE_WOLF,
        PlayerMode::Sheep =>
            MSG_JOIN_PLAYERMODE_SHEEP,
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum PlayerMode {
    /// Spectators can't manipulate the game state.
    Spectator,
    /// Player can manipulate wolves and sheep.
    Both,
    /// Player can manipulate wolves.
    Wolf,
    /// Player can manipulate sheep.
    Sheep,
}

// vim: ts=4 sw=4 expandtab
