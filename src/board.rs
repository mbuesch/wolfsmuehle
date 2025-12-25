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

use crate::coord;
use crate::coord::{Coord, CoordAxis};

pub const BOARD_WIDTH: CoordAxis = 5;
pub const BOARD_HEIGHT: CoordAxis = 7;

#[rustfmt::skip]
pub const BOARD_LINES: [(Coord, Coord); 15] = [
    // Vertical lines
    (coord!(0, 2), coord!(0, 6)),
    (coord!(1, 1), coord!(1, 6)),
    (coord!(2, 0), coord!(2, 6)),
    (coord!(3, 1), coord!(3, 6)),
    (coord!(4, 2), coord!(4, 6)),
    // Horizontal lines
    (coord!(1, 1), coord!(3, 1)),
    (coord!(0, 2), coord!(4, 2)),
    (coord!(0, 3), coord!(4, 3)),
    (coord!(0, 4), coord!(4, 4)),
    (coord!(0, 5), coord!(4, 5)),
    (coord!(0, 6), coord!(4, 6)),
    // Diagonal lines (center)
    (coord!(0, 2), coord!(4, 6)),
    (coord!(0, 6), coord!(4, 2)),
    // Diagonal lines (top)
    (coord!(1, 1), coord!(2, 0)),
    (coord!(2, 0), coord!(3, 1)),
];

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum PosType {
    Invalid,
    Barn,
    Field,
}

macro_rules! invalid {
    () => {
        PosType::Invalid
    };
}
macro_rules! barn {
    () => {
        PosType::Barn
    };
}
macro_rules! field {
    () => {
        PosType::Field
    };
}

#[rustfmt::skip]
pub const BOARD_POSITIONS: [[PosType; BOARD_WIDTH as usize]; BOARD_HEIGHT as usize] = [
    [ invalid!(), invalid!(), barn!(),  invalid!(), invalid!(), ],
    [ invalid!(), barn!(),    barn!(),  barn!(),    invalid!(), ],
    [ barn!(),    barn!(),    barn!(),  barn!(),    barn!(),    ],
    [ field!(),   field!(),   field!(), field!(),   field!(),   ],
    [ field!(),   field!(),   field!(), field!(),   field!(),   ],
    [ field!(),   field!(),   field!(), field!(),   field!(),   ],
    [ field!(),   field!(),   field!(), field!(),   field!(),   ],
];

/// Check if a position is on the board.
pub fn coord_is_on_board(pos: Coord) -> bool {
    pos.x >= 0
        && pos.x < BOARD_WIDTH
        && pos.y >= 0
        && pos.y < BOARD_HEIGHT
        && BOARD_POSITIONS[pos.y as usize][pos.x as usize] != PosType::Invalid
}

/// Check if a position is on the main diagonal lines.
pub fn is_on_main_diag(pos: Coord) -> bool {
    let mut y = 2;
    for x in 0..5 {
        if pos == coord!(x, y) {
            return true;
        }
        y += 1;
    }
    let mut y = 6;
    for x in 0..5 {
        if pos == coord!(x, y) {
            return true;
        }
        y -= 1;
    }
    false
}

pub struct BoardIterator {
    x: CoordAxis,
    y: CoordAxis,
}

impl BoardIterator {
    pub fn new() -> BoardIterator {
        BoardIterator { x: 0, y: 0 }
    }
}

impl Iterator for BoardIterator {
    type Item = Coord;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let mut coord = Some(coord!(self.x, self.y));
            if self.x >= BOARD_WIDTH - 1 {
                self.x = 0;
                if self.y >= BOARD_HEIGHT {
                    self.y = 0;
                    coord = None;
                } else {
                    self.y += 1;
                }
            } else {
                self.x += 1;
            }

            match coord {
                None => break None,
                Some(coord) => {
                    if coord_is_on_board(coord) {
                        break Some(coord);
                    }
                }
            }
        }
    }
}

pub struct BoardPosIterator {
    iter: BoardIterator,
}

impl BoardPosIterator {
    pub fn new() -> BoardPosIterator {
        BoardPosIterator {
            iter: BoardIterator::new(),
        }
    }
}

impl Iterator for BoardPosIterator {
    type Item = (Coord, PosType);

    fn next(&mut self) -> Option<Self::Item> {
        self.iter
            .next()
            .map(|coord| (coord, BOARD_POSITIONS[coord.y as usize][coord.x as usize]))
    }
}

// vim: ts=4 sw=4 expandtab
