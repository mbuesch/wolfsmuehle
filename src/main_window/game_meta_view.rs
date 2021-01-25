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
use crate::{gsignal_connect_to_mut, gsigparam};
use crate::gtk_helpers::*;
use crate::player::{PlayerList, PlayerMode};
use std::cell::RefCell;
use std::rc::Rc;

pub struct GameMetaView {
    game:                   Rc<RefCell<GameState>>,
    roomlist_model:         gtk::ListStore,
    playerlist_model:       gtk::ListStore,
    displayed_playerlist:   PlayerList,
    displayed_roomlist:     Vec<String>,
    player_name_entry:      gtk::Entry,
    player_mode_combo:      gtk::ComboBoxText,
    player_name_editing:    bool,
}

impl GameMetaView {
    pub fn new(game:                Rc<RefCell<GameState>>,
               room_tree_view:      gtk::TreeView,
               player_tree_view:    gtk::TreeView,
               player_name_entry:   gtk::Entry,
               player_mode_combo:   gtk::ComboBoxText) -> GameMetaView {
        // Room list
        for i in 0..2 {
            let column = gtk::TreeViewColumn::new();
            let cell = gtk::CellRendererText::new();
            column.pack_start(&cell, true);
            column.add_attribute(&cell, "text", i);
            column.set_title(["Room name",
                              "joined", ][i as usize]);
            room_tree_view.append_column(&column);
        }
        let roomlist_model = gtk::ListStore::new(&[String::static_type(),
                                                   String::static_type(), ]);
        room_tree_view.set_model(Some(&roomlist_model));

        // Player list
        for i in 0..3 {
            let column = gtk::TreeViewColumn::new();
            let cell = gtk::CellRendererText::new();
            column.pack_start(&cell, true);
            column.add_attribute(&cell, "text", i);
            column.set_title(["Player name",
                              "Mode",
                              "is me", ][i as usize]);
            player_tree_view.append_column(&column);
        }
        let playerlist_model = gtk::ListStore::new(&[String::static_type(),
                                                     String::static_type(),
                                                     String::static_type(), ]);
        player_tree_view.set_model(Some(&playerlist_model));

        GameMetaView {
            game,
            roomlist_model,
            playerlist_model,
            displayed_playerlist: PlayerList::new(vec![]),
            displayed_roomlist: vec![],
            player_name_entry,
            player_mode_combo,
            player_name_editing: false,
        }
    }

    fn do_update_player_list(&mut self, player_list: &PlayerList) {
        self.playerlist_model.clear();
        for player in player_list.iter() {

            self.playerlist_model.insert_with_values(
                None,
                &[0, 1, 2],
                &[&player.name,
                  &format!("{}", player.mode),
                  &if player.is_self { "<---" } else { "" }, ]
            );
        }
        self.displayed_playerlist = player_list.clone();
    }

    fn do_update_local_player(&mut self, player_list: &PlayerList) {
        for player in player_list.iter() {
            if player.is_self {
                if self.player_name_entry.get_text() != player.name {
                    self.player_name_entry.set_text(&player.name);
                }
                if self.get_player_mode() != player.mode {
                    match player.mode {
                        PlayerMode::Spectator =>
                            self.player_mode_combo.set_active_id(Some("spectator")),
                        PlayerMode::Wolf =>
                            self.player_mode_combo.set_active_id(Some("wolf")),
                        PlayerMode::Sheep =>
                            self.player_mode_combo.set_active_id(Some("sheep")),
                        PlayerMode::Both =>
                            self.player_mode_combo.set_active_id(Some("both")),
                    };
                }
            }
        }
    }

    pub fn update_player_list(&mut self, player_list: &PlayerList) {
        if !self.player_name_editing {
            if *player_list != self.displayed_playerlist {
               self.do_update_player_list(player_list);
            }
            self.do_update_local_player(player_list);
        }
    }

    pub fn clear_player_list(&mut self) {
        if !self.displayed_playerlist.is_empty() {
            self.do_update_player_list(&PlayerList::new(vec![]));
        }
    }

    pub fn update_room_list(&mut self, room_list: &Vec<String>) {
        if self.displayed_roomlist != *room_list {

            self.roomlist_model.clear();
            for room_name in room_list {
                let is_joined_room = match self.game.borrow().client_get_joined_room() {
                    Some(r) => r == room_name,
                    None => false,
                };

                self.roomlist_model.insert_with_values(
                    None,
                    &[0, 1, ],
                    &[&room_name,
                      &if is_joined_room { "<---" } else { "" }, ]
                );
            }
            self.displayed_roomlist = room_list.clone();
        }
    }

