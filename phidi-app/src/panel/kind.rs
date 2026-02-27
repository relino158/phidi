use serde::{Deserialize, Serialize};
use strum_macros::EnumIter;

use super::{data::PanelOrder, position::PanelPosition};
use crate::config::icon::PhidiIcons;

#[derive(
    Clone, Copy, PartialEq, Serialize, Deserialize, Hash, Eq, Debug, EnumIter,
)]
pub enum PanelKind {
    Terminal,
    FileExplorer,
    SourceControl,
    Plugin,
    Search,
    Problem,
    Debug,
    CallHierarchy,
    DocumentSymbol,
    References,
    Implementation,
}

impl PanelKind {
    pub fn svg_name(&self) -> &'static str {
        match &self {
            PanelKind::Terminal => PhidiIcons::TERMINAL,
            PanelKind::FileExplorer => PhidiIcons::FILE_EXPLORER,
            PanelKind::SourceControl => PhidiIcons::SCM,
            PanelKind::Plugin => PhidiIcons::EXTENSIONS,
            PanelKind::Search => PhidiIcons::SEARCH,
            PanelKind::Problem => PhidiIcons::PROBLEM,
            PanelKind::Debug => PhidiIcons::DEBUG,
            PanelKind::CallHierarchy => PhidiIcons::TYPE_HIERARCHY,
            PanelKind::DocumentSymbol => PhidiIcons::DOCUMENT_SYMBOL,
            PanelKind::References => PhidiIcons::REFERENCES,
            PanelKind::Implementation => PhidiIcons::IMPLEMENTATION,
        }
    }

    pub fn position(&self, order: &PanelOrder) -> Option<(usize, PanelPosition)> {
        for (pos, panels) in order.iter() {
            let index = panels.iter().position(|k| k == self);
            if let Some(index) = index {
                return Some((index, *pos));
            }
        }
        None
    }

    pub fn default_position(&self) -> PanelPosition {
        match self {
            PanelKind::Terminal => PanelPosition::BottomLeft,
            PanelKind::FileExplorer => PanelPosition::LeftTop,
            PanelKind::SourceControl => PanelPosition::LeftTop,
            PanelKind::Plugin => PanelPosition::LeftTop,
            PanelKind::Search => PanelPosition::BottomLeft,
            PanelKind::Problem => PanelPosition::BottomLeft,
            PanelKind::Debug => PanelPosition::LeftTop,
            PanelKind::CallHierarchy => PanelPosition::BottomLeft,
            PanelKind::DocumentSymbol => PanelPosition::RightTop,
            PanelKind::References => PanelPosition::BottomLeft,
            PanelKind::Implementation => PanelPosition::BottomLeft,
        }
    }
}
