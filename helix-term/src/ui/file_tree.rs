mod icons;

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, TryRecvError},
    time::{Duration, Instant},
};

use helix_vcs::{DiffProviderRegistry, FileChange};
use helix_view::{
    editor::{Action, FileExplorerConfig, FileTreeConfig, FileTreeIconConfig, FileTreeSide},
    graphics::{Rect, Style},
    input::{KeyEvent, MouseButton, MouseEvent, MouseEventKind},
    Editor, Theme,
};
use tui::buffer::Buffer as Surface;

use crate::{file_watcher::FileWatcher, key};

use self::icons::IconSet;

pub struct FileTree {
    pub root: PathBuf,
    pub width: u16,
    pub side: FileTreeSide,
    pub visible: bool,
    icons: Option<IconSet>,
    icon_config: FileTreeIconConfig,

    /// Directories that are currently expanded.
    expanded: HashSet<PathBuf>,
    /// Directory listings are loaded lazily and cached until a refresh.
    cache: HashMap<PathBuf, Vec<Entry>>,
    /// The flattened list of currently visible rows.
    rows: Vec<Row>,
    selected: usize,
    scroll: usize,
    list_area: Rect,
    /// `<space>` was pressed and we are waiting for the next key.
    pending_space: bool,
    watcher: Option<FileWatcher>,

    git_providers: DiffProviderRegistry,
    trust_full: bool,
    git_statuses: HashMap<PathBuf, GitStatus>,
    dir_statuses: HashMap<PathBuf, GitStatus>,
    status_rx: Option<Receiver<anyhow::Result<Vec<FileChange>>>>,
    status_scanning: bool,
    status_dirty: bool,
    last_status_scan: Option<Instant>,
}

#[derive(Clone)]
struct Entry {
    path: PathBuf,
    is_dir: bool,
}

#[derive(Clone)]
struct Row {
    path: PathBuf,
    depth: usize,
    is_dir: bool,
    expanded: bool,
}

pub enum FileTreeOutcome {
    Consumed,
    /// Focus should move back to the editor.
    FocusEditor,
    /// The sidebar should be hidden.
    Hide,
    /// A file operation should be performed by the editor view.
    Action(FileTreeAction),
}

pub enum FileTreeAction {
    CreateFile(PathBuf),
    CreateDirectory(PathBuf),
    Rename(PathBuf),
    Delete(PathBuf),
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum GitStatus {
    Untracked,
    Renamed,
    Deleted,
    Modified,
    Conflict,
}

impl GitStatus {
    fn marker(self) -> &'static str {
        match self {
            Self::Untracked => "?",
            Self::Modified => "M",
            Self::Deleted => "D",
            Self::Renamed => "R",
            Self::Conflict => "!",
        }
    }

    fn style(self, theme: &Theme) -> Style {
        let scope = match self {
            Self::Untracked => "diff.plus",
            Self::Modified => "diff.delta",
            Self::Deleted => "diff.minus",
            Self::Renamed => "diff.delta.moved",
            Self::Conflict => "diff.delta.conflict",
        };
        theme.try_get(scope).unwrap_or_else(|| theme.get("ui.text"))
    }
}

impl FileTree {
    pub fn new(
        root: PathBuf,
        config: &FileTreeConfig,
        git_providers: DiffProviderRegistry,
        trust_full: bool,
    ) -> Self {
        Self {
            root,
            width: config.width,
            side: config.side,
            visible: false,
            icons: config.icons.then(|| IconSet::new(&config.icon_overrides)),
            icon_config: config.icon_overrides.clone(),
            expanded: HashSet::new(),
            cache: HashMap::new(),
            rows: Vec::new(),
            selected: 0,
            scroll: 0,
            list_area: Rect::default(),
            pending_space: false,
            watcher: None,
            git_providers,
            trust_full,
            git_statuses: HashMap::new(),
            dir_statuses: HashMap::new(),
            status_rx: None,
            status_scanning: false,
            status_dirty: false,
            last_status_scan: None,
        }
    }

