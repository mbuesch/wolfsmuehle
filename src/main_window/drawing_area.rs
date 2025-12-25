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

use crate::board::{BOARD_LINES, BoardIterator};
use crate::coord;
use crate::coord::{Coord, CoordAxis};
use crate::game_state::{FieldState, GameState, MoveState, WinState};
use crate::gtk_helpers::*;
use crate::print::Print;
use crate::{gsignal_connect_to, gsignal_connect_to_mut, gsigparam};
use anyhow as ah;
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

const DRAW_DEBUG: bool = false;
const XOFFS: f64 = 50.0;
const YOFFS: f64 = 50.0;
const POSDIST: f64 = 100.0;

/// Convert board coordinates to pixel coodrinates.
fn pos2pix(coord: &Coord) -> (f64, f64) {
    (
        coord.x as f64 * POSDIST + XOFFS,
        coord.y as f64 * POSDIST + YOFFS,
    )
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
    if (xdev < maxdev || xdev > 1.0 - maxdev) && (ydev < maxdev || ydev > 1.0 - maxdev) {
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

#[derive(Copy, Clone, PartialEq, Debug)]
enum MovingToken {
    NoToken,
    Wolf(f64, f64),
    Sheep(f64, f64),
}

fn conv_xpm(data: &str) -> ah::Result<Vec<&str>> {
    let mut ret = vec![];
    for line in data.split('\n') {
        if line.starts_with("\"") {
            let start = line
                .find("\"")
                .ok_or(ah::format_err!("conv_xpm: Start not found."))?;
            let end = line
                .rfind("\"")
                .ok_or(ah::format_err!("conv_xpm: End not found."))?;
            ret.push(&line[start + 1..end]);
        }
    }
    Ok(ret)
}

pub struct DrawingArea {
    widget: gtk::DrawingArea,
    wolf_pixbuf: gdk_pixbuf::Pixbuf,
    wolf_moving_pixbuf: gdk_pixbuf::Pixbuf,
    sheep_pixbuf: gdk_pixbuf::Pixbuf,
    sheep_moving_pixbuf: gdk_pixbuf::Pixbuf,
    game: Rc<RefCell<GameState>>,
    pending_join: bool,
    moving_token: MovingToken,
}

impl DrawingArea {
    pub fn new(widget: gtk::DrawingArea, game: Rc<RefCell<GameState>>) -> ah::Result<DrawingArea> {
        widget.add_events(
            gdk::EventMask::POINTER_MOTION_MASK
                | gdk::EventMask::POINTER_MOTION_HINT_MASK
                | gdk::EventMask::BUTTON_MOTION_MASK
                | gdk::EventMask::BUTTON_PRESS_MASK
                | gdk::EventMask::BUTTON_RELEASE_MASK,
        );

        let wolf_xpm = conv_xpm(include_str!("wolf.xpm"))?;
        let sheep_xpm = conv_xpm(include_str!("sheep.xpm"))?;

        let wolf_pixbuf = gdk_pixbuf::Pixbuf::from_xpm_data(wolf_xpm.as_slice())
            .scale_simple(70, 70, gdk_pixbuf::InterpType::Hyper)
            .ok_or(ah::format_err!("Failed to scale wolf image."))?;
        let wolf_moving_pixbuf = wolf_pixbuf
            .copy()
            .ok_or(ah::format_err!("Failed to copy image."))?;
        wolf_moving_pixbuf.saturate_and_pixelate(&wolf_moving_pixbuf, 0.0, true);

        let sheep_pixbuf = gdk_pixbuf::Pixbuf::from_xpm_data(sheep_xpm.as_slice())
            .scale_simple(50, 50, gdk_pixbuf::InterpType::Hyper)
            .ok_or(ah::format_err!("Failed to scale sheep image."))?;
        let sheep_moving_pixbuf = sheep_pixbuf
            .copy()
            .ok_or(ah::format_err!("Failed to copy image."))?;
        sheep_moving_pixbuf.saturate_and_pixelate(&sheep_moving_pixbuf, 0.0, true);

        Ok(DrawingArea {
            widget,
            wolf_pixbuf,
            wolf_moving_pixbuf,
            sheep_pixbuf,
            sheep_moving_pixbuf,
            game,
            pending_join: false,
            moving_token: MovingToken::NoToken,
        })
    }

    pub fn redraw(&self) {
        self.widget.queue_draw();
    }

    pub fn set_pending_join(&mut self, pending_join: bool) {
        if pending_join != self.pending_join {
            self.pending_join = pending_join;
            self.redraw();
        }
    }

    fn draw_background(&self, cairo: &cairo::Context) {
        // Draw background.
        cairo.set_source_rgb(0.3, 0.44, 0.22);
        cairo.set_line_width(0.0);
        cairo.rectangle(
            0.0,
            0.0,
            self.widget.allocated_width() as f64,
            self.widget.allocated_height() as f64,
        );
        cairo.fill().ok();

        // Draw sky.
        cairo.set_source_rgb(0.0, 0.49, 0.69);
        let pos = pos2pix(&coord!(0, 2));
        cairo.rectangle(0.0, 0.0, self.widget.allocated_width() as f64, pos.1);
        cairo.fill().ok();

        // Draw barn.
        cairo.set_source_rgb(0.35, 0.22, 0.15);
        let pos = pos2pix(&coord!(2, 0));
        cairo.move_to(pos.0, pos.1);
        let pos = pos2pix(&coord!(3, 1));
        cairo.line_to(pos.0, pos.1);
        let pos = pos2pix(&coord!(3, 2));
        cairo.line_to(pos.0, pos.1);
        let pos = pos2pix(&coord!(1, 2));
        cairo.line_to(pos.0, pos.1);
        let pos = pos2pix(&coord!(1, 1));
        cairo.line_to(pos.0, pos.1);
        let pos = pos2pix(&coord!(2, 0));
        cairo.line_to(pos.0, pos.1);
        cairo.fill().ok();
        let pos = pos2pix(&coord!(0, 2));
        cairo.move_to(pos.0, pos.1);
        let pos = pos2pix(&coord!(4, 2));
        cairo.line_to(pos.0, pos.1);
        cairo.line_to(pos.0, pos.1 + POSDIST * 0.20);
        let pos = pos2pix(&coord!(0, 2));
        cairo.line_to(pos.0, pos.1 + POSDIST * 0.20);
        cairo.line_to(pos.0, pos.1);
        cairo.fill().ok();
    }

    fn draw_board_lines(&self, cairo: &cairo::Context) {
        cairo.set_source_rgb(0.1, 0.1, 0.1);
        cairo.set_line_width(4.0);
        for (from, to) in BOARD_LINES.iter() {
            cairo.move_to(pos2pix(from).0, pos2pix(from).1);
            cairo.line_to(pos2pix(to).0, pos2pix(to).1);
        }
        cairo.stroke().ok();
    }

    fn draw_token(&self, cairo: &cairo::Context, pos: (f64, f64), pixbuf: &gdk_pixbuf::Pixbuf) {
        cairo.set_source_pixbuf(
            pixbuf,
            pos.0 - (pixbuf.width() / 2) as f64,
            pos.1 - (pixbuf.height() / 2) as f64,
        );
        cairo.paint().ok();
    }

    fn draw_token_wolf_pix(&self, cairo: &cairo::Context, pos: (f64, f64), moving: bool) {
        let pixbuf = if moving {
            &self.wolf_moving_pixbuf
        } else {
            &self.wolf_pixbuf
        };
        self.draw_token(cairo, pos, pixbuf);
    }

    fn draw_token_sheep_pix(&self, cairo: &cairo::Context, pos: (f64, f64), moving: bool) {
        let pixbuf = if moving {
            &self.sheep_moving_pixbuf
        } else {
            &self.sheep_pixbuf
        };
        self.draw_token(cairo, pos, pixbuf);
    }

    fn draw_token_wolf(&self, cairo: &cairo::Context, pos: Coord, moving: bool) {
        self.draw_token_wolf_pix(cairo, pos2pix(&pos), moving);
    }

    fn draw_token_sheep(&self, cairo: &cairo::Context, pos: Coord, moving: bool) {
        self.draw_token_sheep_pix(cairo, pos2pix(&pos), moving);
    }

    fn draw_tokens(&self, cairo: &cairo::Context) {
        if self.pending_join {
            return;
        }

        let game = self.game.borrow();

        // Draw the board tokens.
        for coord in BoardIterator::new() {
            match game.get_field_state(coord) {
                FieldState::Unused | FieldState::Empty => (),
                FieldState::Wolf => {
                    self.draw_token_wolf(cairo, coord, game.get_field_moving(coord))
                }
                FieldState::Sheep => {
                    self.draw_token_sheep(cairo, coord, game.get_field_moving(coord))
                }
            }
        }

        // Draw the captured tokens.
        let stats = game.get_stats();
        let mut y = 25.0;
        let x = self.widget.allocated_width() as f64 - 35.0;
        for _ in 0..stats.sheep_captured {
            self.draw_token_sheep_pix(cairo, (x, y), false);
            y += 20.0;
        }

        // Draw the moving token.
        match self.moving_token {
            MovingToken::NoToken => (),
            MovingToken::Wolf(x, y) => self.draw_token_wolf_pix(cairo, (x, y), false),
            MovingToken::Sheep(x, y) => self.draw_token_sheep_pix(cairo, (x, y), false),
        }
    }

    fn draw_game_state(&self, cairo: &cairo::Context) {
        let win_state = self.game.borrow().get_win_state();
        if win_state != WinState::Undecided {
            cairo.set_source_rgb(1.0, 0.0, 0.0);
            cairo.set_font_size(40.0);
            cairo.select_font_face("Serif", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
            let text = format!("{} won!", win_state);
            if let Ok(extents) = cairo.text_extents(&text) {
                cairo.move_to(
                    (self.widget.allocated_width() as f64 / 2.0) - (extents.width() / 2.0),
                    (self.widget.allocated_height() as f64 / 2.0) + (extents.height() / 2.0),
                );
                cairo.show_text(&text).ok();
            }
        }
    }

    fn draw(&self, cairo: cairo::Context) {
        if DRAW_DEBUG {
            Print::debug("Redrawing board.");
        }
        self.draw_background(&cairo);
        self.draw_board_lines(&cairo);
        self.draw_tokens(&cairo);
        self.draw_game_state(&cairo);
    }

    fn update_moving_token(&mut self, move_state: MoveState, x: f64, y: f64) {
        self.moving_token = match move_state {
            MoveState::NoMove => MovingToken::NoToken,
            MoveState::Wolf(_pos) => MovingToken::Wolf(x, y),
            MoveState::Sheep(_pos) => MovingToken::Sheep(x, y),
        };
    }

    fn mousemove(&mut self, x: f64, y: f64) {
        let was_moving = self.moving_token != MovingToken::NoToken;
        let move_state = self.game.borrow().get_move_state();
        self.update_moving_token(move_state, x, y);
        if was_moving || self.moving_token != MovingToken::NoToken {
            self.redraw();
        }
    }

    #[allow(clippy::single_match)]
    #[allow(clippy::collapsible_else_if)]
    fn mousebutton(&mut self, x: f64, y: f64, button: u32, press: bool) {
        match button {
            1 => {
                // left button
                {
                    let mut game = self.game.borrow_mut();
                    if let Some(pos) = pix2pos(x, y) {
                        if press {
                            if game.get_move_state() == MoveState::NoMove {
                                match game.get_field_state(pos) {
                                    FieldState::Unused | FieldState::Empty => (),
                                    FieldState::Wolf | FieldState::Sheep => {
                                        game.move_pick(pos).ok();
                                    }
                                }
                            }
                        } else {
                            if game.get_move_state() != MoveState::NoMove {
                                match game.get_field_state(pos) {
                                    FieldState::Empty => {
                                        if game.move_put(pos).is_err() {
                                            game.move_abort();
                                        }
                                    }
                                    FieldState::Unused => {
                                        game.move_abort();
                                    }
                                    FieldState::Wolf | FieldState::Sheep => {
                                        game.move_abort();
                                    }
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
            }
            _ => (),
        };
    }

    pub fn reset_game(&mut self) {
        self.game.borrow_mut().reset_game(false);
        self.moving_token = MovingToken::NoToken;
        self.redraw();
    }

    pub fn load_game(&mut self, filename: &Path) -> ah::Result<()> {
        self.game.borrow_mut().load_game(filename)
    }

    pub fn save_game(&self, filename: &Path) -> ah::Result<()> {
        self.game.borrow().save_game(filename)
    }

    fn gsignal_draw(&self, param: &[glib::Value]) -> Option<glib::Value> {
        let _widget = gsigparam!(param[0], gtk::DrawingArea);
        let cairo = gsigparam!(param[1], cairo::Context);
        self.draw(cairo);
        Some(false.to_value())
    }

    fn gsignal_motionnotify(&mut self, param: &[glib::Value]) -> Option<glib::Value> {
        let _widget = gsigparam!(param[0], gtk::DrawingArea);
        let event = gsigparam!(param[1], gdk::Event);
        let (x, y) = event.coords().unwrap();
        self.mousemove(x, y);
        Some(false.to_value())
    }

    fn gsignal_buttonpress(&mut self, param: &[glib::Value]) -> Option<glib::Value> {
        let _widget = gsigparam!(param[0], gtk::DrawingArea);
        let event = gsigparam!(param[1], gdk::Event);
        let (x, y) = event.coords().unwrap();
        self.mousebutton(x, y, event.button().unwrap(), true);
        Some(false.to_value())
    }

    fn gsignal_buttonrelease(&mut self, param: &[glib::Value]) -> Option<glib::Value> {
        let _widget = gsigparam!(param[0], gtk::DrawingArea);
        let event = gsigparam!(param[1], gdk::Event);
        let (x, y) = event.coords().unwrap();
        self.mousebutton(x, y, event.button().unwrap(), false);
        Some(false.to_value())
    }

    pub fn connect_signals(
        draw: Rc<RefCell<DrawingArea>>,
        handler_name: &str,
    ) -> Option<GSigHandler> {
        match handler_name {
            "handler_drawingarea_draw" => Some(gsignal_connect_to!(
                draw,
                gsignal_draw,
                Some(false.to_value())
            )),
            "handler_drawingarea_motionnotify" => Some(gsignal_connect_to_mut!(
                draw,
                gsignal_motionnotify,
                Some(false.to_value())
            )),
            "handler_drawingarea_buttonpress" => Some(gsignal_connect_to_mut!(
                draw,
                gsignal_buttonpress,
                Some(false.to_value())
            )),
            "handler_drawingarea_buttonrelease" => Some(gsignal_connect_to_mut!(
                draw,
                gsignal_buttonrelease,
                Some(false.to_value())
            )),
            _ => None,
        }
    }
}

// vim: ts=4 sw=4 expandtab
