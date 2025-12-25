// -*- coding: utf-8 -*-
//
// Copyright 2021 Michael Buesch <m@bues.ch>
//
// SPDX-License-Identifier: MIT OR Apache-2.0
//

use crate::net::protocol::{
    MSG_PLAYERMODE_BOTH, MSG_PLAYERMODE_SHEEP, MSG_PLAYERMODE_SPECTATOR, MSG_PLAYERMODE_WOLF,
};
use anyhow as ah;
use std::cmp::{Eq, Ord, PartialEq, PartialOrd};
use std::fmt;

pub fn num_to_player_mode(player_mode: u32) -> ah::Result<PlayerMode> {
    match player_mode {
        MSG_PLAYERMODE_SPECTATOR => Ok(PlayerMode::Spectator),
        MSG_PLAYERMODE_BOTH => Ok(PlayerMode::Both),
        MSG_PLAYERMODE_WOLF => Ok(PlayerMode::Wolf),
        MSG_PLAYERMODE_SHEEP => Ok(PlayerMode::Sheep),
        _ => Err(ah::format_err!(
            "Received invalid player_mode: {}",
            player_mode
        )),
    }
}

pub const fn player_mode_to_num(player_mode: PlayerMode) -> u32 {
    match player_mode {
        PlayerMode::Spectator => MSG_PLAYERMODE_SPECTATOR,
        PlayerMode::Both => MSG_PLAYERMODE_BOTH,
        PlayerMode::Wolf => MSG_PLAYERMODE_WOLF,
        PlayerMode::Sheep => MSG_PLAYERMODE_SHEEP,
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
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
        write!(
            f,
            "{}",
            match self {
                PlayerMode::Spectator => "Spectator",
                PlayerMode::Both => "Wolf and sheep",
                PlayerMode::Wolf => "Wolf",
                PlayerMode::Sheep => "Sheep",
            }
        )
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Player {
    pub name: String,
    pub mode: PlayerMode,
    pub is_self: bool,
}

impl Player {
    pub fn new(name: String, mode: PlayerMode, is_self: bool) -> Player {
        Player {
            name,
            mode,
            is_self,
        }
    }
}

#[allow(clippy::non_canonical_partial_ord_impl)]
impl PartialOrd for Player {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.name.partial_cmp(&other.name)
    }
}

impl Ord for Player {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.name.cmp(&other.name)
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct PlayerList {
    players: Vec<Player>,
}

impl PlayerList {
    pub fn new(players: Vec<Player>) -> PlayerList {
        PlayerList { players }
    }

    pub fn count(&self) -> usize {
        self.players.len()
    }

    pub fn is_empty(&self) -> bool {
        self.count() == 0
    }

    pub fn resize<F>(&mut self, new_size: usize, new_item: F)
    where
        F: Fn() -> Player,
    {
        self.players.resize_with(new_size, new_item);
    }

    pub fn find_player_by_name(&self, name: &str) -> Option<&Player> {
        self.players.iter().find(|player| player.name == name)
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

    pub fn iter(&self) -> PlayerListIterator<'_> {
        PlayerListIterator::new(self)
    }
}

pub struct PlayerListIterator<'a> {
    player_list: &'a PlayerList,
    index: usize,
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
