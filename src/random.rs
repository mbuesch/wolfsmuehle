// -*- coding: utf-8 -*-
//
// Copyright 2021 Michael Buesch <m@bues.ch>
//
// SPDX-License-Identifier: MIT OR Apache-2.0
//

use rand::{Rng, distr::Alphanumeric, rng};

pub fn random_alphanum(num_chars: usize) -> String {
    std::iter::repeat(())
        .map(|_| rng().sample(Alphanumeric))
        .map(char::from)
        .take(num_chars)
        .collect()
}

// vim: ts=4 sw=4 expandtab
