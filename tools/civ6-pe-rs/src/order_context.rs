//! Bounded read-only facts at the two fixed EXE context-holder entries.
use super::*;

pub(super) fn module_at(pid: u32, address: u64) -> Result<Module, String> {
    let snapshot = Handle::new(
        unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid) },
        "context module snapshot",
    )?;
    let mut entry = MODULEENTRY32W {
        dwSize: size_of::<MODULEENTRY32W>() as u32,
        ..Default::default()
    };
    if unsafe { Module32FirstW(snapshot.0, &mut entry) } == 0 {
        return Err(win_error("context Module32FirstW"));
    }
    loop {
        let base = entry.modBaseAddr as u64;
        let size = u64::from(entry.modBaseSize);
        if address >= base && address - base < size {
            return Ok(Module {
                base,
                size,
                path: wide_path(&entry.szExePath)?,
            });
        }
        if unsafe { Module32NextW(snapshot.0, &mut entry) } == 0 {
            break;
        }
    }
    Err("address not in the current module snapshot".into())
}
pub(super) fn module_row(module: &Module) -> Value {
    json!({"base":module.base,"size":module.size,"path":module.path})
}
fn qword(game: &Handle, address: u64) -> Value {
    match read_pointer(game, address) {
        Ok(value) => json!({"address":address,"value":value}),
        Err(error) => json!({"address":address,"value":null,"error":error}),
    }
}
// Raw candidates from the fixed cleanup ABI; reads do not assert a Rust/game
// type, object lifetime, worker completion, or pending-list drainage.
pub(super) fn shutdown(game: &Handle, site: Site, c: &CONTEXT) -> Value {
    let manager = match site.name {
        "manager_cleanup_entry" => Some(c.Rcx),
        "manager_wait_call" | "manager_wait_return" => Some(c.Rbx),
        _ => None,
    };
    let mut row = json!({"manager_candidate":manager,"world_candidate":if site.name=="shutdown_entry"{Some(c.Rcx)}else{None},
        "world_manager_slot":null,"set_address":null,"list_slot":null,"same_set_wait_proven":false,"native_quiescence_proven":false});
    if site.name == "shutdown_entry" {
        row["world_manager_slot"] = c
            .Rcx
            .checked_add(0x250)
            .map(|address| qword(game, address))
            .unwrap_or(json!({"error":"world slot overflow"}));
    }
    if let Some(manager) = manager.filter(|m| *m != 0) {
        row["set_address"] = json!(manager.checked_add(0x400));
        row["list_slot"] = manager
            .checked_add(0x410)
            .map(|address| qword(game, address))
            .unwrap_or(json!({"error":"manager slot overflow"}));
    }
    row
}
fn object(game: &Handle, pid: u32, address: u64, core: Option<&Module>, verified: bool) -> Value {
    if address == 0 {
        return json!({"object":0,"status":"null"});
    }
    let vtable = qword(game, address);
    let mut row = json!({"object":address,"vtable":vtable,"matches_static_gamecore_context":false,"destruction_proven":false});
    if let Some(table) = vtable["value"].as_u64().filter(|v| *v != 0) {
        row["vtable_module"] = match module_at(pid, table) {
            Ok(module) => {
                let mut v = module_row(&module);
                v["rva"] = json!(table - module.base);
                v
            }
            Err(e) => json!({"error":e}),
        };
        for (key, offset) in [("virtual_release", 8), ("virtual_switch", 0x1a0)] {
            row[key] = table
                .checked_add(offset)
                .map(|a| qword(game, a))
                .unwrap_or(json!({"error":"vtable address overflow"}));
        }
        if let Some(core) = core.filter(|_| verified) {
            row["matches_static_gamecore_context"] = json!(
                core.base.checked_add(0xa101e8) == Some(table)
                    && row["virtual_release"]["value"].as_u64() == core.base.checked_add(0x8972d0)
                    && row["virtual_switch"]["value"].as_u64() == core.base.checked_add(0x58ff70)
            );
        }
    }
    row["inner_object"] = address
        .checked_add(8)
        .map(|a| qword(game, a))
        .unwrap_or(json!({"error":"object address overflow"}));
    row
}

