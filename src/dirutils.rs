use std::path::{Path, PathBuf};

use ini::Ini;
use walkdir::{DirEntry, WalkDir};
use xdg::BaseDirectories;

pub fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with("."))
        .unwrap_or(false)
}

pub fn is_desktop_file(entry: &DirEntry) -> bool {
    if let Some(ext) = entry.path().extension() {
        ext == "desktop"
    } else {
        false
    }
}

pub fn search_dirs() -> Vec<PathBuf> {
    let base_dirs = BaseDirectories::new()
        .expect("Can't find xdg directories! Good luck and thanks for all the fish");
    let mut data_dirs: Vec<PathBuf> = Vec::new();
    data_dirs.push(base_dirs.get_data_home());
    data_dirs.append(&mut base_dirs.get_data_dirs());
    data_dirs
}

#[derive(Clone, Debug, PartialEq)]
pub struct SearchResult {
    pub icon_path: Option<String>,
    pub icon_name: String,
    pub desktop_entry_path: Option<String>,
    pub name: String,
    pub description: String,
    pub command: String,
}

impl SearchResult {
    pub fn add_icon(&mut self) {
        if let Some(entry) = WalkDir::new("/usr/share/icons")
            .into_iter()
            .filter_map(|e| e.ok())
            .find(|e| e.path().file_stem().unwrap().to_string_lossy() == self.icon_name)
        {
            self.icon_path = Some(entry.path().to_string_lossy().into());
        }
    }
}

pub fn searchresult_from_desktopentry(desktop_file_path: &Path) -> Option<SearchResult> {
    // If anything we need can't be found, return None
    let info = match Ini::load_from_file(desktop_file_path) {
        Ok(info) => info,
        Err(_) => return None,
    };
    let section = match info.section(Some("Desktop Entry")) {
        Some(sec) => sec,
        None => return None,
    };
    let name = match section.get("Name") {
        Some(name) => name.to_string(),
        None => return None,
    };
    let description = match section.get("Comment") {
        Some(description) => description.to_string(),
        None => return None,
    };
    let icon = match section.get("Icon") {
        Some(icon) => icon,
        None => return None,
    };
    let command = match section.get("Exec") {
        Some(command) => command.to_string(),
        None => return None,
    };

    let desktop_entry_path = match desktop_file_path.to_str() {
        Some(path) => Some(path.to_string()),
        None => return None,
    };

    Some(SearchResult {
        icon_name: icon.to_string(),
        icon_path: None,
        desktop_entry_path,
        name,
        description,
        command,
    })
}
