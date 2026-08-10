use std::io;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, KeyCode, KeyEvent, KeyModifiers, MouseEvent,
    MouseEventKind,
};
use crossterm::execute;

use crate::api::{self, PortfolioSummary};
use crate::db::{Account, Db};
use crate::ui;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Dashboard,
    Accounts,
}

#[derive(Clone)]
pub enum RowState {
    Loading,
    Ok(PortfolioSummary),
    Err(String),
}

enum Msg {
    Account(usize, Result<PortfolioSummary, String>),
    Done,
}

pub struct App {
    pub db: Db,
    pub accounts: Vec<Account>,
    pub tab: Tab,
    pub rows: Vec<RowState>,
    pub selected: usize,
    pub accounts_selected: usize,
    pub detail: Option<usize>,
    pub detail_selected: usize,
    pub loading: bool,
    pub should_quit: bool,
    pub message: String,
    pub spin: u8,
    pub form: Option<FormState>,
    pub confirm_delete: Option<usize>,
    pub hide_amounts: bool,
    rx: Receiver<Msg>,
    tx: Sender<Msg>,
    tick: Instant,
    last_fetch: Instant,
}

pub struct FormState {
    pub fields: Vec<FormField>,
    pub focus: usize,
    pub error: Option<String>,
}

pub struct FormField {
    pub label: &'static str,
    pub value: String,
    pub secret: bool,
}

const SPINNER: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
const POLL_INTERVAL: Duration = Duration::from_secs(30);

impl App {
    pub fn new(db: Db, accounts: Vec<Account>) -> Self {
        let (tx, rx) = mpsc::channel();
        let rows = vec![RowState::Loading; accounts.len()];
        App {
            db,
            accounts,
            tab: Tab::Dashboard,
            rows,
            selected: 0,
            accounts_selected: 0,
            detail: None,
            detail_selected: 0,
            loading: true,
            should_quit: false,
            message: String::new(),
            spin: 0,
            form: None,
            confirm_delete: None,
            hide_amounts: false,
            rx,
            tx,
            tick: Instant::now(),
            last_fetch: Instant::now(),
        }
    }

    pub fn spinner(&self) -> char {
        SPINNER[(self.spin as usize) % SPINNER.len()]
    }

    pub fn tick(&mut self) {
        if self.tick.elapsed() >= Duration::from_millis(80) {
            self.spin = self.spin.wrapping_add(1);
            self.tick = Instant::now();
        }
        if !self.loading && self.last_fetch.elapsed() >= POLL_INTERVAL {
            self.start_fetch();
        }
        self.process_events();
    }

