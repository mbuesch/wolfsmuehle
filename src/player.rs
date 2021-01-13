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
    MSG_PLAYERMODE_SPECTATOR,
    MSG_PLAYERMODE_WOLF,
    MSG_PLAYERMODE_SHEEP,
    MSG_PLAYERMODE_BOTH,
};
use std::fmt;

pub fn num_to_player_mode(player_mode: u32) -> ah::Result<PlayerMode> {
    match player_mode {
        MSG_PLAYERMODE_SPECTATOR =>
            Ok(PlayerMode::Spectator),
        MSG_PLAYERMODE_BOTH =>
            Ok(PlayerMode::Both),
        MSG_PLAYERMODE_WOLF =>
            Ok(PlayerMode::Wolf),
        MSG_PLAYERMODE_SHEEP =>
            Ok(PlayerMode::Sheep),
        _ =>
            Err(ah::format_err!("Received invalid player_mode: {}", player_mode)),
    }
}

pub const fn player_mode_to_num(player_mode: PlayerMode) -> u32 {
    match player_mode {
        PlayerMode::Spectator =>
            MSG_PLAYERMODE_SPECTATOR,
        PlayerMode::Both =>
            MSG_PLAYERMODE_BOTH,
        PlayerMode::Wolf =>
            MSG_PLAYERMODE_WOLF,
        PlayerMode::Sheep =>
            MSG_PLAYERMODE_SHEEP,
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

impl fmt::Display for PlayerMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", match self {
            PlayerMode::Spectator => "Spectator",
            PlayerMode::Both => "Wolf and sheep",
            PlayerMode::Wolf => "Wolf",
            PlayerMode::Sheep => "Sheep",
        })
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Player {
    pub name:       String,
    pub mode:       PlayerMode,
    pub is_self:    bool,
}

impl Player {
    pub fn new(name: String,
               mode: PlayerMode,
               is_self: bool) -> Player {
        Player {
            name,
            mode,
            is_self,
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct PlayerList {
    players:    Vec<Player>,
}

impl PlayerList {
    pub fn new(players: Vec<Player>) -> PlayerList {
        PlayerList {
            players,
        }
    }

    pub fn count(&self) -> usize {
        self.players.len()
    }

    pub fn resize<F>(&mut self, new_size: usize, new_item: F)
    where
        F: Fn() -> Player,
    {
        self.players.resize_with(new_size, || new_item());
    }

    pub fn find_player_by_name(&self, name: &str) -> Option<&Player> {
        for player in &self.players {
            if player.name == name {
                return Some(player);
            }
        }
        None
    }

    pub fn find_players_by_mode(&self, mode: PlayerMode) -> Vec<Player> {
        let mut players = self.players.to_vec();
        players.retain(|p| p.mode == mode);
        players
    }

    pub fn add_player(&mut self, player: Player) {
        self.players.push(player);
    }

    pub fn remove_player_by_name(&mut self, name: &str) {
        self.players.retain(|p| p.name != name);
    }

    pub fn set_player(&mut self, index: usize, player: Player) {
        if index < self.players.len() {
            self.players[index] = player;
        }
    }

    pub fn iter(&self) -> PlayerListIterator {
        PlayerListIterator::new(self)
    }
}

pub struct PlayerListIterator<'a> {
    player_list:    &'a PlayerList,
    index:          usize,
}

impl<'a> PlayerListIterator<'a> {
    fn new(player_list: &'a PlayerList) -> PlayerListIterator<'a> {
        PlayerListIterator {
            player_list,
            index: 0,
        }
    }
}

impl<'a> Iterator for PlayerListIterator<'a> {
    type Item = &'a Player;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.player_list.players.len() {
            None
        } else {
            let i = self.index;
            self.index += 1;
            Some(&self.player_list.players[i])
        }
    }
}

// vim: ts=4 sw=4 expandtab
