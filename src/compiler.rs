use std::collections::HashMap;

use cranelift::codegen::Context;
use cranelift::prelude::*;
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
        program: Vec<Statement>,
    ) -> Result<(), String> {
        for stmt in program {
            match stmt {
                Statement::Fn {
                    name,
                    arguments,
                    code,
                } => {
                    let entry_params: Vec<Type> = arguments
                        .iter()
                        .map(|arg| translate(&arg.variables.0))
                        .collect();

                    self.compile_function(module, name, &entry_params, Some(types::I64), code)?;
                }
                _ => {
                    return Err(format!(
                        "{} {}",
                        "Compilation error:".red().bold(),
                        "Expected a function definition as the program entry point".white()
                    ));
                }
            }
        }
        Ok(())
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

        let mut function_compiler = FunctionCompiler {
            builder: &mut builder,
            variables: HashMap::new(),
            module,
        };

        for (i, arg_type) in arguments.iter().enumerate() {
            let val = function_compiler.builder.block_params(entry_block)[i];
            let var = function_compiler.builder.declare_var(*arg_type);
            function_compiler.builder.def_var(var, val);
        }

        for stmt in code.statements {
            function_compiler.compile_stmt(&stmt)?;
        }

        builder.finalize();

        module
            .define_function(func_id, &mut ctx)
            .map_err(|e| format!("Unable to define function: {e}"))?;

        module.clear_context(&mut ctx);

        Ok(func_id)
    }
}

struct FunctionCompiler<'a, 'b: 'a, M: Module + ?Sized> {
    builder: &'a mut FunctionBuilder<'b>,
    variables: HashMap<String, Variable>,
    #[allow(dead_code)]
    module: &'a mut M,
}

impl<'a, 'b, M: Module + ?Sized> FunctionCompiler<'a, 'b, M> {
    fn compile_expr(&mut self, expr: &Expression) -> Value {
        match expr {
            Expression::Int(n) => self.builder.ins().iconst(types::I64, *n),
            Expression::Float(n) => self.builder.ins().f64const(*n),
            Expression::Boolean(b) => {
                let v: i64 = if *b { 1 } else { 0 };
                self.builder.ins().iconst(types::I8, v)
            }
            Expression::String(s) => todo!("strings ({s}) is not supported yet"),
            Expression::Identifier(name) => {
                let var = self
                    .variables
                    .get(*name)
                    .unwrap_or_else(|| panic!("Unknown variable {name}"));
                self.builder.use_var(*var)
            }
            Expression::Add { lho, rho } => {
                let lhs = self.compile_expr(lho);
                let rhs = self.compile_expr(rho);
                match self.builder.func.dfg.value_type(lhs) {
                    types::F64 => self.builder.ins().fadd(lhs, rhs),
                    _ => self.builder.ins().iadd(lhs, rhs),
                }
            }
            Expression::Sub { lho, rho } => {
                let lhs = self.compile_expr(lho);
                let rhs = self.compile_expr(rho);
                match self.builder.func.dfg.value_type(lhs) {
                    types::F64 => self.builder.ins().fsub(lhs, rhs),
                    _ => self.builder.ins().isub(lhs, rhs),
                }
            }
            Expression::Mul { lho, rho } => {
                let lhs = self.compile_expr(lho);
                let rhs = self.compile_expr(rho);
                match self.builder.func.dfg.value_type(lhs) {
                    types::F64 => self.builder.ins().fmul(lhs, rhs),
                    _ => self.builder.ins().imul(lhs, rhs),
                }
            }
            Expression::Div { lho, rho } => {
                let lhs = self.compile_expr(lho);
                let rhs = self.compile_expr(rho);
                match self.builder.func.dfg.value_type(lhs) {
                    types::F64 => self.builder.ins().fdiv(lhs, rhs),
                    _ => self.builder.ins().sdiv(lhs, rhs),
                }
            }
            Expression::Mod { lho, rho } => {
                let lhs = self.compile_expr(lho);
                let rhs = self.compile_expr(rho);
                self.builder.ins().srem(lhs, rhs)
            }
            Expression::Neg { expr } => {
                let val = self.compile_expr(expr);
                match self.builder.func.dfg.value_type(val) {
                    types::I64 => self.builder.ins().ineg(val),
                    types::F64 => self.builder.ins().fneg(val),
                    _ => panic!("Unary '-' not supported for this type"),
                }
            }
            Expression::Equal { lho, rho } => self.compile_cmp(IntCC::Equal, lho, rho),
            Expression::NotEqual { lho, rho } => self.compile_cmp(IntCC::NotEqual, lho, rho),
            Expression::Greater { lho, rho } => {
                self.compile_cmp(IntCC::SignedGreaterThan, lho, rho)
            }
            Expression::GreaterEqual { lho, rho } => {
                self.compile_cmp(IntCC::SignedGreaterThanOrEqual, lho, rho)
            }
            Expression::Less { lho, rho } => self.compile_cmp(IntCC::SignedLessThan, lho, rho),
            Expression::LessEqual { lho, rho } => {
                self.compile_cmp(IntCC::SignedLessThanOrEqual, lho, rho)
            }
            Expression::Not { expr } => {
                let val = self.compile_expr(expr);
                self.builder.ins().bxor_imm(val, 1)
            }
            Expression::Call { name, arguments } => {
                let mut sig = self.module.make_signature();
                for _ in arguments {
                    sig.params.push(AbiParam::new(types::I64));
                }
                sig.returns.push(AbiParam::new(types::I64));

                let callee = self
                    .module
                    .declare_function(name, Linkage::Import, &sig)
                    .unwrap();

                let local_callee = self.module.declare_func_in_func(callee, self.builder.func);

                let arg_values: Vec<Value> =
                    arguments.iter().map(|arg| self.compile_expr(arg)).collect();

                let call = self.builder.ins().call(local_callee, &arg_values);
                self.builder.inst_results(call)[0]
            }
        }
    }

