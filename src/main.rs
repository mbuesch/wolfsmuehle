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

#![windows_subsystem="windows"]

mod board;
mod coord;
mod game_state;
#[cfg(feature="gui")]
mod gtk_helpers;
#[cfg(feature="gui")]
mod main_window;
#[cfg(feature="server")]
mod net;
mod player;
mod print;
mod random;

use anyhow as ah;
#[cfg(feature="gui")]
use crate::main_window::MainWindow;
#[cfg(feature="gui")]
use crate::player::PlayerMode;
#[cfg(feature="server")]
use crate::net::server::Server;
#[cfg(feature="gui")]
use expect_exit::{ExpectedWithError, exit};
#[cfg(feature="gui")]
use gio::prelude::*;
#[cfg(feature="gui")]
use gtk::prelude::*;
//use std::env;
use structopt::StructOpt;
use crate::print::Print;

#[derive(StructOpt, Debug)]
#[structopt(name="wolfsmühle")]
struct Opts {
    /// Set the log level.
    /// 0 = Silent. Don't print anything.
    /// 1 = Only print errors.
    /// 2 = Print errors and warnings.
    /// 3 = Print errors, warnings and info.
    /// 4 = Print errors, warnings, info and debug.
    #[structopt(short="L", long, default_value="3")]
    log_level: u8,

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
    #[structopt(short="M", long, default_value="10")]
    max_connections: u16,

    /// Server room to open (server) or join (client).
    #[structopt(short, long)]
    room: Option<Vec<String>>,

    /// Restrict the player modes that can join a room.
    /// With this option set, only one Wolf player and only
    /// one Sheep player can join a room.
    /// In restricted mode, the player mode "both" is not allowed.
    #[cfg(feature="server")]
    #[structopt(short="R", long)]
    restrict_player_modes: bool,

    /// Connect to a server.
    #[cfg(feature="gui")]
    #[structopt(short, long)]
    connect: Option<String>,

    /// Use this port for server or client connection.
    #[structopt(short, long, default_value="5596")]
    port: u16,

    /// Use this player name when joining a room, instead of an auto generated one.
    #[cfg(feature="gui")]
    #[structopt(short="n", long)]
    player_name: Option<String>,

    /// Use this player mode when joining a room.
    /// May be "wolf", "sheep", "both" or "spectator".
    #[cfg(feature="gui")]
    #[structopt(short="m", long, default_value="both")]
    player_mode: String,
}

#[cfg(feature="server")]
fn server_fn(opt: &Opts) -> ah::Result<()> {
    let addr = format!("{}:{}", opt.server_bind, opt.port);

    Print::info(&format!("Running dedicated server on {} ...", addr));
    let mut s = Server::new(addr,
                            opt.max_connections,
                            opt.restrict_player_modes)?;

    let default_rooms = vec!["default".to_string()];
    let rooms = match opt.room.as_ref() {
        Some(r) => r,
        None => {
            Print::warning(&format!("No server rooms specified. Using '{}'.",
                                    default_rooms[0]));
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

    let player_mode = match &opt.player_mode.to_lowercase().trim()[..] {
        "wolf" => PlayerMode::Wolf,
        "sheep" => PlayerMode::Sheep,
        "both" => PlayerMode::Both,
        "spectator" => PlayerMode::Spectator,
        _ => exit("Invalid --player-mode."),
    };

    MainWindow::new(app,
                    connect,
                    room_name,
                    opt.player_name,
                    player_mode)
        .expect_or_exit_perror_("Startup failed")
        .borrow()
        .main_window()
        .show();
}

fn main() -> ah::Result<()> {
    let opt = Opts::from_args();
    Print::set_level_number(opt.log_level);

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
        let app = gtk::Application::new(None, gio::ApplicationFlags::FLAGS_NONE);
        app.connect_activate(app_fn);
        let args: Vec<&str> = vec![];
        app.run_with_args(&args);
    }
    Ok(())
}

// vim: ts=4 sw=4 expandtab
