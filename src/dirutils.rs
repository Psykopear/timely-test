use std::path::PathBuf;

use ini::Ini;
use linicon::{lookup_icon, IconPath};
use walkdir::DirEntry;
use xdg::BaseDirectories;

#[derive(Clone, Debug, PartialEq)]
pub struct SearchResult {
    pub icon_path: Option<PathBuf>,
    pub icon_name: String,
    pub desktop_entry_path: Option<PathBuf>,
    pub name: String,
    pub description: String,
    pub command: String,
}

impl SearchResult {
    pub fn with_icon(mut self) -> Self {
        if let Some(Ok(IconPath { path, .. })) = lookup_icon(&self.icon_name).next() {
            self.icon_path = Some(path);
        }
        self
    }
}

impl SearchResult {
    pub fn from_desktop(value: &PathBuf) -> Result<Self, SearchResultError> {
        use SearchResultError::*;
        // If anything we need can't be found, return None
        let info = Ini::load_from_file(value).map_err(|_err| FileNotFound)?;
        let section = info.section(Some("Desktop Entry")).ok_or(MissingSection)?;
        let name = section.get("Name").ok_or(MissingName)?.to_string();
        let description = section
            .get("Comment")
            .ok_or(MissingDescription)?
            .to_string();
        let icon = section.get("Icon").ok_or(MissingIcon)?.to_string();
        let command = section.get("Exec").ok_or(MissingCommand)?.to_string();

        Ok(SearchResult {
            icon_name: icon.to_string(),
            icon_path: None,
            desktop_entry_path: Some(value.clone()),
            name,
            description,
            command,
        })
    }

    pub fn from_bin(value: &PathBuf) -> Result<Self, SearchResultError> {
        let name = value
            .file_stem()
            .ok_or(SearchResultError::MissingName)?
            .to_str()
            .ok_or(SearchResultError::MissingName)?
            .to_string();

        let command = value
            .as_os_str()
            .to_str()
            .ok_or(SearchResultError::MissingDescription)?
            .to_string();

        let description = command.clone();

        Ok(SearchResult {
            icon_name: "terminal".to_string(),
            icon_path: None,
            desktop_entry_path: None,
            name,
            description,
            command,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SearchResultError {
    WrongFileType,
    FileNotFound,
    MissingSection,
    MissingName,
    MissingDescription,
    MissingIcon,
    MissingCommand,
}

impl TryFrom<PathBuf> for SearchResult {
    type Error = SearchResultError;

    fn try_from(value: PathBuf) -> Result<Self, Self::Error> {
        TryFrom::try_from(&value)
    }
}

impl TryFrom<&PathBuf> for SearchResult {
    type Error = SearchResultError;

    fn try_from(value: &PathBuf) -> Result<Self, Self::Error> {
        if let Some(ext) = value.extension() {
            if ext == ".desktop" {
                SearchResult::from_desktop(value)
            } else {
                Err(SearchResultError::WrongFileType)
            }
        } else {
            SearchResult::from_bin(value)
        }
    }
}

pub fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with("."))
        .unwrap_or(false)
}

pub fn is_desktop_file(entry: &DirEntry) -> bool {
    entry.path().extension().unwrap_or_default() == "desktop"
}

pub fn search_dirs() -> Vec<PathBuf> {
    let base_dirs = BaseDirectories::new()
        .expect("Can't find xdg directories! Good luck and thanks for all the fish");
    let mut data_dirs: Vec<PathBuf> = vec![base_dirs.get_data_home()];
    data_dirs.append(&mut base_dirs.get_data_dirs());
    data_dirs
}