    fn compile_cmp(&mut self, cc: IntCC, lho: &Expression, rho: &Expression) -> Value {
        let lhs = self.compile_expr(lho);
        let rhs = self.compile_expr(rho);

        let ty = self.builder.func.dfg.value_type(lhs);
        if ty == types::F64 {
            let float_cc = match cc {
                IntCC::Equal => FloatCC::Equal,
                IntCC::NotEqual => FloatCC::NotEqual,
                IntCC::SignedGreaterThan => FloatCC::GreaterThan,
                IntCC::SignedGreaterThanOrEqual => FloatCC::GreaterThanOrEqual,
                IntCC::SignedLessThan => FloatCC::LessThan,
                IntCC::SignedLessThanOrEqual => FloatCC::LessThanOrEqual,
                _ => FloatCC::Equal,
            };
            self.builder.ins().fcmp(float_cc, lhs, rhs)
        } else {
            self.builder.ins().icmp(cc, lhs, rhs)
        }
    }

    fn compile_stmt(&mut self, stmt: &Statement) -> Result<bool, String> {
        match stmt {
            Statement::Let { name, value } => {
                let val = self.compile_expr(value);
                let ty = self.builder.func.dfg.value_type(val);

                let var = self.builder.declare_var(ty);
                self.variables.insert(name.to_string(), var);

                self.builder.def_var(var, val);
                Ok(false)
            }
            Statement::Ret { value } => {
                let val = self.compile_expr(value);
                self.builder.ins().return_(&[val]);
                Ok(true)
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let condition_val = self.compile_expr(condition);

                let then_block = self.builder.create_block();
                let merge_block = self.builder.create_block();
                let else_block = if else_branch.is_some() {
                    self.builder.create_block()
                } else {
                    merge_block
                };

                self.builder
                    .ins()
                    .brif(condition_val, then_block, &[], else_block, &[]);

                // then
                self.builder.switch_to_block(then_block);
                self.builder.seal_block(then_block);

                let mut then_terminated = false;
                for s in &then_branch.statements {
                    if self.compile_stmt(s)? {
                        then_terminated = true;
                        break;
                    }
                }
                if !then_terminated {
                    self.builder.ins().jump(merge_block, &[]);
                }

                // else
                if let Some(else_branch_content) = else_branch {
                    self.builder.switch_to_block(else_block);
                    self.builder.seal_block(else_block);

                    let mut else_terminated = false;
                    for s in &else_branch_content.statements {
                        if self.compile_stmt(s)? {
                            else_terminated = true;
                            break;
                        }
                    }
                    if !else_terminated {
                        self.builder.ins().jump(merge_block, &[]);
                    }
                }

                // merge
                self.builder.switch_to_block(merge_block);
                self.builder.seal_block(merge_block);

                Ok(false)
            }
            Statement::Call { name, arguments } => {
                let mut sig = self.module.make_signature();

                for _ in arguments {
                    sig.params.push(AbiParam::new(types::I64));
                }
                sig.returns.push(AbiParam::new(types::I64));

                let callee = self
                    .module
                    .declare_function(name, Linkage::Import, &sig)
                    .map_err(|e| format!("Unable to declare function {}: {}", name, e))?;

                let local_callee = self.module.declare_func_in_func(callee, self.builder.func);

                let arg_values: Vec<Value> =
                    arguments.iter().map(|arg| self.compile_expr(arg)).collect();

                self.builder.ins().call(local_callee, &arg_values);

                Ok(false)
            }
            Statement::Fn { .. } => Err("Nested functions are not supported".to_string()),
        }
    }
}

pub fn translate(t: &crate::parser::Type) -> cranelift::prelude::Type {
    match t {
        crate::parser::Type::Int => types::I64,
        crate::parser::Type::Float => types::F64,
        crate::parser::Type::Boolean => types::I8,
        crate::parser::Type::String => todo!("String types not implemented yet"),
    }
}
