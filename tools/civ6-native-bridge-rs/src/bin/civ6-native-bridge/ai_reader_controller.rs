use super::*;
pub(super) fn prepare(
    input: &Value,
    parsed: &Args,
    game: &Handle,
    image: BridgeImage,
    dll: &Path,
    writer: &mut BufWriter<File>,
) -> Result<Value, String> {
    let rows = civ6_native_bridge_rs::ai_reader::candidates(input)?;
    write_json(
        writer,
        json!({"kind":"ai_reader_candidates","pid":parsed.pid,"created_filetime":parsed.created,"read":input}),
    )?;
    let local = LocalDll(unsafe {
        LoadLibraryExW(
            wide(&dll.to_string_lossy()).as_ptr(),
            null_mut(),
            DONT_RESOLVE_DLL_REFERENCES,
        )
    });
    if local.0.is_null() {
        return Err("reader DLL inspection failed".into());
    }
    let address = unsafe { GetProcAddress(local.0, c"BridgeRuntimeAiQuery".as_ptr().cast()) }
        .ok_or("reader query export absent")? as usize;
    let rva = address
        .checked_sub(local.0 as usize)
        .filter(|r| *r < image.size)
        .ok_or("reader query outside image")?;
    let deadline = Instant::now() + Duration::from_secs(8);
    for row in rows {
        if Instant::now() >= deadline {
            return Err("finite native reader selection elapsed".into());
        }
        let mut state: AiRuntimeState = unsafe { zeroed() };
        state.size = size_of::<AiRuntimeState>() as u32;
        state.version = AI_OBSERVATION_VERSION;
        state.game_pid = parsed.pid;
        state.created_filetime = parsed.created;
        state.owner = row["owner"].as_u64().unwrap() as u32;
        state.governor_index = row["index"].as_u64().unwrap() as u32;
        let status = call_struct(game, image.base + rva, &mut state)?;
        let evidence = json!({"kind":"ai_reader_native_query","pid":state.game_pid,"created_filetime":state.created_filetime,
            "owner":state.owner,"index":state.governor_index,"generation":state.generation,"status":status,"ready":state.ready,
            "catalog_cost":state.catalog_cost,"catalog_base":state.catalog_base,"catalog_title1":state.catalog_title1});
        write_json(writer, evidence.clone())?;
        if status != BridgeStatus::Ok as u32 || state.ready != 1 || state.generation == 0 {
            continue;
        }
        if row["cost"] != state.catalog_cost
            || row["base"] != state.catalog_base
            || row["title"] != state.catalog_title1
        {
            continue;
        }
        let request = civ6_native_bridge_rs::ai_reader::request(row, state.generation)?;
        let mut file = BufWriter::new(
            File::create_new(parsed.out.with_extension("ai.request.json"))
                .map_err(|e| e.to_string())?,
        );
        write_json(
            &mut file,
            json!({"request":request,"native_query":evidence,"ui_read":row,
            "initial_install_order":"unknown","drain":"unknown","world_generation_proven":false,"payment_authority":false}),
        )?;
        return Ok(request);
    }
    Err("no real AI owner has a matching committed native rowcatalog; no registration sent".into())
}
