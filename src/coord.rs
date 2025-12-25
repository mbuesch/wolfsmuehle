// -*- coding: utf-8 -*-
//
// Copyright 2021 Michael Buesch <m@bues.ch>
//
// SPDX-License-Identifier: MIT OR Apache-2.0
//

use std::fmt;
use std::ops::{Add, Sub};

pub type CoordAxis = i16;

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Coord {
    pub x: CoordAxis,
    pub y: CoordAxis,
}

impl fmt::Display for Coord {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "(X={}, Y={})", self.x, self.y)
    }
}

impl Add for Coord {
    type Output = Coord;

    fn add(self, other: Self) -> Self::Output {
        Coord {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }
}

impl Sub for Coord {
    type Output = Coord;

    fn sub(self, other: Self) -> Self::Output {
        Coord {
            x: self.x - other.x,
            y: self.y - other.y,
        }
    }
}

#[macro_export]
macro_rules! coord {
    ( $x:expr, $y:expr ) => {
        Coord { x: $x, y: $y }
    };
}

// vim: ts=4 sw=4 expandtab
