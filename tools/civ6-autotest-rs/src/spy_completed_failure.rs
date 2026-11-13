//! Native outcomes shared by the Rust-generated Civic and third-operative seam.
pub const OUTPUT: &str = "poc/JAC_Test_PersonaBoundary/Shared/JAC_PersonaCompletedFailure.lua";
pub const LUA: &str = r#"-- Generated from Rust; known completed failures only, no state writes.
JAC_PersonaCompletedFailure=function(initial,escape)
 local e=EspionageResultTypes
 if type(initial)~='number' or type(escape)~='number' then return false end
 if initial==e.SUCCESS_UNDETECTED or initial==e.SUCCESS_MUST_ESCAPE then return false end
 if initial==e.FAIL_UNDETECTED then return escape==e.NO_RESULT end
 if initial==e.FAIL_MUST_ESCAPE then
  return escape==e.FAIL_MUST_ESCAPE or escape==e.KILLED or escape==e.CAPTURED
 end
 return (initial==e.KILLED or initial==e.CAPTURED) and escape==e.NO_RESULT
end
"#;
pub fn write(root: &std::path::Path) -> std::io::Result<()> {
    let path = root.join(OUTPUT);
    if std::fs::read_to_string(&path).ok().as_deref() != Some(LUA) {
        std::fs::write(path, LUA)?;
    }
    Ok(())
}
