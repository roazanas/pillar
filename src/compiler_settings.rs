use cranelift::prelude::*;
use target_lexicon::Triple;

pub struct CompilerSettings {
    target_triple: Triple,
    flags: settings::Flags,
    isa: isa::OwnedTargetIsa,
}

impl CompilerSettings {
    pub fn new() -> Result<Self, String> {
        let target_triple = cranelift_native::builder()
            .map_err(|e| format!("Unable to detect target host platform: {e}"))?;
        let target_triple = target_triple.triple().clone();

        let mut flag_builder = settings::builder();

        // options: speed, speed_and_size, none
        flag_builder.set("opt_level", "speed").unwrap();

        // flag_builder.set("use_colocated_libcalls", "false").unwrap();

        // JIT: false, AOT: prefer "true"
        flag_builder.set("is_pic", "true").unwrap();

        let flags = settings::Flags::new(flag_builder);

        let isa_builder = cranelift::codegen::isa::lookup(target_triple.clone())
            .map_err(|e| format!("Unsupported platform: {e}"))?;

        let isa = isa_builder
            .finish(flags.clone())
            .map_err(|e| format!("Unable to create ISA: {e}"))?;

        Ok(Self {
            target_triple,
            flags,
            isa,
        })
    }

    pub fn isa(&self) -> &dyn isa::TargetIsa {
        &*self.isa
    }

    pub fn isa_owned(&self) -> isa::OwnedTargetIsa {
        self.isa.clone()
    }

    pub fn target_triple(&self) -> &Triple {
        &self.target_triple
    }
}
