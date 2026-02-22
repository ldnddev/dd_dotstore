// app.rs

use std::collections::VecDeque;
use std::path::Path;
use anyhow::Result;
use std::os::unix::fs::symlink;

pub struct App {
    pub dotfiles: Vec<Dotfile>,
    pub current_name: String,
    pub current_target: String,
    pub input_mode: InputMode,
    pub active_tab: ActiveTab,
    pub error: Option<String>,
    pub selected: Option<usize>,
}

#[derive(PartialEq)]
pub enum InputMode {
    None,
    Name,
    Target,
}


#[derive(PartialEq)]
pub enum ActiveTab {
    List,
    Symlink,
}

#[derive(Clone)]
pub struct Dotfile {
    pub name: String,
    pub target: String, // e.g., "~/.config/app" or "~/.app"
}

impl App {
    pub fn new() -> Self {
        App {
            dotfiles: Vec::new(),
            current_name: String::new(),
            current_target: String::new(),
            active_tab: ActiveTab::List,
            error: None,
            selected: None,
        }
    }

    pub fn add_dotfile(&mut self, name: String, target: String) -> Result<()> {
        if self.dotfiles.iter().any(|d| d.name == name) {
            return Err(anyhow::anyhow!("Dotfile already exists"));
        }
        self.dotfiles.push(Dotfile { name, target });
        Ok(())
    }

    pub fn remove_dotfile(&mut self, index: usize) {
        if index < self.dotfiles.len() {
            self.dotfiles.remove(index);
        }
    }

    pub fn edit_dotfile(&mut self, index: usize, name: String, target: String) {
        if index < self.dotfiles.len() {
            self.dotfiles[index] = Dotfile { name, target };
        }
    }

    pub fn create_symlinks(&self, repo_path: &str) -> Result<()> {
        for dotfile in &self.dotfiles {
            let repo_file = Path::new(repo_path).join(&dotfile.name);
            let home_file = expand_tilde(&dotfile.target);
            symlink(repo_file, home_file)?;
        }
        Ok(())
    }
}

fn expand_tilde(path: &str) -> String {
    if path.starts_with("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return path.replace("~", &home.to_string_lossy());
        }
    }
    path.to_string()
}
