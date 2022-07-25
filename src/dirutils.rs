use std::path::{Path, PathBuf};

use ini::Ini;
use walkdir::{DirEntry, WalkDir};
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
        let themes = vec!["Papirus", "hicolor", "Adwaita", "gnome"];
        let size = "48x48";
        for theme in themes {
            self.icon_path = WalkDir::new(format!("/usr/share/icons/{theme}/{size}"))
                .into_iter()
                .filter_map(|e| e.ok())
                .find(|e| e.path().file_stem().unwrap().to_string_lossy() == self.icon_name)
                .map(|e| e.path().into());
            // Just stop at the first one we find
            if self.icon_path.is_some() {
                break;
            }
        }
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SearchResultError {
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
        use SearchResultError::*;
        // If anything we need can't be found, return None
        let info = Ini::load_from_file(&value).map_err(|_err| FileNotFound)?;
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
            desktop_entry_path: Some(value),
            name,
            description,
            command,
        })
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
