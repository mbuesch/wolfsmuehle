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

use anyhow as ah;
use crate::board::{
    BOARD_LINES,
    BoardIterator,
};
use crate::coord::{
    Coord,
    CoordAxis,
};
use crate::coord;
use crate::game_state::{
    FieldState,
    GameState,
    MoveState,
};
use expect_exit::exit_unwind;
use gdk;
use gio::prelude::*;
use glib;
use gtk::prelude::*;
use gtk::{ButtonsType, DialogFlags, MessageType, MessageDialog};
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

const XOFFS: f64    = 25.0;
const YOFFS: f64    = 25.0;
const POSDIST: f64  = 100.0;

macro_rules! sigparam {
    ($param:expr, $type:ty) => {
        $param.get::<$type>().unwrap().unwrap()
    }
}

/// Convert board coordinates to pixel coodrinates.
fn pos2pix(coord: &Coord) -> (f64, f64) {
    (coord.x as f64 * POSDIST + XOFFS,
     coord.y as f64 * POSDIST + YOFFS)
}

/// Convert pixel coordinates to board coordinates.
fn pix2pos(x: f64, y: f64) -> Option<Coord> {
    let x = x - XOFFS;
    let y = y - YOFFS;
    let xpos = x / POSDIST;
    let ypos = y / POSDIST;
    let xdev = xpos.fract().abs();
    let ydev = ypos.fract().abs();
    let maxdev = 0.4;
    if (xdev < maxdev || xdev > 1.0 - maxdev) &&
       (ydev < maxdev || ydev > 1.0 - maxdev) {
        let xpos = if xdev > 1.0 - maxdev {
            xpos.trunc() as CoordAxis + 1
        } else {
            xpos.trunc() as CoordAxis
        };
        let ypos = if ydev > 1.0 - maxdev {
            ypos.trunc() as CoordAxis + 1
        } else {
            ypos.trunc() as CoordAxis
        };
        Some(coord!(xpos, ypos))
    } else {
        None
    }
}

#[derive(Copy, Clone, Debug)]
enum MovingToken {
    NoToken,
    Wolf(f64, f64),
    Sheep(f64, f64),
}

struct DrawingArea {
    widget:         gtk::DrawingArea,
    game:           Rc<RefCell<GameState>>,
    moving_token:   MovingToken,
}

impl DrawingArea {
    fn new(widget:  gtk::DrawingArea,
           game:    Rc<RefCell<GameState>>) -> DrawingArea {
        widget.add_events(gdk::EventMask::POINTER_MOTION_MASK |
                          gdk::EventMask::POINTER_MOTION_HINT_MASK |
                          gdk::EventMask::BUTTON_MOTION_MASK |
                          gdk::EventMask::BUTTON_PRESS_MASK |
                          gdk::EventMask::BUTTON_RELEASE_MASK);
        DrawingArea {
            widget,
            game,
            moving_token: MovingToken::NoToken,
        }
    }

    pub fn redraw(&self) {
        self.widget.queue_draw();
    }

    fn draw_background(&self, cairo: &cairo::Context) {
        cairo.set_source_rgb(0.1, 0.1, 0.1);
        cairo.set_line_width(0.0);
        cairo.rectangle(0.0,
                        0.0,
                        self.widget.get_allocated_width() as f64,
                        self.widget.get_allocated_height() as f64);
        cairo.fill();
    }

    fn draw_board_lines(&self, cairo: &cairo::Context) {
        cairo.set_source_rgb(0.75, 0.75, 0.75);
        cairo.set_line_width(4.0);
        for (from, to) in BOARD_LINES.iter() {
            cairo.move_to(pos2pix(from).0, pos2pix(from).1);
            cairo.line_to(pos2pix(to).0, pos2pix(to).1);
        }
        cairo.stroke();
    }

    fn draw_token(&self, cairo: &cairo::Context,
                  pos: (f64, f64),
                  color_background: (f64, f64, f64),
                  color_foreground: (f64, f64, f64),
                  text: &str,
                  moving: bool) {
        let fact = if moving { 0.25 } else { 1.0 };

        cairo.set_source_rgb(color_background.0 * fact,
                             color_background.1 * fact,
                             color_background.2 * fact);
        cairo.arc(pos.0, pos.1, 20.0, 0.0, 2.0 * std::f64::consts::PI);
        cairo.fill();
        cairo.set_line_width(1.0);
        cairo.set_source_rgb(0.0, 0.0, 0.0);
        cairo.arc(pos.0, pos.1, 20.0, 0.0, 2.0 * std::f64::consts::PI);
        cairo.stroke();

        cairo.set_source_rgb(color_foreground.0 * fact,
                             color_foreground.1 * fact,
                             color_foreground.2 * fact);
        cairo.set_font_size(16.0);
        cairo.select_font_face("Serif",
                               cairo::FontSlant::Normal,
                               cairo::FontWeight::Bold);
        let extents = cairo.text_extents(text);
        cairo.move_to(pos.0 - (extents.width / 2.0),
                      pos.1 + (extents.height / 2.0));
        cairo.show_text(text);
    }

