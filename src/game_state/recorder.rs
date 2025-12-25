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

use super::{MoveState, WinState};
use crate::board::{BOARD_HEIGHT, BOARD_WIDTH, coord_is_on_board};
use crate::coord;
use crate::coord::{Coord, CoordAxis};
use anyhow as ah;

const X_NAMES: [char; BOARD_WIDTH as usize] = ['a', 'b', 'c', 'd', 'e'];
const Y_NAMES: [char; BOARD_HEIGHT as usize] = ['7', '6', '5', '4', '3', '2', '1'];

fn coord_to_recorder_pos(pos: &Coord) -> String {
    format!("{}{}", X_NAMES[pos.x as usize], Y_NAMES[pos.y as usize])
}

fn recorder_pos_to_coord(chars: &[char]) -> ah::Result<Coord> {
    if chars.len() != 2 {
        return Err(ah::format_err!(
            "Recorder position: Invalid size ({} != 2).",
            chars.len()
        ));
    }
    let x_char: char = chars[0].to_lowercase().next().unwrap();
    let x = match X_NAMES.iter().position(|&x| x == x_char) {
        Some(x) => x,
        None => {
            return Err(ah::format_err!(
                "Recorder position: Invalid X coordinate: {}",
                chars[0]
            ));
        }
    };
    let y_char: char = chars[1].to_lowercase().next().unwrap();
    let y = match Y_NAMES.iter().position(|&y| y == y_char) {
        Some(y) => y,
        None => {
            return Err(ah::format_err!(
                "Recorder position: Invalid Y coordinate: {}",
                chars[1]
            ));
        }
    };
    let pos = coord!(x as CoordAxis, y as CoordAxis);
    if !coord_is_on_board(pos) {
        return Err(ah::format_err!(
            "Recorder position: Position {}{} is not on the board.",
            chars[0],
            chars[1]
        ));
    }
    Ok(pos)
}

/// One recorded game move.
pub struct RecordedMove {
    /// The token that has been moved from the given position.
    pub move_state: MoveState,
    /// The target position of the move.
    pub to_pos: Coord,
    /// True, if a token has been captured during this move.
    pub captured: bool,
    /// The game-win state after this move.
    pub win_state: WinState,
}

impl RecordedMove {
    /// Parse a move record line string.
    fn parse_log_line(line: &str) -> ah::Result<RecordedMove> {
        let chars: Vec<char> = line.chars().collect();
        let mut offset = 0;

        // Moving token ID.
        if chars[offset..].is_empty() {
            return Err(ah::format_err!("Recorder log: No token ID."));
        }
        let move_state_type = match chars[offset] {
            'W' | 'w' => MoveState::Wolf,
            'S' | 's' => MoveState::Sheep,
            other => {
                return Err(ah::format_err!("Recorder log: Invalid token ID: {}", other));
            }
        };
        offset += 1;

        // From-position.
        if chars[offset..].len() < 2 {
            return Err(ah::format_err!("Recorder log: No from-position."));
        }
        let from_pos = recorder_pos_to_coord(&chars[offset..offset + 2])?;
        let move_state = move_state_type(from_pos);
        offset += 2;

        // Move type.
        if chars[offset..].is_empty() {
            return Err(ah::format_err!("Recorder log: No move type."));
        }
        let (win_state, captured) = match chars[offset] {
            '-' => (WinState::Undecided, false),
            'X' | 'x' => (WinState::Undecided, true),
            '#' => match move_state {
                MoveState::Wolf(_) => (WinState::Wolf, true),
                MoveState::Sheep(_) => (WinState::Sheep, false),
                _ => return Err(ah::format_err!("Recorder log: Internal error")),
            },
            other => {
                return Err(ah::format_err!(
                    "Recorder log: Invalid move type: {}",
                    other
                ));
            }
        };
        offset += 1;

        // To-position.
        if chars[offset..].len() < 2 {
            return Err(ah::format_err!("Recorder log: No to-position."));
        }
        let to_pos = recorder_pos_to_coord(&chars[offset..offset + 2])?;

        Ok(RecordedMove {
            move_state,
            to_pos,
            captured,
            win_state,
        })
    }
}

impl std::fmt::Display for RecordedMove {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (from_type, from_pos) = match self.move_state {
            MoveState::NoMove => return write!(f, ""),
            MoveState::Wolf(from_pos) => ("W", from_pos),
            MoveState::Sheep(from_pos) => ("S", from_pos),
        };
        let move_type = match self.win_state {
            WinState::Undecided => {
                if self.captured {
                    "x"
                } else {
                    "-"
                }
            }
            WinState::Wolf | WinState::Sheep => "#",
        };
        write!(
            f,
            "{}{}{}{}",
            from_type,
            &coord_to_recorder_pos(&from_pos),
            move_type,
            &coord_to_recorder_pos(&self.to_pos)
        )
    }
}

pub struct Recorder {
    move_log: Vec<String>,
}

impl Recorder {
    pub fn new() -> Recorder {
        Recorder { move_log: vec![] }
    }

    pub fn reset(&mut self) {
        self.move_log.clear();
    }

    /// Add a move to the move record.
    pub fn record_move(&mut self, recorded_move: &RecordedMove) {
        self.move_log.push(recorded_move.to_string());
    }

    pub fn get_moves(&self) -> &Vec<String> {
        &self.move_log
    }

    pub fn get_moves_as_text(&self) -> String {
        self.get_moves().join("\n")
    }

    pub fn parse_text(&mut self, text: &str) -> ah::Result<()> {
        self.reset();
        for line in text.split("\n").map(|l| l.trim()) {
            if !line.is_empty() {
                let mov = RecordedMove::parse_log_line(line)?;
                self.move_log.push(mov.to_string());
            }
        }
        Ok(())
    }
}

// vim: ts=4 sw=4 expandtab
