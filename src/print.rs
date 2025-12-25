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

use lazy_static::lazy_static;
use std::sync::{Arc, RwLock};

lazy_static! {
    static ref PRINT_SINGLETON: Arc<RwLock<Print>> = Arc::new(RwLock::new(Print::new()));
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Copy, Clone, Debug)]
#[allow(dead_code)]
pub enum PrintLevel {
    Silent,
    Error,
    Warning,
    Info,
    Debug,
}

pub struct Print {
    level: PrintLevel,
}

macro_rules! define_printer {
    ($funcname:ident, $level:path, $prefix:literal) => {
        #[allow(dead_code)]
        pub fn $funcname(msg: &str) {
            let level = PRINT_SINGLETON.read().unwrap().level;
            if level >= $level {
                if level == PrintLevel::Error || level == PrintLevel::Warning {
                    eprintln!("{}{}", $prefix, msg);
                } else {
                    println!("{}{}", $prefix, msg);
                }
            }
        }
    };
}

impl Print {
    fn new() -> Print {
        Print {
            level: PrintLevel::Info,
        }
    }

    pub fn set_level(level: PrintLevel) {
        let mut p = PRINT_SINGLETON.write().unwrap();
        p.level = level;
    }

    pub fn set_level_number(level: u8) {
        Print::set_level(match level {
            0 => PrintLevel::Silent,
            1 => PrintLevel::Error,
            2 => PrintLevel::Warning,
            3 => PrintLevel::Info,
            4 => PrintLevel::Debug,
            _ => PrintLevel::Debug,
        });
    }

    define_printer!(error, PrintLevel::Error, "ERROR: ");
    define_printer!(warning, PrintLevel::Warning, "Warning: ");
    define_printer!(info, PrintLevel::Info, "");
    define_printer!(debug, PrintLevel::Debug, "Debug: ");
}

// vim: ts=4 sw=4 expandtab
