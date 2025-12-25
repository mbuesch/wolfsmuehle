// -*- coding: utf-8 -*-
//
// Copyright 2021 Michael Buesch <m@bues.ch>
//
// SPDX-License-Identifier: MIT OR Apache-2.0
//

use crate::game_state::GameState;
use crate::gtk_helpers::*;
use crate::player::{PlayerList, PlayerMode};
use crate::print::Print;
use std::cell::RefCell;
use std::rc::Rc;

pub struct GameMetaView {
    game: Rc<RefCell<GameState>>,
    roomlist_model: gtk::ListStore,
    playerlist_model: gtk::ListStore,
    displayed_playerlist: PlayerList,
    displayed_roomlist: Vec<String>,
    player_name_entry: gtk::Entry,
    player_mode_combo: gtk::ComboBoxText,
    player_name_editing: bool,
    chat_text: gtk::TextView,
    chat_say_entry: gtk::Entry,
}

impl GameMetaView {
    pub fn new(
        game: Rc<RefCell<GameState>>,
        room_tree_view: gtk::TreeView,
        player_tree_view: gtk::TreeView,
        player_name_entry: gtk::Entry,
        player_mode_combo: gtk::ComboBoxText,
        chat_text: gtk::TextView,
        chat_say_entry: gtk::Entry,
    ) -> GameMetaView {
        // Room list
        for i in 0..2 {
            let column = gtk::TreeViewColumn::new();
            let cell = gtk::CellRendererText::new();
            CellLayoutExt::pack_start(&column, &cell, true);
            column.add_attribute(&cell, "text", i);
            column.set_title(["Room name", "joined"][i as usize]);
            room_tree_view.append_column(&column);
        }
        let roomlist_model = gtk::ListStore::new(&[String::static_type(), String::static_type()]);
        room_tree_view.set_model(Some(&roomlist_model));

        // Player list
        for i in 0..3 {
            let column = gtk::TreeViewColumn::new();
            let cell = gtk::CellRendererText::new();
            CellLayoutExt::pack_start(&column, &cell, true);
            column.add_attribute(&cell, "text", i);
            column.set_title(["Player name", "Mode", "is me"][i as usize]);
            player_tree_view.append_column(&column);
        }
        let playerlist_model = gtk::ListStore::new(&[
            String::static_type(),
            String::static_type(),
            String::static_type(),
        ]);
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
            chat_text,
            chat_say_entry,
        }
    }

    pub fn connect_signals(gmv: &Rc<RefCell<GameMetaView>>) {
        // Room tree row-activated signal.
        // We need to get the tree view from the model. Since the tree view
        // is not stored, we get it via the builder in main_window. Instead,
        // we can store the room_tree in the struct. But for now, let's
        // connect it here via the parent approach — actually the simplest
        // way is to just get the tree view widget. Let's find it by
        // navigating the widget tree from the model.
        // Actually, the cleanest approach: store room_tree in the struct
        // or pass it from outside. For now, let's store it externally
        // and have main_window pass it.

        // We need to connect these signals:
        // 1. room_tree row-activated -> handle_join_room_req
        // 2. player_name_entry changed -> playername_changed
        // 3. player_name_entry activate -> playername_editdone
        // 4. player_name_entry focus-leave -> playername_editdone
        // 5. player_mode_combo changed -> playermode_changed
        // 6. chat_say_entry activate -> handle_chat_say

        // Since the widgets are stored in the struct, we connect them here.
        // We need to clone out the widgets before borrowing mutably.
        let player_name_entry;
        let player_mode_combo;
        let chat_say_entry;
        {
            let gmv_ref = gmv.borrow();
            player_name_entry = gmv_ref.player_name_entry.clone();
            player_mode_combo = gmv_ref.player_mode_combo.clone();
            chat_say_entry = gmv_ref.chat_say_entry.clone();
        }

        // player_name_entry "changed" signal
        let gmv2 = Rc::clone(gmv);
        player_name_entry.connect_changed(move |_| {
            if let Ok(mut g) = gmv2.try_borrow_mut() {
                g.playername_changed();
            }
        });

        // player_name_entry "activate" signal (Enter key)
        let gmv2 = Rc::clone(gmv);
        player_name_entry.connect_activate(move |_| {
            if let Ok(mut g) = gmv2.try_borrow_mut() {
                g.playername_editdone();
            }
        });

        // player_name_entry focus-leave via EventControllerFocus
        let focus_controller = gtk::EventControllerFocus::new();
        let gmv2 = Rc::clone(gmv);
        focus_controller.connect_leave(move |_| {
            if let Ok(mut g) = gmv2.try_borrow_mut() {
                g.playername_editdone();
            }
        });
        player_name_entry.add_controller(focus_controller);

        // player_mode_combo "changed" signal
        let gmv2 = Rc::clone(gmv);
        player_mode_combo.connect_changed(move |_| {
            if let Ok(g) = gmv2.try_borrow() {
                g.playermode_changed();
            }
        });

        // chat_say_entry "activate" signal
        let gmv2 = Rc::clone(gmv);
        chat_say_entry.connect_activate(move |_| {
            if let Ok(mut g) = gmv2.try_borrow_mut() {
                g.handle_chat_say();
            }
        });
    }

    /// Connect the room tree view's row-activated signal.
    /// This is called separately because the tree view is obtained from the builder.
    pub fn connect_room_tree_signal(gmv: &Rc<RefCell<GameMetaView>>, room_tree: &gtk::TreeView) {
        let gmv2 = Rc::clone(gmv);
        room_tree.connect_row_activated(move |_tree_view, path, _column| {
            if let Ok(mut g) = gmv2.try_borrow_mut() {
                g.handle_join_room_req(path);
            }
        });
    }

    fn do_update_player_list(&mut self, player_list: &PlayerList) {
        self.playerlist_model.clear();
        for player in player_list.iter() {
            self.playerlist_model.insert_with_values(
                None,
                &[
                    (0, &player.name),
                    (1, &format!("{}", player.mode)),
                    (2, &if player.is_self { "<---" } else { "" }),
                ],
            );
        }
        self.displayed_playerlist = player_list.clone();
    }

    fn do_update_local_player(&mut self, player_list: &PlayerList) {
        for player in player_list.iter() {
            if player.is_self {
                if self.player_name_entry.text() != player.name {
                    self.player_name_entry.set_text(&player.name);
                }
                if self.get_player_mode() != player.mode {
                    match player.mode {
                        PlayerMode::Spectator => {
                            self.player_mode_combo.set_active_id(Some("spectator"))
                        }
                        PlayerMode::Wolf => self.player_mode_combo.set_active_id(Some("wolf")),
                        PlayerMode::Sheep => self.player_mode_combo.set_active_id(Some("sheep")),
                        PlayerMode::Both => self.player_mode_combo.set_active_id(Some("both")),
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
                    &[
                        (0, &room_name),
                        (1, &if is_joined_room { "<---" } else { "" }),
                    ],
                );
            }
            self.displayed_roomlist = room_list.clone();
        }
    }

    fn handle_join_room_req(&mut self, tree_path: &gtk::TreePath) {
        let index = tree_path.indices()[0];
        if (index as usize) < self.displayed_roomlist.len() {
            let room_name = &self.displayed_roomlist[index as usize].to_string();
            {
                let mut game = self.game.borrow_mut();

                if let Some(joined_room) = game.client_get_joined_room()
                    && joined_room == room_name
                {
                    return;
                }

                let result = game.client_join_room(room_name);
                if let Err(e) = result {
                    Print::error(&format!("Failed to join room: {}", e));
                }
            }
            self.clear_chat_messages();
            self.displayed_roomlist.clear();
        }
    }

    fn playername_changed(&mut self) {
        self.player_name_editing = true;
    }

    fn playername_editdone(&mut self) {
        let new_name = self.player_name_entry.text().as_str().to_string();
        let result = self.game.borrow_mut().set_player_name(&new_name);
        match result {
            Ok(_) => (),
            Err(e) => {
                messagebox_error::<gtk::Window>(
                    None,
                    &format!("Failed set new player name:\n{}", e),
                );
            }
        }
        self.player_name_editing = false;
    }

    fn playermode_changed(&self) {
        let new_mode = match self.player_mode_combo.active_id() {
            Some(id) => match id.as_str() {
                "spectator" => PlayerMode::Spectator,
                "wolf" => PlayerMode::Wolf,
                "sheep" => PlayerMode::Sheep,
                "both" => PlayerMode::Both,
                _ => PlayerMode::Spectator,
            },
            _ => PlayerMode::Spectator,
        };
        let result = self.game.borrow_mut().set_player_mode(new_mode);
        match result {
            Ok(_) => (),
            Err(e) => {
                messagebox_error::<gtk::Window>(
                    None,
                    &format!("Failed set new player mode:\n{}", e),
                );
            }
        }
    }

    fn get_player_mode(&self) -> PlayerMode {
        if let Some(active_id) = self.player_mode_combo.active_id() {
            match active_id.as_str() {
                "spectator" => PlayerMode::Spectator,
                "wolf" => PlayerMode::Wolf,
                "sheep" => PlayerMode::Sheep,
                "both" => PlayerMode::Both,
                _ => PlayerMode::Spectator,
            }
        } else {
            PlayerMode::Spectator
        }
    }

    pub fn clear_chat_messages(&mut self) {
        let buffer = self.chat_text.buffer();
        buffer.set_text("");
    }

    pub fn add_chat_messages(&mut self, messages: &Vec<String>) {
        let buffer = self.chat_text.buffer();
        let parent = self.chat_text.parent().unwrap();
        let scroll = parent.downcast_ref::<gtk::ScrolledWindow>().unwrap();

        // Add all messages to the text view
        let start = buffer.start_iter();
        let end = buffer.end_iter();
        let mut text = buffer.text(&start, &end, true).as_str().to_string();
        for m in messages {
            text.push_str(&format!("{}\n", m));
        }
        buffer.set_text(&text);

        // Scroll to the bottom.
        let adj = scroll.vadjustment();
        adj.set_value(adj.upper());
        scroll.set_vadjustment(Some(&adj));
    }

    fn handle_chat_say(&mut self) {
        let text = self.chat_say_entry.text();
        if !text.as_str().is_empty() {
            Print::debug(&format!("Say: {}", text));
            let ret = self
                .game
                .borrow_mut()
                .client_send_chat_message(text.as_str());
            if let Err(e) = ret {
                messagebox_error::<gtk::Window>(None, &format!("Failed send chat message:\n{}", e));
            } else {
                self.chat_say_entry.set_text("");
            }
        }
    }
}

// vim: ts=4 sw=4 expandtab
