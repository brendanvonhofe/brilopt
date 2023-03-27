use std::collections::{HashMap, HashSet};

use bril_rs::{Code, Instruction, Literal, ValueOps};

use crate::parse::BasicBlock;

#[derive(Eq, PartialEq, Hash, Debug)]
pub enum LVNValue {
    Constant(Literal),
    ValueBinaryOp(ValueOps, usize, usize),
    ValueUnaryOp(ValueOps, usize),
}

pub struct LVN {
    next: usize,
    var2num: HashMap<String, usize>, // the value of variables, multiple variables can have same value
    val2num: HashMap<LVNValue, usize>,
    num2var: HashMap<usize, String>,
    // num2const: HashMap<usize, Literal>,
}

impl LVN {
    pub fn new() -> LVN {
        // LVN table: Number | Value | Variable
        LVN {
            next: 0,
            val2num: HashMap::new(),
            num2var: HashMap::new(),
            var2num: HashMap::new(),
            // num2const: HashMap::new(),
        }
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

    pub fn register_var(&mut self, dest: &String, num: usize, last_write: bool) -> String {
        let var: String;
        if last_write {
            var = dest.clone()
        } else {
            var = format!("lvn.{}", num);
        }
        self.num2var.insert(num, var.clone());
        return var;
    }

    pub fn extend_env(&mut self, var: &String) -> usize {
        let num = self.next;
        self.next += 1;
        self.var2num.insert(var.clone(), num);
        return num;
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

    fn canonicalize_instruction(&self, instr: &Code) -> Option<LVNValue> {
        let canonical_val: Option<LVNValue>;
        match instr {
            Code::Instruction(Instruction::Constant { value, .. }) => {
                canonical_val = Some(LVNValue::Constant(value.clone()));
            }
            Code::Instruction(Instruction::Value { args, op, .. })
                if op == &ValueOps::Not || op == &ValueOps::Id =>
            {
                canonical_val = Some(LVNValue::ValueUnaryOp(
                    op.clone(),
                    *self.var2num.get(&args[0]).unwrap(),
                ));
            }
            Code::Instruction(Instruction::Value { args, op, .. })
                if op != &ValueOps::Not && op != &ValueOps::Id && op != &ValueOps::Call =>
            {
                let mut arg_val0 = *self.var2num.get(&args[0]).unwrap();
                let mut arg_val1 = *self.var2num.get(&args[1]).unwrap();
                if op == &ValueOps::Add || op == &ValueOps::Mul {
                    if arg_val0 > arg_val1 {
                        let tmp = arg_val0.clone();
                        arg_val0 = arg_val1;
                        arg_val1 = tmp;
                    }
                }
                canonical_val = Some(LVNValue::ValueBinaryOp(op.clone(), arg_val0, arg_val1));
            }
            _ => {
                canonical_val = None;
            }
        }
        return canonical_val;
    }

    fn generate_copy_instruction(&mut self, value_number: &usize, instr: &Code) -> Code {
        if let Code::Instruction(Instruction::Value { dest, op_type, .. })
        | Code::Instruction(Instruction::Constant {
            dest,
            const_type: op_type,
            ..
        }) = instr
        {
            self.var2num.insert(dest.clone(), *value_number);

            let var = self.num2var.get(&value_number).unwrap().clone();
            return Code::Instruction(Instruction::Value {
                args: vec![var],
                dest: dest.clone(),
                funcs: vec![],
                labels: vec![],
                op: ValueOps::Id,
                pos: None,
                op_type: op_type.clone(),
            });
        } else {
            panic!(
                "Expected a Code::Instruction(Instruction::Value) | Code::Instruction(Instruction::Constant) and received: {}",
                instr
            );
        }
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
        if let Some(canonical_val) = canonical_val {
            // Copy propagation
            if let LVNValue::ValueUnaryOp(ValueOps::Id, val_num) = canonical_val {
                // Emit copy instruction: dest = instr.dest, args = [num2var[var2num[arg]]]
                return self.generate_copy_instruction(&val_num, instr);
            }

            // Check if value has been seen already
            if let Some(val_num) = self.val2num.get(&canonical_val).cloned() {
                // Emit copy instruction: dest = instr.dest, args = [num2var[val2num[canonical_val]]]
                return self.generate_copy_instruction(&val_num, instr);
            } else {
                // Register value: num = extend_env(dest), val2num.insert(canonical_val, num)
                if let Code::Instruction(Instruction::Constant { dest, .. })
                | Code::Instruction(Instruction::Value { dest, .. }) = instr
                {
                    let val_num = self.extend_env(dest);
                    self.val2num.insert(canonical_val, val_num);
                    new_dest = Some(self.register_var(&dest, val_num, last_write));
                } else {
                    panic!("Expected a Code::Instruction(Instruction::Constant) | Code::Instruction(Instruction::Value) and received: {}", instr);
                }
            }
        }

        // Replace args in instruction
        let ret = self.generate_optimized_instruction(instr, new_dest);
        return ret;
    }
}
