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

mod player_list;
use player_list::PlayerListView;

mod drawing_area;
use drawing_area::DrawingArea;

use anyhow as ah;
use crate::game_state::GameState;
use crate::gsigparam;
use crate::gtk_helpers::*;
use crate::player::PlayerMode;
use expect_exit::exit_unwind;
use std::cell::RefCell;
use std::rc::Rc;

const ABOUT_TEXT: &str =
    "Wolfsmühle - Board game\n\
     \n\
     Copyright 2021 Michael Buesch <m@bues.ch>\n\
     \n\
     This program is free software; you can redistribute it and/or modify\n\
     it under the terms of the GNU General Public License as published by\n\
     the Free Software Foundation; either version 2 of the License, or\n\
     (at your option) any later version.\n\
     \n\
     This program is distributed in the hope that it will be useful,\n\
     but WITHOUT ANY WARRANTY; without even the implied warranty of\n\
     MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the\n\
     GNU General Public License for more details.\n\
     \n\
     You should have received a copy of the GNU General Public License along\n\
     with this program; if not, write to the Free Software Foundation, Inc.,\n\
     51 Franklin Street, Fifth Floor, Boston, MA 02110-1301 USA.";

pub struct MainWindow {
    mainwnd:            gtk::ApplicationWindow,
    draw:               Rc<RefCell<DrawingArea>>,
    game:               Rc<RefCell<GameState>>,
    player_list_view:   Rc<RefCell<PlayerListView>>,
}

impl MainWindow {
    pub fn new(app:               &gtk::Application,
               connect_to_server: Option<String>,
               room_name:         String,
               player_name:       Option<String>,
               player_mode:       PlayerMode)
               -> ah::Result<Rc<RefCell<MainWindow>>> {
        // Create main window.
        let glade_source = include_str!("main_window.glade");
        let builder = gtk::Builder::from_string(glade_source);
        let mainwnd: gtk::ApplicationWindow = builder.get_object("mainwindow").unwrap();
        mainwnd.set_application(Some(app));
        mainwnd.set_title("Wolfsmühle");

        // Create game state.
        let game = Rc::new(RefCell::new(GameState::new(player_mode,
                                                       player_name,
                                                       connect_to_server,
                                                       room_name)?));

        // Create player state area.
        let player_tree: gtk::TreeView = builder.get_object("player_tree").unwrap();
        let player_list_view = Rc::new(RefCell::new(
            PlayerListView::new(player_tree)));

        // Create drawing area.
        let draw = Rc::new(RefCell::new(DrawingArea::new(
            builder.get_object("drawing_area").unwrap(),
            Rc::clone(&game))));

        let ret = Rc::new(RefCell::new(MainWindow {
            mainwnd,
            draw,
            game,
            player_list_view,
        }));

        // Create game polling timer.
        let ret2 = Rc::clone(&ret);
        glib::timeout_add_local(100, move || {
            if let Ok(mut mw) = ret2.try_borrow_mut() {
                mw.poll_timer();
            }
            glib::Continue(true)
        });

        // Connect signals.
        let ret2 = Rc::clone(&ret);
        let draw2 = Rc::clone(&ret.borrow().draw);
        builder.connect_signals(move |_builder, handler_name| {
            let ret2 = Rc::clone(&ret2);
            match DrawingArea::connect_signals(Rc::clone(&draw2), handler_name) {
                Some(handler) => return handler,
                None => (),
            }
            match handler_name {
                "handler_resetgame" =>
                    Box::new(move |p| ret2.borrow_mut().gsignal_resetgame(p)),
                "handler_loadgame" =>
                    Box::new(move |p| ret2.borrow_mut().gsignal_loadgame(p)),
                "handler_savegame" =>
                    Box::new(move |p| ret2.borrow_mut().gsignal_savegame(p)),
                "handler_about" =>
                    Box::new(move |p| ret2.borrow_mut().gsignal_about(p)),
                "handler_quit" =>
                    Box::new(move |p| ret2.borrow_mut().gsignal_quit(p)),
                _ =>
                    Box::new(|_| None)
            }
        });

        Ok(ret)
    }

