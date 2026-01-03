//! Entry point for the accessibility helper binary
//!
//! This is a standalone binary - the ax_helper module must be self-contained.

#[path = "../ax_helper/mod.rs"]
#[allow(unexpected_cfgs)]
mod ax_helper;

fn main() {
    ax_helper::main();
}
