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
mod gtk_helpers;
#[cfg(feature="gui")]
mod main_window;
mod player;
#[cfg(feature="server")]
mod protocol;
mod random;
#[cfg(feature="server")]
mod server;

use anyhow as ah;
#[cfg(feature="gui")]
use crate::main_window::MainWindow;
#[cfg(feature="server")]
use crate::server::Server;
#[cfg(feature="gui")]
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
    #[cfg(feature="gui")]
    #[structopt(short, long)]
    server: bool,

    /// Bind the server to this address.
    #[cfg(feature="server")]
    #[structopt(short="b", long, default_value="0.0.0.0")]
    server_bind: String,

    /// Maximum number of connections to accept in server mode.
    #[cfg(feature="server")]
    #[structopt(short, long, default_value="10")]
    max_connections: u16,

    /// Server room to open/join.
    #[structopt(short, long)]
    room: Option<Vec<String>>,

    /// Connect to a server.
    #[structopt(short, long)]
    connect: Option<String>,

    /// Use this port for server or client connection.
    #[structopt(short, long, default_value="5596")]
    port: u16,
}

#[cfg(feature="server")]
fn server_fn(opt: &Opts) -> ah::Result<()> {
    let addr = format!("{}:{}", opt.server_bind, opt.port);

    println!("Running dedicated server on {} ...", addr);
    let mut s = Server::new(addr, opt.max_connections)?;

    let default_rooms = vec!["default".to_string()];
    let rooms = match opt.room.as_ref() {
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
        Some(connect) => Some(format!("{}:{}", connect, opt.port)),
        None => None,
    };

    let default_room = "default".to_string();
    let room_name = match opt.room {
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

    #[cfg(feature="gui")]
    let run_server = opt.server;
    #[cfg(not(feature="gui"))]
    let run_server = true;

    #[cfg(feature="server")]
    if run_server {
        server_fn(&opt)?;
        return Ok(());
    }

    #[cfg(feature="gui")]
    if !run_server {
        let app = gtk::Application::new(None, gio::ApplicationFlags::FLAGS_NONE)?;
        app.connect_activate(app_fn);
        //let args: Vec<_> = env::args().collect();
        let args = vec![];
        app.run(&args);
    }
    Ok(())
}

// vim: ts=4 sw=4 expandtab
