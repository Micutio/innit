#[macro_use]
extern crate log;
extern crate pretty_env_logger;
extern crate rand;
extern crate rand_core;
extern crate rand_isaac;
extern crate serde;
// #[macro_use]
// extern crate serde_derive;
extern crate rltk;
extern crate serde_json;

mod core;
mod entity;
mod game;
mod test;
mod ui;
mod util;

use crate::core::game_env::GameEnv;
use crate::game::{SCREEN_HEIGHT, SCREEN_WIDTH};
use std::env;

/// For game testing run with
/// (bash) `RUST_LOG=innit=trace RUST_BACKTRACE=1 cargo run`
/// (fish) `env RUST_LOG=innit=trace RUST_BACKTRACE=1 cargo run`
pub fn main() -> rltk::BError {
    pretty_env_logger::init();
    let mut env: GameEnv = GameEnv::new();

    let args: Vec<String> = env::args().collect();
    println!("args: {:?}", args);

    for arg in args {
        if arg.eq("-d") || arg.eq("--debug") {
            env.set_debug_mode(true);
        }
        if arg.eq("-s") || arg.eq("--seeding") {
            env.set_rng_seeding(true);
        }
    }

    // TODO: Create game environment from presets and command line flags!

    use rltk::RltkBuilder;
    let mut context = RltkBuilder::simple(SCREEN_WIDTH, SCREEN_HEIGHT)
        .unwrap()
        .with_advanced_input(true)
        // .with_font("fonts/rex_paint_10x10.png", 10, 10)
        // .with_fancy_console(SCREEN_WIDTH, SCREEN_HEIGHT, "fonts/rex_paint_10x10.png") // menu
        .with_font("fonts/rex_paint_14x14.png", 14, 14)
        .with_fancy_console(SCREEN_WIDTH, SCREEN_HEIGHT, "fonts/rex_paint_14x14.png") // menu
        .with_title("Innit alpha v0.0.4")
        .with_vsync(false)
        .with_fps_cap(60.0)
        .with_automatic_console_resize(true)
        .build()?;

    context.set_active_font(1, false);
    let game = game::Game::new(env);
    rltk::main_loop(context, game)
}
