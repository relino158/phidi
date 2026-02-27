use std::path::PathBuf;

use lsp_types::{Range, SymbolKind};
use phidi_core::line_ending::LineEnding;
use phidi_rpc::dap_types::RunDebugConfig;

use crate::{
    command::{PhidiCommand, PhidiWorkbenchCommand},
    debug::RunDebugMode,
    editor::location::EditorLocation,
    workspace::{PhidiWorkspace, SshHost},
};

#[derive(Clone, Debug, PartialEq)]
pub struct PaletteItem {
    pub content: PaletteItemContent,
    pub filter_text: String,
    pub score: u32,
    pub indices: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PaletteItemContent {
    PaletteHelp {
        cmd: PhidiWorkbenchCommand,
    },
    File {
        path: PathBuf,
        full_path: PathBuf,
    },
    Line {
        line: usize,
        content: String,
    },
    Command {
        cmd: PhidiCommand,
    },
    Workspace {
        workspace: PhidiWorkspace,
    },
    Reference {
        path: PathBuf,
        location: EditorLocation,
    },
    DocumentSymbol {
        kind: SymbolKind,
        name: String,
        range: Range,
        container_name: Option<String>,
    },
    WorkspaceSymbol {
        kind: SymbolKind,
        name: String,
        container_name: Option<String>,
        location: EditorLocation,
    },
    SshHost {
        host: SshHost,
    },
    #[cfg(windows)]
    WslHost {
        host: crate::workspace::WslHost,
    },
    RunAndDebug {
        mode: RunDebugMode,
        config: RunDebugConfig,
    },
    ColorTheme {
        name: String,
    },
    IconTheme {
        name: String,
    },
    Language {
        name: String,
    },
    LineEnding {
        kind: LineEnding,
    },
    SCMReference {
        name: String,
    },
    TerminalProfile {
        name: String,
        profile: phidi_rpc::terminal::TerminalProfile,
    },
}
