use crate::{
    diagnostics::{Diagnostic, Result},
    parser::{Type as Ty, *},
    sema::CheckedProgram,
    source::Span,
};
use cranelift::codegen::ir::MemFlagsData;
use cranelift::prelude::*;
use cranelift_module::{DataDescription, FuncId, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};
use std::collections::BTreeMap;

pub struct ObjectCode {
    pub bytes: Vec<u8>,
    pub ir: String,
}
pub fn emit(program: &CheckedProgram, release: bool) -> Result<ObjectCode> {
    if program.entry.is_none() {
        return Err(Diagnostic::error(
            "E0204",
            "executable requires FN main(): int",
            Span::default(),
        ));
    }
    emit_inner(program, release).map_err(|e| {
        Diagnostic::error(
            "E9000",
            format!("native code generation failed: {e}"),
            Span::default(),
        )
    })
}
fn cl(t: &Ty) -> cranelift::prelude::Type {
    match t {
        Ty::Float => types::F64,
        Ty::Bool => types::I8,
        _ => types::I64,
    }
}
fn emit_inner(p: &CheckedProgram, release: bool) -> std::result::Result<ObjectCode, String> {
    let entry_id = p.entry.expect("entry checked");
    let mut flags = settings::builder();
    flags
        .set("opt_level", if release { "speed" } else { "none" })
        .map_err(|e| e.to_string())?;
    flags.set("is_pic", "true").map_err(|e| e.to_string())?;
    let isa = cranelift_native::builder()
        .map_err(str::to_owned)?
        .finish(settings::Flags::new(flags))
        .map_err(|e| e.to_string())?;
    let mut module = ObjectModule::new(
        ObjectBuilder::new(isa, "pillar", cranelift_module::default_libcall_names())
            .map_err(|e| e.to_string())?,
    );
    let mut ids = Vec::new();
    for f in &p.functions {
        let mut sig = module.make_signature();
        for a in &f.params {
            sig.params.push(AbiParam::new(cl(&a.ty)));
        }
        if f.result != Ty::Unit {
            sig.returns.push(AbiParam::new(cl(&f.result)));
        }
        ids.push(
            module
                .declare_function(&format!("_plr_fn_{}", f.id), Linkage::Local, &sig)
                .map_err(|e| e.to_string())?,
        );
    }
    let mut ir = String::new();
    let mut data_id = 0usize;
    for f in &p.functions {
        let mut ctx = module.make_context();
        for a in &f.params {
            ctx.func.signature.params.push(AbiParam::new(cl(&a.ty)));
        }
        if f.result != Ty::Unit {
            ctx.func
                .signature
                .returns
                .push(AbiParam::new(cl(&f.result)));
        }
        let mut fbctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut fbctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        {
            let mut c = FunctionCode {
                b: &mut builder,
                module: &mut module,
                p,
                ids: &ids,
                data_id: &mut data_id,
                locals: BTreeMap::new(),
                scopes: vec![vec![]],
                loops: vec![],
            };
            c.loc(f.span)?;
            c.runtime("plr_enter", &[], None)?;
            for (i, param) in f.params.iter().enumerate() {
                let v = c.b.block_params(entry)[i];
                c.bind(param.id, param.ty.clone(), v);
            }
            if !c.block(&f.body, false)? {
                c.cleanup(0)?;
                c.runtime("plr_leave", &[], None)?;
                c.b.ins().return_(&[]);
            }
        }
        builder.finalize(module.target_config());
        ir.push_str(&format!("; {}\n{}\n", f.name, ctx.func.display()));
        module
            .define_function(ids[f.id], &mut ctx)
            .map_err(|e| format!("{}: {e:?}\n{}", f.name, ctx.func.display()))?;
    }
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(types::I32));
    sig.params.push(AbiParam::new(types::I64));
    sig.returns.push(AbiParam::new(types::I32));
    let main = module
        .declare_function("main", Linkage::Export, &sig)
        .map_err(|e| e.to_string())?;
    let mut ctx = module.make_context();
    ctx.func.signature = sig;
    let mut fctx = FunctionBuilderContext::new();
    let mut builder = FunctionBuilder::new(&mut ctx.func, &mut fctx);
    let entry = builder.create_block();
    builder.append_block_params_for_function_params(entry);
    builder.switch_to_block(entry);
    builder.seal_block(entry);
    {
        let mut c = FunctionCode {
            b: &mut builder,
            module: &mut module,
            p,
            ids: &ids,
            data_id: &mut data_id,
            locals: BTreeMap::new(),
            scopes: vec![],
            loops: vec![],
        };
        let argc = c.b.block_params(entry)[0];
        let argv = c.b.block_params(entry)[1];
        let argc = c.b.ins().uextend(types::I64, argc);
        c.runtime("plr_init", &[argc, argv], None)?;
        c.loc(p.functions[entry_id].span)?;
        let reference = c.module.declare_func_in_func(ids[entry_id], c.b.func);
        let call = c.b.ins().call(reference, &[]);
        let result = c.b.inst_results(call)[0];
        c.loc(p.functions[entry_id].span)?;
        let status = c
            .runtime("plr_finish", &[result], Some(types::I32))?
            .expect("finish result");
        c.b.ins().return_(&[status]);
    }
    builder.finalize(module.target_config());
    module
        .define_function(main, &mut ctx)
        .map_err(|e| e.to_string())?;
    ir.push_str(&format!("; ABI entry\n{}", ctx.func.display()));
    let bytes = module.finish().emit().map_err(|e| e.to_string())?;
    Ok(ObjectCode { bytes, ir })
}
struct FunctionCode<'a, 'b> {
    b: &'a mut FunctionBuilder<'b>,
    module: &'a mut ObjectModule,
    p: &'a CheckedProgram,
    ids: &'a [FuncId],
    data_id: &'a mut usize,
    locals: BTreeMap<usize, (Variable, Ty)>,
    scopes: Vec<Vec<usize>>,
    loops: Vec<(Block, Block, usize)>,
}
type CGResult<T> = std::result::Result<T, String>;
impl FunctionCode<'_, '_> {
    fn constant(&mut self, n: i64) -> Value {
        self.b.ins().iconst(types::I64, n)
    }
    fn runtime(
        &mut self,
        name: &str,
        args: &[Value],
        ret: Option<cranelift::prelude::Type>,
    ) -> CGResult<Option<Value>> {
        let mut sig = self.module.make_signature();
        for _ in args {
            sig.params.push(AbiParam::new(types::I64));
        }
        if let Some(t) = ret {
            sig.returns.push(AbiParam::new(t));
        }
        let id = self
            .module
            .declare_function(name, Linkage::Import, &sig)
            .map_err(|e| e.to_string())?;
        let rf = self.module.declare_func_in_func(id, self.b.func);
        let inst = self.b.ins().call(rf, args);
        Ok(ret.map(|_| self.b.inst_results(inst)[0]))
    }
    fn rt(&mut self, name: &str, args: &[Value]) -> CGResult<Value> {
        Ok(self
            .runtime(name, args, Some(types::I64))?
            .expect("runtime return"))
    }
    fn data(&mut self, bytes: Vec<u8>) -> CGResult<Value> {
        let name = format!("_plr_data_{}", self.data_id);
        *self.data_id += 1;
        let id = self
            .module
            .declare_data(&name, Linkage::Local, false, false)
            .map_err(|e| e.to_string())?;
        let mut data = DataDescription::new();
        data.define(bytes.into_boxed_slice());
        self.module
            .define_data(id, &data)
            .map_err(|e| e.to_string())?;
        let gv = self.module.declare_data_in_func(id, self.b.func);
        Ok(self.b.ins().symbol_value(types::I64, gv))
    }
    fn loc(&mut self, span: Span) -> CGResult<()> {
        let s = &self.p.sources[span.file].0;
        let pos = s.location(span);
        let mut path = s.path.to_string_lossy().as_bytes().to_vec();
        path.push(0);
        let path = self.data(path)?;
        let line = self.constant(pos.line as i64);
        let col = self.constant(pos.column as i64);
        self.runtime("plr_location", &[path, line, col], None)?;
        Ok(())
    }
    fn bits(&mut self, v: Value, t: &Ty) -> Value {
        match t {
            Ty::Float => self.b.ins().bitcast(types::I64, MemFlagsData::new(), v),
            Ty::Bool => self.b.ins().uextend(types::I64, v),
            _ => v,
        }
    }
    fn typed(&mut self, v: Value, t: &Ty) -> Value {
        match t {
            Ty::Float => self.b.ins().bitcast(types::F64, MemFlagsData::new(), v),
            Ty::Bool => self.b.ins().ireduce(types::I8, v),
            _ => v,
        }
    }
    fn drop_value(&mut self, v: Value, t: &Ty) -> CGResult<()> {
        if t.reference() {
            self.runtime("plr_release", &[v], None)?;
        }
        Ok(())
    }
    fn bind(&mut self, id: usize, ty: Ty, v: Value) {
        let var = self.b.declare_var(cl(&ty));
        self.b.def_var(var, v);
        self.locals.insert(id, (var, ty));
        self.scopes.last_mut().expect("scope").push(id);
    }
    fn cleanup(&mut self, from: usize) -> CGResult<()> {
        let ids = self.scopes[from..]
            .iter()
            .rev()
            .flat_map(|s| s.iter().rev())
            .copied()
            .collect::<Vec<_>>();
        for id in ids {
            let (var, ty) = self.locals[&id].clone();
            if ty.reference() {
                let val = self.b.use_var(var);
                self.drop_value(val, &ty)?;
            }
        }
        Ok(())
    }
    fn block(&mut self, stmts: &[Stmt], scope: bool) -> CGResult<bool> {
        if scope {
            self.scopes.push(vec![]);
        }
        let mut end = false;
        for stmt in stmts {
            if end {
                break;
            }
            end = self.stmt(stmt)?;
        }
        if scope {
            if !end {
                self.cleanup(self.scopes.len() - 1)?;
            }
            self.scopes.pop();
        }
        Ok(end)
    }
    fn stmt(&mut self, s: &Stmt) -> CGResult<bool> {
        match &s.kind {
            StmtKind::Let { value, id, .. } => {
                let v = self.expr(value)?;
                self.bind(*id, value.ty.clone(), v);
            }
            StmtKind::Assign { place, value } => self.assign(place, value)?,
            StmtKind::Expr(e) => {
                let v = self.expr(e)?;
                self.drop_value(v, &e.ty)?;
            }
            StmtKind::Ret(e) => {
                let mut v = e.as_ref().map(|e| self.expr(e)).transpose()?;
                if e.as_ref().is_some_and(|e| e.ty == Ty::Unit) {
                    v = None;
                }
                self.cleanup(0)?;
                self.runtime("plr_leave", &[], None)?;
                self.b.ins().return_(&v.into_iter().collect::<Vec<_>>());
                return Ok(true);
            }
            StmtKind::If { condition, yes, no } => {
                let cond = self.expr(condition)?;
                let a = self.b.create_block();
                let b = self.b.create_block();
                let merge = self.b.create_block();
                self.b.ins().brif(cond, a, &[], b, &[]);
                self.b.switch_to_block(a);
                self.b.seal_block(a);
                let a_end = self.block(yes, true)?;
                if !a_end {
                    self.b.ins().jump(merge, &[]);
                }
                self.b.switch_to_block(b);
                self.b.seal_block(b);
                let b_end = if let Some(no) = no {
                    self.block(no, true)?
                } else {
                    false
                };
                if !b_end {
                    self.b.ins().jump(merge, &[]);
                }
                self.b.seal_block(merge);
                if a_end && b_end {
                    return Ok(true);
                }
                self.b.switch_to_block(merge);
            }
            StmtKind::While { condition, body } => {
                let head = self.b.create_block();
                let body_block = self.b.create_block();
                let exit = self.b.create_block();
                self.b.ins().jump(head, &[]);
                self.b.switch_to_block(head);
                if s.terminates {
                    self.b.ins().jump(body_block, &[]);
                } else {
                    let cond = self.expr(condition)?;
                    self.b.ins().brif(cond, body_block, &[], exit, &[]);
                }
                self.b.switch_to_block(body_block);
                self.b.seal_block(body_block);
                self.loops.push((head, exit, self.scopes.len()));
                let ended = self.block(body, true)?;
                self.loops.pop();
                if !ended {
                    self.b.ins().jump(head, &[]);
                }
                self.b.seal_block(head);
                self.b.seal_block(exit);
                if s.terminates {
                    return Ok(true);
                }
                self.b.switch_to_block(exit);
            }
            StmtKind::Break | StmtKind::Continue => {
                let (head, exit, depth) = *self.loops.last().expect("checked loop");
                self.cleanup(depth)?;
                self.b.ins().jump(
                    if matches!(s.kind, StmtKind::Break) {
                        exit
                    } else {
                        head
                    },
                    &[],
                );
                return Ok(true);
            }
        }
        Ok(false)
    }
    fn assign(&mut self, place: &Expr, rhs: &Expr) -> CGResult<()> {
        enum Step<'a> {
            Index(&'a Expr, Ty),
            Field(usize, Ty),
        }
        fn flatten<'a>(p: &'a Expr, steps: &mut Vec<Step<'a>>) -> usize {
            match &p.kind {
                ExprKind::Var { id, .. } => *id,
                ExprKind::Index(b, i) => {
                    let id = flatten(b, steps);
                    steps.push(Step::Index(i, p.ty.clone()));
                    id
                }
                ExprKind::Field { base, index, .. } => {
                    let id = flatten(base, steps);
                    steps.push(Step::Field(*index, p.ty.clone()));
                    id
                }
                _ => unreachable!("checked place"),
            }
        }
        let mut steps = Vec::new();
        let id = flatten(place, &mut steps);
        let (var, root_ty) = self.locals[&id].clone();
        let mut parents = Vec::new();
        if !steps.is_empty() {
            let value = self.b.use_var(var);
            let mut current = self.rt("plr_retain", &[value])?;
            for (i, step) in steps.iter().enumerate() {
                let (index, ty) = match step {
                    Step::Index(index, ty) => (self.expr(index)?, ty),
                    Step::Field(index, ty) => (self.constant(*index as i64), ty),
                };
                parents.push((current, index, ty.clone()));
                if i + 1 < steps.len() {
                    let retained = self.rt("plr_retain", &[current])?;
                    self.loc(place.span)?;
                    current = self.rt("plr_get", &[retained, index])?;
                }
            }
        }
        let mut value = self.expr(rhs)?;
        if !parents.is_empty() {
            value = self.bits(value, &rhs.ty);
        }
        for (parent, index, ty) in parents.into_iter().rev() {
            let tag = self.constant(ty.tag());
            self.loc(place.span)?;
            value = self.rt("plr_set", &[parent, index, value, tag])?;
        }
        let old = self.b.use_var(var);
        self.drop_value(old, &root_ty)?;
        self.b.def_var(var, value);
        Ok(())
    }
    fn expr(&mut self, e: &Expr) -> CGResult<Value> {
        match &e.kind {
            ExprKind::Int(n) => Ok(self.constant(*n)),
            ExprKind::Float(f) => Ok(self.b.ins().f64const(*f)),
            ExprKind::Bool(b) => Ok(self.b.ins().iconst(types::I8, i64::from(*b))),
            ExprKind::Str(s) => {
                let ptr = self.data(s.as_bytes().to_vec())?;
                let len = self.constant(s.len() as i64);
                self.loc(e.span)?;
                self.rt("plr_string", &[ptr, len])
            }
            ExprKind::Var { id, .. } => {
                let (var, t) = self.locals[id].clone();
                let v = self.b.use_var(var);
                if t.reference() {
                    self.rt("plr_retain", &[v])
                } else {
                    Ok(v)
                }
            }
            ExprKind::Group(e) => self.expr(e),
            ExprKind::Unary(op, inner) => {
                let v = self.expr(inner)?;
                match op {
                    Op::Not => Ok(self.b.ins().bxor_imm_u(v, 1)),
                    Op::Neg if e.ty == Ty::Float => Ok(self.b.ins().fneg(v)),
                    _ => {
                        self.loc(e.span)?;
                        self.rt("plr_neg", &[v])
                    }
                }
            }
            ExprKind::Binary(Op::And | Op::Or, l, r) => {
                let v = self.expr(l)?;
                let rhs = self.b.create_block();
                let short = self.b.create_block();
                let merge = self.b.create_block();
                self.b.append_block_param(merge, types::I8);
                if matches!(&e.kind, ExprKind::Binary(Op::And, _, _)) {
                    self.b.ins().brif(v, rhs, &[], short, &[]);
                } else {
                    self.b.ins().brif(v, short, &[], rhs, &[]);
                }
                self.b.switch_to_block(short);
                self.b.seal_block(short);
                self.b.ins().jump(merge, &[v.into()]);
                self.b.switch_to_block(rhs);
                self.b.seal_block(rhs);
                let value = self.expr(r)?;
                self.b.ins().jump(merge, &[value.into()]);
                self.b.switch_to_block(merge);
                self.b.seal_block(merge);
                Ok(self.b.block_params(merge)[0])
            }
            ExprKind::Binary(op, l, r) => {
                let a = self.expr(l)?;
                let b = self.expr(r)?;
                self.loc(e.span)?;
                if l.ty == Ty::Str {
                    return match op {
                        Op::Add => self.rt("plr_concat", &[a, b]),
                        Op::Eq | Op::Ne => {
                            let same = self.rt("plr_string_equal", &[a, b])?;
                            Ok(self.b.ins().icmp_imm_s(
                                if *op == Op::Eq {
                                    IntCC::NotEqual
                                } else {
                                    IntCC::Equal
                                },
                                same,
                                0,
                            ))
                        }
                        _ => unreachable!(),
                    };
                }
                if op.comparison() {
                    if l.ty == Ty::Float {
                        let cc = match op {
                            Op::Eq => FloatCC::Equal,
                            Op::Ne => FloatCC::NotEqual,
                            Op::Lt => FloatCC::LessThan,
                            Op::Le => FloatCC::LessThanOrEqual,
                            Op::Gt => FloatCC::GreaterThan,
                            Op::Ge => FloatCC::GreaterThanOrEqual,
                            _ => unreachable!(),
                        };
                        Ok(self.b.ins().fcmp(cc, a, b))
                    } else {
                        let cc = match op {
                            Op::Eq => IntCC::Equal,
                            Op::Ne => IntCC::NotEqual,
                            Op::Lt => IntCC::SignedLessThan,
                            Op::Le => IntCC::SignedLessThanOrEqual,
                            Op::Gt => IntCC::SignedGreaterThan,
                            Op::Ge => IntCC::SignedGreaterThanOrEqual,
                            _ => unreachable!(),
                        };
                        Ok(self.b.ins().icmp(cc, a, b))
                    }
                } else if l.ty == Ty::Float {
                    Ok(match op {
                        Op::Add => self.b.ins().fadd(a, b),
                        Op::Sub => self.b.ins().fsub(a, b),
                        Op::Mul => self.b.ins().fmul(a, b),
                        Op::Div => {
                            let a = self.bits(a, &Ty::Float);
                            let b = self.bits(b, &Ty::Float);
                            let value = self.rt("plr_div_float", &[a, b])?;
                            self.typed(value, &Ty::Float)
                        }
                        _ => unreachable!(),
                    })
                } else {
                    let name = match op {
                        Op::Add => "plr_add",
                        Op::Sub => "plr_sub",
                        Op::Mul => "plr_mul",
                        Op::Div => "plr_div",
                        Op::Mod => "plr_mod",
                        _ => unreachable!(),
                    };
                    self.rt(name, &[a, b])
                }
            }
            ExprKind::Call { args, target, .. } => {
                let mut values = Vec::new();
                for arg in args {
                    values.push(self.expr(arg)?);
                }
                self.loc(e.span)?;
                match target.as_ref().expect("checked call") {
                    Target::Function(id) => {
                        let rf = self.module.declare_func_in_func(self.ids[*id], self.b.func);
                        let call = self.b.ins().call(rf, &values);
                        if e.ty == Ty::Unit {
                            Ok(self.constant(0))
                        } else {
                            Ok(self.b.inst_results(call)[0])
                        }
                    }
                    Target::Builtin(name) => self.builtin(name, args, &values, &e.ty),
                }
            }
            ExprKind::Array(values) => {
                let len = self.constant(values.len() as i64);
                let kind = self.constant(4);
                self.loc(e.span)?;
                let mut array = self.rt("plr_collection", &[len, kind])?;
                for (i, v) in values.iter().enumerate() {
                    let value = self.expr(v)?;
                    let value = self.bits(value, &v.ty);
                    let index = self.constant(i as i64);
                    let tag = self.constant(v.ty.tag());
                    array = self.rt("plr_set", &[array, index, value, tag])?;
                }
                Ok(array)
            }
            ExprKind::Index(base, index) => {
                let base = self.expr(base)?;
                let index = self.expr(index)?;
                self.loc(e.span)?;
                let bits = self.rt("plr_get", &[base, index])?;
                Ok(self.typed(bits, &e.ty))
            }
            ExprKind::Field { base, index, .. } => {
                let base = self.expr(base)?;
                let index = self.constant(*index as i64);
                self.loc(e.span)?;
                let bits = self.rt("plr_get", &[base, index])?;
                Ok(self.typed(bits, &e.ty))
            }
            ExprKind::Struct { fields, id, .. } => {
                let def = self.p.structs[*id].clone();
                let len = self.constant(def.fields.len() as i64);
                let kind = self.constant(5);
                self.loc(e.span)?;
                let mut value = self.rt("plr_collection", &[len, kind])?;
                for (name, expr) in fields {
                    let index = def
                        .fields
                        .iter()
                        .position(|f| &f.name == name)
                        .expect("checked field");
                    let field = self.expr(expr)?;
                    let bits = self.bits(field, &expr.ty);
                    let index = self.constant(index as i64);
                    let tag = self.constant(expr.ty.tag());
                    value = self.rt("plr_set", &[value, index, bits, tag])?;
                }
                Ok(value)
            }
        }
    }
    fn builtin(
        &mut self,
        name: &str,
        args: &[Expr],
        values: &[Value],
        result: &Ty,
    ) -> CGResult<Value> {
        let zero = self.constant(0);
        match name {
            "print" | "println" | "output_int" | "output_float" | "output_str" => {
                let bits = self.bits(values[0], &args[0].ty);
                let tag = self.constant(args[0].ty.tag());
                let nl = self.constant(i64::from(name != "print"));
                self.runtime("plr_print", &[bits, tag, nl], None)?;
                Ok(zero)
            }
            "read_int" | "read_float" | "read_line" | "args" => {
                let bits = self.rt(&format!("plr_{name}"), &[])?;
                Ok(self.typed(bits, result))
            }
            "len" | "byte_len" => {
                let bytes = self.constant(i64::from(name == "byte_len"));
                self.rt("plr_len", &[values[0], bytes])
            }
            "int" | "float" | "str" => {
                let bits = self.bits(values[0], &args[0].ty);
                let tag = self.constant(args[0].ty.tag());
                let bits = self.rt(&format!("plr_{name}"), &[bits, tag])?;
                Ok(self.typed(bits, result))
            }
            "push" => {
                let bits = self.bits(values[1], &args[1].ty);
                let tag = self.constant(args[1].ty.tag());
                self.rt("plr_push", &[values[0], bits, tag])
            }
            "assert" => {
                let cond = self.bits(values[0], &Ty::Bool);
                self.runtime("plr_assert", &[cond], None)?;
                Ok(zero)
            }
            "sqrt" | "pow" | "abs" => {
                let bits = values
                    .iter()
                    .zip(args)
                    .map(|(v, e)| self.bits(*v, &e.ty))
                    .collect::<Vec<_>>();
                let name = if name == "abs" {
                    if args[0].ty == Ty::Int {
                        "plr_abs_int"
                    } else {
                        "plr_abs_float"
                    }
                } else if name == "sqrt" {
                    "plr_sqrt"
                } else {
                    "plr_pow"
                };
                let value = self.rt(name, &bits)?;
                Ok(self.typed(value, result))
            }
            "read_file" => self.rt("plr_read_file", values),
            "write_file" => {
                self.runtime("plr_write_file", values, None)?;
                Ok(zero)
            }
            _ => unreachable!("checked builtin"),
        }
    }
}
