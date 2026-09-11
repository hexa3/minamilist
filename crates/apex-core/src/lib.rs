//! Platform-independent editing state for Apex.

mod buffer;
mod command;
mod project;
mod workspace;

pub use buffer::{Buffer, BufferError, Cursor};
pub use command::{Command, CommandId, CommandRegistry};
pub use project::{ProjectEntry, ProjectIndex};
pub use workspace::{EditorWorkspace, Pane, PaneId, WorkspaceError};
