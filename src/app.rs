//! Interface state: active tab, process list (search, sort and selection), modal dialogs
//! and mapping of keys to actions.

use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::TableState;
use serde::Deserialize;

use crate::{
    config::Config,
    gpu, i18n,
    system::{Monitor, Process, SignalError},
};

/// Tabs of the right panel, in the order they are shown (keys `1`–`7`).
/// In the configuration they are written in lowercase (`"summary"`, `"processes"`…).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tab {
    Summary,
    Cpu,
    Ram,
    Storage,
    Gpu,
    Network,
    Processes,
}

impl Tab {
    pub const ALL: [Tab; 7] = [
        Tab::Summary,
        Tab::Cpu,
        Tab::Ram,
        Tab::Storage,
        Tab::Gpu,
        Tab::Network,
        Tab::Processes,
    ];
    /// Tab name in the interface language.
    pub fn title(self) -> &'static str {
        let t = i18n::t();
        match self {
            Tab::Summary => t.tab_summary,
            Tab::Cpu => t.tab_cpu,
            Tab::Ram => t.tab_ram,
            Tab::Storage => t.tab_storage,
            Tab::Gpu => t.tab_gpu,
            Tab::Network => t.tab_network,
            Tab::Processes => t.tab_processes,
        }
    }
    fn index(self) -> usize {
        Self::ALL.iter().position(|p| *p == self).unwrap_or(0)
    }
}

/// Column the Processes tab is sorted by.
/// In the configuration they are written in lowercase (`"cpu"`, `"memory"`…).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProcessSort {
    Cpu,
    Memory,
    Pid,
    Name,
    User,
}

impl ProcessSort {
    /// Numbers from highest to lowest (the heaviest on top); text from A to Z.
    fn descending_by_default(self) -> bool {
        matches!(self, ProcessSort::Cpu | ProcessSort::Memory)
    }
}

/// What receives the keys: the normal view, the search box or a modal dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Searching,
    Help,
    Confirm { pid: u32, name: String, force: bool },
}

pub struct App {
    /// System data; refreshed on every [`App::tick`].
    pub monitor: Monitor,
    pub tab: Tab,
    pub mode: Mode,
    pub paused: bool,
    pub quit: bool,

    pub process_table: TableState,
    pub process_sort: ProcessSort,
    pub descending: bool,
    pub search: String,
    pub mine_only: bool,
    /// Show the logo above the System panel (`logo` in the configuration).
    pub logo: bool,
    selected_pid: Option<u32>,

    /// Message shown in the footer: text, whether it is an error, and when it appeared.
    pub notice: Option<(String, bool, Instant)>,
}

impl App {
    /// Creates the initial state with the tab, sort and filter from the configuration.
    pub fn new(monitor: Monitor, config: &Config) -> Self {
        let sort = config.processes.sort;
        let mut app = Self {
            monitor,
            tab: Tab::Summary,
            mode: Mode::Normal,
            paused: false,
            quit: false,
            process_table: TableState::default().with_selected(0),
            process_sort: sort,
            descending: sort.descending_by_default(),
            search: String::new(),
            mine_only: config.processes.mine_only,
            logo: config.logo,
            selected_pid: None,
            notice: None,
        };
        // `go_to` also enables the NVIDIA GPU queries if the app starts on that tab.
        app.go_to(config.start_tab);
        app.sync_selection();
        app
    }

    pub fn tick(&mut self) {
        if !self.paused {
            self.monitor.refresh();
            self.sync_selection();
        }
        if self
            .notice
            .as_ref()
            .is_some_and(|(_, _, t)| t.elapsed().as_secs() >= 4)
        {
            self.notice = None;
        }
    }

    /// Processes filtered and sorted as in the Processes tab.
    pub fn processes(&self) -> Vec<&Process> {
        let q = self.search.to_lowercase();
        let me = &self.monitor.info.user;
        let mut v: Vec<&Process> = self
            .monitor
            .processes
            .iter()
            .filter(|p| !self.mine_only || &p.user == me)
            .filter(|p| {
                q.is_empty()
                    || p.name.to_lowercase().contains(&q)
                    || p.command.to_lowercase().contains(&q)
                    || p.pid.to_string() == q
                    || p.user.to_lowercase() == q
            })
            .collect();
        v.sort_by(|a, b| {
            let o = match self.process_sort {
                ProcessSort::Cpu => a.cpu.total_cmp(&b.cpu),
                ProcessSort::Memory => a.memory.cmp(&b.memory),
                ProcessSort::Pid => a.pid.cmp(&b.pid),
                ProcessSort::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                ProcessSort::User => a.user.cmp(&b.user),
            };
            if self.descending { o.reverse() } else { o }.then(a.pid.cmp(&b.pid))
        });
        v
    }

    /// Top N processes by CPU or memory (for Summary/CPU/RAM).
    pub fn top(&self, by_memory: bool, n: usize) -> Vec<&Process> {
        let mut v: Vec<&Process> = self.monitor.processes.iter().collect();
        if by_memory {
            v.sort_by_key(|p| std::cmp::Reverse(p.memory));
        } else {
            v.sort_by(|a, b| b.cpu.total_cmp(&a.cpu));
        }
        v.truncate(n);
        v
    }

    /// Keeps the same PID selected even if the list is reordered.
    fn sync_selection(&mut self) {
        let list = self.processes();
        let pos = self
            .selected_pid
            .and_then(|pid| list.iter().position(|p| p.pid == pid))
            .or_else(|| {
                (!list.is_empty()).then(|| {
                    self.process_table
                        .selected()
                        .unwrap_or(0)
                        .min(list.len() - 1)
                })
            });
        self.selected_pid = pos.map(|i| list[i].pid);
        self.process_table.select(pos);
    }

