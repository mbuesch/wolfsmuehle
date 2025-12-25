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

use crate::game_state::GameState;
use crate::net::consts::MAX_PLAYERS;
use crate::player::{Player, PlayerList, PlayerMode};
use anyhow as ah;
use std::cmp::{Eq, Ord, PartialEq, PartialOrd};

pub struct ServerRoom {
    name: String,
    game_state: GameState,
    player_list: PlayerList,
    restrict_player_modes: bool,
}

impl ServerRoom {
    pub fn new(name: String, restrict_player_modes: bool) -> ah::Result<ServerRoom> {
        let mut game_state = GameState::new(PlayerMode::Both, None)?; /* no player name */
        let player_list = PlayerList::new(vec![]);
        game_state.set_room_player_list(player_list.clone());
        Ok(ServerRoom {
            name,
            game_state,
            player_list,
            restrict_player_modes,
        })
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_game_state(&mut self, player_mode: PlayerMode) -> &mut GameState {
        self.game_state
            .set_player_mode(player_mode)
            .expect("game_state.set_player_mode failed.");
        &mut self.game_state
    }

    pub fn can_add_player(
        &self,
        player_name: &str,
        player_mode: PlayerMode,
        ignore_name: Option<&str>,
    ) -> ah::Result<()> {
        let mut player_list = self.player_list.clone();
        if let Some(ignore_name) = ignore_name {
            player_list.remove_player_by_name(ignore_name);
        }

        if self.restrict_player_modes {
            match player_mode {
                PlayerMode::Spectator => (),
                PlayerMode::Both => {
                    return Err(ah::format_err!(
                        "PlayerMode::Both not supported in restricted mode."
                    ));
                }
                PlayerMode::Wolf => {
                    if !player_list
                        .find_players_by_mode(PlayerMode::Wolf)
                        .is_empty()
                    {
                        return Err(ah::format_err!("The game already has a Wolf player."));
                    }
                }
                PlayerMode::Sheep => {
                    if !player_list
                        .find_players_by_mode(PlayerMode::Sheep)
                        .is_empty()
                    {
                        return Err(ah::format_err!("The game already has a Sheep player."));
                    }
                }
            }
        }

        if player_list.count() < MAX_PLAYERS {
            if player_list.find_player_by_name(player_name).is_some() {
                Err(ah::format_err!(
                    "Player name '{}' is already occupied.",
                    player_name
                ))
            } else {
                Ok(())
            }
        } else {
            Err(ah::format_err!(
                "Player '{}' can't join. Too many players in room.",
                player_name
            ))
        }
    }

    pub fn add_player(&mut self, player_name: &str, player_mode: PlayerMode) -> ah::Result<()> {
        self.can_add_player(player_name, player_mode, None)?;

        self.player_list
            .add_player(Player::new(player_name.to_string(), player_mode, false));
        self.game_state
            .set_room_player_list(self.player_list.clone());
        Ok(())
    }

    pub fn remove_player(&mut self, player_name: &str) {
        self.player_list.remove_player_by_name(player_name);
        self.game_state
            .set_room_player_list(self.player_list.clone());
    }

    pub fn get_player_list_ref(&self) -> &PlayerList {
        &self.player_list
    }
}

impl PartialEq for ServerRoom {
    fn eq(&self, other: &Self) -> bool {
        self.name.eq(&other.name)
    }
}

#[allow(clippy::non_canonical_partial_ord_impl)]
impl PartialOrd for ServerRoom {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.name.partial_cmp(&other.name)
    }
}

impl Ord for ServerRoom {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.name.cmp(&other.name)
    }
}

impl Eq for ServerRoom {}

// vim: ts=4 sw=4 expandtab
