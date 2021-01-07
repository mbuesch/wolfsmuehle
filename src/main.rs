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

mod board;
#[cfg(feature="server")]
mod client;
mod coord;
mod game_state;
#[cfg(feature="gui")]
mod main_window;
#[cfg(feature="server")]
mod protocol;
#[cfg(feature="server")]
mod server;

use anyhow as ah;
#[cfg(feature="gui")]
use crate::main_window::MainWindow;
#[cfg(feature="server")]
use crate::server::Server;
use expect_exit::ExpectedWithError;
#[cfg(feature="gui")]
use gio::prelude::*;
#[cfg(feature="gui")]
use gtk::prelude::*;
//use std::env;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name="wolfsmühle")]
struct Opts {
    /// Run a dedicated server.
    #[cfg(feature="server")]
    #[structopt(short, long)]
    server: Option<String>,

    /// Server room to open/join.
    #[cfg(feature="server")]
    #[structopt(short="r", long)]
    server_room: Option<Vec<String>>,

    /// Connect to a server.
    #[cfg(feature="server")]
    #[structopt(short, long)]
    connect: Option<String>,
}

#[cfg(feature="server")]
fn server_fn(opt: &Opts) -> ah::Result<()> {
    let mut addr = &opt.server.as_ref().unwrap()[..];
    if addr == "default" {
        addr = "0.0.0.0:34596";
    }
    println!("Running dedicated server on {} ...", addr);
    let mut s = Server::new(addr)?;

    let default_rooms = vec!["default".to_string()];
    let rooms = match opt.server_room.as_ref() {
        Some(r) => r,
        None => {
            println!("No server rooms specified. Using '{}'.", default_rooms[0]);
            &default_rooms
        },
    };

    s.run(rooms)?;
    Ok(())
}

#[cfg(feature="gui")]
fn app_fn(app: &gtk::Application) {
    let opt = Opts::from_args();

    let connect = match opt.connect {
        Some(connect) => Some(connect.to_string()),
        None => None,
    };

    let default_room = "default".to_string();
    let room_name = match opt.server_room {
        Some(rooms) => {
            if rooms.len() >= 1 {
                rooms[0].to_string()
            } else {
                default_room
            }
        },
        None => default_room,
    };

    MainWindow::new(app, connect, room_name)
        .expect_or_exit_perror("Startup failed")
        .main_window()
        .show_all();
}

fn main() -> ah::Result<()> {
    let opt = Opts::from_args();

    #[cfg(feature="server")]
    if opt.server.is_some() {
        server_fn(&opt)?;
        return Ok(());
    }

    #[cfg(feature="gui")]
    if opt.server.is_none() {
        let app = gtk::Application::new(None, gio::ApplicationFlags::FLAGS_NONE)?;
        app.connect_activate(app_fn);
        //let args: Vec<_> = env::args().collect();
        let args = vec![];
        app.run(&args);
    }
    Ok(())
}

// vim: ts=4 sw=4 expandtab
