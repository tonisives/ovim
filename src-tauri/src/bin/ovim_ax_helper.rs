//! Entry point for the accessibility helper binary
//!
//! This is a standalone binary - the ax_helper module must be self-contained.

#![allow(unexpected_cfgs)]

#[path = "../ax_helper/mod.rs"]
mod ax_helper;

fn main() {
    ax_helper::main();
}
