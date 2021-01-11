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
use player_list::PlayerList;

mod drawing_area;
use drawing_area::DrawingArea;

use anyhow as ah;
use crate::game_state::GameState;
use crate::player::PlayerMode;
use expect_exit::exit_unwind;
use glib;
use gtk::prelude::*;
use gtk;
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
    mainwnd:    gtk::ApplicationWindow,
}

impl MainWindow {
    pub fn new(app:               &gtk::Application,
               connect_to_server: Option<String>,
               room_name:         String) -> ah::Result<MainWindow> {
        // Create main window.
        let glade_source = include_str!("main_window.glade");
        let builder = gtk::Builder::from_string(glade_source);
        let mainwnd: gtk::ApplicationWindow = builder.get_object("mainwindow").unwrap();
        mainwnd.set_application(Some(app));
        mainwnd.set_title("Wolfsmühle");

        // Create game state.
        let player_mode = PlayerMode::Both;
        //TODO
        let game = Rc::new(RefCell::new(GameState::new(player_mode,
                                                       connect_to_server,
                                                       room_name)?));

        // Create player state area.
        let player_tree: gtk::TreeView = builder.get_object("player_tree").unwrap();
        let player_list = PlayerList::new(player_tree);

        // Create drawing area.
        let draw = Rc::new(RefCell::new(DrawingArea::new(
            builder.get_object("drawing_area").unwrap(),
            Rc::clone(&game))));

        // Create game polling timer.
        let game2 = Rc::clone(&game);
        let draw2 = Rc::clone(&draw);
        glib::timeout_add_local(100, move || {
            if game2.borrow_mut().poll_server() {
                draw2.borrow().redraw();
            }
            glib::Continue(true)
        });

        // Connect signals.
        let draw2 = Rc::clone(&draw);
        let mainwnd2 = mainwnd.clone();
        builder.connect_signals(move |_builder, handler_name| {
            let mainwnd2 = mainwnd2.clone();
            match DrawingArea::connect_signals(Rc::clone(&draw2), handler_name) {
                Some(handler) => return handler,
                None => (),
            }
            match handler_name {
                "handler_about" => {
                    Box::new(move |_p| {
                        let msg = gtk::MessageDialog::new(Some(&mainwnd2),
                                                          gtk::DialogFlags::empty(),
                                                          gtk::MessageType::Info,
                                                          gtk::ButtonsType::Ok,
                                                          ABOUT_TEXT);
                        msg.connect_response(move |msg, _resp| msg.close());
                        msg.run();
                        None
                    })
                },
                "handler_quit" =>
                    Box::new(|_| exit_unwind(0)),
                _ =>
                    Box::new(|_| None)
            }
        });

        Ok(MainWindow {
            mainwnd,
        })
    }

    pub fn main_window(self) -> gtk::ApplicationWindow {
        self.mainwnd.clone()
    }
}

// vim: ts=4 sw=4 expandtab
