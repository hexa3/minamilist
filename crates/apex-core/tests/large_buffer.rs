use std::time::{Duration, Instant};

use apex_core::Buffer;

#[test]
fn edits_large_rope_without_linear_pause() {
    let source = "let value = 42;\n".repeat(300_000);
    let mut buffer = Buffer::from_text(&source);
    buffer.set_cursor(buffer.len_chars() / 2);
    let started = Instant::now();
    for _ in 0..1_000 {
        buffer.insert("x");
        buffer.backspace();
    }
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "large-buffer edits exceeded smoke budget: {:?}",
        started.elapsed()
    );
}