    pub fn start_watcher(&mut self) {
        if self.watcher.is_none() {
            match FileWatcher::new(&self.root) {
                Ok(mut watcher) => {
                    watcher.sync(&self.root, &self.expanded);
                    self.watcher = Some(watcher);
                }
                Err(err) => log::warn!("failed to watch {}: {err}", self.root.display()),
            }
        }

        self.start_status_scan();
    }

    pub fn stop_watcher(&mut self) {
        self.watcher = None;
        self.status_rx = None;
        self.status_scanning = false;
        self.status_dirty = false;
    }

    /// Poll the filesystem watcher and refresh the tree if anything changed.
    /// Returns true if the tree was refreshed.
    pub fn poll_events(&mut self, config: &FileExplorerConfig) -> bool {
        let Some(watcher) = self.watcher.as_mut() else {
            return false;
        };

        if watcher.poll() {
            self.refresh(config);
            true
        } else {
            false
        }
    }

    pub fn set_git_context(&mut self, providers: DiffProviderRegistry, trust_full: bool) {
        self.git_providers = providers;
        self.trust_full = trust_full;
    }

    fn start_status_scan(&mut self) {
        if self.status_scanning {
            self.status_dirty = true;
            return;
        }
        if self
            .last_status_scan
            .is_some_and(|last| last.elapsed() < Duration::from_millis(500))
        {
            return;
        }

        let providers = self.git_providers.clone();
        let root = self.root.clone();
        let trust_full = self.trust_full;
        let (tx, rx) = mpsc::channel();

        std::thread::spawn(move || {
            let result = providers.changed_files(&root, trust_full);
            let _ = tx.send(result);
            helix_event::request_redraw();
        });

        self.status_rx = Some(rx);
        self.status_scanning = true;
    }

    /// Poll the background git status scan. Returns true if the markers changed.
    pub fn poll_git_status(&mut self) -> bool {
        let result = match self.status_rx.as_ref() {
            Some(rx) => rx.try_recv(),
            None => return false,
        };

        let updated = match result {
            Ok(result) => {
                self.status_rx = None;
                self.status_scanning = false;
                self.last_status_scan = Some(Instant::now());

                match result {
                    Ok(changes) => {
                        self.update_git_status(changes);
                        true
                    }
                    Err(err) => {
                        log::debug!("failed to get git status: {err}");
                        let had_status =
                            !self.git_statuses.is_empty() || !self.dir_statuses.is_empty();
                        self.git_statuses.clear();
                        self.dir_statuses.clear();
                        had_status
                    }
                }
            }
            Err(TryRecvError::Empty) => return false,
            Err(TryRecvError::Disconnected) => {
                self.status_rx = None;
                self.status_scanning = false;
                self.last_status_scan = Some(Instant::now());
                false
            }
        };

        // A filesystem change arrived while the scan was running. Run another
        // scan so the markers reflect the latest state.
        if std::mem::take(&mut self.status_dirty) {
            self.last_status_scan = None;
            self.start_status_scan();
        }

        updated
    }

    fn update_git_status(&mut self, changes: Vec<FileChange>) {
        self.git_statuses.clear();
        self.dir_statuses.clear();

        for change in changes {
            let (status, path, old_path) = match change {
                FileChange::Untracked { path } => (GitStatus::Untracked, path, None),
                FileChange::Modified { path } => (GitStatus::Modified, path, None),
                FileChange::Conflict { path } => (GitStatus::Conflict, path, None),
                FileChange::Deleted { path } => (GitStatus::Deleted, path, None),
                FileChange::Renamed { from_path, to_path } => {
                    (GitStatus::Renamed, to_path, Some(from_path))
                }
            };

            if let Ok(relative) = path.strip_prefix(&self.root) {
                let relative = relative.to_owned();
                self.git_statuses.insert(path, status);
                self.mark_ancestors(&relative, status);
            }

            if let Some(old_path) = old_path {
                if let Ok(relative) = old_path.strip_prefix(&self.root) {
                    let relative = relative.to_owned();
                    self.mark_ancestors(&relative, status);
                }
            }
        }
    }