    /// After searching, filtering or sorting: go back to the first process in the list.
    fn reset_selection(&mut self) {
        self.selected_pid = None;
        self.process_table = TableState::default().with_selected(0);
        self.sync_selection();
    }

    fn move_selection(&mut self, delta: isize) {
        let list = self.processes();
        if list.is_empty() {
            return;
        }
        let i = self.process_table.selected().unwrap_or(0) as isize;
        let n = (i.saturating_add(delta)).clamp(0, list.len() as isize - 1) as usize;
        self.selected_pid = Some(list[n].pid);
        self.process_table.select(Some(n));
    }

    fn selected(&self) -> Option<&Process> {
        let pid = self.selected_pid?;
        self.monitor.processes.iter().find(|p| p.pid == pid)
    }

    fn go_to(&mut self, tab: Tab) {
        self.tab = tab;
        gpu::QUERY_NVIDIA.store(tab == Tab::Gpu, std::sync::atomic::Ordering::Relaxed);
    }

    fn sort_by(&mut self, sort: ProcessSort) {
        if self.process_sort == sort {
            self.descending = !self.descending;
        } else {
            self.process_sort = sort;
            self.descending = sort.descending_by_default();
        }
        self.reset_selection();
    }

    fn notify(&mut self, text: impl Into<String>, error: bool) {
        self.notice = Some((text.into(), error, Instant::now()));
    }

    pub fn key(&mut self, k: KeyEvent) {
        if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        match self.mode.clone() {
            Mode::Help => self.mode = Mode::Normal,
            Mode::Confirm { pid, name, force } => {
                if matches!(
                    k.code,
                    KeyCode::Char('s' | 'S' | 'y' | 'Y') | KeyCode::Enter
                ) {
                    let t = i18n::t();
                    match self.monitor.signal(pid, force) {
                        Ok(()) => self.notify(
                            format!(
                                "{} {} {name} ({pid})",
                                if force { "SIGKILL" } else { "SIGTERM" },
                                t.notice_sent_to
                            ),
                            false,
                        ),
                        Err(e) => {
                            let reason = match e {
                                SignalError::NoSuchProcess => t.signal_no_such_process,
                                SignalError::PermissionDenied => t.signal_permission_denied,
                                SignalError::Unsupported => t.signal_unsupported,
                            };
                            self.notify(
                                format!("{} {name}: {reason}", t.notice_could_not_terminate),
                                true,
                            )
                        }
                    }
                }
                self.mode = Mode::Normal;
            }
            Mode::Searching => match k.code {
                KeyCode::Esc => {
                    self.search.clear();
                    self.mode = Mode::Normal;
                    self.reset_selection();
                }
                KeyCode::Enter => self.mode = Mode::Normal,
                KeyCode::Backspace => {
                    self.search.pop();
                    self.reset_selection();
                }
                KeyCode::Up => self.move_selection(-1),
                KeyCode::Down => self.move_selection(1),
                KeyCode::Char(c) => {
                    self.search.push(c);
                    self.reset_selection();
                }
                _ => {}
            },
            Mode::Normal => self.key_normal(k),
        }
    }

    fn key_normal(&mut self, k: KeyEvent) {
        let i = self.tab.index();
        let n = Tab::ALL.len();
        match k.code {
            KeyCode::Char('q') | KeyCode::Esc => self.quit = true,
            KeyCode::Char('?') => self.mode = Mode::Help,
            KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => self.go_to(Tab::ALL[(i + 1) % n]),
            KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => {
                self.go_to(Tab::ALL[(i + n - 1) % n])
            }
            KeyCode::Char(c @ '1'..='7') => self.go_to(Tab::ALL[c as usize - '1' as usize]),
            KeyCode::Char(' ') => self.paused = !self.paused,
            _ if self.tab == Tab::Processes => self.key_processes(k),
            _ => {}
        }
    }

    fn key_processes(&mut self, k: KeyEvent) {
        match k.code {
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
            KeyCode::PageUp => self.move_selection(-15),
            KeyCode::PageDown => self.move_selection(15),
            KeyCode::Home | KeyCode::Char('g') => self.move_selection(isize::MIN),
            KeyCode::End | KeyCode::Char('G') => self.move_selection(isize::MAX),
            KeyCode::Char('/') => self.mode = Mode::Searching,
            KeyCode::Char('c') => self.sort_by(ProcessSort::Cpu),
            KeyCode::Char('m') => self.sort_by(ProcessSort::Memory),
            KeyCode::Char('p') => self.sort_by(ProcessSort::Pid),
            KeyCode::Char('n') => self.sort_by(ProcessSort::Name),
            KeyCode::Char('u') => self.sort_by(ProcessSort::User),
            KeyCode::Char('o') => {
                self.mine_only = !self.mine_only;
                self.reset_selection();
            }
            KeyCode::Char('x') => {
                self.search.clear();
                self.mine_only = false;
                self.reset_selection();
            }
            KeyCode::Char('t') | KeyCode::Char('K') | KeyCode::F(9) => {
                if let Some(p) = self.selected() {
                    self.mode = Mode::Confirm {
                        pid: p.pid,
                        name: p.name.clone(),
                        force: k.code == KeyCode::Char('K'),
                    };
                }
            }
            _ => {}
        }
    }
}
