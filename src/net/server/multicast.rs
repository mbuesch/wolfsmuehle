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

use crate::print::Print;
use std::sync::{
    Arc,
    atomic::{
        AtomicBool,
        Ordering,
    },
    mpsc::{
        channel,
        Sender,
        Receiver,
    },
};
use std::thread::sleep;
use std::time::{
    Duration,
    Instant,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MulticastSync {
    NoSync,
    ToRouter,
}

/// The packet sent over the multicast channels.
#[derive(Clone, Debug)]
pub struct MulticastPacket {
    pub data:           Vec<u8>,
    pub meta_data:      Vec<u8>,
    pub include_self:   bool,
    pub sync:           MulticastSync,
}

#[derive(Debug)]
struct MulticastRouterSub {
    from_sub:           Receiver<MulticastPacket>,
    to_sub:             Sender<MulticastPacket>,
    killed:             Arc<AtomicBool>,
    router_received:    Arc<AtomicBool>,
}

#[derive(Debug)]
pub struct MulticastRouter {
    subs:   Vec<MulticastRouterSub>,
}

impl MulticastRouter {
    /// Create a new multicast router.
    pub fn new() -> MulticastRouter {
        MulticastRouter {
            subs: vec![],
        }
    }

    /// Create a new multicast subscriber and connect it to this router.
    pub fn new_subscriber(&mut self) -> MulticastSubscriber {
        let (to_sub, from_router) = channel();
        let (to_router, from_sub) = channel();
        let killed = Arc::new(AtomicBool::new(false));
        let killed2 = Arc::clone(&killed);
        let router_received = Arc::new(AtomicBool::new(false));
        let router_received2 = Arc::clone(&router_received);
        let mc_router_sub = MulticastRouterSub {
            from_sub,
            to_sub,
            killed,
            router_received,
        };
        self.subs.push(mc_router_sub);
        MulticastSubscriber::new(from_router,
                                 to_router,
                                 killed2,
                                 router_received2)
    }

    /// Run the router main loop.
    pub fn run_router(&mut self) {
        // Remove killed subscribers.
        self.subs.retain(|sub| !sub.killed.load(Ordering::Acquire));

        // Route all broadcasts.
        for from_sub in &self.subs {
            // Does the sender have broadcast packets?
            if let Ok(pack) = from_sub.from_sub.try_recv() {
                from_sub.router_received.store(true, Ordering::SeqCst);
                // For each receiver.
                for to_sub in &self.subs {
                    // Send it.
                    // Include ourselves only, if requested.
                    if !std::ptr::eq(from_sub, to_sub) ||
                       pack.include_self {

                        if let Err(e) = to_sub.to_sub.send(pack.clone()) {
                            // This may happen, if the channel has just been closed,
                            // but the killed-check hasn't caught it, yet.
                            Print::debug(&format!("Failed to route: {}", e));
                        }
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct MulticastSubscriber {
    from_router:        Receiver<MulticastPacket>,
    to_router:          Sender<MulticastPacket>,
    killed:             Arc<AtomicBool>,
    router_received:    Arc<AtomicBool>,
}

impl MulticastSubscriber {
    fn new(from_router:     Receiver<MulticastPacket>,
           to_router:       Sender<MulticastPacket>,
           killed:          Arc<AtomicBool>,
           router_received: Arc<AtomicBool>) -> MulticastSubscriber {
        MulticastSubscriber {
            from_router,
            to_router,
            killed,
            router_received,
        }
    }

    /// Send data to all subscribers on our router.
    pub fn send_broadcast(&self, pack: MulticastPacket) {
        let sync = pack.sync;

        // Send the packet to the router.
        self.router_received.store(false, Ordering::SeqCst);
        if let Err(e) = self.to_router.send(pack) {
            Print::error(&format!("Failed to send broadcast: {}", e));
        } else {
            // Wait for the router to receive this packet, if requested.
            match sync {
                MulticastSync::NoSync => (),
                MulticastSync::ToRouter => {
                    let begin = Instant::now();
                    while !self.router_received.load(Ordering::SeqCst) {
                        if Instant::now().duration_since(begin).as_millis() > 1000 {
                            Print::error("Send broacast: Sync-to-router timeout.");
                        }
                        sleep(Duration::from_millis(10));
                    }
                },
            }
        }
    }

    /// Try to receive data from the router.
    pub fn receive(&self) -> Option<MulticastPacket> {
        if let Ok(pack) = self.from_router.try_recv() {
            Some(pack)
        } else {
            None
        }
    }
}

impl Drop for MulticastSubscriber {
    fn drop(&mut self) {
        self.killed.store(true, Ordering::Release);
    }
}

// vim: ts=4 sw=4 expandtab
