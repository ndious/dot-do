//! SQLite database: users, sessions and the repository registry.

use std::path::Path;
use std::sync::Mutex;

use anyhow::{bail, Context, Result};
use argon2::password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use rusqlite::Connection;

/// A managed repository.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Repo {
    pub id: i64,
    pub name: String,
    pub url: String,
}

/// SQLite database behind the server. The connection is behind a
/// std Mutex: handlers use it only synchronously (no await while held).
pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("cannot open database {}", path.display()))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS users (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 username TEXT UNIQUE NOT NULL,
                 password_hash TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS sessions (
                 token TEXT PRIMARY KEY,
                 user_id INTEGER NOT NULL REFERENCES users(id),
                 created_at TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS repos (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 name TEXT UNIQUE NOT NULL,
                 url TEXT NOT NULL,
                 created_at TEXT NOT NULL
             );",
        )?;
        Ok(Db { conn: Mutex::new(conn) })
    }

    /// Bootstrap the first admin account from the environment
    /// (TOD_ADMIN_USER / TOD_ADMIN_PASS) when no user exists yet.
    pub fn bootstrap_from_env(&self) -> Result<bool> {
        let count: i64 = self.conn.lock().unwrap().query_row(
            "SELECT COUNT(*) FROM users", [], |r| r.get(0),
        )?;
        if count > 0 {
            return Ok(false);
        }
        let user = std::env::var("TOD_ADMIN_USER").ok();
        let pass = std::env::var("TOD_ADMIN_PASS").ok();
        match (user, pass) {
            (Some(u), Some(p)) if !u.is_empty() && !p.is_empty() => {
                self.create_user(&u, &p)?;
                println!("bootstrap: created admin user '{u}'");
                Ok(true)
            }
            _ => {
                eprintln!(
                    "warning: no user in the database and no TOD_ADMIN_USER/TOD_ADMIN_PASS                      in the environment: nobody can log in yet"
                );
                Ok(false)
            }
        }
    }

    pub fn create_user(&self, username: &str, password: &str) -> Result<()> {
        if username.trim().is_empty() || password.len() < 4 {
            bail!("username must be non-empty and password at least 4 chars");
        }
        let salt = SaltString::generate(&mut OsRng);
        let hash = Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| anyhow::anyhow!("cannot hash password: {e}"))?
            .to_string();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO users (username, password_hash) VALUES (?1, ?2)",
            rusqlite::params![username.trim(), hash],
        )
        .map_err(|e| anyhow::anyhow!("cannot create user: {e}"))?;
        Ok(())
    }

    /// Verify credentials and return a fresh session token.
    pub fn login(&self, username: &str, password: &str) -> Result<String> {
        let (user_id, hash): (i64, String) = {
            let conn = self.conn.lock().unwrap();
            conn.query_row(
                "SELECT id, password_hash FROM users WHERE username = ?1",
                rusqlite::params![username],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|_| anyhow::anyhow!("invalid username or password"))?
        };
        let parsed = PasswordHash::new(&hash)
            .map_err(|e| anyhow::anyhow!("corrupted password hash: {e}"))?;
        if Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_err()
        {
            bail!("invalid username or password");
        }
        let token = new_token();
        self.conn.lock().unwrap().execute(
            "INSERT INTO sessions (token, user_id, created_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![token, user_id, chrono::Utc::now().to_rfc3339()],
        )?;
        Ok(token)
    }

    /// Check a bearer token. Ok(()) when the session is valid.
    pub fn check_session(&self, token: &str) -> Result<()> {
        self.conn
            .lock()
            .unwrap()
            .query_row("SELECT 1 FROM sessions WHERE token = ?1", rusqlite::params![token], |_| Ok(()))
            .map_err(|_| anyhow::anyhow!("invalid or expired session"))
    }

    pub fn list_repos(&self) -> Result<Vec<Repo>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, name, url FROM repos ORDER BY name")?;
        let rows = stmt
            .query_map([], |r| Ok(Repo { id: r.get(0)?, name: r.get(1)?, url: r.get(2)? }))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn add_repo(&self, name: &str, url: &str) -> Result<Repo> {
        let name = name.trim();
        if name.is_empty() || url.trim().is_empty() {
            bail!("name and url must be non-empty");
        }
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO repos (name, url, created_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![name, url.trim(), chrono::Utc::now().to_rfc3339()],
        )
        .map_err(|e| anyhow::anyhow!("cannot add repository: {e}"))?;
        let id = conn.last_insert_rowid();
        Ok(Repo { id, name: name.to_string(), url: url.trim().to_string() })
    }

    pub fn get_repo(&self, id: i64) -> Result<Repo> {
        self.conn.lock().unwrap().query_row(
            "SELECT id, name, url FROM repos WHERE id = ?1",
            rusqlite::params![id],
            |r| Ok(Repo { id: r.get(0)?, name: r.get(1)?, url: r.get(2)? }),
        )
        .map_err(|_| anyhow::anyhow!("unknown repository #{id}"))
    }

    pub fn delete_repo(&self, id: i64) -> Result<()> {
        let n = self.conn.lock().unwrap().execute("DELETE FROM repos WHERE id = ?1", rusqlite::params![id])?;
        if n == 0 {
            bail!("unknown repository #{id}");
        }
        Ok(())
    }
}

fn new_token() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> (tempfile::TempDir, Db) {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("test.db")).unwrap();
        (dir, db)
    }

    #[test]
    fn user_login_and_session_round_trip() {
        let (_d, db) = db();
        db.create_user("nicolas", "secret").unwrap();
        assert!(db.login("nicolas", "wrong").is_err());
        assert!(db.login("nobody", "secret").is_err());
        let token = db.login("nicolas", "secret").unwrap();
        assert!(db.check_session(&token).is_ok());
        assert!(db.check_session("nope").is_err());
    }

    #[test]
    fn duplicate_usernames_are_rejected() {
        let (_d, db) = db();
        db.create_user("nicolas", "secret").unwrap();
        assert!(db.create_user("nicolas", "other").is_err());
    }

    #[test]
    fn repos_crud() {
        let (_d, db) = db();
        let r = db.add_repo("dot-do", "https://github.com/ndious/dot-do.git").unwrap();
        assert_eq!(db.list_repos().unwrap().len(), 1);
        assert_eq!(db.get_repo(r.id).unwrap().name, "dot-do");
        db.delete_repo(r.id).unwrap();
        assert!(db.list_repos().unwrap().is_empty());
        assert!(db.delete_repo(r.id).is_err());
    }
}
