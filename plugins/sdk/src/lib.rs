use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_BOOK_BYTES: u64 = 256 * 1024 * 1024;
pub const MAX_PAGE_SIZE: usize = 100;

#[derive(Debug, Deserialize, Serialize)]
pub struct Manifest {
    pub protocol: u32,
    pub id: String,
    pub name: String,
    pub executable: String,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Request {
    pub protocol: u32,
    pub data_dir: PathBuf,
    pub output_dir: PathBuf,
    #[serde(flatten)]
    pub call: Call,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "method", content = "params")]
pub enum Call {
    #[serde(rename = "source.list")]
    List {
        folder: Option<String>,
        cursor: Option<String>,
        query: Option<String>,
        limit: usize,
    },
    #[serde(rename = "source.fetch")]
    Fetch { ids: Vec<String> },
    #[serde(rename = "source.configure")]
    Configure,
    #[serde(rename = "source.disconnect")]
    Disconnect,
    #[serde(rename = "settings.directory.get")]
    DirectorySettings,
    #[serde(rename = "settings.directory.set")]
    SetDirectory { directory: PathBuf },
}

impl Call {
    pub fn capability(&self) -> &'static str {
        match self {
            Self::List { .. } => "source.list",
            Self::Fetch { .. } => "source.fetch",
            Self::Configure => "source.configure",
            Self::Disconnect => "source.disconnect",
            Self::DirectorySettings => "settings.directory.get",
            Self::SetDirectory { .. } => "settings.directory.set",
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DirectorySettings {
    pub directory: Option<PathBuf>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Listing {
    pub items: Vec<Item>,
    pub next_cursor: Option<String>,
    #[serde(default)]
    pub needs_connection: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    Folder,
    Book,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub kind: ItemKind,
    pub size: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Download {
    pub files: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    Progress {
        message: String,
        completed: Option<u64>,
        total: Option<u64>,
    },
    Result {
        value: Value,
    },
    Error {
        message: String,
    },
}
