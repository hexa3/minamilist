use std::env;

use anyhow::Result;
use apex_core::Buffer;
use apex_ui::EditorApp;

fn main() -> Result<()> {
    let buffer = match env::args_os().nth(1) {
        Some(path) => Buffer::open(path)?,
        None => Buffer::new(),
    };
    let app = EditorApp::new(buffer);
    println!(
        "Apex initialized ({} characters)",
        app.buffer().text().chars().count()
    );
    Ok(())
}
