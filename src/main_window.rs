// -*- coding: utf-8 -*-
//
// Copyright 2021 Michael Buesch <m@bues.ch>
//
// SPDX-License-Identifier: MIT OR Apache-2.0
//

mod game_meta_view;
use game_meta_view::GameMetaView;

mod drawing_area;
use drawing_area::DrawingArea;

use crate::game_state::GameState;
use crate::gsignal_connect_to_mut;
use crate::gtk_helpers::*;
use crate::player::PlayerMode;
use crate::print::Print;
use anyhow as ah;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

const ABOUT_TEXT: &str = "Wolfsmühle - Board game\n\
     \n\
     Copyright 2021 Michael Buesch <m@bues.ch>\n\
     \n\
     SPDX-License-Identifier: MIT OR Apache-2.0";

pub struct MainWindow {
    appwindow: gtk::ApplicationWindow,
    button_connect: gtk::MenuItem,
    button_disconnect: gtk::MenuItem,
    status_label: gtk::Label,
    game_meta_info_grid: gtk::Grid,
    draw: Rc<RefCell<DrawingArea>>,
    game: Rc<RefCell<GameState>>,
    game_meta_view: Rc<RefCell<GameMetaView>>,
}

impl MainWindow {
    pub fn new(
        app: &gtk::Application,
        connect_to_server: Option<String>,
        room_name: String,
        player_name: Option<String>,
        player_mode: PlayerMode,
    ) -> ah::Result<Rc<RefCell<MainWindow>>> {
        // Create main window.
        let glade_source = include_str!("main_window.glade");
        let builder = gtk::Builder::from_string(glade_source);
        let appwindow: gtk::ApplicationWindow = builder.object("mainwindow").unwrap();
        appwindow.set_application(Some(app));
        appwindow.set_title("Wolfsmühle");

        // Other widgets.
        let button_connect: gtk::MenuItem = builder.object("menubutton_connect").unwrap();
        let button_disconnect: gtk::MenuItem = builder.object("menubutton_disconnect").unwrap();
        button_connect.set_sensitive(connect_to_server.is_none());
        button_disconnect.set_sensitive(connect_to_server.is_some());
        let status_label: gtk::Label = builder.object("status_label").unwrap();
        let game_meta_info_grid: gtk::Grid = builder.object("game_meta_info_grid").unwrap();

        // Create game state.
        let game = Rc::new(RefCell::new(GameState::new(player_mode, player_name)?));
        if let Some(connect_to_server) = &connect_to_server {
            let mut game = game.borrow_mut();
            game.client_connect(connect_to_server)?;
            game.client_join_room(&room_name)?;
            game_meta_info_grid.show();
        } else {
            game_meta_info_grid.hide();
        }

        // Create player state area.
        let room_tree: gtk::TreeView = builder.object("room_tree").unwrap();
        let player_tree: gtk::TreeView = builder.object("player_tree").unwrap();
        let player_name_entry: gtk::Entry = builder.object("player_name_entry").unwrap();
        let player_mode_combo: gtk::ComboBoxText = builder.object("player_mode_combo").unwrap();
        let chat_text: gtk::TextView = builder.object("chat_text_view").unwrap();
        let chat_say_entry: gtk::Entry = builder.object("chat_say_entry").unwrap();
        let game_meta_view = Rc::new(RefCell::new(GameMetaView::new(
            Rc::clone(&game),
            room_tree,
            player_tree,
            player_name_entry,
            player_mode_combo,
            chat_text,
            chat_say_entry,
        )));

        // Create drawing area.
        let draw = Rc::new(RefCell::new(DrawingArea::new(
            builder.object("drawing_area").unwrap(),
            Rc::clone(&game),
        )?));

        let mainwnd = Rc::new(RefCell::new(MainWindow {
            appwindow,
            button_connect,
            button_disconnect,
            status_label,
            game_meta_info_grid,
            draw,
            game,
            game_meta_view,
        }));

        // Create game polling timer.
        let mainwnd2 = Rc::clone(&mainwnd);
        let period = Duration::from_millis(200);
        glib::timeout_add_local(period, move || {
            if let Ok(mut mw) = mainwnd2.try_borrow_mut() {
                mw.poll_timer();
            }
            glib::ControlFlow::Continue
        });

        // Connect signals.
        let mainwnd2 = Rc::clone(&mainwnd);
        let draw2 = Rc::clone(&mainwnd.borrow().draw);
        let game_meta_view2 = Rc::clone(&mainwnd.borrow().game_meta_view);
        builder.connect_signals(move |_builder, handler_name| {
            let mainwnd2 = Rc::clone(&mainwnd2);
            let game_meta_view2 = Rc::clone(&game_meta_view2);

            if let Some(handler) = DrawingArea::connect_signals(Rc::clone(&draw2), handler_name) {
                return handler;
            }
            if let Some(handler) =
                GameMetaView::connect_signals(Rc::clone(&game_meta_view2), handler_name)
            {
                return handler;
            }

            match handler_name {
                "handler_resetgame" => gsignal_connect_to_mut!(mainwnd2, gsignal_resetgame, None),
                "handler_loadgame" => gsignal_connect_to_mut!(mainwnd2, gsignal_loadgame, None),
                "handler_savegame" => gsignal_connect_to_mut!(mainwnd2, gsignal_savegame, None),
                "handler_connect" => gsignal_connect_to_mut!(mainwnd2, gsignal_connect, None),
                "handler_disconnect" => gsignal_connect_to_mut!(mainwnd2, gsignal_disconnect, None),
                "handler_record_show" => {
                    gsignal_connect_to_mut!(mainwnd2, gsignal_record_show, None)
                }
                "handler_about" => gsignal_connect_to_mut!(mainwnd2, gsignal_about, None),
                "handler_quit" => gsignal_connect_to_mut!(mainwnd2, gsignal_quit, None),
                name => {
                    Print::error(&format!("Unhandled signal: {}", name));
                    Box::new(|_| None)
                }
            }
        });

        mainwnd.borrow().update_status();

        Ok(mainwnd)
    }

