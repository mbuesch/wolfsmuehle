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
use crate::gtk_helpers::*;
use crate::player::PlayerMode;
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
        let ui_source = include_str!("main_window.ui");
        let builder = gtk::Builder::from_string(ui_source);
        let appwindow: gtk::ApplicationWindow = builder.object("mainwindow").unwrap();
        appwindow.set_application(Some(app));
        appwindow.set_title(Some("Wolfsmühle"));

        // Other widgets.
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

        // Set up drawing area signals.
        DrawingArea::connect_signals(&mainwnd.borrow().draw);

        // Set up game meta view signals.
        GameMetaView::connect_signals(&mainwnd.borrow().game_meta_view);
        let room_tree: gtk::TreeView = builder.object("room_tree").unwrap();
        GameMetaView::connect_room_tree_signal(&mainwnd.borrow().game_meta_view, &room_tree);

        // Set up window actions for menu items.
        Self::setup_actions(&mainwnd);

        mainwnd.borrow().update_status();

        Ok(mainwnd)
    }

    fn setup_actions(mainwnd: &Rc<RefCell<MainWindow>>) {
        let appwindow = mainwnd.borrow().appwindow.clone();

        // Reset game action
        let action = gio::SimpleAction::new("resetgame", None);
        let mw = Rc::clone(mainwnd);
        action.connect_activate(move |_, _| {
            if let Ok(mw) = mw.try_borrow() {
                mw.draw.borrow_mut().reset_game();
            }
        });
        appwindow.add_action(&action);

        // Load game action
        let action = gio::SimpleAction::new("loadgame", None);
        let mw = Rc::clone(mainwnd);
        action.connect_activate(move |_, _| {
            if let Ok(mut mw) = mw.try_borrow_mut() {
                mw.load_game();
            }
        });
        appwindow.add_action(&action);

        // Save game action
        let action = gio::SimpleAction::new("savegame", None);
        let mw = Rc::clone(mainwnd);
        action.connect_activate(move |_, _| {
            if let Ok(mw) = mw.try_borrow() {
                mw.save_game();
            }
        });
        appwindow.add_action(&action);

        // Quit action
        let action = gio::SimpleAction::new("quit", None);
        action.connect_activate(|_, _| {
            std::process::exit(0);
        });
        appwindow.add_action(&action);

        // Connect action
        let action = gio::SimpleAction::new("connect", None);
        let mw = Rc::clone(mainwnd);
        action.connect_activate(move |_, _| {
            if let Ok(mut mw) = mw.try_borrow_mut() {
                mw.connect_game();
            }
        });
        appwindow.add_action(&action);

        // Disconnect action
        let action = gio::SimpleAction::new("disconnect", None);
        let mw = Rc::clone(mainwnd);
        action.connect_activate(move |_, _| {
            if let Ok(mut mw) = mw.try_borrow_mut() {
                mw.disconnect_game();
            }
        });
        appwindow.add_action(&action);

        // Record show action
        let action = gio::SimpleAction::new("record_show", None);
        let mw = Rc::clone(mainwnd);
        action.connect_activate(move |_, _| {
            if let Ok(mw) = mw.try_borrow() {
                mw.record_show();
            }
        });
        appwindow.add_action(&action);

        // About action
        let action = gio::SimpleAction::new("about", None);
        let mw = Rc::clone(mainwnd);
        action.connect_activate(move |_, _| {
            if let Ok(mw) = mw.try_borrow() {
                mw.about();
            }
        });
        appwindow.add_action(&action);
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

        let win = gtk::Window::builder()
            .title("Game record")
            .transient_for(&self.appwindow)
            .modal(true)
            .default_width(300)
            .default_height(400)
            .build();

        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 8);
        vbox.set_margin_top(12);
        vbox.set_margin_bottom(12);
        vbox.set_margin_start(12);
        vbox.set_margin_end(12);

        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .vexpand(true)
            .build();

        let text_view = gtk::TextView::new();
        text_view.set_editable(false);
        text_view.set_cursor_visible(false);
        text_view.set_monospace(true);
        text_view.buffer().set_text(&log);
        scrolled.set_child(Some(&text_view));
        vbox.append(&scrolled);

        let button_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        button_box.set_halign(gtk::Align::End);
        let close_btn = gtk::Button::with_label("Close");
        button_box.append(&close_btn);
        vbox.append(&button_box);

        win.set_child(Some(&vbox));

        let win2 = win.clone();
        close_btn.connect_clicked(move |_| {
            win2.close();
        });

        win.show();
    }

    fn connect_game(&mut self) {
        let draw = Rc::clone(&self.draw);
        let game = Rc::clone(&self.game);
        let game_meta_view = Rc::clone(&self.game_meta_view);
        let game_meta_info_grid = self.game_meta_info_grid.clone();

        // Create a custom dialog window for server connection input.
        let win = gtk::Window::builder()
            .title("Connect to server")
            .transient_for(&self.appwindow)
            .modal(true)
            .default_width(400)
            .default_height(100)
            .build();

        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 8);
        vbox.set_margin_top(12);
        vbox.set_margin_bottom(12);
        vbox.set_margin_start(12);
        vbox.set_margin_end(12);

        let label = gtk::Label::new(Some(
            "Connect to a game server.\nEnter the server address here:",
        ));
        vbox.append(&label);

        let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        hbox.append(&gtk::Label::new(Some("Host:")));
        let entry_addr = gtk::Entry::new();
        entry_addr.set_hexpand(true);
        entry_addr.set_text("127.0.0.1");
        hbox.append(&entry_addr);

        hbox.append(&gtk::Label::new(Some("Port:")));
        let entry_port = gtk::Entry::new();
        entry_port.set_text("5596");
        hbox.append(&entry_port);
        vbox.append(&hbox);

        let button_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        button_box.set_halign(gtk::Align::End);
        let cancel_btn = gtk::Button::with_label("Cancel");
        let ok_btn = gtk::Button::with_label("OK");
        button_box.append(&cancel_btn);
        button_box.append(&ok_btn);
        vbox.append(&button_box);

        win.set_child(Some(&vbox));

        let win2 = win.clone();
        cancel_btn.connect_clicked(move |_| {
            win2.close();
        });

        let win2 = win.clone();
        ok_btn.connect_clicked(move |_| {
            let addr = format!(
                "{}:{}",
                entry_addr.text().as_str(),
                entry_port.text().as_str()
            );

            draw.borrow_mut().reset_game();

            let result = game.borrow_mut().client_connect(&addr);
            if let Err(e) = result {
                messagebox_error(Some(&win2), &format!("Failed to connect to server:\n{}", e));
            } else {
                game_meta_view.borrow_mut().clear_player_list();
                game_meta_info_grid.show();
            }
            win2.close();
        });

        win.present();
    }

    fn disconnect_game(&mut self) {
        self.game.borrow_mut().client_disconnect();

        self.draw.borrow_mut().reset_game();
        self.game_meta_view.borrow_mut().clear_player_list();
        self.game_meta_view.borrow_mut().clear_chat_messages();
        self.game_meta_info_grid.hide();

        self.update_status();
    }

    fn load_game(&mut self) {
        let dlg = gtk::FileChooserDialog::new(
            Some("Load game state"),
            Some(&self.appwindow),
            gtk::FileChooserAction::Open,
            &[
                ("_Cancel", gtk::ResponseType::Cancel),
                ("_Open", gtk::ResponseType::Accept),
            ],
        );

        let draw = Rc::clone(&self.draw);
        dlg.connect_response(move |dialog, response| {
            if response == gtk::ResponseType::Accept
                && let Some(file) = dialog.file()
                && let Some(path) = file.path()
                && let Err(e) = draw.borrow_mut().load_game(path.as_path())
            {
                messagebox_error(Some(dialog), &format!("Failed to load game:\n{}", e));
            }
            dialog.close();
        });
        dlg.show();
    }

    fn save_game(&self) {
        let dlg = gtk::FileChooserDialog::new(
            Some("Save game state"),
            Some(&self.appwindow),
            gtk::FileChooserAction::Save,
            &[
                ("_Cancel", gtk::ResponseType::Cancel),
                ("_Save", gtk::ResponseType::Accept),
            ],
        );

        let draw = Rc::clone(&self.draw);
        dlg.connect_response(move |dialog, response| {
            if response == gtk::ResponseType::Accept
                && let Some(file) = dialog.file()
                && let Some(path) = file.path()
                && let Err(e) = draw.borrow().save_game(path.as_path())
            {
                messagebox_error(Some(dialog), &format!("Failed to save game:\n{}", e));
            }
            dialog.close();
        });
        dlg.show();
    }
}

// vim: ts=4 sw=4 expandtab
