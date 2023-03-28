use std::collections::{HashMap, HashSet};

use bril_rs::{Code, ConstOps, Instruction, Literal, Type, ValueOps};

use crate::parse::BasicBlock;

#[derive(Eq, PartialEq, Hash, Debug)]
pub enum LVNValue {
    Constant(Literal),
    ValueBinaryOp(ValueOps, usize, usize),
    ValueUnaryOp(ValueOps, usize),
}

pub struct LVN {
    next: usize,
    folding: bool,
    var2num: HashMap<String, usize>, // the value of variables, multiple variables can have same value
    val2num: HashMap<LVNValue, usize>,
    num2var: HashMap<usize, String>,
    num2const: HashMap<usize, Literal>,
}

impl LVN {
    pub fn new(folding: bool) -> LVN {
        // LVN table: Number | Value | Variable
        LVN {
            next: 0,
            folding,
            val2num: HashMap::new(),
            num2var: HashMap::new(),
            var2num: HashMap::new(),
            num2const: HashMap::new(),
        }
    }

    fn get_const_if_fold(&self, num: &usize) -> Option<&Literal> {
        if !self.folding {
            return None;
        }
        return self.num2const.get(num);
    }

    // check for variables that are overwritten
    pub fn last_writes(block: &BasicBlock) -> Vec<bool> {
        let ret = block
            .iter()
            .rev()
            .scan(
                HashSet::new(),
                |written_to: &mut HashSet<&String>, instr| {
                    let mut ret = false;
                    if let Code::Instruction(Instruction::Value { dest, .. })
                    | Code::Instruction(Instruction::Constant { dest, .. }) = instr
                    {
                        if !written_to.contains(&dest) {
                            ret = true;
                        }
                        written_to.insert(dest);
                    }
                    return Some(ret);
                },
            )
            .collect::<Vec<bool>>()
            .into_iter()
            .rev()
            .collect();
        return ret;
    }

    pub fn read_first(&mut self, block: &BasicBlock) -> HashSet<String> {
        let mut read: HashSet<String> = HashSet::new();
        let mut written: HashSet<String> = HashSet::new();
        for instr in block {
            if let Code::Instruction(Instruction::Value { args, dest, .. }) = instr {
                read.extend(
                    args.clone()
                        .into_iter()
                        .filter(|arg| !written.contains(arg)),
                );
                written.insert(dest.clone());
            }
            if let Code::Instruction(Instruction::Constant { dest, .. }) = instr {
                written.insert(dest.clone());
            }
        }
        return read;
    }

    pub fn register_var(&mut self, var: &String) -> usize {
        let num = self.next;
        self.next += 1;
        self.var2num.insert(var.clone(), num);
        return num;
    }

    pub fn register_dest(&mut self, dest: &String, num: usize, last_write: bool) -> String {
        let var: String;
        if last_write {
            var = dest.clone()
        } else {
            var = format!("lvn.{}", num);
        }
        self.num2var.insert(num, var.clone());
        return var;
    }

    fn register_val(&mut self, dest: &String, val: LVNValue, last_write: bool) -> (String, usize) {
        let val_num = self.register_var(dest);
        self.fold_value(val_num, &val);
        self.val2num.insert(val, val_num);
        (self.register_dest(&dest, val_num, last_write), val_num)
    }

    fn replace_args(&self, args: &Vec<String>) -> Vec<String> {
        return args
            .iter()
            .map(|arg| {
                let num = self.var2num.get(arg).unwrap();
                let var = self.num2var.get(num).unwrap();
                var.clone()
            })
            .collect();
    }

