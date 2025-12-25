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

use crate::board::{BOARD_HEIGHT, BOARD_WIDTH};
use crate::net::data_repr::{FromNet32, FromNetStr, ToNet32, ToNetStr};
use anyhow as ah;
use std::cmp::min;

pub const MSG_BUFFER_SIZE: usize = 0x1000;

pub const MSG_PLAYERMODE_SPECTATOR: u32 = 0;
pub const MSG_PLAYERMODE_WOLF: u32 = 1;
pub const MSG_PLAYERMODE_SHEEP: u32 = 2;
pub const MSG_PLAYERMODE_BOTH: u32 = 3;

const MSG_MAXROOMNAME: usize = 64;
const MSG_MAXPLAYERNAME: usize = 64;

const MSG_MAGIC: u32 = 0xAA0E1F37;

const MSG_ID_NOP: u32 = 0;
const MSG_ID_RESULT: u32 = 1;
const MSG_ID_PING: u32 = 2;
const MSG_ID_PONG: u32 = 3;
const MSG_ID_JOIN: u32 = 4;
const MSG_ID_LEAVE: u32 = 5;
const MSG_ID_RESET: u32 = 6;
const MSG_ID_REQROOMLIST: u32 = 7;
const MSG_ID_ROOMLIST: u32 = 8;
const MSG_ID_REQPLAYERLIST: u32 = 9;
const MSG_ID_PLAYERLIST: u32 = 10;
const MSG_ID_REQGAMESTATE: u32 = 11;
const MSG_ID_GAMESTATE: u32 = 12;
const MSG_ID_MOVE: u32 = 13;
const MSG_ID_SAY: u32 = 14;
const MSG_ID_REQRECORD: u32 = 15;
const MSG_ID_RECORD: u32 = 16;

type FieldsArray = [[u32; BOARD_WIDTH as usize]; BOARD_HEIGHT as usize];