    fn mark_ancestors(&mut self, relative: &Path, status: GitStatus) {
        let mut parent = relative.parent();
        while let Some(dir) = parent {
            if dir.as_os_str().is_empty() {
                break;
            }

            self.dir_statuses
                .entry(self.root.join(dir))
                .and_modify(|current| {
                    if status > *current {
                        *current = status;
                    }
                })
                .or_insert(status);

            parent = dir.parent();
        }
    }

    fn sync_watcher(&mut self) {
        if let Some(watcher) = self.watcher.as_mut() {
            watcher.sync(&self.root, &self.expanded);
        }
    }

    pub fn rebuild(&mut self, config: &FileExplorerConfig) {
        self.rows.clear();
        let root = self.root.clone();
        self.collect(&root, 0, config);
        self.selected = self.selected.min(self.rows.len().saturating_sub(1));
        self.sync_watcher();
    }

    fn collect(&mut self, dir: &Path, depth: usize, config: &FileExplorerConfig) {
        if !self.cache.contains_key(dir) {
            self.cache.insert(dir.to_owned(), read_dir(dir, config));
        }

        for entry in self.cache[dir].clone() {
            let expanded = entry.is_dir && self.expanded.contains(&entry.path);
            self.rows.push(Row {
                path: entry.path.clone(),
                depth,
                is_dir: entry.is_dir,
                expanded,
            });
            if expanded {
                self.collect(&entry.path, depth + 1, config);
            }
        }
    }

    pub fn refresh(&mut self, config: &FileExplorerConfig) {
        // Keep the selection on the same path across automatic refreshes.
        let selected = self.selected_path();
        self.cache.clear();
        self.rebuild(config);
        if let Some(path) = selected {
            if let Some(index) = self.rows.iter().position(|row| row.path == path) {
                self.selected = index;
            }
        }
        self.start_status_scan();
    }

    pub fn apply_config(&mut self, config: &FileTreeConfig) {
        self.width = config.width;
        self.side = config.side;

        if config.icons != self.icons.is_some() || config.icon_overrides != self.icon_config {
            self.icons = config.icons.then(|| IconSet::new(&config.icon_overrides));
            self.icon_config = config.icon_overrides.clone();
        }
    }

    /// Expand the ancestors of the active document and select it.
    pub fn reveal_active(&mut self, editor: &Editor, config: &FileExplorerConfig) {
        if let Some(path) = doc!(editor).path().map(ToOwned::to_owned) {
            self.reveal_path(&path, config);
        }
    }

    pub fn reveal_path(&mut self, path: &Path, config: &FileExplorerConfig) {
        let Ok(relative) = path.strip_prefix(&self.root) else {
            return;
        };

        // Expand every directory between the workspace root and the file.
        let mut parent = relative.parent();
        while let Some(dir) = parent {
            if dir.as_os_str().is_empty() {
                break;
            }
            let dir_path = self.root.join(dir);
            self.cache.remove(&dir_path);
            self.expanded.insert(dir_path);
            parent = dir.parent();
        }

        self.rebuild(config);

        if let Some(index) = self.rows.iter().position(|row| row.path == path) {
            self.selected = index;
        }
    }

    pub fn clear_pending(&mut self) {
        self.pending_space = false;
    }

