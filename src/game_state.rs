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
    BOARD_HEIGHT,
    BOARD_POSITIONS,
    BOARD_WIDTH,
    BoardIterator,
    PosType,
    coord_is_on_board,
    is_on_main_diag,
};
use crate::coord::{
    Coord,
    CoordAxis,
};
use crate::coord;

const PRINT_STATE: bool = true;

const BEAT_OFFSETS: [Coord; 8] = [
    coord!(-2, 0),
    coord!(-2, -2),
    coord!(0, -2),
    coord!(2, -2),
    coord!(2, 0),
    coord!(2, 2),
    coord!(0, 2),
    coord!(-2, 2),
];

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum FieldState {
    Unused,
    Empty,
    Wolf,
    Sheep,
}

macro_rules! unused { () => { FieldState::Unused } }
macro_rules! empty { () => { FieldState::Empty } }
macro_rules! wolf { () => { FieldState::Wolf } }
macro_rules! sheep { () => { FieldState::Sheep } }

const INITIAL_STATE: [[FieldState; BOARD_WIDTH as usize]; BOARD_HEIGHT as usize] = [
    [unused!(), unused!(), empty!(), unused!(), unused!(), ],
    [unused!(), empty!(),  empty!(), empty!(),  unused!(), ],
    [empty!(),  wolf!(),   empty!(), wolf!(),   empty!(),  ],
    [empty!(),  empty!(),  empty!(), empty!(),  empty!(),  ],
    [sheep!(),  sheep!(),  sheep!(), sheep!(),  sheep!(),  ],
    [sheep!(),  sheep!(),  sheep!(), sheep!(),  sheep!(),  ],
    [sheep!(),  sheep!(),  sheep!(), sheep!(),  sheep!(),  ],
];

pub fn is_opposite_token(a: FieldState, b: FieldState) -> bool {
    (a == FieldState::Sheep && b == FieldState::Wolf) ||
    (a == FieldState::Wolf  && b == FieldState::Sheep)
}

