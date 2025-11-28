use cranelift::{codegen::Context, prelude::*};
use cranelift_module::{FuncId, Linkage, Module};

use crate::parser::{Block, Expression, Statement};

pub struct IRCompiler {
    builder_context: FunctionBuilderContext,
    data_context: Context,
}

impl IRCompiler {
    pub fn new() -> Self {
        Self {
            builder_context: FunctionBuilderContext::new(),
            data_context: Context::new(),
        }
    }

    pub fn compile_function<M: Module>(
        &mut self,
        module: &mut M,
        name: &str,
        arguments: &[Type],
        return_type: Option<Type>,
        code: Block,
    ) -> Result<FuncId, String> {
        let mut sig = module.make_signature();

        for &param in arguments {
            sig.params.push(AbiParam::new(param));
        }

        if let Some(ret) = return_type {
            sig.returns.push(AbiParam::new(ret));
        }

        let func_id = module
            .declare_function(name, Linkage::Export, &sig)
            .map_err(|e| format!("Unable to declare function: {e}"))?;

        let mut ctx = module.make_context();
        ctx.func.signature = sig;

        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut self.builder_context);
        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        for stmt in code.statements {
            Self::compile_stmt(&mut builder, &stmt)?;
        }

        builder.finalize();

        module
            .define_function(func_id, &mut ctx)
            .map_err(|e| format!("Unable to define function: {e}"))?;

        module.clear_context(&mut ctx);

        Ok(func_id)
    }

    pub fn compile_expr(builder: &mut FunctionBuilder, expr: &Expression) -> Value {
        match expr {
            Expression::Int(n) => builder.ins().iconst(types::I64, *n),
            Expression::Add { lho, rho } => {
                let lho_compiled = Self::compile_expr(builder, lho);
                let rho_compiled = Self::compile_expr(builder, rho);
                builder.ins().iadd(lho_compiled, rho_compiled)
            }

            _ => todo!(),
        }
    }

    pub fn compile_stmt(builder: &mut FunctionBuilder, stmt: &Statement) -> Result<(), String> {
        match stmt {
            Statement::Ret { value } => {
                let ret_compiled = Self::compile_expr(builder, value);
                builder.ins().return_(&[ret_compiled]);
                Ok(())
            }

            _ => todo!(),
        }
    }
}

pub fn translate(t: &crate::parser::Type) -> cranelift::prelude::Type {
    match t {
        crate::parser::Type::Int => types::I64,
        crate::parser::Type::Float => types::F64,
        crate::parser::Type::Boolean => types::I8,
        crate::parser::Type::String => todo!("do not know"),

        _ => todo!(),
    }
}
