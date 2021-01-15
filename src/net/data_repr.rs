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
use std::convert::TryInto;

pub trait ToNet32 {
    /// Convert to network byte order.
    fn to_net(&self) -> [u8; 4];
}

pub trait FromNet32 {
    /// Convert from network byte order.
    fn from_net(data: &[u8]) -> ah::Result<u32>;
}

impl ToNet32 for u32 {
    fn to_net(&self) -> [u8; 4] {
        self.to_be_bytes()
    }
}

impl FromNet32 for u32 {
    fn from_net(data: &[u8]) -> ah::Result<u32> {
        if data.len() >= 4 {
            Ok(u32::from_be_bytes(data[0..4].try_into()?))
        } else {
            return Err(ah::format_err!("from_net u32: Not enough data."))
        }
    }
}

// vim: ts=4 sw=4 expandtab
