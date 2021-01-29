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

mod recorder;
mod serialize;

use anyhow as ah;
use crate::board::{
    BOARD_HEIGHT,
    BOARD_WIDTH,
    BoardIterator,
    BoardPosIterator,
    PosType,
    coord_is_on_board,
    is_on_main_diag,
};
use crate::net::{
    client::Client,
    consts::{
        MAX_PLAYERS,
        MAX_ROOMS,
    },
    protocol::{
        MSG_MOVE_ACTION_ABORT,
        MSG_MOVE_ACTION_MOVE,
        MSG_MOVE_ACTION_PICK,
        MSG_MOVE_ACTION_PUT,
        MSG_MOVE_TOKEN_CURRENT,
        MSG_MOVE_TOKEN_SHEEP,
        MSG_MOVE_TOKEN_WOLF,
        Message,
        MsgGameState,
        MsgMove,
        MsgPlayerList,
        MsgRoomList,
        MsgType,
    },
};
use crate::coord::{
    Coord,
    CoordAxis,
};
use crate::coord;
use crate::game_state::recorder::{
    RecordedMove,
    Recorder,
};
use crate::player::{
    Player,
    PlayerList,
    PlayerMode,
    num_to_player_mode,
};
use crate::print::Print;
use crate::random::random_alphanum;
use std::fmt;
use std::time;

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

const fn field_state_to_num(field_state: FieldState) -> u32 {
    match field_state {
        FieldState::Unused => 0,
        FieldState::Empty =>  1,
        FieldState::Wolf =>   2,
        FieldState::Sheep =>  3,
    }
}

fn num_to_field_state(field_state: u32) -> ah::Result<FieldState> {
    match field_state {
        0 => Ok(FieldState::Unused),
        1 => Ok(FieldState::Empty),
        2 => Ok(FieldState::Wolf),
        3 => Ok(FieldState::Sheep),
        s => Err(ah::format_err!("Unknown field state value: {}", s)),
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum WinState {
    Undecided,
    Wolf,
    Sheep,
}

impl fmt::Display for WinState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", match self {
            WinState::Undecided => "undecided",
            WinState::Wolf => "wolf",
            WinState::Sheep => "sheep",
        })
    }
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

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum MoveState {
    NoMove,
    Wolf(Coord),
    Sheep(Coord),
}

const fn move_state_to_num(move_state: &MoveState) -> (u32, u32, u32) {
    match move_state {
        MoveState::NoMove =>        (0, 0, 0),
        MoveState::Wolf(coord) =>   (1, coord.x as u32, coord.y as u32),
        MoveState::Sheep(coord) =>  (2, coord.x as u32, coord.y as u32),
    }
}