    pub fn start_fetch(&mut self) {
        self.last_fetch = Instant::now();
        self.loading = true;
        self.rows = vec![RowState::Loading; self.accounts.len()];
        let accounts = self.accounts.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            for (i, acc) in accounts.iter().enumerate() {
                let result = api::fetch_account(acc).map_err(|e| e.to_string());
                let _ = tx.send(Msg::Account(i, result));
            }
            let _ = tx.send(Msg::Done);
        });
    }

    fn process_events(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Msg::Account(i, result) => {
                    if let Some(row) = self.rows.get_mut(i) {
                        *row = match result {
                            Ok(p) => RowState::Ok(p),
                            Err(e) => RowState::Err(e),
                        };
                    }
                }
                Msg::Done => self.loading = false,
            }
        }
    }

    pub fn on_key(&mut self, key: KeyEvent) {
        if self.form.is_some() {
            self.handle_form_key(key);
            return;
        }
        if let Some(idx) = self.confirm_delete {
            self.handle_confirm_key(key, idx);
            return;
        }
        if self.detail.is_some() {
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter | KeyCode::Char('h') => {
                    self.detail = None;
                }
                KeyCode::Char('s') => self.hide_amounts = !self.hide_amounts,
                KeyCode::Down | KeyCode::Char('j') => self.move_detail_selection(1),
                KeyCode::Up | KeyCode::Char('k') => self.move_detail_selection(-1),
                _ => {}
            }
            return;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('c') | KeyCode::Char('q') => self.should_quit = true,
                _ => {}
            }
            return;
        }
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('s') => self.hide_amounts = !self.hide_amounts,
            KeyCode::Char('r') => self.start_fetch(),
            KeyCode::Tab => {
                self.tab = match self.tab {
                    Tab::Dashboard => Tab::Accounts,
                    Tab::Accounts => Tab::Dashboard,
                }
            }
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
            KeyCode::Enter | KeyCode::Char('l')
                if self.tab == Tab::Dashboard
                    && matches!(self.rows.get(self.selected), Some(RowState::Ok(_))) =>
            {
                self.detail = Some(self.selected);
                self.detail_selected = 0;
            }
            KeyCode::Char('a') if self.tab == Tab::Accounts => {
                self.form = Some(FormState {
                    fields: vec![
                        FormField {
                            label: "Name",
                            value: String::new(),
                            secret: false,
                        },
                        FormField {
                            label: "Client ID",
                            value: String::new(),
                            secret: false,
                        },
                        FormField {
                            label: "Username",
                            value: String::new(),
                            secret: false,
                        },
                        FormField {
                            label: "Password",
                            value: String::new(),
                            secret: true,
                        },
                        FormField {
                            label: "Demat",
                            value: String::new(),
                            secret: false,
                        },
                        FormField {
                            label: "Client Code",
                            value: String::new(),
                            secret: false,
                        },
                    ],
                    focus: 0,
                    error: None,
                });
            }
            KeyCode::Char('d') if self.tab == Tab::Accounts && !self.accounts.is_empty() => {
                self.confirm_delete = Some(self.accounts_selected);
            }
            _ => {}
        }
    }

    fn move_selection(&mut self, delta: i32) {
        let len = match self.tab {
            Tab::Dashboard => self.rows.len(),
            Tab::Accounts => self.accounts.len(),
        };
        if len == 0 {
            return;
        }
        let sel = match self.tab {
            Tab::Dashboard => &mut self.selected,
            Tab::Accounts => &mut self.accounts_selected,
        };
        let next = (*sel as i32 + delta).clamp(0, len as i32 - 1) as usize;
        *sel = next;
    }

    fn move_detail_selection(&mut self, delta: i32) {
        let len = match self.detail.and_then(|idx| self.rows.get(idx)) {
            Some(RowState::Ok(p)) => p.holdings.len(),
            _ => return,
        };
        if len == 0 {
            return;
        }
        let next = (self.detail_selected as i32 + delta).clamp(0, len as i32 - 1) as usize;
        self.detail_selected = next;
    }

    fn on_mouse(&mut self, event: MouseEvent) {
        if self.form.is_some() || self.confirm_delete.is_some() {
            return;
        }
        match event.kind {
            MouseEventKind::ScrollDown => {
                if self.detail.is_some() {
                    self.move_detail_selection(1);
                } else {
                    self.move_selection(1);
                }
            }
            MouseEventKind::ScrollUp => {
                if self.detail.is_some() {
                    self.move_detail_selection(-1);
                } else {
                    self.move_selection(-1);
                }
            }
            _ => {}
        }
    }

    fn handle_confirm_key(&mut self, key: KeyEvent, idx: usize) {
        match key.code {
            KeyCode::Char('y') => {
                if let Some(acc) = self.accounts.get(idx) {
                    let _ = self.db.delete(acc.id);
                    self.message = format!("Deleted {}", acc.name);
                }
                self.accounts.remove(idx);
                if self.accounts_selected >= self.accounts.len() && !self.accounts.is_empty() {
                    self.accounts_selected = self.accounts.len() - 1;
                }
                self.confirm_delete = None;
                self.start_fetch();
            }
            KeyCode::Char('n') | KeyCode::Esc => self.confirm_delete = None,
            _ => {}
        }
    }

    fn handle_form_key(&mut self, key: KeyEvent) {
        let form = self.form.as_mut().unwrap();
        match key.code {
            KeyCode::Esc => self.form = None,
            KeyCode::Tab | KeyCode::Down => {
                form.focus = (form.focus + 1) % form.fields.len();
            }
            KeyCode::Up => {
                if form.focus == 0 {
                    form.focus = form.fields.len() - 1;
                } else {
                    form.focus -= 1;
                }
            }
            KeyCode::Backspace => {
                form.fields[form.focus].value.pop();
            }
            KeyCode::Enter => {
                let vals: Vec<String> = form
                    .fields
                    .iter()
                    .map(|f| f.value.trim().to_string())
                    .collect();
                let empty = vals.iter().any(|v| v.is_empty());
                match vals[1].parse::<i64>() {
                    Err(_) => form.error = Some("Client ID must be a number".into()),
                    Ok(client_id) if !empty => {
                        let _ = self
                            .db
                            .add(&vals[0], client_id, &vals[2], &vals[3], &vals[4], &vals[5]);
                        self.message = format!("Added {}", vals[0]);
                        self.accounts = self.db.list().unwrap_or_default();
                        self.form = None;
                        self.start_fetch();
                    }
                    _ => form.error = Some("All fields are required".into()),
                }
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                form.fields[form.focus].value.push(c);
            }
            _ => {}
        }
    }

    pub fn run(&mut self, terminal: &mut ratatui::DefaultTerminal) -> Result<()> {
        execute!(io::stdout(), EnableMouseCapture)?;
        let result = self.run_loop(terminal);
        execute!(io::stdout(), DisableMouseCapture)?;
        result
    }

    fn run_loop(&mut self, terminal: &mut ratatui::DefaultTerminal) -> Result<()> {
        loop {
            terminal.draw(|f| ui::render(f, self))?;
            if crossterm::event::poll(Duration::from_millis(50))? {
                match crossterm::event::read()? {
                    crossterm::event::Event::Key(key) => self.on_key(key),
                    crossterm::event::Event::Mouse(mouse) => self.on_mouse(mouse),
                    _ => {}
                }
            }
            self.tick();
            if self.should_quit {
                return Ok(());
            }
        }
    }
}
