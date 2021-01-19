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
use crate::gsignal_connect_to_mut;
use crate::gtk_helpers::*;
use crate::player::{PlayerList, PlayerMode};
use std::cell::RefCell;
use std::rc::Rc;

pub struct PlayerListView {
    game:                   Rc<RefCell<GameState>>,
    tree_view:              gtk::TreeView,
    model:                  gtk::ListStore,
    displayed_list:         PlayerList,
    player_name_entry:      gtk::Entry,
    player_mode_combo:      gtk::ComboBoxText,
    player_name_editing:    bool,
}

impl PlayerListView {
    pub fn new(game:                Rc<RefCell<GameState>>,
               tree_view:           gtk::TreeView,
               player_name_entry:   gtk::Entry,
               player_mode_combo:   gtk::ComboBoxText) -> PlayerListView {
        for i in 0..3 {
            let column = gtk::TreeViewColumn::new();
            let cell = gtk::CellRendererText::new();
            column.pack_start(&cell, true);
            column.add_attribute(&cell, "text", i);
            column.set_title(["Player name",
                              "Mode",
                              ""][i as usize]);
            tree_view.append_column(&column);
        }
        let model = gtk::ListStore::new(&[String::static_type(),
                                          String::static_type(),
                                          String::static_type()]);
        tree_view.set_model(Some(&model));

        PlayerListView {
            game,
            tree_view,
            model,
            displayed_list: PlayerList::new(vec![]),
            player_name_entry,
            player_mode_combo,
            player_name_editing: false,
        }
    }

    fn do_update_player_list(&mut self, player_list: &PlayerList) {
        self.model.clear();
        for player in player_list.iter() {

            self.model.insert_with_values(
                None,
                &[0, 1, 2],
                &[&player.name,
                  &format!("{}", player.mode),
                  &format!("{}", if player.is_self { "*" } else { "" })]
            );
        }
        self.displayed_list = player_list.clone();
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

    pub fn update(&mut self, player_list: &PlayerList) {
        if !self.player_name_editing {
            if *player_list != self.displayed_list {
               self.do_update_player_list(player_list);
            }
            self.do_update_local_player(player_list);
        }
    }

    pub fn clear_player_list(&mut self) {
        if !self.displayed_list.is_empty() {
            self.do_update_player_list(&PlayerList::new(vec![]));
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
        //TODO
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

    pub fn connect_signals(_self: Rc<RefCell<PlayerListView>>,
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
            _ => None,
        }
    }
}

// vim: ts=4 sw=4 expandtab
