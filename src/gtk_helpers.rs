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

pub use gdk;
pub use gdk::prelude::*;
pub use gdk_pixbuf;
pub use gio::prelude::*;
pub use glib;
pub use gtk;
pub use gtk::prelude::*;

#[macro_export]
macro_rules! gsigparam {
    ($param:expr, $type:ty) => {
        $param.get::<$type>().unwrap()
    };
}

#[macro_export]
macro_rules! gsignal_connect_to {
    ($instance:ident, $method:ident, $error_return:expr) => {
        Box::new(move |param| match $instance.try_borrow() {
            Ok(inst) => inst.$method(param),
            Err(_) => $error_return,
        })
    };
}

#[macro_export]
macro_rules! gsignal_connect_to_mut {
    ($instance:ident, $method:ident, $error_return:expr) => {
        Box::new(move |param| match $instance.try_borrow_mut() {
            Ok(mut inst) => inst.$method(param),
            Err(_) => $error_return,
        })
    };
}

pub type GSigHandler = Box<dyn Fn(&[glib::Value]) -> Option<glib::Value> + 'static>;

fn prepare_message_dialog(msg: &mut gtk::MessageDialog) {
    // Make the text selectable.
    let area = msg.message_area();
    if let Some(cont) = area.downcast_ref::<gtk::Container>() {
        for child in cont.children() {
            if let Some(label) = child.downcast_ref::<gtk::Label>() {
                label.set_selectable(true);
            }
        }
    }
    // Auto-close the dialog.
    msg.connect_response(|msg, _resp| msg.close());
}

pub fn messagebox_info<T: glib::IsA<gtk::Window>>(parent: Option<&T>, text: &str) {
    let mut msg = gtk::MessageDialog::new(
        parent,
        gtk::DialogFlags::MODAL,
        gtk::MessageType::Info,
        gtk::ButtonsType::Ok,
        text,
    );
    prepare_message_dialog(&mut msg);
    msg.run();
}

pub fn messagebox_error<T: glib::IsA<gtk::Window>>(parent: Option<&T>, text: &str) {
    let mut msg = gtk::MessageDialog::new(
        parent,
        gtk::DialogFlags::MODAL,
        gtk::MessageType::Error,
        gtk::ButtonsType::Ok,
        text,
    );
    prepare_message_dialog(&mut msg);
    msg.run();
}

// vim: ts=4 sw=4 expandtab
