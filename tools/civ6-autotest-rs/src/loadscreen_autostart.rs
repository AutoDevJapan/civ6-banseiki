//! The official LoadScreen consumes only this Automation flag, not a profile.
//! Keep it scoped to one owned single-player host/load operation.
use super::Runner;
use crate::{scenario::SessionType, Error, ErrorKind, Result};
use serde_json::json;

pub const READ: &str = "assert(Automation and type(Automation.IsAutoStartEnabled)=='function' and type(Automation.SetAutoStartEnabled)=='function','dedicated auto-start API unavailable')\nlocal value=Automation.IsAutoStartEnabled()\nassert(type(value)=='boolean','auto-start getter must return a boolean')\nreturn value";

pub fn write(value: bool) -> String {
    format!("{READ_PREFIX}\nAutomation.SetAutoStartEnabled({value})\nassert(Automation.IsAutoStartEnabled()=={value},'auto-start setting did not persist')\nreturn true")
}
const READ_PREFIX: &str = "assert(Automation and type(Automation.IsAutoStartEnabled)=='function' and type(Automation.SetAutoStartEnabled)=='function','dedicated auto-start API unavailable')";

fn boolean(value: &str) -> Result<bool> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(Error::scenario(format!(
            "Unexpected auto-start boolean: {value:?}"
        ))),
    }
}

impl Runner {
    pub(super) fn with_loadscreen_autostart<T>(
        &mut self,
        deadline: f64,
        operation: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        if self.scenario["loadscreen_autostart"] != true {
            return operation(self);
        }
        if self.owner_start
            || SessionType::from_scenario(&self.scenario)? != SessionType::SinglePlayer
        {
            return Err(Error::scenario(
                "loadscreen_autostart requires automatic single-player start",
            ));
        }
        let process = self
            .backend
            .single_process(self.budget(deadline, 5.0)?)?
            .ok_or_else(|| Error::input("No owned game for auto-start scope"))?;
        let previous = boolean(&self.send_frontend_transition(READ, deadline)?)?;
        self.transition_observation(
            "autostart_original",
            deadline,
            &json!({"previous":previous,"process":process}),
        )?;
        if previous {
            return operation(self);
        }
        // The previous value is known before this mutation. Even a lost setter
        // reply must restore it; never dispatch Host/Load on an unknown setter.
        let result = (|| {
            if self.backend.single_process(self.budget(deadline, 5.0)?)? != Some(process) {
                return Err(Error::input(
                    "Game process changed before enabling auto-start",
                ));
            }
            let reply = self.send_frontend_transition(&write(true), deadline)?;
            if reply != "true" {
                return Err(Error::scenario("Auto-start enable was not acknowledged"));
            }
            self.transition_observation(
                "autostart_enabled",
                deadline,
                &json!({"previous":false,"process":process}),
            )?;
            operation(self)
        })();
        // This is a separate, bounded cleanup budget, never extra time for the
        // transition or another Host/Load. The chain closes its game on failure.
        let cleanup_deadline = self.clock.now() + 10.0;
        let restored = crate::time::with_interrupt_cleanup(|| {
            self.restore_loadscreen_autostart(process, previous, cleanup_deadline)
        });
        let evidence = match &restored {
            Ok(status) => json!({"status":status,"previous":previous,"process":process}),
            Err(error) => {
                json!({"status":"unconfirmed","previous":previous,"process":process,"error":error.to_string()})
            }
        };
        let logged = self.transition_observation("autostart_restore", cleanup_deadline, &evidence);
        match result {
            Err(original) => {
                if let Err(cleanup) = restored {
                    eprintln!("Auto-start restoration unconfirmed: {cleanup}");
                }
                if let Err(error) = logged {
                    eprintln!("Auto-start cleanup evidence could not be written: {error}");
                }
                Err(original)
            }
            Ok(value) => {
                if restored? != "restored" {
                    return Err(Error::input(
                        "Game exited; auto-start restoration cannot be confirmed",
                    ));
                }
                logged?;
                Ok(value)
            }
        }
    }

    fn restore_loadscreen_autostart(
        &mut self,
        process: (u32, f64),
        previous: bool,
        deadline: f64,
    ) -> Result<&'static str> {
        self.retry_when(
            deadline,
            |this| {
                match this.backend.single_process(this.budget(deadline, 5.0)?)? {
                    None => return Ok("process_exited"),
                    Some(current) if current == process => {}
                    _ => {
                        return Err(Error::input(
                            "Game process changed; auto-start restoration refused",
                        ))
                    }
                }
                let states = this.states(this.budget(deadline, 5.0)?)?;
                let state = ["FrontEnd", "InGame", "LoadScreen"]
                    .into_iter()
                    .find(|name| crate::tuner::state_available(&states, name))
                    .ok_or_else(|| Error::tuner("No state available to restore auto-start"))?;
                let observed = boolean(&this.lua(state, READ, this.budget(deadline, 5.0)?)?)?;
                if observed == previous {
                    return Ok("restored");
                }
                let reply = this.lua(state, &write(previous), this.budget(deadline, 5.0)?)?;
                if reply != "true" {
                    return Err(Error::scenario("Auto-start restore was not acknowledged"));
                }
                Ok("restored")
            },
            |error| matches!(error.kind, ErrorKind::Tuner | ErrorKind::LuaResponseUnknown),
        )
    }
}
