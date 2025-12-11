use std::{fs::File, io::Write, path::PathBuf, process::Command};

use cranelift_object::{ObjectBuilder, ObjectModule};

use crate::compiler_settings::CompilerSettings;

pub struct AOTBackend {
    module: ObjectModule,
    output_path: String,
}

impl AOTBackend {
    pub fn new(settings: &CompilerSettings, output_path: &str) -> Result<Self, String> {
        let builder = ObjectBuilder::new(
            settings.isa_owned(),
            output_path,
            cranelift_module::default_libcall_names(),
        )
        .map_err(|e| format!("Unable to create ObjectBuilder: {e}"))?;

        let module = ObjectModule::new(builder);

        Ok(Self {
            module,
            output_path: output_path.to_string(),
        })
    }

    pub fn module_mut(&mut self) -> &mut ObjectModule {
        &mut self.module
    }

    pub fn finalize(self) -> Result<(), String> {
        let product = self.module.finish();

        let mut obj_path = PathBuf::from(&self.output_path);
        obj_path.set_extension("o");

        let mut file =
            File::create(&obj_path).map_err(|e| format!("Unable to create file: {e}"))?;

        file.write_all(
            &product
                .emit()
                .map_err(|e| format!("Unable to generate object code: {e}"))?,
        )
        .map_err(|e| format!("Unable to write file: {e}"))?;

        drop(file);

        check_gcc().unwrap();

        let status = if cfg!(target_os = "windows") {
            Command::new("gcc")
                .arg(&obj_path)
                .arg("-o")
                .arg(&self.output_path)
                .status()
        } else {
            Command::new("cc")
                .arg(&obj_path)
                .arg("-o")
                .arg(&self.output_path)
                .status()
        }
        .map_err(|_| "Failed to invoke linker".to_string())?;
        if !status.success() {
            return Err(format!("Linking failed with status: {status}"));
        }

        std::fs::remove_file(&obj_path).ok();

        Ok(())
    }
}

fn check_gcc() -> Result<(), String> {
    if Command::new("gcc").arg("--version").output().is_err() {
        eprintln!("GCC not found!\n");
        #[cfg(target_os = "windows")]
        eprintln!(
            "Install GCC using one of these methods:\n\n\
             Option 1 (Recommended - Scoop):\n\
             > Set-ExecutionPolicy RemoteSigned -Scope CurrentUser\n\
             > irm get.scoop.sh | iex\n\
             > scoop install mingw\n\n\
             Option 2 (Chocolatey):\n\
             > choco install mingw\n\n\
             Option 3 (winget):\n\
             > winget install MSYS2.MSYS2\n\
             Then run in MSYS2: pacman -S mingw-w64-ucrt-x86_64-gcc"
        );
        return Err("Missing dependency".into());
    }
    Ok(())
}
