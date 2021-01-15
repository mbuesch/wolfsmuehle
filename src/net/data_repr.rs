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

pub trait ToNetStr {
    /// Convert to network byte format.
    fn to_net(&self, bytes: &mut [u8], truncate: bool) -> ah::Result<()>;
}

pub trait FromNet32 {
    /// Convert from network byte order.
    fn from_net(data: &[u8]) -> ah::Result<u32>;
}

pub trait FromNetStr {
    /// Convert from network byte format.
    fn from_net(bytes: &[u8], lossy: bool) -> ah::Result<String>;
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

impl ToNetStr for str {
    fn to_net(&self, bytes: &mut [u8], truncate: bool) -> ah::Result<()> {
        let mut len = self.as_bytes().len();
        if len > bytes.len() {
            if !truncate {
                return Err(ah::format_err!("to_net str: String is too long."));
            }
            len = bytes.len()
        }
        bytes[0..len].copy_from_slice(&self.as_bytes());
        Ok(())
    }
}

impl FromNetStr for String {
    fn from_net(bytes: &[u8], lossy: bool) -> ah::Result<String> {
        // Remove trailing zeros.
        let mut len = bytes.len();
        for i in (0..bytes.len()).rev() {
            if bytes[i] != 0 {
                break;
            }
            len -= 1;
        }
        if lossy {
            Ok(String::from_utf8_lossy(&bytes[0..len]).to_string())
        } else {
            Ok(String::from_utf8(bytes[0..len].to_vec())?)
        }
    }
}

// vim: ts=4 sw=4 expandtab