    fn fold_value(&mut self, val_num: usize, canonical_val: &LVNValue) {
        match canonical_val {
            LVNValue::Constant(value) => {
                self.num2const.insert(val_num, value.clone());
            }
            LVNValue::ValueBinaryOp(op, arg_num0, arg_num1) => {
                if let (Some(arg_val0), Some(arg_val1)) =
                    (self.num2const.get(&arg_num0), self.num2const.get(&arg_num1))
                {
                    if let Some(val) = Self::calculate_binary_op(op, arg_val0, arg_val1) {
                        self.num2const.insert(val_num, val);
                    }
                } else if arg_num0 == arg_num1 {
                    if let ValueOps::Eq | ValueOps::Le | ValueOps::Ge = op {
                        self.num2const.insert(val_num, Literal::Bool(true));
                    }
                } else if let (ValueOps::Or, Some(&Literal::Bool(true)), _)
                | (ValueOps::Or, _, Some(&Literal::Bool(true))) = (
                    op,
                    self.num2const.get(&arg_num0),
                    self.num2const.get(&arg_num1),
                ) {
                    self.num2const.insert(val_num, Literal::Bool(true));
                } else if let (ValueOps::And, Some(&Literal::Bool(false)), _)
                | (ValueOps::And, _, Some(&Literal::Bool(false))) = (
                    op,
                    self.num2const.get(&arg_num0),
                    self.num2const.get(&arg_num1),
                ) {
                    self.num2const.insert(val_num, Literal::Bool(false));
                }
            }
            LVNValue::ValueUnaryOp(op, arg_num) => {
                if let Some(arg_val) = self.num2const.get(arg_num) {
                    if let Some(val) = Self::calculate_unary_op(op, arg_val) {
                        self.num2const.insert(val_num, val);
                    }
                }
            }
        }
    }

    fn canonicalize_instruction(&self, instr: &Code) -> Option<(LVNValue, String, Type)> {
        match instr {
            Code::Instruction(Instruction::Constant {
                value,
                dest,
                const_type,
                ..
            }) => Some((
                LVNValue::Constant(value.clone()),
                dest.clone(),
                const_type.clone(),
            )),
            Code::Instruction(Instruction::Value {
                args,
                op,
                dest,
                op_type,
                ..
            }) if op == &ValueOps::Not || op == &ValueOps::Id => Some((
                LVNValue::ValueUnaryOp(op.clone(), *self.var2num.get(&args[0]).unwrap()),
                dest.clone(),
                op_type.clone(),
            )),
            Code::Instruction(Instruction::Value {
                args,
                op,
                dest,
                op_type,
                ..
            }) if op != &ValueOps::Not && op != &ValueOps::Id && op != &ValueOps::Call => {
                // canonicalize order of args for commutative ops
                let mut arg_val0 = *self.var2num.get(&args[0]).unwrap();
                let mut arg_val1 = *self.var2num.get(&args[1]).unwrap();
                if op == &ValueOps::Add || op == &ValueOps::Mul {
                    if arg_val0 > arg_val1 {
                        let tmp = arg_val0.clone();
                        arg_val0 = arg_val1;
                        arg_val1 = tmp;
                    }
                }
                Some((
                    LVNValue::ValueBinaryOp(op.clone(), arg_val0, arg_val1),
                    dest.clone(),
                    op_type.clone(),
                ))
            }
            _ => None,
        }
    }

    fn calculate_binary_op(op: &ValueOps, arg0: &Literal, arg1: &Literal) -> Option<Literal> {
        match (arg0, arg1) {
            (Literal::Int(val0), Literal::Int(val1)) => match op {
                ValueOps::Add => Some(Literal::Int(val0 + val1)),
                ValueOps::Sub => Some(Literal::Int(val0 - val1)),
                ValueOps::Mul => Some(Literal::Int(val0 * val1)),
                ValueOps::Div => {
                    if *val1 == 0 {
                        None
                    } else {
                        Some(Literal::Int(val0 / val1))
                    }
                }
                ValueOps::Eq => Some(Literal::Bool(val0 == val1)),
                ValueOps::Lt => Some(Literal::Bool(val0 < val1)),
                ValueOps::Gt => Some(Literal::Bool(val0 > val1)),
                ValueOps::Le => Some(Literal::Bool(val0 <= val1)),
                ValueOps::Ge => Some(Literal::Bool(val0 >= val1)),
                ValueOps::And => Some(Literal::Bool((*val0 != 0) && (*val1 != 0))),
                ValueOps::Or => Some(Literal::Bool((*val0 != 0) || (*val1 != 0))),
                _ => None,
            },
            (Literal::Bool(val0), Literal::Bool(val1)) => match op {
                ValueOps::Eq => Some(Literal::Bool(val0 == val1)),
                ValueOps::Lt => Some(Literal::Bool(val0 < val1)),
                ValueOps::Gt => Some(Literal::Bool(val0 > val1)),
                ValueOps::Le => Some(Literal::Bool(val0 <= val1)),
                ValueOps::Ge => Some(Literal::Bool(val0 >= val1)),
                ValueOps::And => Some(Literal::Bool(*val0 && *val1)),
                ValueOps::Or => Some(Literal::Bool(*val0 || *val1)),
                _ => None,
            },
            _ => None,
        }
    }

