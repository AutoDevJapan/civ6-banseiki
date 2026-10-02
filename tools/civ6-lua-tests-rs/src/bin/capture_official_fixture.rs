//! Refresh with: cargo run --manifest-path tools/civ6-lua-tests-rs/Cargo.toml
//! --bin capture_official_fixture -- <Civ6 install directory>
use civ6_lua_tests_rs::{official, repo_root};

fn main() {
    let game = std::env::args_os()
        .nth(1)
        .expect("provide the Civ6 asset directory");
    let db = official::installed_official_db(std::path::Path::new(&game));
    let path = repo_root().join("tools/civ6-lua-tests-rs/fixtures/official/pre_sengoku.sql.gz");
    official::capture_snapshot(&db, &path);
    println!(
        "公式GSデータの全行とスキーマを照合し、戦国パックのSQLを検証したfixtureを保存しました: {}",
        path.display()
    );
}