    #[allow(clippy::collapsible_if)]
    #[allow(clippy::collapsible_else_if)]
    fn poll_timer(&mut self) {
        if let Ok(mut draw) = self.draw.try_borrow_mut()
            && let Ok(mut game_meta_view) = self.game_meta_view.try_borrow_mut()
        {
            let redraw;
            let player_list;
            let room_list;
            let chat_messages;
            let is_joined_room;
            let is_connected;
            if let Ok(mut game) = self.game.try_borrow_mut() {
                redraw = game.poll_server();
                player_list = Some(game.get_room_player_list().clone());
                room_list = Some(game.get_room_list().clone());
                chat_messages = Some(game.client_get_chat_messages());
                is_joined_room = game.client_get_joined_room().is_some();
                is_connected = Some(game.client_is_connected());
            } else {
                redraw = false;
                player_list = None;
                room_list = None;
                chat_messages = None;
                is_joined_room = false;
                is_connected = None;
            }

            if let Some(player_list) = player_list {
                if !is_joined_room {
                    game_meta_view.clear_player_list();
                } else {
                    game_meta_view.update_player_list(&player_list);
                }
            }
            if let Some(room_list) = room_list {
                game_meta_view.update_room_list(&room_list);
            }
            if is_joined_room {
                if let Some(chat_messages) = chat_messages {
                    if !chat_messages.is_empty() {
                        game_meta_view.add_chat_messages(&chat_messages);
                    }
                }
            } else {
                game_meta_view.clear_chat_messages();
            }
            if let Some(is_connected) = is_connected {
                let pending_join = is_connected && !is_joined_room;
                draw.set_pending_join(pending_join);
            }
            if redraw {
                draw.redraw();
            }
        }
        self.update_status();
    }

    pub fn main_window(&self) -> gtk::ApplicationWindow {
        self.appwindow.clone()
    }

    /// Update the status bar.
    fn update_status(&self) {
        let mut status = None;

        if let Ok(game) = self.game.try_borrow() {
            match game.client_get_addr() {
                None => status = Some("Local game. Not connected to server.".to_string()),
                Some(addr) => match game.client_get_joined_room() {
                    None => status = Some(format!("Connected to '{}' and not in a room.", addr)),
                    Some(room) => {
                        status = Some(format!("Connected to '{}' in room '{}'.", addr, room))
                    }
                },
            }
        }

        if let Some(status) = status {
            self.status_label.set_text(&status);
        }
    }

    fn about(&self) {
        messagebox_info(Some(&self.appwindow), ABOUT_TEXT);
    }

    fn record_show(&self) {
        let log = self.game.borrow_mut().get_recorder().get_moves_as_text();
        messagebox_info(Some(&self.appwindow), &log);
    }

    fn connect_game(&mut self) {
        let dlg = gtk::MessageDialog::new(
            Some(&self.appwindow),
            gtk::DialogFlags::MODAL,
            gtk::MessageType::Question,
            gtk::ButtonsType::OkCancel,
            "Connect to a game server.\n\
             Enter the server address here:",
        );
        dlg.set_default_response(gtk::ResponseType::Ok);
        let content = dlg.content_area();
        let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 0);

