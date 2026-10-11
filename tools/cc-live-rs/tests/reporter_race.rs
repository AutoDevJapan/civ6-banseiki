#[cfg(windows)]
#[test]
fn enumerated_reporter_exit_is_success_but_termination_errors_still_fail() {
    use cc_live::{chain::REPORTER_COMMAND, process::CREATE_NO_WINDOW};
    use std::{os::windows::process::CommandExt, process::Command};

    // Execute the actual cleanup script with process cmdlets replaced; no real
    // game or reporter is inspected or killed by this regression test.
    for (error_id, succeeds) in [
        ("NoProcessFoundForGivenId", true),
        ("CouldNotStopProcess", false),
        ("UnexpectedFailure", false),
    ] {
        let script = format!(
            r#"
function Get-Process {{
    [CmdletBinding()] param([string[]]$Name, [int]$Id)
    return $null
}}
function Get-CimInstance {{
    [CmdletBinding()] param([string]$ClassName, [string]$Filter)
    [pscustomobject]@{{
        ExecutablePath = "C:\Program Files (x86)\Steam\steamapps\common\Sid Meier's Civilization VI\Base\Binaries\Win64Steam\FiraxisBugReporter.exe"
        ParentProcessId = 1
        ProcessId = 2
    }}
}}
function Stop-Process {{
    [CmdletBinding()] param([int]$Id)
    $record = [System.Management.Automation.ErrorRecord]::new(
        [System.InvalidOperationException]::new('synthetic termination failure'),
        '{error_id}', [System.Management.Automation.ErrorCategory]::ObjectNotFound, $Id)
    $PSCmdlet.ThrowTerminatingError($record)
}}
{REPORTER_COMMAND}
Write-Output 'cleanup completed'
"#
        );
        let output = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .unwrap();
        assert_eq!(
            output.status.success(),
            succeeds,
            "{error_id}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).contains("cleanup completed"),
            succeeds
        );
    }
}