    fn draw_token_wolf_pix(&self, cairo: &cairo::Context,
                           pos: (f64, f64), moving: bool) {
        self.draw_token(cairo,
                        pos,
                        (1.0, 1.0, 0.0),
                        (1.0, 0.0, 0.0),
                        "[W]",
                        moving);
    }

    fn draw_token_sheep_pix(&self, cairo: &cairo::Context,
                            pos: (f64, f64), moving: bool) {
        self.draw_token(cairo,
                        pos,
                        (1.0, 1.0, 1.0),
                        (0.0, 0.0, 1.0),
                        "[S]",
                        moving);
    }

    fn draw_token_wolf(&self, cairo: &cairo::Context,
                       pos: Coord, moving: bool) {
        self.draw_token_wolf_pix(cairo, pos2pix(&pos), moving);
    }

    fn draw_token_sheep(&self, cairo: &cairo::Context,
                        pos: Coord, moving: bool) {
        self.draw_token_sheep_pix(cairo, pos2pix(&pos), moving);
    }

    fn draw_tokens(&self, cairo: &cairo::Context) {
        let game = self.game.borrow();

        // Draw the board tokens.
        for coord in BoardIterator::new() {
            match game.get_field_state(coord) {
                FieldState::Unused |
                FieldState::Empty => (),
                FieldState::Wolf =>
                    self.draw_token_wolf(cairo, coord,
                                         game.get_field_moving(coord)),
                FieldState::Sheep =>
                    self.draw_token_sheep(cairo, coord,
                                          game.get_field_moving(coord)),
            }
        }

        // Draw the beaten tokens.
        let stats = game.get_stats();
        let mut y = 25.0;
        let x = self.widget.get_allocated_width() as f64 - 25.0;
        for _ in 0..stats.sheep_beaten {
            self.draw_token_sheep_pix(cairo, (x, y), false);
            y += 10.0;
        }

        // Draw the moving token.
        match self.moving_token {
            MovingToken::NoToken => (),
            MovingToken::Wolf(x, y) =>
                self.draw_token_wolf_pix(cairo, (x, y), false),
            MovingToken::Sheep(x, y) =>
                self.draw_token_sheep_pix(cairo, (x, y), false),
        }
    }

    fn draw(&self, cairo: cairo::Context) {
        self.draw_background(&cairo);
        self.draw_board_lines(&cairo);
        self.draw_tokens(&cairo);
    }

    fn update_moving_token(&mut self, move_state: MoveState, x: f64, y: f64) {
        self.moving_token = match move_state {
            MoveState::NoMove => MovingToken::NoToken,
            MoveState::Wolf(_pos) => MovingToken::Wolf(x, y),
            MoveState::Sheep(_pos) => MovingToken::Sheep(x, y),
        };
    }

    fn mousemove(&mut self, x: f64, y: f64) {
        let move_state = self.game.borrow().get_move_state();
        self.update_moving_token(move_state, x, y);
        if move_state != MoveState::NoMove {
            self.redraw();
        }
    }

    fn mousebutton(&mut self, x: f64, y: f64, button: u32, press: bool) {
        match button {
            1 => { // left button
                {
                    let mut game = self.game.borrow_mut();
                    if let Some(pos) = pix2pos(x, y) {
                        if press {
                            if game.get_move_state() == MoveState::NoMove {
                                match game.get_field_state(pos) {
                                    FieldState::Unused | FieldState::Empty => (),
                                    FieldState::Wolf |
                                    FieldState::Sheep => {
                                        game.move_pick(pos).ok();
                                    },
                                }
                            }
                        } else {
                            if game.get_move_state() != MoveState::NoMove {
                                match game.get_field_state(pos) {
                                    FieldState::Empty => {
                                        if let Err(_) = game.move_put(pos) {
                                            game.move_abort();
                                        }
                                    },
                                    FieldState::Unused => {
                                        game.move_abort();
                                    },
                                    FieldState::Wolf |
                                    FieldState::Sheep => {
                                        game.move_abort();
                                    },
                                }
                            }
                        }
                    } else {
                        game.move_abort();
                    }
                }
                let move_state = self.game.borrow().get_move_state();
                self.update_moving_token(move_state, x, y);
                self.redraw();
            },
            _ => (),
        };
    }

