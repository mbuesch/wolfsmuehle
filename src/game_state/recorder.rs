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

use crate::board::{BOARD_WIDTH, BOARD_HEIGHT};
use crate::coord::Coord;
use super::MoveState;

const X_NAMES: [&str; BOARD_WIDTH as usize] = ["a", "b", "c", "d", "e"];
const Y_NAMES: [&str; BOARD_HEIGHT as usize] = ["7", "6", "5", "4", "3", "2", "1"];

fn coord_to_recorder_pos(pos: &Coord) -> String {
    format!("{}{}", X_NAMES[pos.x as usize], Y_NAMES[pos.y as usize])
}

pub struct Recorder {
    move_log: Vec<String>,
}

impl Recorder {
    pub fn new() -> Recorder {
        Recorder {
            move_log: vec![],
        }
    }

    pub fn reset(&mut self) {
        self.move_log.clear();
    }

    fn do_record_move(&mut self,
                      from_type: &str,
                      from_pos: &Coord,
                      captured: bool,
                      to_pos: &Coord) {
        self.move_log.push(format!("{}{}{}{}",
                from_type,
                &coord_to_recorder_pos(from_pos),
                if captured { "x" } else { "-" },
                &coord_to_recorder_pos(to_pos)));
    }

    pub fn record_move(&mut self,
                       move_state: &MoveState,
                       to_pos: &Coord,
                       captured: bool) {
        match move_state {
            MoveState::NoMove => (),
            MoveState::Wolf(from_pos) => {
                self.do_record_move("W", &from_pos, captured, to_pos);
            },
            MoveState::Sheep(from_pos) => {
                self.do_record_move("S", &from_pos, captured, to_pos);
            },
        }
    }

    pub fn get_recorded_moves(&self) -> &Vec<String> {
        &self.move_log
    }
}

// vim: ts=4 sw=4 expandtab
