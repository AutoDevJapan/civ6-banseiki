//! Match the original class-wide official GS database setup. Every assertion
//! queries the full schema after the current worktree's Sengoku SQL is applied.
use rusqlite::Connection;
use std::sync::{LazyLock, Mutex, MutexGuard};

static DB: LazyLock<Mutex<Connection>> =
    LazyLock::new(|| Mutex::new(civ6_lua_tests_rs::official::sengoku_db()));

pub fn fixture_db() -> MutexGuard<'static, Connection> {
    DB.lock().unwrap()
}