    pub fn render(&mut self, area: Rect, surface: &mut Surface, editor: &Editor) {
        let theme = &editor.theme;
        let background = theme
            .try_get("ui.file-tree.background")
            .unwrap_or_else(|| theme.get("ui.background"));
        let selected_style = theme
            .try_get("ui.file-tree.selected")
            .unwrap_or_else(|| theme.get("ui.selection"));
        let directory_style = theme
            .try_get("ui.file-tree.directory")
            .unwrap_or_else(|| theme.get("ui.text.directory"));

        surface.set_style(area, background);

        // Keep the last row free for the per-view statusline.
        let list = area.clip_bottom(1);
        self.list_area = list;
        self.ensure_selected_visible(list.height);

        if self.rows.is_empty() {
            return;
        }

        for (index, row) in self
            .rows
            .iter()
            .enumerate()
            .skip(self.scroll)
            .take(list.height as usize)
        {
            let y = list.y + (index - self.scroll) as u16;
            let base_style = if index == self.selected {
                surface.set_style(Rect::new(list.x, y, list.width, 1), selected_style);
                selected_style
            } else {
                background
            };

            let arrow = if row.is_dir {
                if row.expanded {
                    "▾ "
                } else {
                    "▸ "
                }
            } else {
                "  "
            };
            let icon = self.icons.as_ref().and_then(|icons| {
                let icon = if row.is_dir {
                    icons.directory(row.expanded)
                } else {
                    icons.file(&row.path)
                };
                (!icon.is_empty()).then_some(icon)
            });
            let name = row.path.file_name().unwrap_or_default().to_string_lossy();
            let indent = "  ".repeat(row.depth);
            let text = if let Some(icon) = icon {
                format!("{indent}{icon} {name}")
            } else {
                format!("{indent}{arrow}{name}")
            };
            let style = if row.is_dir {
                base_style.patch(directory_style)
            } else {
                base_style
            };

            let git_status = if row.is_dir {
                self.dir_statuses.get(&row.path).copied()
            } else {
                self.git_statuses.get(&row.path).copied()
            };
            let text_width = if git_status.is_some() {
                list.width.saturating_sub(3)
            } else {
                list.width.saturating_sub(2)
            };

            surface.set_stringn(list.x + 1, y, &text, text_width as usize, style);

            if let Some(status) = git_status {
                let marker_style = base_style.patch(status.style(theme));
                surface.set_stringn(
                    list.right().saturating_sub(2),
                    y,
                    status.marker(),
                    1,
                    marker_style,
                );
            }
        }
    }