    fn handle_join_room_req(&mut self, tree_path: &gtk::TreePath) {
        let index = tree_path.get_indices()[0];
        if (index as usize) < self.displayed_roomlist.len() {
            let room_name = &self.displayed_roomlist[index as usize].to_string();
            {
                let mut game = self.game.borrow_mut();

                if let Some(joined_room) = game.client_get_joined_room() {
                    if joined_room == room_name {
                        return;
                    }
                }

                let result = game.client_join_room(room_name);
                if let Err(e) = result {
                    eprintln!("Failed to join room: {}", e);
                }
            }
            self.displayed_roomlist.clear();
        }
    }

    fn playername_changed(&mut self) {
        self.player_name_editing = true;
    }

    fn playername_editdone(&mut self) {
        let new_name = self.player_name_entry.get_text().as_str().to_string();
        let result = self.game.borrow_mut().set_player_name(&new_name);
        match result {
            Ok(_) => (),
            Err(e) => {
                messagebox_error::<gtk::Window>(
                    None,
                    &format!("Failed set new player name:\n{}", e));
            }
        }
        self.player_name_editing = false;
    }

    fn playermode_changed(&self) {
        let new_mode = match self.player_mode_combo.get_active_id() {
            Some(id) => {
                match id.as_str() {
                    "spectator" => PlayerMode::Spectator,
                    "wolf" => PlayerMode::Wolf,
                    "sheep" => PlayerMode::Sheep,
                    "both" => PlayerMode::Both,
                    _ => PlayerMode::Spectator,
                }
            },
            _ => PlayerMode::Spectator,
        };
        let result = self.game.borrow_mut().set_player_mode(new_mode);
        match result {
            Ok(_) => (),
            Err(e) => {
                messagebox_error::<gtk::Window>(
                    None,
                    &format!("Failed set new player mode:\n{}", e));
            }
        }
    }

    fn get_player_mode(&self) -> PlayerMode {
        if let Some(active_id) = self.player_mode_combo.get_active_id() {
            match active_id.as_str() {
                "spectator" => PlayerMode::Spectator,
                "wolf"      => PlayerMode::Wolf,
                "sheep"     => PlayerMode::Sheep,
                "both"      => PlayerMode::Both,
                _           => PlayerMode::Spectator,
            }
        } else {
            PlayerMode::Spectator
        }
    }

    fn gsignal_playername_changed(&mut self, _param: &[glib::Value]) -> Option<glib::Value> {
        self.playername_changed();
        None
    }

    fn gsignal_playername_editdone(&mut self, _param: &[glib::Value]) -> Option<glib::Value> {
        self.playername_editdone();
        None
    }

    fn gsignal_playername_focusout(&mut self, _param: &[glib::Value]) -> Option<glib::Value> {
        self.playername_editdone();
        Some(false.to_value())
    }

    fn gsignal_playermode_changed(&mut self, _param: &[glib::Value]) -> Option<glib::Value> {
        self.playermode_changed();
        None
    }

    fn gsignal_roomtree_rowactivated(&mut self, param: &[glib::Value]) -> Option<glib::Value> {
        let _tree_view = gsigparam!(param[0], gtk::TreeView);
        let path = gsigparam!(param[1], gtk::TreePath);
        let _column = gsigparam!(param[2], gtk::TreeViewColumn);
        self.handle_join_room_req(&path);
        None
    }

    pub fn connect_signals(_self: Rc<RefCell<GameMetaView>>,
                           handler_name: &str) -> Option<GSigHandler> {
        match handler_name {
            "handler_playername_changed" =>
                Some(gsignal_connect_to_mut!(_self, gsignal_playername_changed, None)),
            "handler_playername_activate" =>
                Some(gsignal_connect_to_mut!(_self, gsignal_playername_editdone, None)),
            "handler_playername_editdone" =>
                Some(gsignal_connect_to_mut!(_self, gsignal_playername_editdone, None)),
            "handler_playername_focusout" =>
                Some(gsignal_connect_to_mut!(_self, gsignal_playername_focusout, Some(false.to_value()))),
            "handler_playermode_changed" =>
                Some(gsignal_connect_to_mut!(_self, gsignal_playermode_changed, None)),
            "handler_roomtree_rowactivated" =>
                Some(gsignal_connect_to_mut!(_self, gsignal_roomtree_rowactivated, None)),
            _ => None,
        }
    }
}

// vim: ts=4 sw=4 expandtab