    fn poll_timer(&mut self) {
        if let Ok(draw) = self.draw.try_borrow() {
            if let Ok(mut game) = self.game.try_borrow_mut() {
                if let Ok(mut player_list_view) = self.player_list_view.try_borrow_mut() {

                    let redraw = game.poll_server();
                    player_list_view.update(game.get_room_player_list());
                    if redraw {
                        draw.redraw();
                    }
                }
            }
        }
    }

    pub fn main_window(&self) -> gtk::ApplicationWindow {
        self.mainwnd.clone()
    }

    fn load_game(&mut self) {
        let mut err = None;
        let dlg = gtk::FileChooserDialog::with_buttons(
            Some("Load game state"),
            Some(&self.mainwnd),
            gtk::FileChooserAction::Open,
            &[("_Cancel", gtk::ResponseType::Cancel), ("_Open", gtk::ResponseType::Accept)]);
        if dlg.run() == gtk::ResponseType::Accept {
            if let Some(filename) = dlg.get_filename() {
                if let Err(e) = self.draw.borrow_mut().load_game(filename.as_path()) {
                    err = Some(e);
                }
            }
        }
        if let Some(e) = err {
            let text = format!("Failed to load game:\n{}", e);
            let msg = gtk::MessageDialog::new(Some(&dlg),
                                              gtk::DialogFlags::empty(),
                                              gtk::MessageType::Error,
                                              gtk::ButtonsType::Ok,
                                              &text);
            msg.connect_response(move |msg, _resp| msg.close());
            msg.run();
        }
        dlg.close();
    }

    fn save_game(&self) {
        let mut err = None;
        let dlg = gtk::FileChooserDialog::with_buttons(
            Some("Save game state"),
            Some(&self.mainwnd),
            gtk::FileChooserAction::Save,
            &[("_Cancel", gtk::ResponseType::Cancel), ("_Save", gtk::ResponseType::Accept)]);
        if dlg.run() == gtk::ResponseType::Accept {
            if let Some(filename) = dlg.get_filename() {
                if let Err(e) = self.draw.borrow().save_game(filename.as_path()) {
                    err = Some(e);
                }
            }
        }
        if let Some(e) = err {
            let text = format!("Failed to save game:\n{}", e);
            let msg = gtk::MessageDialog::new(Some(&dlg),
                                              gtk::DialogFlags::empty(),
                                              gtk::MessageType::Error,
                                              gtk::ButtonsType::Ok,
                                              &text);
            msg.connect_response(move |msg, _resp| msg.close());
            msg.run();
        }
        dlg.close();
    }

    fn gsignal_quit(&mut self, param: &[glib::Value]) -> Option<glib::Value> {
        let _menu_item = gsigparam!(param[0], gtk::MenuItem);
        exit_unwind(0);
    }

    fn gsignal_about(&mut self, param: &[glib::Value]) -> Option<glib::Value> {
        let _menu_item = gsigparam!(param[0], gtk::MenuItem);
        let msg = gtk::MessageDialog::new(Some(&self.mainwnd),
                                          gtk::DialogFlags::empty(),
                                          gtk::MessageType::Info,
                                          gtk::ButtonsType::Ok,
                                          ABOUT_TEXT);
        msg.connect_response(move |msg, _resp| msg.close());
        msg.run();
        None
    }

    fn gsignal_resetgame(&mut self, param: &[glib::Value]) -> Option<glib::Value> {
        let _menu_item = gsigparam!(param[0], gtk::MenuItem);
        self.draw.borrow_mut().reset_game();
        None
    }

    fn gsignal_loadgame(&mut self, param: &[glib::Value]) -> Option<glib::Value> {
        let _menu_item = gsigparam!(param[0], gtk::MenuItem);
        self.load_game();
        None
    }

    fn gsignal_savegame(&mut self, param: &[glib::Value]) -> Option<glib::Value> {
        let _menu_item = gsigparam!(param[0], gtk::MenuItem);
        self.save_game();
        None
    }
}

// vim: ts=4 sw=4 expandtab
