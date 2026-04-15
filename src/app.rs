use anyhow::{Context, Result};
use crossterm::event::KeyEvent;
use ratatui::Frame;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    inputs::handle_key,
    state::{Action, AppState, Theme, ThemeStatus, load, load_theme, save},
    tree::{build_tree, flatten_visible, set_action_mode, set_dest},
    ui::draw,
};

pub struct App {
    pub state: AppState,
}

impl App {
    pub fn new() -> Result<Self> {
        let project_root = std::env::current_dir().context("Cannot get current directory")?;
        Self::new_with_root(&project_root)
    }

    pub fn new_with_root(project_root: &Path) -> Result<Self> {
        let config_path = project_root.join(".dd_dotstore.json");

        let mut state = if config_path.exists() {
            load(&config_path)?
        } else {
            AppState::default_with_root(project_root.to_path_buf())
        };

        state.project_root = project_root.to_path_buf();
        state.config_path = config_path;
        match load_theme(project_root) {
            Ok(theme) => {
                state.theme_status = ThemeStatus::healthy(theme.source, theme.version);
                state.theme = theme;
            }
            Err(err) => {
                state.theme = Theme::default();
                state.theme_status =
                    ThemeStatus::warning(format!("Theme warning: {err}; using built-in defaults"));
            }
        }
        state.header_copy = random_header_copy().to_string();
        state.tree = build_tree(project_root, &state.ignore_patterns);

        for (rel, dest) in state.persisted_symlinks.clone() {
            set_dest(&mut state.tree, Path::new(&rel), Some(dest.into()));
        }
        for (rel, action_mode) in state.persisted_modes.clone() {
            set_action_mode(&mut state.tree, Path::new(&rel), action_mode);
        }

        flatten_visible(&mut state);
        state.update_symlink_statuses()?;

        if !state.nodes.is_empty() {
            state.list_state.select(Some(0));
        }

        Ok(Self { state })
    }

    pub fn save(&self) -> Result<()> {
        save(&self.state, &self.state.config_path)
    }

    pub fn draw(&mut self, f: &mut Frame) {
        draw(f, &mut self.state);
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        handle_key(&mut self.state, key)
    }

    pub fn tick(&mut self) {
        self.state.clear_expired_toast();
    }

    pub fn reload(&mut self) -> Result<()> {
        self.state.tree = build_tree(&self.state.project_root, &self.state.ignore_patterns);
        flatten_visible(&mut self.state);
        self.state.update_symlink_statuses()?;
        Ok(())
    }

    pub fn push_action(&mut self, action: Action) {
        crate::actions::push_history(&mut self.state, action);
    }
}

fn random_header_copy() -> &'static str {
    const COPIES: [&str; 5] = [
        "Don't Fear the . (Dot) - Tame It.",
        ". (Dot) file Domination done right.",
        ". (Dot) file management fatiuge is real. Or use to be.",
        ". (Dot) file sync setup in seconds - okay, fast.",
        ". (Dot) file management for the Ricer at heart.",
    ];

    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as usize)
        .unwrap_or(0)
        ^ std::process::id() as usize;
    COPIES[seed % COPIES.len()]
}
