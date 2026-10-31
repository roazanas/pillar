use crate::{
    compiler,
    diagnostics::{Diagnostic, Result},
    sema::CheckedProgram,
    source::Span,
};
use std::{ffi::OsString, io::Write, path::Path, process::Command};
const RUNTIME: &[u8] = include_bytes!(env!("PILLAR_RUNTIME"));
#[derive(Default)]
pub struct BuildOptions {
    pub release: bool,
    pub linker: Option<OsString>,
}
pub fn atomic_write(
    path: &Path,
    bytes: &[u8],
    permissions: Option<std::fs::Permissions>,
) -> std::io::Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(bytes)?;
    if let Some(p) = permissions.or_else(|| std::fs::metadata(path).ok().map(|m| m.permissions())) {
        file.as_file().set_permissions(p)?;
    }
    file.as_file().sync_all()?;
    file.persist(path).map_err(|e| e.error)?;
    Ok(())
}
pub fn build(program: &CheckedProgram, path: &Path, options: &BuildOptions) -> Result<()> {
    let object = compiler::emit(program, options.release)?;
    link(&object.bytes, path, options)
}
pub fn link(object: &[u8], path: &Path, options: &BuildOptions) -> Result<()> {
    let fail = |message: String| Diagnostic::error("E0500", message, Span::default());
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let dir = tempfile::Builder::new()
        .prefix(".pillar-build-")
        .tempdir_in(parent)
        .map_err(|e| fail(format!("cannot create temporary directory: {e}")))?;
    let object_path = dir.path().join("program.o");
    let archive = dir.path().join(if cfg!(target_env = "msvc") {
        "pillar_runtime.lib"
    } else {
        "libpillar_runtime.a"
    });
    let output = dir.path().join(if cfg!(windows) {
        "program.exe"
    } else {
        "program"
    });
    std::fs::write(&object_path, object).map_err(|e| fail(e.to_string()))?;
    std::fs::write(&archive, RUNTIME).map_err(|e| fail(e.to_string()))?;
    let override_linker = options
        .linker
        .clone()
        .or_else(|| std::env::var_os("PILLAR_LINKER"))
        .or_else(|| std::env::var_os("CC"));
    let mut cmd = if let Some(linker) = override_linker {
        Command::new(linker)
    } else {
        default_linker()?
    };
    if cfg!(target_env = "msvc") {
        cmd.arg("/nologo")
            .arg(&object_path)
            .arg(&archive)
            .arg(format!("/Fe:{}", output.display()))
            .arg("/link")
            .arg("/SUBSYSTEM:CONSOLE")
            .arg("shell32.lib");
    } else {
        cmd.arg(&object_path)
            .arg(&archive)
            .arg("-lm")
            .arg("-o")
            .arg(&output);
    }
    let result = cmd.output().map_err(|e| {
        fail(format!(
            "cannot invoke linker {:?}: {e}; install the native C toolchain or use --linker",
            cmd.get_program()
        ))
    })?;
    if !result.status.success() {
        return Err(fail(format!(
            "linker failed ({}):\n{}{}",
            result.status,
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        )));
    }
    // Publish the linker's closed inode. Reopening it for writing can race with
    // parallel fork/exec and cause ETXTBSY on Linux.
    std::fs::rename(&output, path)
        .map_err(|e| fail(format!("cannot install executable {}: {e}", path.display())))?;
    Ok(())
}
fn default_linker() -> Result<Command> {
    #[cfg(target_env = "msvc")]
    {
        cc::windows_registry::find_tool(env!("PILLAR_TARGET"),"cl.exe").map(|t|t.to_command()).ok_or_else(||Diagnostic::error("E0500","MSVC C++ Build Tools and Windows SDK are required; use a Developer Command Prompt or --linker",Span::default()))
    }
    #[cfg(not(target_env = "msvc"))]
    {
        Ok(Command::new("cc"))
    }
}