    fn reset_game(&mut self) {
        let mut game = self.game.borrow_mut();
        game.reset_game();
        self.moving_token = MovingToken::NoToken;
        self.redraw();
    }

    fn gsignal_draw(&self, param: &[glib::Value]) -> Option<glib::Value> {
        let _widget = sigparam!(param[0], gtk::DrawingArea);
        let cairo = sigparam!(param[1], cairo::Context);
        self.draw(cairo);
        Some(false.to_value())
    }

    fn gsignal_motionnotify(&mut self, param: &[glib::Value]) -> Option<glib::Value> {
        let _widget = sigparam!(param[0], gtk::DrawingArea);
        let event = sigparam!(param[1], gdk::Event);
        let (x, y) = event.get_coords().unwrap();
        self.mousemove(x, y);
        Some(false.to_value())
    }

    fn gsignal_buttonpress(&mut self, param: &[glib::Value]) -> Option<glib::Value> {
        let _widget = sigparam!(param[0], gtk::DrawingArea);
        let event = sigparam!(param[1], gdk::Event);
        let (x, y) = event.get_coords().unwrap();
        self.mousebutton(x, y, event.get_button().unwrap(), true);
        Some(false.to_value())
    }

    fn gsignal_buttonrelease(&mut self, param: &[glib::Value]) -> Option<glib::Value> {
        let _widget = sigparam!(param[0], gtk::DrawingArea);
        let event = sigparam!(param[1], gdk::Event);
        let (x, y) = event.get_coords().unwrap();
        self.mousebutton(x, y, event.get_button().unwrap(), false);
        Some(false.to_value())
    }

    fn gsignal_newgame(&mut self, param: &[glib::Value]) -> Option<glib::Value> {
        let _menu_item = sigparam!(param[0], gtk::MenuItem);
        self.reset_game();
        None
    }
}

pub struct MainWindow {
    mainwnd:    gtk::ApplicationWindow,
    //draw:       Rc<RefCell<DrawingArea>>,
    //game:       Rc<RefCell<GameState>>,
}

impl MainWindow {
    pub fn new(app:               &gtk::Application,
               connect_to_server: Option<String>,
               room_name:         String) -> ah::Result<MainWindow> {
        let glade_source = include_str!("main_window.glade");
        let builder = gtk::Builder::from_string(glade_source);

        let mainwnd: gtk::ApplicationWindow = builder.get_object("mainwindow").unwrap();
        mainwnd.set_application(Some(app));
        mainwnd.set_title("Wolfsmühle");

        let game = Rc::new(RefCell::new(GameState::new(connect_to_server, room_name)?));

        let draw = Rc::new(RefCell::new(DrawingArea::new(
            builder.get_object("drawing_area").unwrap(),
            Rc::clone(&game))));

        let game2 = Rc::clone(&game);
        let draw2 = Rc::clone(&draw);
        glib::timeout_add_local(100, move || {
            if game2.borrow_mut().poll_server() {
                draw2.borrow().redraw();
            }
            glib::Continue(true)
        });

        let draw2 = Rc::clone(&draw);
        let mainwnd2 = mainwnd.clone();
        builder.connect_signals(move |_builder, handler_name| {
            let mainwnd2 = mainwnd2.clone();
            let draw2 = Rc::clone(&draw2);
            match handler_name {
                "handler_drawingarea_draw" =>
                    Box::new(move |p| draw2.borrow().gsignal_draw(p)),
                "handler_drawingarea_motionnotify" =>
                    Box::new(move |p| draw2.borrow_mut().gsignal_motionnotify(p)),
                "handler_drawingarea_buttonpress" =>
                    Box::new(move |p| draw2.borrow_mut().gsignal_buttonpress(p)),
                "handler_drawingarea_buttonrelease" =>
                    Box::new(move |p| draw2.borrow_mut().gsignal_buttonrelease(p)),
                "handler_newgame" =>
                    Box::new(move |p| draw2.borrow_mut().gsignal_newgame(p)),
                "handler_about" => {
                    Box::new(move |_p| {
                        let msg = MessageDialog::new(Some(&mainwnd2),
                                                     DialogFlags::empty(),
                                                     MessageType::Info,
                                                     ButtonsType::Ok,
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
            //draw,
            //game,
        })
    }

    pub fn main_window(self) -> gtk::ApplicationWindow {
        self.mainwnd.clone()
    }
}

// vim: ts=4 sw=4 expandtab