fn caller(game: &Handle, rsp: u64, entry: u64, exe: &Module) -> Value {
    let mut row = json!({"stack_return":qword(game,rsp),"unwind_performed":false,"direct_call_to_entry":false});
    let Some(ret) = row["stack_return"]["value"].as_u64() else {
        return row;
    };
    if ret < exe.base + 5 || ret - exe.base >= exe.size {
        return row;
    }
    row["return_exe_rva"] = json!(ret - exe.base);
    match read_exact::<5>(game, ret - 5) {
        Ok(bytes) => {
            row["preceding_bytes"] = json!(bytes);
            if bytes[0] == 0xe8 {
                let delta = i32::from_le_bytes(bytes[1..5].try_into().unwrap());
                let target = ret.checked_add_signed(i64::from(delta));
                row["call_exe_rva"] = json!(ret - 5 - exe.base);
                row["direct_target"] = json!(target);
                row["direct_call_to_entry"] = json!(target == Some(entry));
            }
        }
        Err(e) => row["read_error"] = json!(e),
    }
    row
}

pub(super) fn capture(d: &Debugger, exe: &Module, site: Site, c: &CONTEXT, old: u64) -> Value {
    let core_result = core_module(d.pid);
    let core = core_result.as_ref().ok().and_then(|m| m.as_ref());
    let verified = core.is_some_and(|m| {
        same_path(&m.path, &Path::new(GAME_ROOT).join(MODULE_REL))
            && sha256(&m.path).is_ok_and(|sha| sha == plan::CORE_SHA)
    });
    let core_row = core.map(module_row).unwrap_or_else(
        || json!({"status":"absent_or_unreadable","error":core_result.as_ref().err()}),
    );
    json!({"sample":"callee_entry_before_original_instruction","holder":c.Rcx,
        "slot_address":c.Rcx.checked_add(0x450),"holder_vtable":qword(&d.game,c.Rcx),
        "global_context":qword(&d.game,exe.base+0x22d60c0),
        "old":object(&d.game,d.pid,old,core,verified),
        "incoming":if site.name=="context_replace" {object(&d.game,d.pid,c.Rdx,core,verified)}else{Value::Null},
        "same_pointer_noop":site.name=="context_replace" && old==c.Rdx,
        "core_module":core_row,"core_disk_sha_verified":verified,
        "caller":caller(&d.game,c.Rsp,c.Rip,exe),
        "world_generation":"unknown","old_object_destroyed":"unknown","runtime_activation_allowed":false})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn order_capture_context_reads_real_os_memory_without_game_type_claim() {
        let pid = std::process::id();
        let game = process(pid, true).unwrap();
        let mut table = [0u64; 53];
        table[1] = 11;
        table[52] = 22;
        let value = [table.as_ptr() as u64, 0x12345678];
        let row = object(&game, pid, value.as_ptr() as u64, None, false);
        assert_eq!(row["vtable"]["value"], table.as_ptr() as u64);
        assert_eq!(row["virtual_release"]["value"], 11);
        assert_eq!(row["virtual_switch"]["value"], 22);
        assert_eq!(row["inner_object"]["value"], 0x12345678);
        assert_eq!(row["matches_static_gamecore_context"], false);
        assert_eq!(row["destruction_proven"], false);
        assert_eq!(object(&game, pid, 0, None, false)["status"], "null");
        assert!(object(&game, pid, 1, None, false)["vtable"]["error"].is_string());
        let local = unsafe { GetModuleHandleW(std::ptr::null()) } as u64;
        assert_eq!(module_at(pid, local).unwrap().base, local);
        assert!(module_at(pid, 1).is_err());
    }
    #[test]
    fn order_capture_context_stack_return_checks_actual_e8_and_target() {
        let game = process(std::process::id(), true).unwrap();
        // Local owned memory passed through real ReadProcessMemory, not a
        // forged game stack. This tests the exact entry-return decoder only.
        let bytes = [0xe8u8, 0x10, 0, 0, 0, 0x90];
        let base = bytes.as_ptr() as u64;
        let ret = base + 5;
        let module = Module {
            base,
            size: 6,
            path: PathBuf::from("owned_decoder_fixture"),
        };
        let rsp = &ret as *const u64 as u64;
        assert_eq!(
            caller(&game, rsp, ret + 16, &module)["direct_call_to_entry"],
            true
        );
        assert_eq!(
            caller(&game, rsp, ret + 17, &module)["direct_call_to_entry"],
            false
        );
        assert!(caller(&game, 1, ret + 16, &module)["stack_return"]["error"].is_string());
    }
}
