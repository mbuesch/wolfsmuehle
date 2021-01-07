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
    BOARD_WIDTH,
    BOARD_HEIGHT,
};
use std::convert::TryInto;

pub const MSG_BUFFER_SIZE: usize    = 0x100;

const MSG_MAGIC: u32                = 0xAA0E1F37;

const MSG_ID_NOP: u32               = 0;
const MSG_ID_RESULT: u32            = 1;
const MSG_ID_PING: u32              = 2;
const MSG_ID_PONG: u32              = 3;
const MSG_ID_JOIN: u32              = 4;
const MSG_ID_LEAVE: u32             = 5;
const MSG_ID_RESET: u32             = 6;
const MSG_ID_REQGAMESTATE: u32      = 7;
const MSG_ID_GAMESTATE: u32         = 8;
const MSG_ID_MOVE: u32              = 9;

/// Convert to network byte order.
fn to_net(data: u32) -> [u8; 4] {
    data.to_be_bytes()
}

/// Convert from network byte order.
fn from_net(data: &[u8]) -> ah::Result<u32> {
    if data.len() >= 4 {
        Ok(u32::from_be_bytes(data[0..4].try_into()?))
    } else {
        return Err(ah::format_err!("from_net: Not enough data."))
    }
}

fn str2bytes(bytes: &mut [u8], string: &str) -> ah::Result<()> {
    let len = string.as_bytes().len();
    if len > bytes.len() {
        return Err(ah::format_err!("str2bytes: String is too long."));
    }
    bytes[0..len].copy_from_slice(&string.as_bytes());
    Ok(())
}

fn bytes2string(bytes: &[u8]) -> ah::Result<String> {
    let mut bytes = bytes.to_vec();
    bytes.retain(|&x| x != 0);
    Ok(String::from_utf8(bytes)?)
}

type FieldsArray = [[u32; BOARD_WIDTH as usize]; BOARD_HEIGHT as usize];

