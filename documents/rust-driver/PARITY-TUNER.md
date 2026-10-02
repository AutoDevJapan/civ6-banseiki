# FireTuner migration notes

This slice implements the FireTuner frame codec, TCP client, and `civ6-tuner` command in Rust. It does not connect to Civ VI during tests. All integration tests use a loopback fake TCP server; the tests run on Linux as well as Windows.

| Python behavior | Rust location |
| --- | --- |
| `PORT`, `MAX_FRAME`, `MAX_LUA_CODE_BYTES`, `Frame`, `LuaResult` | `src/tuner.rs` public API |
| `_lua_payload`, `validate_lua_code`, `send_frame`, `recv_frame` | `src/tuner.rs` public API and private deadline-aware I/O |
| `TunerClient.__enter__` (`APP:` then nonempty `LSQ:`) | `TunerClient::connect` with one connection deadline |
| `list_state_pairs`, `list_states`, `state_index` | Corresponding `TunerClient` methods; LSQ indices are used for `CMD:` |
| `run_lua` and unique print acknowledgment | `TunerClient::run_lua`; one command write, `OK` value/output, `ERR` as `LuaExecution`, lost acknowledgment as `LuaResponseUnknown` |
| `main` (`states` / `lua`) | `src/bin/civ6-tuner.rs` |

The Rust client closes its TCP socket after an unknown Lua outcome. This prevents a late acknowledgment from being mistaken for a reply to a later operation. A confirmed Lua `ERR` remains `LuaExecution` and is not retried. Query errors remain `Tuner` errors so the scenario layer can decide whether a read-only query may be retried on a fresh connection.

State matching uses Unicode lowercase in Rust. Python uses full Unicode `casefold`; Civ VI state identifiers used by this driver are ASCII, for which the behavior matches. A full Unicode casefold table would require an additional dependency, which this assigned slice cannot add.

Verification in this worktree used an isolated copy of this crate under `tests/.compile-check` because `tools/Cargo.lock` is absent and the workspace lock file is outside this assignment. The copy was built offline with the same source and manifest dependencies. The final workspace command remains:

```text
C:/Users/makhmeto/.cargo/bin/cargo.exe test --manifest-path tools/civ6-autotest-rs/Cargo.toml --target-dir tools/target
```
