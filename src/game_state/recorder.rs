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
use super::{MoveState, WinState};

const X_NAMES: [char; BOARD_WIDTH as usize] = ['a', 'b', 'c', 'd', 'e'];
const Y_NAMES: [char; BOARD_HEIGHT as usize] = ['7', '6', '5', '4', '3', '2', '1'];

fn coord_to_recorder_pos(pos: &Coord) -> String {
    format!("{}{}", X_NAMES[pos.x as usize], Y_NAMES[pos.y as usize])
}

/// One recorded game move.
pub struct RecordedMove {
    /// The token that has been moved from the given position.
    pub move_state:     MoveState,
    /// The target position of the move.
    pub to_pos:         Coord,
    /// True, if a token has been captured during this move.
    pub captured:       bool,
    /// The game-win state after this move.
    pub win_state:      WinState,
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

    /// Add a move to the move record.
    pub fn record_move(&mut self, recorded_move: &RecordedMove) {
        let (from_type, from_pos) = match recorded_move.move_state {
            MoveState::NoMove => return,
            MoveState::Wolf(from_pos) => ("W", from_pos),
            MoveState::Sheep(from_pos) => ("S", from_pos),
        };
        let move_type = match recorded_move.win_state {
            WinState::Undecided => if recorded_move.captured { "x" } else { "-" },
            WinState::Wolf | WinState::Sheep => "#",
        };
        self.move_log.push(format!("{}{}{}{}",
                from_type,
                &coord_to_recorder_pos(&from_pos),
                move_type,
                &coord_to_recorder_pos(&recorded_move.to_pos)));
    }

    pub fn get_recorded_moves(&self) -> &Vec<String> {
        &self.move_log
    }
}

// vim: ts=4 sw=4 expandtab
