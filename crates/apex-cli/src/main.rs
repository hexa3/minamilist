use std::env;

use anyhow::Result;
use apex_core::Buffer;

fn main() -> Result<()> {
    let buffer = match env::args_os().nth(1) {
        Some(path) => Buffer::open(path)?,
        None => Buffer::new(),
    };
    apex_ui::run(buffer)
}