#[derive(Debug)]
pub enum MsgType<'a> {
    MsgTypeNop(&'a MsgNop),
    MsgTypeResult(&'a MsgResult),
    MsgTypePing(&'a MsgPing),
    MsgTypePong(&'a MsgPong),
    MsgTypeJoin(&'a MsgJoin),
    MsgTypeLeave(&'a MsgLeave),
    MsgTypeReset(&'a MsgReset),
    MsgTypeReqGameState(&'a MsgReqGameState),
    MsgTypeGameState(&'a MsgGameState),
    MsgTypeMove(&'a MsgMove),
}

pub trait Message {
    fn get_id(&self) -> u32;

    fn to_bytes(&self) -> Vec<u8>;

    fn get_message(&self) -> MsgType;
}

/// Helper function: Skip a given length in a buffer.
pub fn buffer_skip(mut buffer: Vec<u8>, skip_len: usize) -> Vec<u8> {
    if skip_len > 0 {
        if skip_len >= buffer.len() {
            buffer.clear();
            buffer
        } else {
            buffer.split_off(skip_len)
        }
    } else {
        buffer
    }
}

/// Try to synchronize to the data stream by finding the magic word.
pub fn net_sync(data: &[u8]) -> Option<usize> {
    let len = data.len();
    if len >= 4 {
        for skip in 0..(len - 4) {
            match from_net(&data[skip..]) {
                Ok(MSG_MAGIC) => return Some(skip),
                Ok(_) | Err(_) => (),
            }
        }
    }
    None
}

/// Try to parse a data stream.
/// Returns the message and the number of consumed bytes.
/// If the returned message is None,
/// then there were not enough bytes to fully parse the message.
pub fn message_from_bytes(data: &[u8]) -> ah::Result<(usize, Option<Box<dyn Message>>)> {
    if data.len() < MSG_HEADER_SIZE as usize {
        return Ok((0, None));
    }

    let mut offset = 0;
    let magic = from_net(&data[offset..])?;
    offset += 4;
    if magic != MSG_MAGIC {
        return Err(ah::format_err!("from_bytes: Invalid Message magic (0x{:X} != 0x{:X}).",
                                   magic, MSG_MAGIC))
    }
    let size = from_net(&data[offset..])?;
    offset += 4;
    if size < MSG_HEADER_SIZE {
        return Err(ah::format_err!("from_bytes: Invalid Message length ({} < {}).",
                                   size, MSG_HEADER_SIZE))
    }
    if size > MSG_BUFFER_SIZE as u32 {
        return Err(ah::format_err!("from_bytes: Invalid Message length ({} > {}).",
                                   size, MSG_BUFFER_SIZE));
    }
    if data.len() < size as usize {
        return Ok((0, None));
    }
    let id = from_net(&data[offset..])?;
    offset += 4;
    // Skip reserved.
    offset += 5 * 4;

    let header = MsgHeader::new(magic, size, id);

    let (sub_size, message) = match id {
        MSG_ID_NOP =>
            MsgNop::from_bytes(header, &data[offset..])?,
        MSG_ID_RESULT =>
            MsgResult::from_bytes(header, &data[offset..])?,
        MSG_ID_PING =>
            MsgPing::from_bytes(header, &data[offset..])?,
        MSG_ID_PONG =>
            MsgPong::from_bytes(header, &data[offset..])?,
        MSG_ID_JOIN =>
            MsgJoin::from_bytes(header, &data[offset..])?,
        MSG_ID_LEAVE =>
            MsgLeave::from_bytes(header, &data[offset..])?,
        MSG_ID_RESET =>
            MsgReset::from_bytes(header, &data[offset..])?,
        MSG_ID_REQGAMESTATE =>
            MsgReqGameState::from_bytes(header, &data[offset..])?,
        MSG_ID_GAMESTATE =>
            MsgGameState::from_bytes(header, &data[offset..])?,
        MSG_ID_MOVE =>
            MsgMove::from_bytes(header, &data[offset..])?,
        _ =>
            return Err(ah::format_err!("from_bytes: Unknown ID ({}).", id)),
    };

    Ok((offset + sub_size, Some(message)))
}

//////////////////////////////////////////////////////////////////////////////
// Message header.
//////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub struct MsgHeader {
    magic:          u32,
    size:           u32,
    id:             u32,
    reserved:       [u32; 5],
}

const MSG_HEADER_SIZE: u32  = 4 * 8;

impl MsgHeader {
    fn new(magic: u32, size: u32, id: u32) -> MsgHeader {
        MsgHeader {
            magic,
            size,
            id,
            reserved:   [0; 5],
        }
    }

    fn to_bytes(&self, data: &mut Vec<u8>) {
        data.extend_from_slice(&to_net(self.magic));
        data.extend_from_slice(&to_net(self.size));
        data.extend_from_slice(&to_net(self.id));
        for word in &self.reserved {
            data.extend_from_slice(&to_net(*word));
        }
    }
}

//////////////////////////////////////////////////////////////////////////////
// Trivial messages without payload.
//////////////////////////////////////////////////////////////////////////////

macro_rules! define_trivial_message {
    ($struct_name:ident, $msg_type:ident, $id:ident) => {
        #[derive(Debug)]
        pub struct $struct_name {
            header:     MsgHeader,
        }

        impl $struct_name {
            pub fn new() -> $struct_name {
                $struct_name {
                    header:     MsgHeader::new(MSG_MAGIC,
                                               MSG_HEADER_SIZE,
                                               $id),
                }
            }

            fn from_bytes(header: MsgHeader, _data: &[u8]) -> ah::Result<(usize, Box<dyn Message>)> {
                Ok((0, Box::new($struct_name { header, })))
            }
        }

        impl Message for $struct_name {
            fn get_id(&self) -> u32 {
                $id
            }

            fn to_bytes(&self) -> Vec<u8> {
                let mut data = Vec::with_capacity(MSG_HEADER_SIZE as usize);
                self.header.to_bytes(&mut data);
                data
            }

            fn get_message(&self) -> MsgType {
                MsgType::$msg_type(self)
            }
        }
    }
}

define_trivial_message!(MsgNop, MsgTypeNop, MSG_ID_NOP);
define_trivial_message!(MsgPing, MsgTypePing, MSG_ID_PING);
define_trivial_message!(MsgPong, MsgTypePong, MSG_ID_PONG);
define_trivial_message!(MsgLeave, MsgTypeLeave, MSG_ID_LEAVE);
define_trivial_message!(MsgReset, MsgTypeReset, MSG_ID_RESET);
define_trivial_message!(MsgReqGameState, MsgTypeReqGameState, MSG_ID_REQGAMESTATE);

//////////////////////////////////////////////////////////////////////////////
// MsgResult
//////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub struct MsgResult {
    header:         MsgHeader,
    in_reply_to_id: u32,
    result_code:    u32,
}

const MSG_RESULT_SIZE: u32 = MSG_HEADER_SIZE + (2 * 4);

pub const MSG_RESULT_OK: u32    = 0;
pub const MSG_RESULT_NOK: u32   = 1;
#[allow(dead_code)]
pub const MSG_RESULT_USER: u32  = 0x10000;

impl MsgResult {
    pub fn new(in_reply_to_msg: &dyn Message,
               result_code:     u32) -> ah::Result<MsgResult> {
        Ok(MsgResult {
            header:         MsgHeader::new(MSG_MAGIC,
                                           MSG_RESULT_SIZE,
                                           MSG_ID_RESULT),
            in_reply_to_id: in_reply_to_msg.get_id(),
            result_code,
        })
    }

    fn from_bytes(header: MsgHeader, data: &[u8]) -> ah::Result<(usize, Box<dyn Message>)> {
        if data.len() >= (MSG_RESULT_SIZE - MSG_HEADER_SIZE) as usize {
            let mut offset = 0;

            let in_reply_to_id = from_net(&data[offset..])?;
            offset += 4;
            let result_code = from_net(&data[offset..])?;
            offset += 4;

            let msg_result = MsgResult {
                header,
                in_reply_to_id,
                result_code
            };
            Ok((offset, Box::new(msg_result)))
        } else {
            Err(ah::format_err!("MsgResult: Not enough data."))
        }
    }

    pub fn is_in_reply_to(&self, other: &dyn Message) -> bool {
        self.in_reply_to_id == other.get_id()
    }

    pub fn get_result_code(&self) -> u32 {
        self.result_code
    }

    pub fn is_ok(&self) -> bool {
        self.get_result_code() == MSG_RESULT_OK
    }
}

impl Message for MsgResult {
    fn get_id(&self) -> u32 {
        MSG_ID_RESULT
    }

    fn to_bytes(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(MSG_RESULT_SIZE as usize);
        self.header.to_bytes(&mut data);
        data.extend_from_slice(&to_net(self.in_reply_to_id));
        data.extend_from_slice(&to_net(self.result_code));
        data
    }

    fn get_message(&self) -> MsgType {
        MsgType::MsgTypeResult(self)
    }
}

//////////////////////////////////////////////////////////////////////////////
// MsgJoin
//////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub struct MsgJoin {
    header:     MsgHeader,
    room_name:  [u8; MSG_JOIN_MAXROOMNAME],
}

const MSG_JOIN_MAXROOMNAME: usize = 64;
const MSG_JOIN_SIZE: u32 = MSG_HEADER_SIZE + MSG_JOIN_MAXROOMNAME as u32;

impl MsgJoin {
    pub fn new(room_name: &str) -> ah::Result<MsgJoin> {
        let mut room_name_bytes = [0; MSG_JOIN_MAXROOMNAME];
        str2bytes(&mut room_name_bytes, room_name)?;
        Ok(MsgJoin {
            header:     MsgHeader::new(MSG_MAGIC,
                                       MSG_JOIN_SIZE,
                                       MSG_ID_JOIN),
            room_name:  room_name_bytes,
        })
    }

    fn from_bytes(header: MsgHeader, data: &[u8]) -> ah::Result<(usize, Box<dyn Message>)> {
        if data.len() >= (MSG_JOIN_SIZE - MSG_HEADER_SIZE) as usize {
            let mut offset = 0;

            let mut room_name = [0; MSG_JOIN_MAXROOMNAME];
            room_name.copy_from_slice(&data[offset..offset+MSG_JOIN_MAXROOMNAME]);
            offset += MSG_JOIN_MAXROOMNAME;

            let msg_join = MsgJoin {
                header,
                room_name,
            };
            Ok((offset, Box::new(msg_join)))
        } else {
            Err(ah::format_err!("MsgJoin: Not enough data."))
        }
    }

    pub fn get_room_name(&self) -> ah::Result<String> {
        bytes2string(&self.room_name)
    }
}

impl Message for MsgJoin {
    fn get_id(&self) -> u32 {
        MSG_ID_JOIN
    }

    fn to_bytes(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(MSG_JOIN_SIZE as usize);
        self.header.to_bytes(&mut data);
        data.extend_from_slice(&self.room_name);
        data
    }

    fn get_message(&self) -> MsgType {
        MsgType::MsgTypeJoin(self)
    }
}

//////////////////////////////////////////////////////////////////////////////
// MsgGameState
//////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub struct MsgGameState {
    header:         MsgHeader,
    fields:         FieldsArray,
    moving_state:   u32,
    moving_x:       u32,
    moving_y:       u32,
    turn:           u32,
}

const MSG_GAME_STATE_SIZE: u32 = MSG_HEADER_SIZE +
                                 (BOARD_WIDTH as u32 * BOARD_HEIGHT as u32 * 4) +
                                 (4 * 4);

const MSG_FIELD_INVALID: u32   = 0;

impl MsgGameState {
    pub fn new(fields:          FieldsArray,
               moving_state:    u32,
               moving_x:        u32,
               moving_y:        u32,
               turn:            u32) -> MsgGameState {
        MsgGameState {
            header:     MsgHeader::new(MSG_MAGIC,
                                       MSG_GAME_STATE_SIZE,
                                       MSG_ID_GAMESTATE),
            fields,
            moving_state,
            moving_x,
            moving_y,
            turn,
        }
    }

    fn from_bytes(header: MsgHeader, data: &[u8]) -> ah::Result<(usize, Box<dyn Message>)> {
        if data.len() >= (MSG_GAME_STATE_SIZE - MSG_HEADER_SIZE) as usize {
            let mut offset = 0;

            let mut fields = [[MSG_FIELD_INVALID; BOARD_WIDTH as usize]; BOARD_HEIGHT as usize];
            for y in 0..(BOARD_HEIGHT as usize) {
                for x in 0..(BOARD_WIDTH as usize) {
                    fields[y][x] = from_net(&data[offset..])?;
                    offset += 4;
                }
            }
            let moving_state = from_net(&data[offset..])?;
            offset += 4;
            let moving_x = from_net(&data[offset..])?;
            offset += 4;
            let moving_y = from_net(&data[offset..])?;
            offset += 4;
            let turn = from_net(&data[offset..])?;
            offset += 4;

            Ok((offset, Box::new(MsgGameState {
                                 header,
                                 fields,
                                 moving_state,
                                 moving_x,
                                 moving_y,
                                 turn, })
            ))
        } else {
            Err(ah::format_err!("MsgGameState: Not enough data."))
        }
    }

    pub fn get_fields(&self) -> &FieldsArray {
        &self.fields
    }

    pub fn get_moving(&self) -> (u32, u32, u32) {
        (self.moving_state, self.moving_x, self.moving_y)
    }

    pub fn get_turn(&self) -> u32 {
        self.turn
    }
}

impl Message for MsgGameState {
    fn get_id(&self) -> u32 {
        MSG_ID_GAMESTATE
    }

    fn to_bytes(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(MSG_GAME_STATE_SIZE as usize);
        self.header.to_bytes(&mut data);
        for y in 0..(BOARD_HEIGHT as usize) {
            for x in 0..(BOARD_WIDTH as usize) {
                data.extend_from_slice(&to_net(self.fields[y][x]));
            }
        }
        data.extend_from_slice(&to_net(self.moving_state));
        data.extend_from_slice(&to_net(self.moving_x));
        data.extend_from_slice(&to_net(self.moving_y));
        data.extend_from_slice(&to_net(self.turn));
        data
    }

    fn get_message(&self) -> MsgType {
        MsgType::MsgTypeGameState(self)
    }
}

//////////////////////////////////////////////////////////////////////////////
// MsgMove
//////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub struct MsgMove {
    header:         MsgHeader,
    action:         u32,
    token:          u32,
    coord_x:        u32,
    coord_y:        u32,
}

const MSG_MOVE_SIZE: u32 = MSG_HEADER_SIZE + (4 * 4);

pub const MSG_MOVE_ACTION_PICK: u32         = 0;
pub const MSG_MOVE_ACTION_MOVE: u32         = 1;
pub const MSG_MOVE_ACTION_PUT: u32          = 2;
pub const MSG_MOVE_ACTION_ABORT: u32        = 3;

pub const MSG_MOVE_TOKEN_CURRENT: u32       = 0;
pub const MSG_MOVE_TOKEN_WOLF: u32          = 1;
pub const MSG_MOVE_TOKEN_SHEEP: u32         = 2;

impl MsgMove {
    pub fn new(action:  u32,
               token:   u32,
               coord_x: u32,
               coord_y: u32) -> MsgMove {
        MsgMove {
            header:         MsgHeader::new(MSG_MAGIC,
                                           MSG_MOVE_SIZE,
                                           MSG_ID_MOVE),
            action,
            token,
            coord_x,
            coord_y,
        }
    }

    fn from_bytes(header: MsgHeader, data: &[u8]) -> ah::Result<(usize, Box<dyn Message>)> {
        if data.len() >= (MSG_MOVE_SIZE - MSG_HEADER_SIZE) as usize {
            let mut offset = 0;

            let action = from_net(&data[offset..])?;
            offset += 4;
            let token = from_net(&data[offset..])?;
            offset += 4;
            let coord_x = from_net(&data[offset..])?;
            offset += 4;
            let coord_y = from_net(&data[offset..])?;
            offset += 4;

            let msg_result = MsgMove {
                header,
                action,
                token,
                coord_x,
                coord_y,
            };
            Ok((offset, Box::new(msg_result)))
        } else {
            Err(ah::format_err!("MsgMove: Not enough data."))
        }
    }

    pub fn get_action(&self) -> (u32, u32, u32) {
        (self.action, self.coord_x, self.coord_y)
    }
}

impl Message for MsgMove {
    fn get_id(&self) -> u32 {
        MSG_ID_MOVE
    }

    fn to_bytes(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(MSG_MOVE_SIZE as usize);
        self.header.to_bytes(&mut data);
        data.extend_from_slice(&to_net(self.action));
        data.extend_from_slice(&to_net(self.token));
        data.extend_from_slice(&to_net(self.coord_x));
        data.extend_from_slice(&to_net(self.coord_y));
        data
    }

    fn get_message(&self) -> MsgType {
        MsgType::MsgTypeMove(self)
    }
}

// vim: ts=4 sw=4 expandtab