    fn calculate_unary_op(op: &ValueOps, arg: &Literal) -> Option<Literal> {
        match arg {
            Literal::Int(val) => match op {
                ValueOps::Not => Some(Literal::Bool(*val == 0)),
                _ => None,
            },
            Literal::Bool(val) => match op {
                ValueOps::Not => Some(Literal::Bool(!val)),
                _ => None,
            },
        }
    }

    fn generate_copy_instruction(&self, value_number: &usize, dest: String, op_type: Type) -> Code {
        let var = self.num2var.get(&value_number).unwrap().clone();
        Code::Instruction(Instruction::Value {
            args: vec![var],
            dest: dest,
            funcs: vec![],
            labels: vec![],
            op: ValueOps::Id,
            pos: None,
            op_type: op_type,
        })
    }

    fn generate_const_instruction(value: &Literal, dest: String) -> Code {
        Code::Instruction(Instruction::Constant {
            dest: dest,
            op: ConstOps::Const,
            pos: None,
            const_type: match value {
                Literal::Bool(_) => Type::Bool,
                Literal::Int(_) => Type::Int,
            },
            value: value.clone(),
        })
    }

    fn generate_optimized_instruction(&self, instr: &Code, dest: Option<String>) -> Code {
        match instr {
            Code::Label { .. } => return instr.clone(),
            Code::Instruction(Instruction::Constant {
                op,
                pos,
                const_type,
                value,
                ..
            }) => {
                return Code::Instruction(Instruction::Constant {
                    dest: dest.unwrap(),
                    op: op.clone(),
                    pos: pos.clone(),
                    const_type: const_type.clone(),
                    value: value.clone(),
                });
            }
            Code::Instruction(Instruction::Value {
                args,
                funcs,
                labels,
                op,
                pos,
                op_type,
                ..
            }) => {
                return Code::Instruction(Instruction::Value {
                    args: self.replace_args(args),
                    dest: dest.unwrap(),
                    funcs: funcs.clone(),
                    labels: labels.clone(),
                    op: op.clone(),
                    pos: pos.clone(),
                    op_type: op_type.clone(),
                });
            }
            Code::Instruction(Instruction::Effect {
                args,
                funcs,
                labels,
                op,
                pos,
            }) => {
                return Code::Instruction(Instruction::Effect {
                    args: self.replace_args(args),
                    funcs: funcs.clone(),
                    labels: labels.clone(),
                    op: op.clone(),
                    pos: pos.clone(),
                })
            }
        }
    }

    pub fn optimize_instruction(&mut self, instr: &Code, last_write: bool) -> Code {
        // Get canonical value of instruction (if instruction is a value instruction)
        let canonical_val = self.canonicalize_instruction(instr);

        let mut new_dest: Option<String> = None;
        if let Some((canonical_val, dest, op_type)) = canonical_val {
            // Copy propagation
            if let LVNValue::ValueUnaryOp(ValueOps::Id, val_num) = canonical_val {
                self.var2num.insert(dest.clone(), val_num);
                return self.generate_copy_instruction(&val_num, dest, op_type);
            }

            // check if value has been seen already
            if let Some(val_num) = self.val2num.get(&canonical_val).cloned() {
                self.var2num.insert(dest.clone(), val_num);
                if let Some(value) = self.get_const_if_fold(&val_num) {
                    return Self::generate_const_instruction(value, dest);
                } else {
                    return self.generate_copy_instruction(&val_num, dest, op_type);
                }
            } else {
                let (dest, val_num) = self.register_val(&dest, canonical_val, last_write);
                new_dest = Some(dest.clone());

                // fold value if possible
                if let Some(value) = self.get_const_if_fold(&val_num) {
                    return Self::generate_const_instruction(value, dest);
                }
            }
        }

        // Replace args in instruction
        return self.generate_optimized_instruction(instr, new_dest);
    }
}
