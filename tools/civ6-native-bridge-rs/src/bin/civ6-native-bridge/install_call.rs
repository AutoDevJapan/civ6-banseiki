//! Copy install phase words around the original eight-second remote call.
//! No diagnostic thread or extra native call; always retain the original error.
use super::*;
fn snapshot(game:&Handle,address:Option<usize>)->Value {
    let Some(address)=address else {return json!({"available":false})};
    let mut bytes=[0u8;20];
    if let Err(error)=read_remote(game,address,&mut bytes) {return json!({"available":false,"error":error});}
    let words:Vec<_>=bytes.chunks_exact(4).map(|w|u32::from_le_bytes(w.try_into().unwrap())).collect();
    json!({"available":true,"raw":words,"schema":words[0],"build_flags":words[1],"phase":words[2],"thread":words[3],
        "native_return":if words[4]==u32::MAX {Value::Null}else{json!(words[4])}})
}
pub(super) fn invoke(game:&Handle,entry:usize,parameter:usize,progress:Option<usize>,parsed:&Args)->Result<u32,String> {
    let mut writer=BufWriter::new(File::create_new(parsed.out.with_extension("install.jsonl")).map_err(|e|e.to_string())?);
    write_json(&mut writer,json!({"kind":"install_call_before","pid":parsed.pid,"created_filetime":parsed.created,
        "entry":entry,"timeout_seconds":REMOTE_CALL_SECONDS,"progress":snapshot(game,progress)}))?;
    let start=Instant::now();
    let result=remote_call(game,entry,parameter,"BridgeRuntimeInstall");
    let log=write_json(&mut writer,json!({"kind":"install_call_after","pid":parsed.pid,"created_filetime":parsed.created,
        "elapsed_ms":start.elapsed().as_millis() as u64,"returned":result.is_ok(),"status":result.as_ref().ok(),
        "error":result.as_ref().err(),"progress":snapshot(game,progress)}));
    match (result,log) {(result,Ok(()))=>result,(Err(error),Err(log))=>Err(format!("{error}; install diagnostic: {log}")),(Ok(_),Err(log))=>Err(log)}
}
#[cfg(test)]
mod tests {
    use super::*;
    unsafe extern "system" {fn BridgeRuntimeInstall(input:*mut c_void)->u32;static BRIDGE_RUNTIME_INSTALL_PROGRESS:[u32;5];}
    #[test]
    fn install_call_records_real_native_entry_and_refusal_without_a_game() {
        let game=open_process(unsafe {GetCurrentProcessId()},PROCESS_CREATE_THREAD|PROCESS_VM_READ|PROCESS_QUERY_LIMITED_INFORMATION).unwrap();
        let dir=tempfile::tempdir().unwrap();
        let args=Args {pid:unsafe {GetCurrentProcessId()},created:process_created(&game).unwrap(),duration:1,out:dir.path().join("runtime.jsonl"),mode:BridgeMode::ProductionAdmission,paid_experiment:false,runtime:true};
        let address=std::ptr::addr_of!(BRIDGE_RUNTIME_INSTALL_PROGRESS) as usize;
        assert_eq!(invoke(&game,BridgeRuntimeInstall as *const () as usize,0,Some(address),&args).unwrap(),BridgeStatus::InvalidInput as u32);
        let rows:Vec<Value>=std::fs::read_to_string(args.out.with_extension("install.jsonl")).unwrap().lines().map(|s|serde_json::from_str(s).unwrap()).collect();
        assert_eq!(rows.len(),2);assert_eq!(rows[0]["kind"],"install_call_before");
        assert_eq!(rows[0]["timeout_seconds"],8);
        assert_eq!(rows[1]["progress"]["phase"],1);assert_eq!(rows[1]["progress"]["native_return"],BridgeStatus::InvalidInput as u32);
        assert!(rows[1]["progress"]["thread"].as_u64().unwrap()>0);
        assert_eq!(rows[1]["returned"],true);
    }
    #[test]
    fn install_call_retains_unknown_failure_and_does_not_mint_native_return() {
        let game=open_process(unsafe {GetCurrentProcessId()},PROCESS_VM_READ|PROCESS_QUERY_LIMITED_INFORMATION).unwrap();
        let dir=tempfile::tempdir().unwrap();
        let args=Args {pid:unsafe {GetCurrentProcessId()},created:process_created(&game).unwrap(),duration:1,out:dir.path().join("runtime.jsonl"),mode:BridgeMode::ProductionAdmission,paid_experiment:false,runtime:true};
        assert!(invoke(&game,0,0,None,&args).unwrap_err().contains("address missing"));
        let rows:Vec<Value>=std::fs::read_to_string(args.out.with_extension("install.jsonl")).unwrap().lines().map(|s|serde_json::from_str(s).unwrap()).collect();
        assert_eq!(rows[1]["returned"],false);assert!(rows[1]["status"].is_null());assert_eq!(rows[1]["progress"]["available"],false);
    }
}
