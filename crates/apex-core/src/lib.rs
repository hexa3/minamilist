//! Platform-independent editing state for minamilist.

mod buffer;
mod command;
mod project;
mod telemetry;
mod workspace;

pub use buffer::{Buffer, BufferError, Cursor};
pub use command::{Command, CommandId, CommandRegistry};
pub use project::{ProjectEntry, ProjectIndex};
pub use telemetry::{LatencyBudget, LatencySample};
pub use workspace::{EditorWorkspace, Pane, PaneId, WorkspaceError};
