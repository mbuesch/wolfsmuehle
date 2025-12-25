// -*- coding: utf-8 -*-
//
// Copyright 2021 Michael Buesch <m@bues.ch>
//
// SPDX-License-Identifier: MIT OR Apache-2.0
//

pub use gtk4::{self as gtk, cairo, gdk::prelude::*, gio, glib, prelude::*};

pub fn messagebox_info<T: IsA<gtk::Window>>(parent: Option<&T>, text: &str) {
    let dlg = gtk::MessageDialog::new(
        parent,
        gtk::DialogFlags::MODAL,
        gtk::MessageType::Info,
        gtk::ButtonsType::Ok,
        text,
    );
    dlg.connect_response(|dlg, _| dlg.close());
    dlg.show();
}

pub fn messagebox_error<T: IsA<gtk::Window>>(parent: Option<&T>, text: &str) {
    let dlg = gtk::MessageDialog::new(
        parent,
        gtk::DialogFlags::MODAL,
        gtk::MessageType::Error,
        gtk::ButtonsType::Ok,
        text,
    );
    dlg.connect_response(|dlg, _| dlg.close());
    dlg.show();
}

// vim: ts=4 sw=4 expandtab
