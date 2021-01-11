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

use gio::prelude::*;
use gtk::prelude::*;
use gtk;

pub struct PlayerList {
    tree_view:      gtk::TreeView,
}

impl PlayerList {
    pub fn new(tree_view: gtk::TreeView) -> PlayerList {
        let column = gtk::TreeViewColumn::new();
        let cell = gtk::CellRendererText::new();
        column.pack_start(&cell, true);
        column.add_attribute(&cell, "text", 0);
        tree_view.append_column(&column);
        //TODO
        let model = gtk::ListStore::new(&[String::static_type()]);
        for entry in &["player 0", "player 1"] {
            model.insert_with_values(None, &[0], &[&entry]);
        }
        tree_view.set_model(Some(&model));

        PlayerList {
            tree_view,
        }
    }
}

// vim: ts=4 sw=4 expandtab
