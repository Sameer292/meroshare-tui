mod api;
mod app;
mod db;
mod ui;

use std::env;
use std::path::PathBuf;
use std::process::exit;

use anyhow::Result;

use crate::app::App;
use crate::db::Db;

fn db_path() -> Result<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    Ok(PathBuf::from(home)
        .join(".config")
        .join("meroshare-tui")
        .join("accounts.db"))
}

fn main() -> Result<()> {
    let base: String = env::var("BASE_URL").expect("Base Url must be set");
    if base.is_empty() {
        exit(401);
    }
    let db = Db::open(&db_path()?)?;
    let accounts = db.list().unwrap_or_default();

    let mut app = App::new(db, accounts);
    app.start_fetch();

    let mut terminal = ratatui::init();
    let result = app.run(&mut terminal);
    ratatui::restore();
    result
}
