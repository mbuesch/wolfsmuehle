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

use crate::gtk_helpers::*;
use crate::player::PlayerList;

pub struct PlayerListView {
    model:          gtk::ListStore,
    displayed_list: PlayerList,
}

impl PlayerListView {
    pub fn new(tree_view: gtk::TreeView) -> PlayerListView {
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
            model,
            displayed_list: PlayerList::new(vec![]),
        }
    }

    pub fn update(&mut self, player_list: &PlayerList) {
        if *player_list != self.displayed_list {
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
    }

    pub fn clear(&mut self) {
        self.update(&PlayerList::new(vec![]));
    }
}

// vim: ts=4 sw=4 expandtab