#[derive(Debug)]
pub enum MsgType<'a> {
    #[allow(dead_code)]
    Nop(&'a MsgNop),
    Result(&'a MsgResult),
    Ping(&'a MsgPing),
    #[allow(dead_code)]
    Pong(&'a MsgPong),
    Join(&'a MsgJoin),
    Leave(&'a MsgLeave),
    Reset(&'a MsgReset),
    ReqRoomList(&'a MsgReqRoomList),
    RoomList(&'a MsgRoomList),
    ReqPlayerList(&'a MsgReqPlayerList),
    PlayerList(&'a MsgPlayerList),
    ReqGameState(&'a MsgReqGameState),
    GameState(&'a MsgGameState),
    Move(&'a MsgMove),
    Say(&'a MsgSay),
    ReqRecord(&'a MsgReqRecord),
    Record(&'a MsgRecord),
}

pub trait Message {
    fn get_header(&self) -> &MsgHeader;
    fn get_header_mut(&mut self) -> &mut MsgHeader;
    fn to_bytes(&self) -> Vec<u8>;
    fn get_message(&self) -> MsgType<'_>;
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
            if let Ok(MSG_MAGIC) = u32::from_net(&data[skip..]) {
                return Some(skip);
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

    let (offset, header) = MsgHeader::from_bytes(data)?;
    let msg_len = header.get_size();
    if data.len() < msg_len as usize {
        return Ok((0, None));
    }

    let (_sub_size, message) = match header.get_id() {
        MSG_ID_NOP => MsgNop::from_bytes(header, &data[offset..])?,
        MSG_ID_RESULT => MsgResult::from_bytes(header, &data[offset..])?,
        MSG_ID_PING => MsgPing::from_bytes(header, &data[offset..])?,
        MSG_ID_PONG => MsgPong::from_bytes(header, &data[offset..])?,
        MSG_ID_JOIN => MsgJoin::from_bytes(header, &data[offset..])?,
        MSG_ID_LEAVE => MsgLeave::from_bytes(header, &data[offset..])?,
        MSG_ID_RESET => MsgReset::from_bytes(header, &data[offset..])?,
        MSG_ID_REQROOMLIST => MsgReqRoomList::from_bytes(header, &data[offset..])?,
        MSG_ID_ROOMLIST => MsgRoomList::from_bytes(header, &data[offset..])?,
        MSG_ID_REQPLAYERLIST => MsgReqPlayerList::from_bytes(header, &data[offset..])?,
        MSG_ID_PLAYERLIST => MsgPlayerList::from_bytes(header, &data[offset..])?,
        MSG_ID_REQGAMESTATE => MsgReqGameState::from_bytes(header, &data[offset..])?,
        MSG_ID_GAMESTATE => MsgGameState::from_bytes(header, &data[offset..])?,
        MSG_ID_MOVE => MsgMove::from_bytes(header, &data[offset..])?,
        MSG_ID_SAY => MsgSay::from_bytes(header, &data[offset..])?,
        MSG_ID_REQRECORD => MsgReqRecord::from_bytes(header, &data[offset..])?,
        MSG_ID_RECORD => MsgRecord::from_bytes(header, &data[offset..])?,
        _ => {
            return Err(ah::format_err!(
                "from_bytes: Unknown ID ({}).",
                header.get_id()
            ));
        }
    };

    Ok((msg_len as usize, Some(message)))
}

/// Common message implementation details.
macro_rules! msg_trait_define_common {
    ($msg_type:ident) => {
        fn get_header(&self) -> &MsgHeader {
            &self.header
        }

        fn get_header_mut(&mut self) -> &mut MsgHeader {
            &mut self.header
        }

        fn get_message(&self) -> MsgType<'_> {
            MsgType::$msg_type(self)
        }
    };
}

macro_rules! extract_str {
    ($max_len:path, $data:expr, $offset:expr) => {{
        let mut offset = $offset;
        let len = min(u32::from_net(&$data[offset..])?, $max_len as u32);
        offset += 4;
        let mut bytes = [0; $max_len];
        bytes[0..(len as usize)].copy_from_slice(&$data[offset..offset + (len as usize)]);
        offset += $max_len;
        (len, bytes, offset)
    }};
}

//////////////////////////////////////////////////////////////////////////////
// Message header.
//////////////////////////////////////////////////////////////////////////////

#[derive(Clone, Debug)]
pub struct MsgHeader {
    magic: u32,
    size: u32,
    id: u32,
    sequence: u32,
    reserved: [u32; 4],
}

const MSG_HEADER_SIZE: u32 = 4 * 8;

impl MsgHeader {
    fn new(magic: u32, size: u32, id: u32, sequence: u32) -> MsgHeader {
        MsgHeader {
            magic,
            size,
            id,
            sequence,
            reserved: [0; 4],
        }
    }

    pub fn get_size(&self) -> u32 {
        self.size
    }

    pub fn get_id(&self) -> u32 {
        self.id
    }

    pub fn get_sequence(&self) -> u32 {
        self.sequence
    }

    pub fn set_sequence(&mut self, sequence: u32) {
        self.sequence = sequence;
    }

    pub fn from_bytes(data: &[u8]) -> ah::Result<(usize, MsgHeader)> {
        if data.len() >= MSG_HEADER_SIZE as usize {
            let mut offset = 0;
            let magic = u32::from_net(&data[offset..])?;
            offset += 4;
            if magic != MSG_MAGIC {
                return Err(ah::format_err!(
                    "from_bytes: Invalid Message magic (0x{:X} != 0x{:X}).",
                    magic,
                    MSG_MAGIC
                ));
            }
            let size = u32::from_net(&data[offset..])?;
            offset += 4;
            if size < MSG_HEADER_SIZE {
                return Err(ah::format_err!(
                    "from_bytes: Invalid Message length ({} < {}).",
                    size,
                    MSG_HEADER_SIZE
                ));
            }
            if size > MSG_BUFFER_SIZE as u32 {
                return Err(ah::format_err!(
                    "from_bytes: Invalid Message length ({} > {}).",
                    size,
                    MSG_BUFFER_SIZE
                ));
            }
            let id = u32::from_net(&data[offset..])?;
            offset += 4;
            let sequence = u32::from_net(&data[offset..])?;
            offset += 4;
            // Skip reserved.
            offset += 4 * 4;

            let header = MsgHeader::new(magic, size, id, sequence);
            assert_eq!(offset, MSG_HEADER_SIZE as usize);
            Ok((offset, header))
        } else {
            Err(ah::format_err!("MsgHeader: Not enough data."))
        }
    }

    pub fn to_bytes(&self, data: &mut Vec<u8>) {
        let initial_len = data.len();
        data.extend_from_slice(&self.magic.to_net());
        data.extend_from_slice(&self.size.to_net());
        data.extend_from_slice(&self.id.to_net());
        data.extend_from_slice(&self.sequence.to_net());
        for word in &self.reserved {
            data.extend_from_slice(&word.to_net());
        }
        assert_eq!(data.len() - initial_len, MSG_HEADER_SIZE as usize);
    }
}

//////////////////////////////////////////////////////////////////////////////
// Trivial messages without payload.
//////////////////////////////////////////////////////////////////////////////

macro_rules! define_trivial_message {
    ($struct_name:ident, $msg_type:ident, $id:ident) => {
        #[derive(Clone, Debug)]
        pub struct $struct_name {
            header: MsgHeader,
        }

        impl $struct_name {
            pub fn new() -> $struct_name {
                $struct_name {
                    header: MsgHeader::new(MSG_MAGIC, MSG_HEADER_SIZE, $id, 0),
                }
            }

            pub fn from_bytes(
                header: MsgHeader,
                _data: &[u8],
            ) -> ah::Result<(usize, Box<dyn Message>)> {
                Ok((0, Box::new($struct_name { header })))
            }
        }

        impl Message for $struct_name {
            msg_trait_define_common!($msg_type);

            fn to_bytes(&self) -> Vec<u8> {
                let mut data = Vec::with_capacity(MSG_HEADER_SIZE as usize);
                self.header.to_bytes(&mut data);
                assert_eq!(data.len(), MSG_HEADER_SIZE as usize);
                data
            }
        }
    };
}

define_trivial_message!(MsgNop, Nop, MSG_ID_NOP);
define_trivial_message!(MsgPing, Ping, MSG_ID_PING);
define_trivial_message!(MsgPong, Pong, MSG_ID_PONG);
define_trivial_message!(MsgLeave, Leave, MSG_ID_LEAVE);
define_trivial_message!(MsgReset, Reset, MSG_ID_RESET);
define_trivial_message!(MsgReqRoomList, ReqRoomList, MSG_ID_REQROOMLIST);
define_trivial_message!(MsgReqPlayerList, ReqPlayerList, MSG_ID_REQPLAYERLIST);
define_trivial_message!(MsgReqGameState, ReqGameState, MSG_ID_REQGAMESTATE);
define_trivial_message!(MsgReqRecord, ReqRecord, MSG_ID_REQRECORD);

//////////////////////////////////////////////////////////////////////////////
// MsgResult
//////////////////////////////////////////////////////////////////////////////

#[derive(Clone, Debug)]
pub struct MsgResult {
    header: MsgHeader,
    in_reply_to_header: MsgHeader,
    result_code: u32,
    message_len: u32,
    message: [u8; MSG_RESULT_MAXMSGLEN],
}

const MSG_RESULT_MAXMSGLEN: usize = 0x200;
const MSG_RESULT_SIZE: u32 =
    MSG_HEADER_SIZE + MSG_HEADER_SIZE + (2 * 4) + MSG_RESULT_MAXMSGLEN as u32;

pub const MSG_RESULT_OK: u32 = 0;
pub const MSG_RESULT_NOK: u32 = 1;
#[allow(dead_code)]
pub const MSG_RESULT_USER: u32 = 0x10000;

impl MsgResult {
    pub fn new(
        in_reply_to_msg: &dyn Message,
        result_code: u32,
        message: &str,
    ) -> ah::Result<MsgResult> {
        let mut message_bytes = [0; MSG_RESULT_MAXMSGLEN];
        let message_len = match message.to_net(&mut message_bytes, true) {
            Ok(message_len) => message_len as u32,
            Err(_) => 0,
        };
        Ok(MsgResult {
            header: MsgHeader::new(MSG_MAGIC, MSG_RESULT_SIZE, MSG_ID_RESULT, 0),
            in_reply_to_header: in_reply_to_msg.get_header().clone(),
            result_code,
            message_len,
            message: message_bytes,
        })
    }

    pub fn from_bytes(header: MsgHeader, data: &[u8]) -> ah::Result<(usize, Box<dyn Message>)> {
        if data.len() >= (MSG_RESULT_SIZE - MSG_HEADER_SIZE) as usize {
            let mut offset = 0;

            let (count, in_reply_to_header) = MsgHeader::from_bytes(&data[offset..])?;
            offset += count;
            let result_code = u32::from_net(&data[offset..])?;
            offset += 4;
            let (message_len, message, offset) = extract_str!(MSG_RESULT_MAXMSGLEN, data, offset);

            let msg = MsgResult {
                header,
                in_reply_to_header,
                result_code,
                message_len,
                message,
            };
            assert_eq!(offset, (MSG_RESULT_SIZE - MSG_HEADER_SIZE) as usize);
            Ok((offset, Box::new(msg)))
        } else {
            Err(ah::format_err!("MsgResult: Not enough data."))
        }
    }

    pub fn is_in_reply_to(&self, other: &dyn Message) -> bool {
        let repl_header = &self.in_reply_to_header;
        let other_header = other.get_header();

        repl_header.get_id() == other_header.get_id()
            && repl_header.get_sequence() == other_header.get_sequence()
    }

    pub fn get_result_code(&self) -> u32 {
        self.result_code
    }

    pub fn is_ok(&self) -> bool {
        self.get_result_code() == MSG_RESULT_OK
    }

    pub fn get_text(&self) -> String {
        match String::from_net(&self.message, self.message_len as usize, true) {
            Ok(m) => m,
            Err(_) => "Failed to parse MsgResult.".to_string(),
        }
    }
}

impl Message for MsgResult {
    msg_trait_define_common!(Result);

    fn to_bytes(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(MSG_RESULT_SIZE as usize);
        self.header.to_bytes(&mut data);
        self.in_reply_to_header.to_bytes(&mut data);
        data.extend_from_slice(&self.result_code.to_net());
        data.extend_from_slice(&self.message_len.to_net());
        data.extend_from_slice(&self.message);
        assert_eq!(data.len(), MSG_RESULT_SIZE as usize);
        data
    }
}

//////////////////////////////////////////////////////////////////////////////
// MsgJoin
//////////////////////////////////////////////////////////////////////////////

#[derive(Clone, Debug)]
pub struct MsgJoin {
    header: MsgHeader,
    room_name_len: u32,
    room_name: [u8; MSG_MAXROOMNAME],
    player_name_len: u32,
    player_name: [u8; MSG_MAXPLAYERNAME],
    player_mode: u32,
}

const MSG_JOIN_SIZE: u32 =
    MSG_HEADER_SIZE + 4 + MSG_MAXROOMNAME as u32 + 4 + MSG_MAXPLAYERNAME as u32 + 4;

impl MsgJoin {
    pub fn new(room_name: &str, player_name: &str, player_mode: u32) -> ah::Result<MsgJoin> {
        let mut room_name_bytes = [0; MSG_MAXROOMNAME];
        let room_name_len = room_name.to_net(&mut room_name_bytes, false)? as u32;
        let mut player_name_bytes = [0; MSG_MAXPLAYERNAME];
        let player_name_len = player_name.to_net(&mut player_name_bytes, false)? as u32;
        Ok(MsgJoin {
            header: MsgHeader::new(MSG_MAGIC, MSG_JOIN_SIZE, MSG_ID_JOIN, 0),
            room_name_len,
            room_name: room_name_bytes,
            player_name_len,
            player_name: player_name_bytes,
            player_mode,
        })
    }

    pub fn from_bytes(header: MsgHeader, data: &[u8]) -> ah::Result<(usize, Box<dyn Message>)> {
        if data.len() >= (MSG_JOIN_SIZE - MSG_HEADER_SIZE) as usize {
            let offset = 0;

            let (room_name_len, room_name, offset) = extract_str!(MSG_MAXROOMNAME, data, offset);
            let (player_name_len, player_name, mut offset) =
                extract_str!(MSG_MAXPLAYERNAME, data, offset);
            let player_mode = u32::from_net(&data[offset..])?;
            offset += 4;

            let msg = MsgJoin {
                header,
                room_name_len,
                room_name,
                player_name_len,
                player_name,
                player_mode,
            };
            assert_eq!(offset, (MSG_JOIN_SIZE - MSG_HEADER_SIZE) as usize);
            Ok((offset, Box::new(msg)))
        } else {
            Err(ah::format_err!("MsgJoin: Not enough data."))
        }
    }

    pub fn get_room_name(&self) -> ah::Result<String> {
        String::from_net(&self.room_name, self.room_name_len as usize, false)
    }

    pub fn get_player_name(&self) -> ah::Result<String> {
        String::from_net(&self.player_name, self.player_name_len as usize, false)
    }

    pub fn get_player_mode(&self) -> u32 {
        self.player_mode
    }
}

impl Message for MsgJoin {
    msg_trait_define_common!(Join);

    fn to_bytes(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(MSG_JOIN_SIZE as usize);
        self.header.to_bytes(&mut data);
        data.extend_from_slice(&self.room_name_len.to_net());
        data.extend_from_slice(&self.room_name);
        data.extend_from_slice(&self.player_name_len.to_net());
        data.extend_from_slice(&self.player_name);
        data.extend_from_slice(&self.player_mode.to_net());
        assert_eq!(data.len(), MSG_JOIN_SIZE as usize);
        data
    }
}

//////////////////////////////////////////////////////////////////////////////
// MsgGameState
//////////////////////////////////////////////////////////////////////////////

#[derive(Clone, Debug)]
pub struct MsgGameState {
    header: MsgHeader,
    fields: FieldsArray,
    moving_state: u32,
    moving_x: u32,
    moving_y: u32,
    turn: u32,
}

const MSG_GAME_STATE_SIZE: u32 =
    MSG_HEADER_SIZE + (BOARD_WIDTH as u32 * BOARD_HEIGHT as u32 * 4) + (4 * 4);

const MSG_FIELD_INVALID: u32 = 0;

impl MsgGameState {
    pub fn new(
        fields: FieldsArray,
        moving_state: u32,
        moving_x: u32,
        moving_y: u32,
        turn: u32,
    ) -> MsgGameState {
        MsgGameState {
            header: MsgHeader::new(MSG_MAGIC, MSG_GAME_STATE_SIZE, MSG_ID_GAMESTATE, 0),
            fields,
            moving_state,
            moving_x,
            moving_y,
            turn,
        }
    }

    #[allow(clippy::needless_range_loop)]
    pub fn from_bytes(header: MsgHeader, data: &[u8]) -> ah::Result<(usize, Box<dyn Message>)> {
        if data.len() >= (MSG_GAME_STATE_SIZE - MSG_HEADER_SIZE) as usize {
            let mut offset = 0;

            let mut fields = [[MSG_FIELD_INVALID; BOARD_WIDTH as usize]; BOARD_HEIGHT as usize];
            for y in 0..(BOARD_HEIGHT as usize) {
                for x in 0..(BOARD_WIDTH as usize) {
                    fields[y][x] = u32::from_net(&data[offset..])?;
                    offset += 4;
                }
            }
            let moving_state = u32::from_net(&data[offset..])?;
            offset += 4;
            let moving_x = u32::from_net(&data[offset..])?;
            offset += 4;
            let moving_y = u32::from_net(&data[offset..])?;
            offset += 4;
            let turn = u32::from_net(&data[offset..])?;
            offset += 4;

            let msg = MsgGameState {
                header,
                fields,
                moving_state,
                moving_x,
                moving_y,
                turn,
            };
            assert_eq!(offset, (MSG_GAME_STATE_SIZE - MSG_HEADER_SIZE) as usize);
            Ok((offset, Box::new(msg)))
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
    msg_trait_define_common!(GameState);

    fn to_bytes(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(MSG_GAME_STATE_SIZE as usize);
        self.header.to_bytes(&mut data);
        for y in 0..(BOARD_HEIGHT as usize) {
            for x in 0..(BOARD_WIDTH as usize) {
                data.extend_from_slice(&self.fields[y][x].to_net());
            }
        }
        data.extend_from_slice(&self.moving_state.to_net());
        data.extend_from_slice(&self.moving_x.to_net());
        data.extend_from_slice(&self.moving_y.to_net());
        data.extend_from_slice(&self.turn.to_net());
        assert_eq!(data.len(), MSG_GAME_STATE_SIZE as usize);
        data
    }
}

//////////////////////////////////////////////////////////////////////////////
// MsgRecord
//////////////////////////////////////////////////////////////////////////////

#[derive(Clone, Debug)]
pub struct MsgRecord {
    header: MsgHeader,
    total_count: u32,
    index: u32,
    record_len: u32,
    record: [u8; MSG_MAXRECORDLEN],
}

const MSG_MAXRECORDLEN: usize = 0x200;
const MSG_RECORD_SIZE: u32 = MSG_HEADER_SIZE + (3 * 4) + MSG_MAXRECORDLEN as u32;

impl MsgRecord {
    pub fn new(record: &str) -> Vec<MsgRecord> {
        let record_bytes = record.as_bytes();
        let total_count = record_bytes.len().div_ceil(MSG_MAXRECORDLEN);
        let mut ret = Vec::with_capacity(total_count);
        for i in 0..total_count {
            let len = min(
                record_bytes.len() - (i * MSG_MAXRECORDLEN),
                MSG_MAXRECORDLEN,
            );
            let mut rec = [0; MSG_MAXRECORDLEN];
            rec[0..len]
                .copy_from_slice(&record_bytes[i * MSG_MAXRECORDLEN..i * MSG_MAXRECORDLEN + len]);
            ret.push(MsgRecord {
                header: MsgHeader::new(MSG_MAGIC, MSG_RECORD_SIZE, MSG_ID_RECORD, 0),
                total_count: total_count as u32,
                index: i as u32,
                record_len: len as u32,
                record: rec,
            })
        }
        ret
    }

    pub fn from_bytes(header: MsgHeader, data: &[u8]) -> ah::Result<(usize, Box<dyn Message>)> {
        if data.len() >= (MSG_RECORD_SIZE - MSG_HEADER_SIZE) as usize {
            let mut offset = 0;

            let total_count = u32::from_net(&data[offset..])?;
            offset += 4;
            let index = u32::from_net(&data[offset..])?;
            offset += 4;
            let (record_len, record, offset) = extract_str!(MSG_MAXRECORDLEN, data, offset);

            let msg = MsgRecord {
                header,
                total_count,
                index,
                record_len,
                record,
            };
            assert_eq!(offset, (MSG_RECORD_SIZE - MSG_HEADER_SIZE) as usize);
            Ok((offset, Box::new(msg)))
        } else {
            Err(ah::format_err!("MsgRecord: Not enough data."))
        }
    }

    pub fn get_total_count(&self) -> u32 {
        self.total_count
    }

    pub fn get_index(&self) -> u32 {
        self.index
    }

    fn get_record_part(&self) -> &[u8] {
        &self.record[0..self.record_len as usize]
    }

    pub fn assemble_parts(parts: Vec<MsgRecord>) -> ah::Result<String> {
        let bytes: Vec<u8> = parts
            .iter()
            .map(|m| m.get_record_part())
            .fold(vec![], |mut a, b| {
                a.extend_from_slice(b);
                a
            });
        String::from_net(&bytes, bytes.len(), false)
    }
}

impl Message for MsgRecord {
    msg_trait_define_common!(Record);

    fn to_bytes(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(MSG_RECORD_SIZE as usize);
        self.header.to_bytes(&mut data);
        data.extend_from_slice(&self.total_count.to_net());
        data.extend_from_slice(&self.index.to_net());
        data.extend_from_slice(&self.record_len.to_net());
        data.extend_from_slice(&self.record);
        assert_eq!(data.len(), MSG_RECORD_SIZE as usize);
        data
    }
}

//////////////////////////////////////////////////////////////////////////////
// MsgRoomList
//////////////////////////////////////////////////////////////////////////////

#[derive(Clone, Debug)]
pub struct MsgRoomList {
    header: MsgHeader,
    total_count: u32,
    index: u32,
    room_name_len: u32,
    room_name: [u8; MSG_MAXROOMNAME],
}

const MSG_ROOM_LIST_SIZE: u32 = MSG_HEADER_SIZE + (3 * 4) + MSG_MAXROOMNAME as u32;

impl MsgRoomList {
    pub fn new(total_count: u32, index: u32, room_name: &str) -> ah::Result<MsgRoomList> {
        let mut room_name_bytes = [0; MSG_MAXROOMNAME];
        let room_name_len = room_name.to_net(&mut room_name_bytes, false)? as u32;
        Ok(MsgRoomList {
            header: MsgHeader::new(MSG_MAGIC, MSG_ROOM_LIST_SIZE, MSG_ID_ROOMLIST, 0),
            total_count,
            index,
            room_name_len,
            room_name: room_name_bytes,
        })
    }

    pub fn from_bytes(header: MsgHeader, data: &[u8]) -> ah::Result<(usize, Box<dyn Message>)> {
        if data.len() >= (MSG_ROOM_LIST_SIZE - MSG_HEADER_SIZE) as usize {
            let mut offset = 0;

            let total_count = u32::from_net(&data[offset..])?;
            offset += 4;
            let index = u32::from_net(&data[offset..])?;
            offset += 4;
            let (room_name_len, room_name, offset) = extract_str!(MSG_MAXROOMNAME, data, offset);

            let msg = MsgRoomList {
                header,
                total_count,
                index,
                room_name_len,
                room_name,
            };
            assert_eq!(offset, (MSG_ROOM_LIST_SIZE - MSG_HEADER_SIZE) as usize);
            Ok((offset, Box::new(msg)))
        } else {
            Err(ah::format_err!("MsgRoomList: Not enough data."))
        }
    }

    pub fn get_total_count(&self) -> u32 {
        self.total_count
    }

    pub fn get_index(&self) -> u32 {
        self.index
    }

    pub fn get_room_name(&self) -> ah::Result<String> {
        String::from_net(&self.room_name, self.room_name_len as usize, false)
    }
}

impl Message for MsgRoomList {
    msg_trait_define_common!(RoomList);

    fn to_bytes(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(MSG_ROOM_LIST_SIZE as usize);
        self.header.to_bytes(&mut data);
        data.extend_from_slice(&self.total_count.to_net());
        data.extend_from_slice(&self.index.to_net());
        data.extend_from_slice(&self.room_name_len.to_net());
        data.extend_from_slice(&self.room_name);
        assert_eq!(data.len(), MSG_ROOM_LIST_SIZE as usize);
        data
    }
}

//////////////////////////////////////////////////////////////////////////////
// MsgPlayerList
//////////////////////////////////////////////////////////////////////////////

#[derive(Clone, Debug)]
pub struct MsgPlayerList {
    header: MsgHeader,
    total_count: u32,
    index: u32,
    player_name_len: u32,
    player_name: [u8; MSG_MAXPLAYERNAME],
    player_mode: u32,
}

const MSG_PLAYER_LIST_SIZE: u32 = MSG_HEADER_SIZE + (3 * 4) + MSG_MAXPLAYERNAME as u32 + 4;

impl MsgPlayerList {
    pub fn new(
        total_count: u32,
        index: u32,
        player_name: &str,
        player_mode: u32,
    ) -> ah::Result<MsgPlayerList> {
        let mut player_name_bytes = [0; MSG_MAXPLAYERNAME];
        let player_name_len = player_name.to_net(&mut player_name_bytes, false)? as u32;
        Ok(MsgPlayerList {
            header: MsgHeader::new(MSG_MAGIC, MSG_PLAYER_LIST_SIZE, MSG_ID_PLAYERLIST, 0),
            total_count,
            index,
            player_name_len,
            player_name: player_name_bytes,
            player_mode,
        })
    }

    pub fn from_bytes(header: MsgHeader, data: &[u8]) -> ah::Result<(usize, Box<dyn Message>)> {
        if data.len() >= (MSG_PLAYER_LIST_SIZE - MSG_HEADER_SIZE) as usize {
            let mut offset = 0;

            let total_count = u32::from_net(&data[offset..])?;
            offset += 4;
            let index = u32::from_net(&data[offset..])?;
            offset += 4;
            let (player_name_len, player_name, mut offset) =
                extract_str!(MSG_MAXPLAYERNAME, data, offset);
            let player_mode = u32::from_net(&data[offset..])?;
            offset += 4;

            let msg = MsgPlayerList {
                header,
                total_count,
                index,
                player_name_len,
                player_name,
                player_mode,
            };
            assert_eq!(offset, (MSG_PLAYER_LIST_SIZE - MSG_HEADER_SIZE) as usize);
            Ok((offset, Box::new(msg)))
        } else {
            Err(ah::format_err!("MsgPlayerList: Not enough data."))
        }
    }

    pub fn get_total_count(&self) -> u32 {
        self.total_count
    }

    pub fn get_index(&self) -> u32 {
        self.index
    }

    pub fn get_player_name(&self) -> ah::Result<String> {
        String::from_net(&self.player_name, self.player_name_len as usize, false)
    }

    pub fn get_player_mode(&self) -> u32 {
        self.player_mode
    }
}

impl Message for MsgPlayerList {
    msg_trait_define_common!(PlayerList);

    fn to_bytes(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(MSG_PLAYER_LIST_SIZE as usize);
        self.header.to_bytes(&mut data);
        data.extend_from_slice(&self.total_count.to_net());
        data.extend_from_slice(&self.index.to_net());
        data.extend_from_slice(&self.player_name_len.to_net());
        data.extend_from_slice(&self.player_name);
        data.extend_from_slice(&self.player_mode.to_net());
        assert_eq!(data.len(), MSG_PLAYER_LIST_SIZE as usize);
        data
    }
}

//////////////////////////////////////////////////////////////////////////////
// MsgMove
//////////////////////////////////////////////////////////////////////////////

#[derive(Clone, Debug)]
pub struct MsgMove {
    header: MsgHeader,
    action: u32,
    token: u32,
    coord_x: u32,
    coord_y: u32,
}

const MSG_MOVE_SIZE: u32 = MSG_HEADER_SIZE + (4 * 4);

pub const MSG_MOVE_ACTION_PICK: u32 = 0;
pub const MSG_MOVE_ACTION_MOVE: u32 = 1;
pub const MSG_MOVE_ACTION_PUT: u32 = 2;
pub const MSG_MOVE_ACTION_ABORT: u32 = 3;

pub const MSG_MOVE_TOKEN_CURRENT: u32 = 0;
pub const MSG_MOVE_TOKEN_WOLF: u32 = 1;
pub const MSG_MOVE_TOKEN_SHEEP: u32 = 2;

impl MsgMove {
    pub fn new(action: u32, token: u32, coord_x: u32, coord_y: u32) -> MsgMove {
        MsgMove {
            header: MsgHeader::new(MSG_MAGIC, MSG_MOVE_SIZE, MSG_ID_MOVE, 0),
            action,
            token,
            coord_x,
            coord_y,
        }
    }

    pub fn from_bytes(header: MsgHeader, data: &[u8]) -> ah::Result<(usize, Box<dyn Message>)> {
        if data.len() >= (MSG_MOVE_SIZE - MSG_HEADER_SIZE) as usize {
            let mut offset = 0;

            let action = u32::from_net(&data[offset..])?;
            offset += 4;
            let token = u32::from_net(&data[offset..])?;
            offset += 4;
            let coord_x = u32::from_net(&data[offset..])?;
            offset += 4;
            let coord_y = u32::from_net(&data[offset..])?;
            offset += 4;

            let msg = MsgMove {
                header,
                action,
                token,
                coord_x,
                coord_y,
            };
            assert_eq!(offset, (MSG_MOVE_SIZE - MSG_HEADER_SIZE) as usize);
            Ok((offset, Box::new(msg)))
        } else {
            Err(ah::format_err!("MsgMove: Not enough data."))
        }
    }

    pub fn get_action(&self) -> (u32, u32, u32) {
        (self.action, self.coord_x, self.coord_y)
    }
}

impl Message for MsgMove {
    msg_trait_define_common!(Move);

    fn to_bytes(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(MSG_MOVE_SIZE as usize);
        self.header.to_bytes(&mut data);
        data.extend_from_slice(&self.action.to_net());
        data.extend_from_slice(&self.token.to_net());
        data.extend_from_slice(&self.coord_x.to_net());
        data.extend_from_slice(&self.coord_y.to_net());
        assert_eq!(data.len(), MSG_MOVE_SIZE as usize);
        data
    }
}

//////////////////////////////////////////////////////////////////////////////
// MsgSay
//////////////////////////////////////////////////////////////////////////////

#[derive(Clone, Debug)]
pub struct MsgSay {
    header: MsgHeader,
    player_name_len: u32,
    player_name: [u8; MSG_MAXPLAYERNAME],
    message_len: u32,
    message: [u8; MSG_SAY_MAXMSGLEN],
}

const MSG_SAY_MAXMSGLEN: usize = 0x200;
const MSG_SAY_SIZE: u32 =
    MSG_HEADER_SIZE + 4 + MSG_MAXPLAYERNAME as u32 + 4 + MSG_SAY_MAXMSGLEN as u32;

impl MsgSay {
    pub fn new(player_name: &str, message: &str) -> ah::Result<MsgSay> {
        let mut player_name_bytes = [0; MSG_MAXPLAYERNAME];
        let player_name_len = player_name.to_net(&mut player_name_bytes, true)?;
        let mut message_bytes = [0; MSG_SAY_MAXMSGLEN];
        let message_len = message.to_net(&mut message_bytes, true)?;
        Ok(MsgSay {
            header: MsgHeader::new(MSG_MAGIC, MSG_SAY_SIZE, MSG_ID_SAY, 0),
            player_name_len: player_name_len as u32,
            player_name: player_name_bytes,
            message_len: message_len as u32,
            message: message_bytes,
        })
    }

    pub fn from_bytes(header: MsgHeader, data: &[u8]) -> ah::Result<(usize, Box<dyn Message>)> {
        if data.len() >= (MSG_SAY_SIZE - MSG_HEADER_SIZE) as usize {
            let offset = 0;

            let (player_name_len, player_name, offset) =
                extract_str!(MSG_MAXPLAYERNAME, data, offset);
            let (message_len, message, offset) = extract_str!(MSG_SAY_MAXMSGLEN, data, offset);

            let msg = MsgSay {
                header,
                player_name_len,
                player_name,
                message_len,
                message,
            };
            assert_eq!(offset, (MSG_SAY_SIZE - MSG_HEADER_SIZE) as usize);
            Ok((offset, Box::new(msg)))
        } else {
            Err(ah::format_err!("MsgSay: Not enough data."))
        }
    }

    pub fn set_player_name(&mut self, player_name: &str) -> ah::Result<()> {
        self.player_name_len = player_name.to_net(&mut self.player_name, true)? as u32;
        Ok(())
    }

    pub fn get_player_name(&self) -> String {
        match String::from_net(&self.player_name, self.player_name_len as usize, true) {
            Ok(m) => m,
            Err(_) => "Failed to parse MsgSay.".to_string(),
        }
    }

    pub fn get_text(&self) -> String {
        match String::from_net(&self.message, self.message_len as usize, true) {
            Ok(m) => m,
            Err(_) => "Failed to parse MsgSay.".to_string(),
        }
    }
}

impl Message for MsgSay {
    msg_trait_define_common!(Say);

    fn to_bytes(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(MSG_SAY_SIZE as usize);
        self.header.to_bytes(&mut data);
        data.extend_from_slice(&self.player_name_len.to_net());
        data.extend_from_slice(&self.player_name);
        data.extend_from_slice(&self.message_len.to_net());
        data.extend_from_slice(&self.message);
        assert_eq!(data.len(), MSG_SAY_SIZE as usize);
        data
    }
}

// vim: ts=4 sw=4 expandtab
