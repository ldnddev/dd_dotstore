use anyhow::{Context, Result};
use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::Frame;
use std::path::Path;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::{
    domain::{Action, AppState, PERSIST_IDLE, load},
    input::{handle_key, handle_mouse},
    theme::{OmarchyChoice, Theme, ThemeStatus, load_theme_resolved},
    tree::{AssignmentSource, rebuild_tree},
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
        let omarchy_paths = ldnddev_theme::OmarchyPaths::system();
        let config_home = ldnddev_theme::default_config_home();
        match load_theme_resolved(project_root, config_home.as_deref(), &omarchy_paths) {
            Ok(theme) => {
                state.theme_status = ThemeStatus::healthy(theme.source, theme.version);
                state.theme = theme;
            }
            Err(err) => {
                state.theme = Theme::default();
                state.theme_status = ThemeStatus::warning(format!(
                    "Theme warning: {err:#}; using built-in defaults"
                ));
            }
        }
        state.omarchy_paths = omarchy_paths;
        state.follow_stamp = match &state.theme.omarchy {
            Some(OmarchyChoice::Follow) => ldnddev_theme::follow_stamp(&state.omarchy_paths),
            _ => None,
        };
        state.header_copy = random_header_copy(&state.theme.header_quotes);
        rebuild_tree(&mut state, AssignmentSource::Persisted)?;

        if !state.nodes.is_empty() {
            state.list_state.select(Some(0));
        }

        Ok(Self { state })
    }

    pub fn save(&mut self) -> Result<()> {
        self.state.persist_now()
    }

    pub fn draw(&mut self, f: &mut Frame) {
        draw(f, &mut self.state);
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        handle_key(&mut self.state, key)
    }

    pub fn handle_mouse(&mut self, mouse: MouseEvent) -> Result<bool> {
        handle_mouse(&mut self.state, mouse)
    }

    pub fn tick(&mut self) {
        self.state.clear_expired_toast();

        let Some(since) = self.state.dirty_since else {
            return;
        };
        if since.elapsed() < PERSIST_IDLE {
            return;
        }
        if self
            .state
            .persist_retry_at
            .is_some_and(|at| Instant::now() < at)
        {
            return;
        }
        if let Err(err) = self.state.persist_now() {
            self.state.record_persist_error(&err);
        }
    }

    pub fn reload(&mut self) -> Result<()> {
        rebuild_tree(&mut self.state, AssignmentSource::LiveTree)
    }

    pub fn push_action(&mut self, action: Action) {
        crate::actions::push_history(&mut self.state, action);
    }
}

impl AppState {
    /// Re-read `omarchy: follow` when the desktop theme file changes.
    /// Leaves the header line alone.
    pub fn refresh_followed_theme(&mut self) -> bool {
        if self.theme_ui_open() {
            return false;
        }
        if !matches!(self.theme.omarchy, Some(OmarchyChoice::Follow)) {
            return false;
        }
        let stamp = ldnddev_theme::follow_stamp(&self.omarchy_paths);
        if stamp == self.follow_stamp {
            return false;
        }
        let config_home = ldnddev_theme::default_config_home();
        match load_theme_resolved(
            &self.project_root,
            config_home.as_deref(),
            &self.omarchy_paths,
        ) {
            Ok(theme) => {
                self.theme_status = ThemeStatus::healthy(theme.source, theme.version);
                self.theme = theme;
            }
            Err(err) => {
                let quotes = self.theme.header_quotes.clone();
                self.theme = Theme::default();
                self.theme.header_quotes = quotes;
                let message = format!("Theme warning: {err:#}; using built-in defaults");
                self.theme_status = ThemeStatus::warning(message.clone());
                self.show_toast(crate::ui::toast::ToastLevel::Warning, message);
            }
        }
        self.follow_stamp = stamp;
        true
    }
}

fn random_header_copy(quotes: &[String]) -> String {
    if quotes.is_empty() {
        return "No quotes configured.".to_string();
    }
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as usize)
        .unwrap_or(0)
        ^ std::process::id() as usize;
    quotes[seed % quotes.len()].clone()
}