    fn ensure_selected_visible(&mut self, height: u16) {
        let height = height as usize;
        if height == 0 {
            return;
        }
        if self.selected < self.scroll {
            self.scroll = self.selected;
        }
        if self.selected >= self.scroll + height {
            self.scroll = self.selected + 1 - height;
        }
    }

    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        editor: &mut Editor,
        config: &FileExplorerConfig,
    ) -> FileTreeOutcome {
        // `<space>e` toggles the sidebar even while it has focus.
        if self.pending_space {
            self.pending_space = false;
            if key == key!('e') {
                return FileTreeOutcome::Hide;
            }
        }

        if key == key!(' ') {
            self.pending_space = true;
            return FileTreeOutcome::Consumed;
        }

        match key {
            key!('j') | key!(Down) => self.move_selection(1),
            key!('k') | key!(Up) => self.move_selection(-1),
            key!('g') => self.selected = 0,
            key!('G') => self.selected = self.rows.len().saturating_sub(1),
            key!('h') | key!(Left) => self.collapse(config),
            key!('l') | key!(Right) => {
                if self.expand(editor, config) {
                    return FileTreeOutcome::FocusEditor;
                }
            }
            key!(Enter) => {
                if self.activate(editor, config) {
                    return FileTreeOutcome::FocusEditor;
                }
            }
            key!('r') => self.refresh(config),
            key!('R') => self.reveal_active(editor, config),
            key!('a') => {
                return FileTreeOutcome::Action(FileTreeAction::CreateFile(
                    self.selected_directory(),
                ));
            }
            key!('A') => {
                return FileTreeOutcome::Action(FileTreeAction::CreateDirectory(
                    self.selected_directory(),
                ));
            }
            key!('m') => {
                if let Some(path) = self.selected_path() {
                    return FileTreeOutcome::Action(FileTreeAction::Rename(path));
                }
            }
            key!('d') => {
                if let Some(path) = self.selected_path() {
                    return FileTreeOutcome::Action(FileTreeAction::Delete(path));
                }
            }
            key!('v') => {
                if self.open_selected(editor, Action::VerticalSplit) {
                    return FileTreeOutcome::FocusEditor;
                }
            }
            key!('s') => {
                if self.open_selected(editor, Action::HorizontalSplit) {
                    return FileTreeOutcome::FocusEditor;
                }
            }
            key!('q') => return FileTreeOutcome::Hide,
            _ => {}
        }

        FileTreeOutcome::Consumed
    }

    /// Returns `true` if focus should be moved back to the editor.
    pub fn handle_mouse(
        &mut self,
        event: &MouseEvent,
        editor: &mut Editor,
        config: &FileExplorerConfig,
    ) -> bool {
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(index) = self.index_at(event.row, event.column) {
                    self.selected = index;
                    return self.activate(editor, config);
                }
            }
            MouseEventKind::ScrollDown => self.move_selection(3),
            MouseEventKind::ScrollUp => self.move_selection(-3),
            _ => {}
        }

        false
    }

    fn selected_path(&self) -> Option<PathBuf> {
        self.rows.get(self.selected).map(|row| row.path.clone())
    }

    fn selected_directory(&self) -> PathBuf {
        match self.rows.get(self.selected) {
            Some(row) if row.is_dir => row.path.clone(),
            Some(row) => row
                .path
                .parent()
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| self.root.clone()),
            None => self.root.clone(),
        }
    }

    fn index_at(&self, row: u16, column: u16) -> Option<usize> {
        let area = self.list_area;
        if column < area.x || column >= area.right() || row < area.y || row >= area.bottom() {
            return None;
        }

        let index = self.scroll + (row - area.y) as usize;
        (index < self.rows.len()).then_some(index)
    }

    fn move_selection(&mut self, delta: isize) {
        if self.rows.is_empty() {
            return;
        }
        let last = self.rows.len() - 1;
        self.selected = self.selected.saturating_add_signed(delta).min(last);
    }

    fn collapse(&mut self, config: &FileExplorerConfig) {
        let Some(row) = self.rows.get(self.selected).cloned() else {
            return;
        };

        if row.is_dir && row.expanded {
            self.expanded.remove(&row.path);
            self.rebuild(config);
            return;
        }

        if let Some(parent) = row.path.parent() {
            if let Some(index) = self.rows.iter().position(|row| row.path == parent) {
                self.selected = index;
            }
        }
    }

    fn expand(&mut self, editor: &mut Editor, config: &FileExplorerConfig) -> bool {
        let Some(row) = self.rows.get(self.selected).cloned() else {
            return false;
        };

        if !row.is_dir {
            return self.open_selected(editor, Action::Replace);
        }

        if !self.expanded.contains(&row.path) {
            self.cache.remove(&row.path);
            self.expanded.insert(row.path);
            self.rebuild(config);
        } else if let Some(next) = self.rows.get(self.selected + 1) {
            if next.depth > row.depth {
                self.selected += 1;
            }
        }

        false
    }

    fn activate(&mut self, editor: &mut Editor, config: &FileExplorerConfig) -> bool {
        let Some(row) = self.rows.get(self.selected).cloned() else {
            return false;
        };

        if !row.is_dir {
            return self.open_selected(editor, Action::Replace);
        }

        if self.expanded.contains(&row.path) {
            self.expanded.remove(&row.path);
        } else {
            self.cache.remove(&row.path);
            self.expanded.insert(row.path);
        }
        self.rebuild(config);
        false
    }

    fn open_selected(&mut self, editor: &mut Editor, action: Action) -> bool {
        let Some(row) = self.rows.get(self.selected).cloned() else {
            return false;
        };
        if row.is_dir {
            return false;
        }

        match editor.open(&row.path, action) {
            Ok(_) => true,
            Err(error) => {
                editor.set_error(format!("{error}"));
                false
            }
        }
    }
}

fn read_dir(dir: &Path, config: &FileExplorerConfig) -> Vec<Entry> {
    let mut entries: Vec<Entry> = ignore::WalkBuilder::new(dir)
        .hidden(config.hidden)
        .parents(config.parents)
        .ignore(config.ignore)
        .follow_links(config.follow_symlinks)
        .git_ignore(config.git_ignore)
        .git_global(config.git_global)
        .git_exclude(config.git_exclude)
        .max_depth(Some(1))
        .add_custom_ignore_filename(helix_loader::config_dir().join("ignore"))
        .add_custom_ignore_filename(".helix/ignore")
        .build()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path() != dir)
        .map(|entry| {
            let path = entry.into_path();
            let is_dir = path.is_dir();
            Entry { path, is_dir }
        })
        .collect();

    entries.sort_by(|a, b| (!a.is_dir, &a.path).cmp(&(!b.is_dir, &b.path)));
    entries
}
