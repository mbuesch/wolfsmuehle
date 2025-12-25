// -*- coding: utf-8 -*-
//
// Copyright 2021 Michael Buesch <m@bues.ch>
//
// SPDX-License-Identifier: MIT OR Apache-2.0
//

pub use gtk4::{
    self as gtk, cairo,
    gdk::{self, prelude::*},
    gio::{self, prelude::*},
    glib::{self, prelude::*},
    prelude::*,
};

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
    if let Some(cont) = area.downcast_ref::<gtk::Box>() {
        let mut child: Option<gtk::Widget> = cont.first_child();
        while let Some(w) = child {
            if let Some(label) = w.downcast_ref::<gtk::Label>() {
                label.set_selectable(true);
            }
            child = w.next_sibling();
        }
    }
    // Auto-close the dialog.
    msg.connect_response(|msg, _resp| msg.close());
}

pub fn messagebox_info<T: IsA<gtk::Window>>(parent: Option<&T>, text: &str) {
    let mut msg = gtk::MessageDialog::new(
        parent,
        gtk::DialogFlags::MODAL,
        gtk::MessageType::Info,
        gtk::ButtonsType::Ok,
        text,
    );
    prepare_message_dialog(&mut msg);
    msg.show();
}

pub fn messagebox_error<T: IsA<gtk::Window>>(parent: Option<&T>, text: &str) {
    let mut msg = gtk::MessageDialog::new(
        parent,
        gtk::DialogFlags::MODAL,
        gtk::MessageType::Error,
        gtk::ButtonsType::Ok,
        text,
    );
    prepare_message_dialog(&mut msg);
    msg.show();
}

// vim: ts=4 sw=4 expandtab
