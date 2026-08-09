use std::path::Path;
use std::fs;

use anyhow::{Context, Result};
use rusqlite::{params, Connection};

#[derive(Debug, Clone)]
pub struct Account {
    pub id: i64,
    pub name: String,
    pub client_id: i64,
    pub username: String,
    pub password: String,
    pub demat: String,
    pub client_code: String,
}

pub struct Db {
    conn: Connection,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).context("creating data dir")?;
        }
        let conn = Connection::open(path).context("opening database")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS accounts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                client_id INTEGER NOT NULL,
                username TEXT NOT NULL,
                password TEXT NOT NULL,
                demat TEXT NOT NULL,
                client_code TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );",
        )
        .context("creating schema")?;
        Ok(Self { conn })
    }

    pub fn list(&self) -> Result<Vec<Account>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id,name,client_id,username,password,demat,client_code FROM accounts ORDER BY name COLLATE NOCASE")?;
        let rows = stmt.query_map([], |r| {
            Ok(Account {
                id: r.get(0)?,
                name: r.get(1)?,
                client_id: r.get(2)?,
                username: r.get(3)?,
                password: r.get(4)?,
                demat: r.get(5)?,
                client_code: r.get(6)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn add(
        &self,
        name: &str,
        client_id: i64,
        username: &str,
        password: &str,
        demat: &str,
        client_code: &str,
    ) -> Result<()> {
        self.conn
            .execute(
                "INSERT INTO accounts (name,client_id,username,password,demat,client_code) VALUES (?1,?2,?3,?4,?5,?6)",
                params![name, client_id, username, password, demat, client_code],
            )
            .context("inserting account")?;
        Ok(())
    }

    pub fn delete(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM accounts WHERE id=?1", params![id])
            .context("deleting account")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let tmp = std::env::temp_dir().join(format!("meroshare-test-{}.db", std::process::id()));
        let db = Db::open(&tmp).unwrap();
        db.add("A", 130, "u", "p", "1300000", "130-0").unwrap();
        db.add("B", 58, "v", "q", "5800000", "58-1").unwrap();
        let accounts = db.list().unwrap();
        assert_eq!(accounts.len(), 2);
        db.delete(accounts[0].id).unwrap();
        assert_eq!(db.list().unwrap().len(), 1);
        let _ = std::fs::remove_file(tmp);
    }
}
