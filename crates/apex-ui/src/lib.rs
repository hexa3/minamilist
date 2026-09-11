//! Native UI boundary. The wgpu renderer is implemented behind this API.

use apex_core::Buffer;

pub struct EditorApp {
    buffer: Buffer,
}

impl EditorApp {
    pub fn new(buffer: Buffer) -> Self {
        Self { buffer }
    }

    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    pub fn buffer_mut(&mut self) -> &mut Buffer {
        &mut self.buffer
    }
}
