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
        obj_path.add_extension(".o");

        let mut file =
            File::create(&obj_path).map_err(|e| format!("Unable to create file: {e}"))?;

        file.write_all(
            &product
                .emit()
                .map_err(|e| format!("Unable to generate object code: {e}"))?,
        )
        .map_err(|e| format!("Unable to write file: {e}"))?;

        drop(file);

        let status = if cfg!(target_os = "windows") {
            Command::new("cl.exe")
                .arg(&obj_path)
                .arg(format!("/Fe:{}", self.output_path))
                .status()
        } else {
            Command::new("cc")
                .arg(&obj_path)
                .arg("-o")
                .arg(&self.output_path)
                .status()
        }
        .map_err(|e| format!("Failed to invoke linker: {e}"))?;

        if !status.success() {
            return Err(format!("Linking failed with status: {status}"));
        }

        std::fs::remove_file(&obj_path).ok();

        Ok(())
    }
}
