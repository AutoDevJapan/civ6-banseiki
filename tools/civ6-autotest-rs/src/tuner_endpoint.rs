//! Civ6 can move its FireTuner listener while hosting hotseat (4318 -> 4319).
//! Resolve the game's actual loopback listener; never probe another process's port.
#[cfg(any(windows, test))]
use crate::Error;
use crate::Result;

#[cfg(any(windows, test))]
#[derive(Clone, Copy, Debug)]
struct Listener {
    pid: u32,
    address: [u8; 4],
    port: u16,
}

#[cfg(any(windows, test))]
fn select_port(requested: u16, pid: u32, rows: &[Listener]) -> Result<u16> {
    let ports: std::collections::BTreeSet<_> = rows
        .iter()
        .filter(|r| r.pid == pid && r.address == [127, 0, 0, 1])
        .map(|r| r.port)
        .filter(|p| {
            *p == requested || (requested == crate::tuner::PORT && (4318..=4333).contains(p))
        })
        .collect();
    if ports.contains(&requested) {
        return Ok(requested);
    }
    if ports.len() == 1 {
        return Ok(*ports.first().unwrap());
    }
    Err(Error::tuner(format!(
        "No unique Civ6 loopback FireTuner listener for PID {pid}: {ports:?}"
    )))
}

#[cfg(windows)]
fn listeners() -> Result<Vec<Listener>> {
    use windows_sys::Win32::{
        Foundation::ERROR_INSUFFICIENT_BUFFER,
        NetworkManagement::IpHelper::{
            GetExtendedTcpTable, MIB_TCPROW_OWNER_PID, TCP_TABLE_OWNER_PID_LISTENER,
        },
    };
    let mut size = 0u32;
    let status = unsafe {
        GetExtendedTcpTable(
            std::ptr::null_mut(),
            &mut size,
            0,
            2,
            TCP_TABLE_OWNER_PID_LISTENER,
            0,
        )
    };
    if status != ERROR_INSUFFICIENT_BUFFER {
        return Err(Error::tuner(format!(
            "Read TCP listener size failed: {status}"
        )));
    }
    // u32 alignment matches the table and row layout. Retry only a finite size race.
    for _ in 0..3 {
        let mut buffer = vec![0u32; (size as usize).div_ceil(4)];
        let status = unsafe {
            GetExtendedTcpTable(
                buffer.as_mut_ptr().cast(),
                &mut size,
                0,
                2,
                TCP_TABLE_OWNER_PID_LISTENER,
                0,
            )
        };
        if status == ERROR_INSUFFICIENT_BUFFER {
            continue;
        }
        if status != 0 {
            return Err(Error::tuner(format!("Read TCP listeners failed: {status}")));
        }
        if size < 4 || buffer.is_empty() {
            return Err(Error::tuner("Invalid TCP listener table size"));
        }
        let count = buffer[0] as usize;
        let row_size = std::mem::size_of::<MIB_TCPROW_OWNER_PID>();
        if count > (size as usize - 4) / row_size {
            return Err(Error::tuner("Invalid TCP listener table size"));
        }
        let ptr = unsafe { buffer.as_ptr().add(1).cast::<MIB_TCPROW_OWNER_PID>() };
        let rows = unsafe { std::slice::from_raw_parts(ptr, count) };
        return Ok(rows
            .iter()
            .map(|r| Listener {
                pid: r.dwOwningPid,
                address: r.dwLocalAddr.to_ne_bytes(),
                port: u16::from_be(r.dwLocalPort as u16),
            })
            .collect());
    }
    Err(Error::tuner("TCP listener table kept changing"))
}

#[cfg(windows)]
pub fn resolve(requested: u16) -> Result<u16> {
    let (pid, _) = crate::restart::single_civ_process(0.0)?
        .ok_or_else(|| Error::tuner("Civ6 is not running"))?;
    select_port(requested, pid, &listeners()?)
}

#[cfg(not(windows))]
pub fn resolve(requested: u16) -> Result<u16> {
    Ok(requested)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn row(pid: u32, port: u16) -> Listener {
        Listener {
            pid,
            port,
            address: [127, 0, 0, 1],
        }
    }
    #[test]
    fn hosting_port_move_uses_only_the_game_process() {
        assert_eq!(
            select_port(4318, 7, &[row(8, 4318), row(7, 4319)]).unwrap(),
            4319
        );
        assert!(select_port(4318, 7, &[row(8, 4318)]).is_err());
    }
    #[test]
    fn default_and_explicit_ports_are_preserved() {
        assert_eq!(
            select_port(4318, 7, &[row(7, 4318), row(7, 4319)]).unwrap(),
            4318
        );
        assert_eq!(
            select_port(1234, 7, &[row(7, 1234), row(7, 4319)]).unwrap(),
            1234
        );
        assert!(select_port(1234, 7, &[row(7, 4319)]).is_err());
    }
    #[test]
    fn ambiguous_or_non_loopback_listeners_are_not_guessed() {
        assert!(select_port(4318, 7, &[row(7, 4319), row(7, 4320)]).is_err());
        assert!(select_port(
            4318,
            7,
            &[Listener {
                address: [0, 0, 0, 0],
                ..row(7, 4319)
            }]
        )
        .is_err());
        assert!(select_port(4318, 7, &[row(7, 5000)]).is_err());
    }
    #[cfg(windows)]
    #[test]
    fn native_snapshot_reads_real_loopback_listener_and_owner() {
        let socket = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = socket.local_addr().unwrap().port();
        assert!(listeners()
            .unwrap()
            .iter()
            .any(|r| r.pid == std::process::id() && r.address == [127, 0, 0, 1] && r.port == port));
    }
}
