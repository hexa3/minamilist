use std::{collections::HashMap, path::Path};

use thiserror::Error;

use crate::{Buffer, BufferError};

pub type BufferId = u64;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PaneId(pub u64);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pane {
    pub id: PaneId,
    pub tabs: Vec<BufferId>,
    pub active_tab: usize,
}

#[derive(Debug)]
pub struct EditorWorkspace {
    buffers: HashMap<BufferId, Buffer>,
    panes: Vec<Pane>,
    active_pane: PaneId,
    next_id: u64,
}

#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error(transparent)]
    Buffer(#[from] BufferError),
    #[error("pane {0:?} does not exist")]
    MissingPane(PaneId),
}

impl Default for EditorWorkspace {
    fn default() -> Self {
        Self::new()
    }
}

impl EditorWorkspace {
    pub fn new() -> Self {
        let pane = Pane {
            id: PaneId(1),
            tabs: Vec::new(),
            active_tab: 0,
        };
        Self {
            buffers: HashMap::new(),
            panes: vec![pane],
            active_pane: PaneId(1),
            next_id: 2,
        }
    }

    pub fn open(&mut self, path: impl AsRef<Path>) -> Result<BufferId, WorkspaceError> {
        let buffer = Buffer::open(path)?;
        let id = self.next_id;
        self.next_id += 1;
        self.buffers.insert(id, buffer);
        let pane = self
            .panes
            .iter_mut()
            .find(|pane| pane.id == self.active_pane)
            .ok_or(WorkspaceError::MissingPane(self.active_pane))?;
        pane.tabs.push(id);
        pane.active_tab = pane.tabs.len() - 1;
        Ok(id)
    }

    pub fn split_right(&mut self) -> Result<PaneId, WorkspaceError> {
        let current = self
            .panes
            .iter()
            .find(|pane| pane.id == self.active_pane)
            .ok_or(WorkspaceError::MissingPane(self.active_pane))?;
        let id = PaneId(self.next_id);
        self.next_id += 1;
        let pane = Pane {
            id,
            tabs: current
                .tabs
                .get(current.active_tab)
                .copied()
                .into_iter()
                .collect(),
            active_tab: 0,
        };
        self.panes.push(pane);
        self.active_pane = id;
        Ok(id)
    }

    pub fn panes(&self) -> &[Pane] {
        &self.panes
    }

    pub fn buffer(&self, id: BufferId) -> Option<&Buffer> {
        self.buffers.get(&id)
    }

    pub fn buffer_mut(&mut self, id: BufferId) -> Option<&mut Buffer> {
        self.buffers.get_mut(&id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_workspace_can_split() {
        let mut workspace = EditorWorkspace::new();
        let pane = workspace.split_right().unwrap();
        assert_eq!(workspace.panes().len(), 2);
        assert_eq!(workspace.panes()[1].id, pane);
    }
}
