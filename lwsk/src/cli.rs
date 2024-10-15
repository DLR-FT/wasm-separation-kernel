use std::path::PathBuf;

use clap::Parser;

/// The Linux Wasm Seperation Kernel. Or Lighweight Wucke13 & Seven Kernel?
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Blueprint to load
    pub blueprint: PathBuf,

    /// Just parse and validate the blueprint, terminate then
    #[clap(short, long)]
    pub only_validate: bool,

    /// Require every wasm function to parse successfully
    #[clap(short, long)]
    pub strict: bool,
}