fn num_to_move_state(move_state: (u32, u32, u32)) -> ah::Result<MoveState> {
    match move_state {
        (0, _, _) => Ok(MoveState::NoMove),
        (1, x, y) => Ok(MoveState::Wolf(coord!(x as i16, y as i16))),
        (2, x, y) => Ok(MoveState::Sheep(coord!(x as i16, y as i16))),
        (a, b, c) => Err(ah::format_err!("Unknown move state values: {} {} {}",
                                         a, b, c)),
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
enum Turn {
    Sheep,
    Wolf,
    WolfchainOrSheep,
}

const fn turn_to_num(turn: &Turn) -> u32 {
    match turn {
        Turn::Sheep =>              0,
        Turn::Wolf =>               1,
        Turn::WolfchainOrSheep =>   2,
    }
}

fn num_to_turn(turn: u32) -> ah::Result<Turn> {
    match turn {
        0 => Ok(Turn::Sheep),
        1 => Ok(Turn::Wolf),
        2 => Ok(Turn::WolfchainOrSheep),
        turn => Err(ah::format_err!("Unknown turn value: {}", turn)),
    }
}

#[derive(PartialEq, Debug)]
enum ValidationResult {
    Invalid,
    Valid,
    ValidBeat(Coord),
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Stats {
    pub wolves:         u8,
    pub sheep:          u8,
    pub sheep_beaten:   u8,
}

pub struct GameState {
    player_mode:        PlayerMode,
    player_name:        String,
    room_player_list:   PlayerList,
    room_list:          Vec<String>,

    fields:             [[FieldState; BOARD_WIDTH as usize]; BOARD_HEIGHT as usize],
    moving:             MoveState,
    i_am_moving:        bool,
    stats:              Stats,
    turn:               Turn,
    just_beaten:        Option<Coord>,
    orig_sheep_count:   u8,
    recorder:           Recorder,

    client:             Option<Client>,
    client_addr:        Option<String>,
    joined_room:        Option<String>,
    roomlist_time:      time::Instant,
    playerlist_time:    time::Instant,
}

impl GameState {
    /// Construct a new game state.
    pub fn new(player_mode:         PlayerMode,
               player_name:         Option<String>)
               -> ah::Result<GameState> {

        let fields = [[FieldState::Unused; BOARD_WIDTH as usize]; BOARD_HEIGHT as usize];
        let stats = Stats {
            wolves:         0,
            sheep:          0,
            sheep_beaten:   0,
        };
        let room_player_list = PlayerList::new(vec![
            Player::new("Player".to_string(),
                        player_mode,
                        true)]);
        let player_name = match player_name {
            Some(n) => n,
            None => format!("Player-{}", random_alphanum(5)),
        };
        let mut game = GameState {
            player_mode,
            player_name,
            room_player_list,
            room_list:          vec![],
            fields,
            moving:             MoveState::NoMove,
            i_am_moving:        false,
            stats,
            turn:               Turn::Sheep,
            just_beaten:        None,
            orig_sheep_count:   0,
            recorder:           Recorder::new(),
            client:             None,
            client_addr:        None,
            joined_room:        None,
            roomlist_time:      time::Instant::now(),
            playerlist_time:    time::Instant::now(),
        };
        game.reset_game(true);
        game.print_turn();
        Ok(game)
    }

    pub fn get_recorder(&self) -> &Recorder {
        &self.recorder
    }

    pub fn reset_game(&mut self, force: bool) {
        if self.player_mode == PlayerMode::Spectator && !force {
            Print::error("reset_game: Player is spectator. Not allowed to reset the game.");
            return;
        }

        self.orig_sheep_count = 0;
        for coord in BoardIterator::new() {
            let x = coord.x as usize;
            let y = coord.y as usize;
            self.fields[y][x] = INITIAL_STATE[y][x];
            match self.fields[y][x] {
                FieldState::Sheep =>
                    self.orig_sheep_count += 1,
                FieldState::Wolf | FieldState::Unused | FieldState::Empty =>
                    (),
            }
        }

        self.moving = MoveState::NoMove;
        self.i_am_moving = false;
        self.turn = Turn::Sheep;
        self.just_beaten = None;

        self.recorder.reset();
        self.recalc_stats();
        self.client_send_reset_game();
    }

    pub fn set_player_mode(&mut self, player_mode: PlayerMode) -> ah::Result<()> {
        if self.player_mode != player_mode {
            self.do_join_room(None, None, Some(player_mode))?;
        }
        Ok(())
    }

    pub fn set_player_name(&mut self, player_name: &str) -> ah::Result<()> {
        if self.player_name != player_name {
            self.do_join_room(None, Some(player_name), None)?;
        }
        Ok(())
    }

    pub fn set_room_player_list(&mut self, room_player_list: PlayerList) {
        self.room_player_list = room_player_list;
    }

    pub fn get_room_player_list(&self) -> &PlayerList {
        &self.room_player_list
    }

    pub fn get_room_list(&self) -> &Vec<String> {
        &self.room_list
    }

    fn recalc_stats(&mut self) {
        self.stats.wolves = 0;
        self.stats.sheep = 0;
        for coord in BoardIterator::new() {
            let x = coord.x as usize;
            let y = coord.y as usize;
            match self.fields[y][x] {
                FieldState::Wolf =>
                    self.stats.wolves += 1,
                FieldState::Sheep =>
                    self.stats.sheep += 1,
                FieldState::Unused | FieldState::Empty =>
                    (),
            }
        }
        self.stats.sheep_beaten = self.orig_sheep_count - self.stats.sheep;
    }

    /// Get statistics.
    pub fn get_stats(&self) -> Stats {
        self.stats
    }

    pub fn get_win_state(&self) -> WinState {
        if self.get_stats().sheep < 7 {
            WinState::Wolf
        } else {
            let mut sheep_win = true;
            for (coord, pos_type) in BoardPosIterator::new() {
                let x = coord.x as usize;
                let y = coord.y as usize;
                match pos_type {
                    PosType::Invalid => (),
                    PosType::Field => (),
                    PosType::Barn => {
                        if self.fields[y][x] != FieldState::Sheep {
                            sheep_win = false;
                        }
                    },
                }
            }
            if sheep_win {
                WinState::Sheep
            } else {
                //TODO check: wolf is unable to move -> sheep win.
                WinState::Undecided
            }
        }
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
                Print::error("Internal error: Cannot beat empty fields."),
            FieldState::Wolf =>
                Print::error("Internal error: Cannot beat wolves."),
            FieldState::Sheep => {
                self.stats.sheep -= 1;
                self.stats.sheep_beaten += 1;
                self.just_beaten = Some(to_pos);
                self.set_field_state(beat_pos, FieldState::Empty);
                Print::debug(&format!("Beaten sheep at {}", beat_pos));
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

        // Check if the player is allowed to move this token.
        match self.player_mode {
            PlayerMode::Spectator =>
                return ValidationResult::Invalid,
            PlayerMode::Both =>
                (),
            PlayerMode::Wolf => {
                if from_state != FieldState::Wolf {
                    return ValidationResult::Invalid;
                }
            },
            PlayerMode::Sheep => {
                if from_state != FieldState::Sheep {
                    return ValidationResult::Invalid;
                }
            },
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
        } else if from_pos.x != to_pos.x && from_pos.y != to_pos.y {
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
                } else if (from_pos == coord!(1, 1) && to_pos == coord!(2, 0)) ||
                          (from_pos == coord!(3, 1) && to_pos == coord!(2, 0)) ||
                          (from_pos == coord!(2, 0) && to_pos == coord!(1, 1)) ||
                          (from_pos == coord!(2, 0) && to_pos == coord!(3, 1)) {
                    // Wolf move to/from barn top.
                    result = ValidationResult::Valid;
                }
            } else if from_state == FieldState::Sheep &&
                      (from_pos == coord!(1, 1) || from_pos == coord!(3, 1)) {
                // Sheep move to barn top.
                result = ValidationResult::Valid;
            }
        } else if from_pos.x != to_pos.x && from_pos.y == to_pos.y {
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
        } else if from_pos.x == to_pos.x && from_pos.y != to_pos.y {
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
            Print::error("Internal error: validate_move() invalid state.");
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
        if self.get_win_state() == WinState::Undecided {
            Print::debug(&format!("Next turn is: {:?}", self.turn));
        }
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
                            Print::debug("Wolf can beat more sheep.");
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
                        Print::error("Internal error: next_turn() no move."),
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
            return Err(ah::format_err!("move_pick: Coordinates ({}) out of bounds.", pos));
        }
        if self.moving != MoveState::NoMove {
            return Err(ah::format_err!("move_pick: Already moving."));
        }
        if self.player_mode == PlayerMode::Spectator {
            return Err(ah::format_err!("move_pick: Player is spectator. Not allowed to move."));
        }
        let win_state = self.get_win_state();
        if win_state != WinState::Undecided {
            return Err(ah::format_err!("move_pick: Already decided: {}", win_state));
        }

        // Try to pick the token. This might fail.
        let result = match self.get_field_state(pos) {
            FieldState::Unused | FieldState::Empty => {
                Err(ah::format_err!("move_pick: Move from empty field."))
            },
            FieldState::Wolf => {
                self.client_send_move_pick(pos, MSG_MOVE_TOKEN_WOLF)?;
                self.moving = MoveState::Wolf(pos);
                self.set_field_state(pos, FieldState::Wolf);
                Ok(())
            },
            FieldState::Sheep => {
                self.client_send_move_pick(pos, MSG_MOVE_TOKEN_SHEEP)?;
                self.moving = MoveState::Sheep(pos);
                self.set_field_state(pos, FieldState::Sheep);
                Ok(())
            },
        };
        self.i_am_moving = result.is_ok();
        result
    }

    /// Actually commit the move-put.
    fn do_move_put(&mut self, to_pos: Coord, captured: bool) {
        match self.moving {
            MoveState::NoMove =>
                Print::error("Internal error: Invalid move source."),
            MoveState::Wolf(from_pos) => {
                self.set_field_state(to_pos, FieldState::Wolf);
                self.set_field_state(from_pos, FieldState::Empty);
            },
            MoveState::Sheep(from_pos) => {
                self.set_field_state(to_pos, FieldState::Sheep);
                self.set_field_state(from_pos, FieldState::Empty);
            },
        }
        self.recalc_stats();
        self.next_turn();
        let recorded_move = RecordedMove {
            move_state: self.moving,
            to_pos,
            captured,
            win_state: self.get_win_state(),
        };
        self.recorder.record_move(&recorded_move);
        self.moving = MoveState::NoMove;
    }

    /// End a move operation.
    pub fn move_put(&mut self, pos: Coord) -> ah::Result<()> {
        if pos.x >= BOARD_WIDTH || pos.y >= BOARD_HEIGHT {
            return Err(ah::format_err!("move_put: Coordinates out of bounds."));
        }
        if self.player_mode == PlayerMode::Spectator {
            return Err(ah::format_err!("move_put: Player is spectator. Not allowed to move."));
        }

        let (from_pos, token_id) = match self.moving {
            MoveState::NoMove =>
                return Err(ah::format_err!("move_put: Not moving.")),
            MoveState::Wolf(p) => (p, MSG_MOVE_TOKEN_WOLF),
            MoveState::Sheep(p) => (p, MSG_MOVE_TOKEN_SHEEP),
        };

        // Try to put the token. This might fail.
        let result = match self.get_field_state(pos) {
            FieldState::Unused |
            FieldState::Wolf |
            FieldState::Sheep => {
                Err(ah::format_err!("move_put: Field occupied."))
            },
            FieldState::Empty => {
                match self.validate_move(from_pos, pos) {
                    ValidationResult::Invalid =>
                        Err(ah::format_err!("move_put: Invalid move.")),
                    ValidationResult::Valid => {
                        self.client_send_move_put(pos, token_id)?;
                        self.do_move_put(pos, false);
                        Ok(())
                    },
                    ValidationResult::ValidBeat(beat_pos) => {
                        self.client_send_move_put(pos, token_id)?;
                        self.beat(from_pos, pos, beat_pos);
                        self.do_move_put(pos, true);
                        Ok(())
                    },
                }
            },
        };
        self.i_am_moving = !result.is_ok();
        result
    }

    /// Abort a move operation.
    pub fn move_abort(&mut self) {
        if self.player_mode == PlayerMode::Spectator {
            Print::error("move_abort: Player is spectator. Not allowed to move.");
            return;
        }

        match self.moving {
            MoveState::NoMove => {
                self.i_am_moving = false;
            },
            MoveState::Wolf(coord) |
            MoveState::Sheep(coord) => {
                self.moving = MoveState::NoMove;
                self.i_am_moving = false;
                self.client_send_move_abort(coord).ok();
            },
        }
    }

    pub fn make_state_message(&self) -> MsgGameState {
        let mut fields = [[field_state_to_num(FieldState::Unused);
                           BOARD_WIDTH as usize];
                          BOARD_HEIGHT as usize];
        for coord in BoardIterator::new() {
            let x = coord.x as usize;
            let y = coord.y as usize;
            fields[y][x] = field_state_to_num(self.fields[y][x]);
        }
        let (moving_state, moving_x, moving_y) = move_state_to_num(&self.moving);
        let turn = turn_to_num(&self.turn);
        MsgGameState::new(fields, moving_state, moving_x, moving_y, turn)
    }

    pub fn read_state_message(&mut self,
                              msg: &MsgGameState,
                              force: bool) -> ah::Result<bool> {
        if !force && self.player_mode == PlayerMode::Spectator {
            return Err(ah::format_err!("Player is spectator. Not allowed to load game state."));
        }

        let mut changed = false;
        if !self.i_am_moving {
            for coord in BoardIterator::new() {
                let x = coord.x as usize;
                let y = coord.y as usize;
                let field = match num_to_field_state(msg.get_fields()[y][x]) {
                    Ok(field) => field,
                    Err(e) => {
                        Print::error(&format!("Received invalid field state: {}", e));
                        self.fields[y][x]
                    },
                };
                if field != self.fields[y][x] {
                    self.fields[y][x] = field;
                    changed = true;
                }
            }

            let moving = match num_to_move_state(msg.get_moving()) {
                Ok(moving) => moving,
                Err(e) => {
                    Print::error(&format!("Received invalid moving state: {}", e));
                    self.moving
                },
            };
            if moving != self.moving {
                self.moving = moving;
                changed = true;
            }

            let turn = match num_to_turn(msg.get_turn()) {
                Ok(turn) => turn,
                Err(e) => {
                    Print::error(&format!("Received invalid turn state: {}", e));
                    self.turn
                },
            };
            if turn != self.turn {
                self.turn = turn;
                changed = true;
            }

            if changed {
                self.recorder.reset();
                self.recalc_stats();
            }
        }
        Ok(changed)
    }
}

impl Drop for GameState {
    fn drop(&mut self) {
        self.client_disconnect();
    }
}

//////////////////////////////////////////////////////////////////////////////
// Client interface.
//////////////////////////////////////////////////////////////////////////////

impl GameState {
    fn client_handle_rx_msg_gamestate(&mut self, msg: &MsgGameState) -> bool {
        match self.read_state_message(msg, true) {
            Ok(changed) => changed,
            Err(_) => false,
        }
    }

    fn client_handle_rx_msg_roomlist(&mut self, msg: &MsgRoomList) {
        let total_count = msg.get_total_count();
        if total_count > MAX_ROOMS as u32 {
            Print::error(&format!("Received RoomList with too many rooms: {}",
                                  total_count));
            return;
        }

        self.room_list.resize_with(total_count as usize,
                                   || "".to_string());

        let room_name = match msg.get_room_name() {
            Ok(n) => n,
            Err(e) => {
                Print::error(&format!("Received RoomList with invalid room name: {}", e));
                return;
            },
        };

        let index = msg.get_index() as usize;
        if index >= self.room_list.len() {
            Print::error("Received RoomList with invalid index.");
            return;
        }

        self.room_list[index] = room_name;
    }

    fn client_handle_rx_msg_playerlist(&mut self, msg: &MsgPlayerList) {
        let total_count = msg.get_total_count();
        if total_count > MAX_PLAYERS as u32 {
            Print::error(&format!("Received PlayerList with too many players: {}",
                                  total_count));
            return;
        }

        self.room_player_list.resize(total_count as usize,
                                     || Player::new("<unknown>".to_string(),
                                                    PlayerMode::Spectator,
                                                    false));

        let player_name = match msg.get_player_name() {
            Ok(n) => n,
            Err(e) => {
                Print::error(&format!("Received PlayerList with invalid player name: {}", e));
                return;
            }
        };
        let player_mode = match num_to_player_mode(msg.get_player_mode()) {
            Ok(m) => m,
            Err(e) => {
                Print::error(&format!("Received PlayerList with invalid player mode '{}': {}",
                                      msg.get_player_mode(), e));
                return;
            },
        };
        let is_self = player_name == self.player_name;

        let index = msg.get_index() as usize;
        if index >= self.room_player_list.count() {
            Print::error("Received PlayerList with invalid index.");
            return;
        }

        self.room_player_list.set_player(index,
                                         Player::new(player_name,
                                                     player_mode,
                                                     is_self));
    }

    fn client_handle_rx_messages(&mut self, messages: Vec<Box<dyn Message>>) -> bool {
        let mut redraw = false;
        for message in &messages {
            let message = message.get_message();

            match message {
                MsgType::Nop(_) |
                MsgType::Result(_) |
                MsgType::Ping(_) |
                MsgType::Pong(_) |
                MsgType::Join(_) |
                MsgType::Leave(_) |
                MsgType::Reset(_) |
                MsgType::ReqGameState(_) |
                MsgType::ReqRoomList(_) |
                MsgType::ReqPlayerList(_) |
                MsgType::Move(_) => {
                    // Ignore.
                },
                MsgType::GameState(msg) => {
                    if self.joined_room.is_some() {
                        if self.client_handle_rx_msg_gamestate(msg) {
                            redraw = true;
                        }
                    }
                },
                MsgType::RoomList(msg) => {
                    self.client_handle_rx_msg_roomlist(msg);
                },
                MsgType::PlayerList(msg) => {
                    if self.joined_room.is_some() {
                        self.client_handle_rx_msg_playerlist(msg);
                    }
                },
            }
        }

        if let Some(client) = self.client.as_mut() {
            let now = time::Instant::now();

            if self.joined_room.is_some() {
                if now.duration_since(self.playerlist_time).as_millis() >= 200 {
                    client.send_request_playerlist().ok();
                    self.playerlist_time = now;
                }
                client.send_request_gamestate().ok();
            }
            if now.duration_since(self.roomlist_time).as_millis() >= 1000 {
                client.send_request_roomlist().ok();
                self.roomlist_time = now;
            }
        }

        redraw
    }

    /// Poll the game server state.
    pub fn poll_server(&mut self) -> bool {
        if let Some(client) = self.client.as_mut() {
            if let Some(messages) = client.poll() {
                self.client_handle_rx_messages(messages)
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Connect to a game server.
    pub fn client_connect(&mut self, addr: &str) -> ah::Result<()> {
        self.client_disconnect();
        Print::info(&format!("Connecting to server {} ...", addr));
        let mut client = Client::new(addr)?;
        client.send_ping()?;
        client.send_nop()?;
        self.client = Some(client);
        self.client_addr = Some(addr.to_string());
        self.roomlist_time = time::Instant::now() - time::Duration::new(1000, 0);
        self.playerlist_time = self.roomlist_time;
        Ok(())
    }

    fn do_join_room(&mut self,
                    room_name: Option<&str>,
                    player_name: Option<&str>,
                    player_mode: Option<PlayerMode>) -> ah::Result<()> {
        let room_name = match room_name {
            Some(room_name) =>
                Some(room_name.to_string()),
            None =>
                match self.joined_room.as_ref() {
                    Some(joined_room) => Some(joined_room.to_string()),
                    None => None,
                },
        };

        let player_name = match player_name {
            Some(player_name) => player_name,
            None => &self.player_name,
        };

        let player_mode = match player_mode {
            Some(player_mode) => player_mode,
            None => self.player_mode,
        };

        let old_joined_room = self.joined_room.take();

        if let Some(client) = self.client.as_mut() {
            match room_name {
                Some(room_name) => {
                    match client.send_join(&room_name,
                                           player_name,
                                           player_mode) {
                        Ok(_) => {
                            self.joined_room = Some(room_name);
                        },
                        Err(e) => {
                            self.joined_room = old_joined_room;
                            return Err(e);
                        },
                    }
                },
                None => (),
            }
        }

        self.player_name = player_name.to_string();
        self.player_mode = player_mode;

        Ok(())
    }

    /// Join a room on the server.
    pub fn client_join_room(&mut self, room_name: &str) -> ah::Result<()> {
        if self.client.is_none() {
            return Err(ah::format_err!("Cannot join room. Not connected to a server."));
        }
        Print::info(&format!("Joining room '{}' ...", room_name));
        self.do_join_room(Some(room_name), None, None)?;
        if let Some(client) = self.client.as_mut() {
            client.send_request_gamestate()?;
            self.fields = [[FieldState::Unused; BOARD_WIDTH as usize]; BOARD_HEIGHT as usize];
        }
        Ok(())
    }

    /// Disconnect from a game server.
    pub fn client_disconnect(&mut self) {
        if let Some(client) = self.client.take() {
            client.disconnect();
            self.client_addr = None;
            Print::info("Disconnected from server.");
        }
        self.joined_room = None;
        self.room_list.clear();
    }

    pub fn client_is_connected(&self) -> bool {
        self.client.is_some()
    }

    /// Get the address of the connected server, if any.
    pub fn client_get_addr(&self) -> Option<&str> {
        match &self.client_addr {
            None => None,
            Some(a) => Some(a),
        }
    }

    /// Get the name of the joined room, if any.
    pub fn client_get_joined_room(&self) -> Option<&str> {
        match &self.joined_room {
            None => None,
            Some(r) => Some(r),
        }
    }

    fn client_send_reset_game(&mut self) {
        if let Some(client) = self.client.as_mut() {
            if let Err(e) = client.send_reset() {
                Print::error(&format!("Failed to game-reset: {}", e));
            }
        }
    }

    /// Send the move-pick to the server.
    fn client_send_move_pick(&mut self, pos: Coord, token_id: u32) -> ah::Result<()> {
        if let Some(client) = self.client.as_mut() {
            if let Err(e) = client.send_move_token(MSG_MOVE_ACTION_PICK,
                                                   token_id,
                                                   pos.x as u32,
                                                   pos.y as u32) {
                let msg = format!("Move-pick failed on server: {}", e);
                Print::error(&msg);
                return Err(ah::format_err!("{}", msg));
            }
        }
        Ok(())
    }

    /// Send the move-put to the server.
    fn client_send_move_put(&mut self, pos: Coord, token_id: u32) -> ah::Result<()> {
        if let Some(client) = self.client.as_mut() {
            if let Err(e) = client.send_move_token(MSG_MOVE_ACTION_PUT,
                                                   token_id,
                                                   pos.x as u32,
                                                   pos.y as u32) {
                let msg = format!("Move failed on server: {}", e);
                Print::error(&msg);
                return Err(ah::format_err!("{}", msg));
            }
        }
        Ok(())
    }

    /// Send the move-abort to the server.
    fn client_send_move_abort(&mut self, pos: Coord) -> ah::Result<()> {
        if let Some(client) = self.client.as_mut() {
            if let Err(e) = client.send_move_token(MSG_MOVE_ACTION_ABORT,
                                                   MSG_MOVE_TOKEN_CURRENT,
                                                   pos.x as u32,
                                                   pos.y as u32) {
                Print::error(&format!("Move-abort failed on server: {}", e));
            }
        }
        Ok(())
    }

    fn client_send_full_gamestate(&mut self) -> ah::Result<()> {
        let mut game_state_msg = self.make_state_message();
        if let Some(client) = self.client.as_mut() {
            client.send_msg_wait_for_ok("GameState", 3.0, &mut game_state_msg)?;
        }
        Ok(())
    }
}

//////////////////////////////////////////////////////////////////////////////
// Server interface.
//////////////////////////////////////////////////////////////////////////////

impl GameState {
    pub fn server_handle_rx_msg_move(&mut self, msg: &MsgMove) -> ah::Result<()> {
        match msg.get_action() {
            (MSG_MOVE_ACTION_PICK, x, y) => {
                self.move_pick(coord!(x as i16, y as i16))?;
            },
            (MSG_MOVE_ACTION_MOVE, _x, _y) => {
                //TODO
            },
            (MSG_MOVE_ACTION_PUT, x, y) => {
                self.move_put(coord!(x as i16, y as i16))?;
            },
            (MSG_MOVE_ACTION_ABORT, _x, _y) => {
                self.move_abort();
            },
            (action, _, _) => {
                Print::error(&format!("Received invalid move action: {}", action));
            },
        }
        Ok(())
    }
}

// vim: ts=4 sw=4 expandtab