        hbox.add(&gtk::Label::new(Some("Host:")));
        let entry_addr = gtk::Entry::new();
        entry_addr.set_size_request(300, 0);
        entry_addr.set_text("127.0.0.1");
        entry_addr.set_activates_default(true);
        hbox.add(&entry_addr);

        hbox.add(&gtk::Label::new(Some("Port:")));
        let entry_port = gtk::Entry::new();
        entry_port.set_size_request(300, 0);
        entry_port.set_text("5596");
        entry_port.set_activates_default(true);
        hbox.add(&entry_port);

        content.pack_end(&hbox, false, false, 0);
        dlg.show_all();

        let result = dlg.run();
        let addr = format!(
            "{}:{}",
            entry_addr.text().as_str(),
            entry_port.text().as_str()
        );

        if result == gtk::ResponseType::Ok {
            self.draw.borrow_mut().reset_game();

            let result = self.game.borrow_mut().client_connect(&addr);
            if let Err(e) = result {
                messagebox_error(Some(&dlg), &format!("Failed to connect to server:\n{}", e));
            } else {
                self.game_meta_view.borrow_mut().clear_player_list();
                self.button_connect.set_sensitive(false);
                self.button_disconnect.set_sensitive(true);
                self.game_meta_info_grid.show();
            }
        }
        dlg.close();

        self.update_status();
    }

    fn disconnect_game(&mut self) {
        self.game.borrow_mut().client_disconnect();

        self.draw.borrow_mut().reset_game();
        self.game_meta_view.borrow_mut().clear_player_list();
        self.game_meta_view.borrow_mut().clear_chat_messages();
        self.button_connect.set_sensitive(true);
        self.button_disconnect.set_sensitive(false);
        self.game_meta_info_grid.hide();

        self.update_status();
    }

    fn load_game(&mut self) {
        let dlg = gtk::FileChooserDialog::with_buttons(
            Some("Load game state"),
            Some(&self.appwindow),
            gtk::FileChooserAction::Open,
            &[
                ("_Cancel", gtk::ResponseType::Cancel),
                ("_Open", gtk::ResponseType::Accept),
            ],
        );
        if dlg.run() == gtk::ResponseType::Accept
            && let Some(filename) = dlg.filename()
            && let Err(e) = self.draw.borrow_mut().load_game(filename.as_path())
        {
            messagebox_error(Some(&dlg), &format!("Failed to load game:\n{}", e));
        }
        dlg.close();
    }

    fn save_game(&self) {
        let dlg = gtk::FileChooserDialog::with_buttons(
            Some("Save game state"),
            Some(&self.appwindow),
            gtk::FileChooserAction::Save,
            &[
                ("_Cancel", gtk::ResponseType::Cancel),
                ("_Save", gtk::ResponseType::Accept),
            ],
        );
        if dlg.run() == gtk::ResponseType::Accept
            && let Some(filename) = dlg.filename()
            && let Err(e) = self.draw.borrow().save_game(filename.as_path())
        {
            messagebox_error(Some(&dlg), &format!("Failed to save game:\n{}", e));
        }
        dlg.close();
    }

    fn gsignal_record_show(&mut self, _param: &[glib::Value]) -> Option<glib::Value> {
        self.record_show();
        None
    }

    fn gsignal_quit(&mut self, _param: &[glib::Value]) -> Option<glib::Value> {
        std::process::exit(0);
    }

    fn gsignal_about(&mut self, _param: &[glib::Value]) -> Option<glib::Value> {
        self.about();
        None
    }

    fn gsignal_connect(&mut self, _param: &[glib::Value]) -> Option<glib::Value> {
        self.connect_game();
        None
    }

    fn gsignal_disconnect(&mut self, _param: &[glib::Value]) -> Option<glib::Value> {
        self.disconnect_game();
        None
    }

    fn gsignal_resetgame(&mut self, _param: &[glib::Value]) -> Option<glib::Value> {
        self.draw.borrow_mut().reset_game();
        None
    }

    fn gsignal_loadgame(&mut self, _param: &[glib::Value]) -> Option<glib::Value> {
        self.load_game();
        None
    }

    fn gsignal_savegame(&mut self, _param: &[glib::Value]) -> Option<glib::Value> {
        self.save_game();
        None
    }
}

// vim: ts=4 sw=4 expandtab
