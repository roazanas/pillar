use std::{fs::File, io::Write};

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

        let mut file =
            File::create(&self.output_path).map_err(|e| format!("Unable to create file: {e}"))?;

        file.write_all(
            &product
                .emit()
                .map_err(|e| format!("Unable to generate object code: {e}"))?,
        )
        .map_err(|e| format!("Unable to write file: {e}"))?;

        Ok(())
    }
}