fn print_state(msg: &str) {
    if PRINT_STATE {
        println!("{}", msg);
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum MoveState {
    NoMove,
    Wolf(Coord),
    Sheep(Coord),
}

#[derive(PartialEq, Debug)]
enum ValidationResult {
    Invalid,
    Valid,
    ValidBeat(Coord),
}

#[derive(Copy, Clone, PartialEq, Debug)]
enum Turn {
    Sheep,
    WolfchainOrSheep,
    Wolf,
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Stats {
    pub wolves:         u8,
    pub sheep:          u8,
    pub sheep_beaten:   u8,
}

pub struct GameState {
    fields:         [[FieldState; BOARD_WIDTH as usize]; BOARD_HEIGHT as usize],
    moving:         MoveState,
    stats:          Stats,
    turn:           Turn,
    just_beaten:    Option<Coord>,
}

impl GameState {
    /// Construct a new game state.
    pub fn new() -> GameState {
        let mut fields = [[FieldState::Unused; BOARD_WIDTH as usize]; BOARD_HEIGHT as usize];
        let mut stats = Stats {
            wolves: 0,
            sheep: 0,
            sheep_beaten: 0,
        };

        for coord in BoardIterator::new() {
            let x = coord.x as usize;
            let y = coord.y as usize;
            match BOARD_POSITIONS[y][x] {
                PosType::Invalid =>
                    (),
                PosType::Barn | PosType::Field => {
                    fields[y][x] = INITIAL_STATE[y][x];
                    match fields[y][x] {
                        FieldState::Wolf =>
                            stats.wolves += 1,
                        FieldState::Sheep =>
                            stats.sheep += 1,
                        FieldState::Unused | FieldState::Empty =>
                            (),
                    }
                },
            }
        }

        let game = GameState {
            fields,
            moving:         MoveState::NoMove,
            stats,
            turn:           Turn::Sheep,
            just_beaten:    None,
        };
        game.print_turn();
        game
    }

    /// Get statistics.
    pub fn get_stats(&self) -> Stats {
        self.stats
    }

    /// Set the state of a board field.
    fn set_field_state(&mut self, pos: Coord, state: FieldState) {
        if coord_is_on_board(pos) {
            self.fields[pos.y as usize][pos.x as usize] = state;
        }
    }

    /// Get the current state of a board field.
    pub fn get_field_state(&self, pos: Coord) -> FieldState {
        if coord_is_on_board(pos) {
            self.fields[pos.y as usize][pos.x as usize]
        } else {
            FieldState::Unused
        }
    }

    /// Get the moving state of a position.
    pub fn get_field_moving(&self, pos: Coord) -> bool {
        match self.get_move_state() {
            MoveState::NoMove => false,
            MoveState::Wolf(p) => p == pos,
            MoveState::Sheep(p) => p == pos,
        }
    }

    /// Get the global move status.
    pub fn get_move_state(&self) -> MoveState {
        self.moving
    }

    /// Beat one token at pos.
    fn beat(&mut self, _from_pos: Coord, to_pos: Coord, beat_pos: Coord) {
        match self.get_field_state(beat_pos) {
            FieldState::Unused | FieldState::Empty =>
                eprintln!("Internal error: Cannot beat empty fields."),
            FieldState::Wolf =>
                eprintln!("Internal error: Cannot beat wolves."),
            FieldState::Sheep => {
                self.stats.sheep -= 1;
                self.stats.sheep_beaten += 1;
                self.just_beaten = Some(to_pos);
                self.set_field_state(beat_pos, FieldState::Empty);
                print_state(&format!("Beaten sheep at {}", beat_pos));
            },
        }
    }

    /// Check if a move from from_pos to to_pos is valid.
    fn validate_move(&self, from_pos: Coord, to_pos: Coord) -> ValidationResult {
        // Check if positions are on the board.
        if !coord_is_on_board(from_pos) || !coord_is_on_board(to_pos) {
            return ValidationResult::Invalid;
        }

        // Check if from position has a token.
        let from_state = self.get_field_state(from_pos);
        match from_state {
            FieldState::Unused | FieldState::Empty =>
                return ValidationResult::Invalid,
            FieldState::Wolf | FieldState::Sheep =>
                (),
        }
        // Check if to position has no token.
        let to_state = self.get_field_state(to_pos);
        match to_state {
            FieldState::Unused | FieldState::Wolf | FieldState::Sheep =>
                return ValidationResult::Invalid,
            FieldState::Empty =>
                (),
        }

        let distx = to_pos.x as isize - from_pos.x as isize;
        let centerx = from_pos.x as isize + (distx / 2);
        let disty = to_pos.y as isize - from_pos.y as isize;
        let centery = from_pos.y as isize + (disty / 2);

        let center_pos = coord!(centerx as CoordAxis, centery as CoordAxis);
        let center_state = self.get_field_state(center_pos);

        let mut result = ValidationResult::Invalid;

        if from_state == FieldState::Sheep &&
           to_pos.y > from_pos.y {
            // Invalid sheep backward move.
        } else if from_pos.x != to_pos.x &&
                  from_pos.y != to_pos.y {
            // Diagonal move.
            if from_state == FieldState::Wolf {
                if is_on_main_diag(from_pos) && is_on_main_diag(to_pos) {
                    // Wolf diagonal move.
                    if distx.abs() == 1 && disty.abs() == 1 {
                        // Diagonal move by one field.
                        result = ValidationResult::Valid;
                    } else if distx.abs() == 2 && disty.abs() == 2 {
                        if is_opposite_token(from_state, center_state) {
                            // Beaten.
                            result = ValidationResult::ValidBeat(center_pos)
                        }
                    }
                } else if (from_pos.x == 1 && from_pos.y == 1 &&
                             to_pos.x == 2 &&   to_pos.y == 0) ||
                          (from_pos.x == 3 && from_pos.y == 1 &&
                             to_pos.x == 2 &&   to_pos.y == 0) ||
                          (from_pos.x == 2 && from_pos.y == 0 &&
                             to_pos.x == 1 &&   to_pos.y == 1) ||
                          (from_pos.x == 2 && from_pos.y == 0 &&
                             to_pos.x == 3 &&   to_pos.y == 1) {
                    // Wolf move to/from barn top.
                    result = ValidationResult::Valid;
                }
            } else if from_state == FieldState::Sheep &&
                      ((from_pos.x == 1 && from_pos.y == 1) ||
                       (from_pos.x == 3 && from_pos.y == 1)) {
                // Sheep move to barn top.
                result = ValidationResult::Valid;
            }
        } else if from_pos.y == to_pos.y {
            // Horizontal move.
            if distx.abs() == 1 {
                result = ValidationResult::Valid;
            } else if distx.abs() == 2 {
                if from_state == FieldState::Wolf &&
                   is_opposite_token(from_state, center_state) {
                    // Beaten.
                    result = ValidationResult::ValidBeat(center_pos)
                }
            }
        } else if from_pos.x == to_pos.x {
            // Vertical move.
            if disty.abs() == 1 {
                result = ValidationResult::Valid;
            } else if disty.abs() == 2 {
                if from_state == FieldState::Wolf &&
                   is_opposite_token(from_state, center_state) {
                    // Beaten.
                    result = ValidationResult::ValidBeat(center_pos)
                }
            }
        } else { // Can never happen.
            eprintln!("Internal error: validate_move() invalid state.");
        }

        // Check if this is our turn.
        match self.turn {
            Turn::Sheep => {
                if from_state != FieldState::Sheep {
                    return ValidationResult::Invalid;
                }
            },
            Turn::WolfchainOrSheep => {
                if from_state == FieldState::Wolf {
                    match result {
                        ValidationResult::Invalid |
                        ValidationResult::Valid => {
                            // Wolf chain jump is only valid,
                            // if it beats more sheep.
                            return ValidationResult::Invalid;
                        },
                        ValidationResult::ValidBeat(_) =>
                            (), // Ok
                    }
                }
            },
            Turn::Wolf => {
                if from_state != FieldState::Wolf {
                    return ValidationResult::Invalid;
                }
            },
        }

        result
    }

    fn print_turn(&self) {
        print_state(&format!("Next turn is: {:?}", self.turn));
    }

    fn next_turn(&mut self) {
        let calc_wolf_turn = || {
            // The next turn is sheep, except if a wolf has just beaten a sheep
            // and it can beat another one.
            if let Some(wolf_pos) = self.just_beaten {
                for offset in &BEAT_OFFSETS {
                    let to_pos = wolf_pos + *offset;
                    match self.validate_move(wolf_pos, to_pos) {
                        ValidationResult::ValidBeat(_) => {
                            print_state("Wolf can beat more sheep.");
                            return Turn::WolfchainOrSheep;
                        },
                        ValidationResult::Invalid | ValidationResult::Valid =>
                            (),
                    }
                }
            }
            Turn::Sheep
        };

        match self.turn {
            Turn::Sheep =>
                self.turn = Turn::Wolf,
            Turn::WolfchainOrSheep => {
                match self.moving {
                    MoveState::NoMove =>
                        eprintln!("Internal error: next_turn() no move."),
                    MoveState::Wolf(_) =>
                        self.turn = calc_wolf_turn(),
                    MoveState::Sheep(_) =>
                        self.turn = Turn::Wolf,
                }
            },
            Turn::Wolf =>
                self.turn = calc_wolf_turn(),
        }
        self.just_beaten = None;
        self.print_turn();
    }

    /// Start a move operation.
    pub fn move_pick(&mut self, pos: Coord) -> ah::Result<()> {
        if pos.x >= BOARD_WIDTH || pos.y >= BOARD_HEIGHT {
            return Err(ah::format_err!("move_pick: Coordinates out of bounds."));
        }
        if self.moving != MoveState::NoMove {
            return Err(ah::format_err!("move_pick: Already moving."))
        }

        match self.get_field_state(pos) {
            FieldState::Unused | FieldState::Empty => {
                Err(ah::format_err!("move_pick: Move from empty field."))
            },
            FieldState::Wolf => {
                self.moving = MoveState::Wolf(pos);
                self.set_field_state(pos, FieldState::Wolf);
                Ok(())
            },
            FieldState::Sheep => {
                self.moving = MoveState::Sheep(pos);
                self.set_field_state(pos, FieldState::Sheep);
                Ok(())
            },
        }
    }

    fn do_move_place(&mut self, pos: Coord) {
        match self.moving {
            MoveState::NoMove =>
                eprintln!("Internal error: Invalid move source."),
            MoveState::Wolf(from_pos) => {
                self.set_field_state(pos, FieldState::Wolf);
                self.set_field_state(from_pos, FieldState::Empty);
            },
            MoveState::Sheep(from_pos) => {
                self.set_field_state(pos, FieldState::Sheep);
                self.set_field_state(from_pos, FieldState::Empty);
            },
        }
        self.next_turn();
        self.moving = MoveState::NoMove;
    }

    /// End a move operation.
    pub fn move_place(&mut self, pos: Coord) -> ah::Result<()> {
        if pos.x >= BOARD_WIDTH || pos.y >= BOARD_HEIGHT {
            return Err(ah::format_err!("move_pick: Coordinates out of bounds."));
        }
        let from_pos = match self.moving {
            MoveState::NoMove =>
                return Err(ah::format_err!("move_place: Not moving.")),
            MoveState::Wolf(p) => p,
            MoveState::Sheep(p) => p,
        };

        match self.get_field_state(pos) {
            FieldState::Unused |
            FieldState::Wolf |
            FieldState::Sheep => {
                Err(ah::format_err!("move_place: Field occupied."))
            },
            FieldState::Empty => {
                match self.validate_move(from_pos, pos) {
                    ValidationResult::Invalid =>
                        Err(ah::format_err!("move_place: Invalid move.")),
                    ValidationResult::Valid => {
                        self.do_move_place(pos);
                        Ok(())
                    },
                    ValidationResult::ValidBeat(beat_pos) => {
                        self.beat(from_pos, pos, beat_pos);
                        self.do_move_place(pos);
                        Ok(())
                    },
                }
            },
        }
    }

    /// Abort a move operation.
    pub fn move_abort(&mut self) {
        if self.moving != MoveState::NoMove {
            self.moving = MoveState::NoMove;
        }
    }
}

// vim: ts=4 sw=4 expandtab
