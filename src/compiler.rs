use cranelift::prelude::InstBuilder;
use std::collections::HashMap;

use cranelift::{codegen::Context, prelude::*};
use cranelift_module::{FuncId, Linkage, Module};
use owo_colors::OwoColorize;

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

    pub fn compile_program<M: Module>(
        &mut self,
        module: &mut M,
        ast: Statement,
    ) -> Result<FuncId, String> {
        match ast {
            Statement::Fn {
                name,
                arguments,
                code,
            } => {
                let entry_params: Vec<Type> = arguments
                    .iter()
                    .map(|arg| translate(&arg.variables.0))
                    .collect();

                self.compile_function(module, name, &entry_params, Some(types::I64), code)
            }
            _ => Err(format!(
                "{} {}",
                "Compilation error:".red().bold(),
                "Expected a function definition as the program entry point".white()
            )),
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
            .map_err(|e| format!("{} {}", "Unable to declare function:".red().bold(), e))?;

        let mut ctx = module.make_context();
        ctx.func.signature = sig;

        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut self.builder_context);
        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        let mut next_var = 0usize;
        let mut variables: HashMap<String, Variable> = HashMap::new();

        for stmt in code.statements {
            Self::compile_stmt(&mut builder, &mut next_var, &mut variables, &stmt)?;
        }

        builder.finalize();

        module
            .define_function(func_id, &mut ctx)
            .map_err(|e| format!("{} {}", "Unable to define function:".red().bold(), e))?;

        module.clear_context(&mut ctx);

        Ok(func_id)
    }

    fn compile_expr(
        builder: &mut FunctionBuilder,
        variables: &HashMap<String, Variable>,
        expr: &Expression,
    ) -> Value {
        match expr {
            Expression::Int(n) => builder.ins().iconst(types::I64, *n),
            Expression::Float(n) => builder.ins().f64const(*n),
            Expression::Boolean(b) => {
                let v: i64 = if *b { 1 } else { 0 };
                builder.ins().iconst(types::I8, v)
            }
            Expression::String(s) => todo!("strings ({s}) is not supported yet"),
            Expression::Identifier(name) => {
                let var = variables
                    .get(*name)
                    .unwrap_or_else(|| panic!("Unknown variable {name}"));
                builder.use_var(*var)
            }

            Expression::Add { lho, rho } => {
                let lho_compiled = Self::compile_expr(builder, variables, lho);
                let rho_compiled = Self::compile_expr(builder, variables, rho);
                builder.ins().iadd(lho_compiled, rho_compiled)
            }
            Expression::Sub { lho, rho } => {
                let lho_compiled = Self::compile_expr(builder, variables, lho);
                let rho_compiled = Self::compile_expr(builder, variables, rho);
                builder.ins().isub(lho_compiled, rho_compiled)
            }
            Expression::Mul { lho, rho } => {
                let lho_compiled = Self::compile_expr(builder, variables, lho);
                let rho_compiled = Self::compile_expr(builder, variables, rho);
                builder.ins().imul(lho_compiled, rho_compiled)
            }
            Expression::Div { lho, rho } => {
                let lho_compiled = Self::compile_expr(builder, variables, lho);
                let rho_compiled = Self::compile_expr(builder, variables, rho);
                builder.ins().sdiv(lho_compiled, rho_compiled)
            }
            Expression::Mod { lho, rho } => {
                let lho_compiled = Self::compile_expr(builder, variables, lho);
                let rho_compiled = Self::compile_expr(builder, variables, rho);
                builder.ins().srem(lho_compiled, rho_compiled)
            }

            Expression::Neg { expr } => {
                let expr_compiled = Self::compile_expr(builder, variables, expr);
                let ty = builder.func.dfg.value_type(expr_compiled);

                match ty {
                    types::I64 => builder.ins().ineg(expr_compiled),
                    types::F64 => builder.ins().fneg(expr_compiled),
                    _ => panic!("Unary '-' is not supported for type {:?}", ty),
                }
            }

            Expression::Equal { lho, rho } => {
                let lho_compiled = Self::compile_expr(builder, variables, lho);
                let rho_compiled = Self::compile_expr(builder, variables, rho);
                builder.ins().icmp(IntCC::Equal, lho_compiled, rho_compiled)
            }

            Expression::NotEqual { lho, rho } => {
                let lho_compiled = Self::compile_expr(builder, variables, lho);
                let rho_compiled = Self::compile_expr(builder, variables, rho);
                builder
                    .ins()
                    .icmp(IntCC::NotEqual, lho_compiled, rho_compiled)
            }

            Expression::Greater { lho, rho } => {
                let lho_compiled = Self::compile_expr(builder, variables, lho);
                let rho_compiled = Self::compile_expr(builder, variables, rho);
                builder
                    .ins()
                    .icmp(IntCC::SignedGreaterThan, lho_compiled, rho_compiled)
            }

            Expression::GreaterEqual { lho, rho } => {
                let lho_compiled = Self::compile_expr(builder, variables, lho);
                let rho_compiled = Self::compile_expr(builder, variables, rho);
                builder
                    .ins()
                    .icmp(IntCC::SignedGreaterThanOrEqual, lho_compiled, rho_compiled)
            }

            Expression::Less { lho, rho } => {
                let lho_compiled = Self::compile_expr(builder, variables, lho);
                let rho_compiled = Self::compile_expr(builder, variables, rho);
                builder
                    .ins()
                    .icmp(IntCC::SignedLessThan, lho_compiled, rho_compiled)
            }

            Expression::LessEqual { lho, rho } => {
                let lho_compiled = Self::compile_expr(builder, variables, lho);
                let rho_compiled = Self::compile_expr(builder, variables, rho);
                builder
                    .ins()
                    .icmp(IntCC::SignedLessThanOrEqual, lho_compiled, rho_compiled)
            }

            Expression::Not { expr } => {
                let expr_compiled = Self::compile_expr(builder, variables, expr);
                builder.ins().bxor_imm(expr_compiled, 1)
            }

            _ => todo!(),
        }
    }

    fn compile_stmt(
        builder: &mut FunctionBuilder,
        next_var: &mut usize,
        variables: &mut HashMap<String, Variable>,
        stmt: &Statement,
    ) -> Result<bool, String> {
        match stmt {
            Statement::Let { name, value } => {
                let let_compiled = Self::compile_expr(builder, variables, value);

                let var = Self::alloc_var(builder, next_var, variables, name);
                builder.def_var(var, let_compiled);
                Ok(false)
            }
            Statement::Ret { value } => {
                let ret_compiled = Self::compile_expr(builder, variables, value);
                builder.ins().return_(&[ret_compiled]);
                Ok(true)
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let condition_val = Self::compile_expr(builder, variables, condition);

                let then_block = builder.create_block();
                let merge_block = builder.create_block();

                let else_block = if else_branch.is_some() {
                    builder.create_block()
                } else {
                    merge_block
                };

                builder
                    .ins()
                    .brif(condition_val, then_block, &[], else_block, &[]);

                // then
                builder.switch_to_block(then_block);
                builder.seal_block(then_block);

                let mut then_terminated = false;
                for stmt in &then_branch.statements {
                    if Self::compile_stmt(builder, next_var, variables, stmt)? {
                        then_terminated = true;
                        break;
                    }
                }

                if !then_terminated {
                    builder.ins().jump(merge_block, &[]);
                }

                // else
                if let Some(else_branch_content) = else_branch {
                    builder.switch_to_block(else_block);
                    builder.seal_block(else_block);

                    let mut then_terminated = false;
                    for stmt in &else_branch_content.statements {
                        if Self::compile_stmt(builder, next_var, variables, stmt)? {
                            then_terminated = true;
                            break;
                        }
                    }

                    if !then_terminated {
                        builder.ins().jump(merge_block, &[]);
                    }
                }

                // merge
                builder.switch_to_block(merge_block);
                builder.seal_block(merge_block);

                Ok(false)
            }

            _ => todo!(),
        }
    }

    fn alloc_var(
        builder: &mut FunctionBuilder,
        next_var: &mut usize,
        variables: &mut HashMap<String, Variable>,
        name: &str,
    ) -> Variable {
        // NOTE: change I64 to something later
        let var = builder.declare_var(types::I64);
        *next_var += 1;
        variables.insert(name.to_string(), var);
        var
    }
}

pub fn translate(t: &crate::parser::Type) -> cranelift::prelude::Type {
    match t {
        crate::parser::Type::Int => types::I64,
        crate::parser::Type::Float => types::F64,
        crate::parser::Type::Boolean => types::I8,
        crate::parser::Type::String => todo!("do not know"),
    }
}
