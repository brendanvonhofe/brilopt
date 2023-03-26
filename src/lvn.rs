use std::collections::HashMap;

use bril_rs::{Code, ConstOps, Instruction, Literal, ValueOps};

use crate::parse::BasicBlock;

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum OpType {
    Const(ConstOps),
    Value(ValueOps),
}

#[derive(Eq, Hash, PartialEq, Clone, Debug)]
pub enum LVNValue {
    Constant(Literal),
    ValueOp(OpType, Option<usize>, Option<usize>),
}

pub struct LVN {
    table: HashMap<LVNValue, (usize, String)>,
    number_map: HashMap<usize, LVNValue>,
    env: HashMap<Option<String>, usize>,
}

impl LVN {
    pub fn new() -> LVN {
        LVN {
            table: HashMap::new(),
            number_map: HashMap::new(),
            env: HashMap::new(),
        }
    }

    fn will_be_overwritten(dst: &str, start: usize, block: &BasicBlock) -> bool {
        for i in start + 1..block.len() {
            if let Code::Instruction(Instruction::Constant { dest, .. })
            | Code::Instruction(Instruction::Value { dest, .. }) = &block[i]
            {
                if dst == dest {
                    return true;
                }
            }
        }
        return false;
    }

    fn replace_args(&self, args: &Vec<String>, start: usize, block: &BasicBlock) -> Vec<String> {
        args.iter()
            .map(|a| {
                let arg = Self::check_dest(a, start, block);
                // dbg!(&arg, &self.env);

                match self.env.get(&Some(arg.clone())) {
                    Some(idx) => {
                        let val = self.number_map.get(idx).unwrap();
                        let (_, var) = self.table.get(val).unwrap();
                        // dbg!(var);
                        return var.clone();
                    }
                    None => {
                        // dbg!(&arg);
                        return arg.clone();
                    }
                }
                // let idx = self.env.get(&Some(arg.clone())).unwrap();
                // let val = self.number_map.get(idx).unwrap();
                // let (_, var) = self.table.get(val).unwrap();
                // return var.clone();
            })
            .collect()
    }

    fn insert_table(&mut self, canonical_val: &LVNValue, dest: &String) -> usize {
        let lvn_num = self.table.len();
        self.table
            .insert(canonical_val.clone(), (lvn_num, dest.clone()));
        self.number_map.insert(lvn_num, canonical_val.clone());
        lvn_num
    }

    fn check_dest(dest: &String, i: usize, block: &BasicBlock) -> String {
        if Self::will_be_overwritten(&dest, i, block) {
            dest.clone() + "_" + &i.to_string()
        } else {
            dest.clone()
        }
    }

    pub fn process_instr(&mut self, i: usize, instr: &Code, block: &BasicBlock) -> Code {
        // dbg!(instr);
        let canonical_val: LVNValue;
        let lvn_num: usize;
        let ret: Code;
        let new_dest: String;
        match instr {
            Code::Instruction(Instruction::Constant {
                dest,
                op,
                pos,
                const_type,
                value,
            }) => {
                canonical_val = LVNValue::Constant(value.clone());
                // dbg!(&canonical_val);
                match self.table.get(&canonical_val) {
                    Some((idx, var)) => {
                        lvn_num = idx.clone();
                        new_dest = Self::check_dest(dest, i, block);
                        ret = Code::Instruction(Instruction::Value {
                            args: vec![var.clone()],
                            dest: new_dest.clone(),
                            funcs: vec![],
                            labels: vec![],
                            op: ValueOps::Id,
                            pos: None,
                            op_type: const_type.clone(),
                        });
                    }
                    None => {
                        new_dest = Self::check_dest(dest, i, block);
                        lvn_num = self.insert_table(&canonical_val, &new_dest);
                        ret = Code::Instruction(Instruction::Constant {
                            dest: new_dest.clone(),
                            op: op.clone(),
                            pos: pos.clone(),
                            const_type: const_type.clone(),
                            value: value.clone(),
                        });
                    }
                }
                self.env.insert(Some(new_dest.clone()), lvn_num);
            }
            Code::Instruction(Instruction::Value {
                args,
                dest,
                funcs,
                labels,
                op,
                pos,
                op_type,
            }) => {
                let arg0 = self.env.get(&args.get(0).cloned()).cloned();
                let arg1 = self.env.get(&args.get(1).cloned()).cloned();
                match (arg0, arg1) {
                    (Some(num1), Some(num2)) => {
                        canonical_val = LVNValue::ValueOp(
                            OpType::Value(op.clone()),
                            Some(std::cmp::min(num1, num2)),
                            Some(std::cmp::max(num1, num2)),
                        );
                    }
                    (Some(num), None) | (None, Some(num)) => {
                        canonical_val =
                            LVNValue::ValueOp(OpType::Value(op.clone()), Some(num), None);
                    }
                    (None, None) => {
                        canonical_val = LVNValue::ValueOp(OpType::Value(op.clone()), None, None);
                    }
                }
                // dbg!(&canonical_val);

                match self.table.get(&canonical_val) {
                    Some((idx, var)) => {
                        lvn_num = idx.clone();
                        new_dest = Self::check_dest(dest, i, block);
                        ret = Code::Instruction(Instruction::Value {
                            args: vec![var.clone()],
                            dest: new_dest.clone(),
                            funcs: vec![],
                            labels: vec![],
                            op: ValueOps::Id,
                            pos: None,
                            op_type: op_type.clone(),
                        });
                    }
                    None => {
                        new_dest = Self::check_dest(dest, i, block);
                        lvn_num = self.insert_table(&canonical_val, &new_dest);
                        ret = Code::Instruction(Instruction::Value {
                            args: self.replace_args(args, i, block),
                            dest: new_dest.clone(),
                            funcs: funcs.clone(),
                            labels: labels.clone(),
                            op: op.clone(),
                            pos: pos.clone(),
                            op_type: op_type.clone(),
                        });
                    }
                }
                self.env.insert(Some(new_dest.clone()), lvn_num);
            }
            Code::Instruction(Instruction::Effect {
                args,
                funcs,
                labels,
                op,
                pos,
            }) => {
                ret = Code::Instruction(Instruction::Effect {
                    args: self.replace_args(args, i, block),
                    funcs: funcs.clone(),
                    labels: labels.clone(),
                    op: op.clone(),
                    pos: pos.clone(),
                });
            }
            Code::Label { label, pos } => {
                ret = Code::Label {
                    label: label.clone(),
                    pos: pos.clone(),
                }
            }
        }
        // dbg!(&self.table, &self.env, &ret);
        // println!("\n\n");
        return ret;
    }
}
