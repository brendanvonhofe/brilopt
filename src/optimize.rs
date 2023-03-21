use std::collections::HashMap;

use bril_rs::{Code, Function, Instruction};

use crate::parse::BasicBlock;

pub fn dead_variable_elim(f: &Function) -> Function {
    let mut last = f.clone();
    loop {
        let used_vars: Vec<String> = last
            .instrs
            .iter()
            .flat_map(|line| -> Vec<String> {
                match &line {
                    Code::Instruction(Instruction::Value { args, .. })
                    | Code::Instruction(Instruction::Effect { args, .. }) => args.clone(),
                    _ => vec![],
                }
            })
            .collect();

        let func = Function {
            name: last.name.clone(),
            args: last.args.clone(),
            return_type: last.return_type.clone(),
            pos: last.pos.clone(),
            instrs: last
                .instrs
                .iter()
                .filter(|&x| -> bool {
                    match &x {
                        Code::Instruction(Instruction::Constant { dest, .. })
                        | Code::Instruction(Instruction::Value { dest, .. }) => {
                            return if used_vars.contains(&dest) {
                                true
                            } else {
                                false
                            };
                        }
                        _ => true,
                    }
                })
                .map(|x| x.clone())
                .collect(),
        };

        if func == last {
            break;
        }
        last = func;
    }
    return last;
}

pub fn dead_store_elim(b: &BasicBlock) -> BasicBlock {
    let mut last = b.clone();
    loop {
        let (block, _) = last.iter().enumerate().fold(
            (last.clone(), HashMap::new()),
            |(mut block, mut unused_defs), (i, instr)| {
                // Check for variable uses
                if let Code::Instruction(Instruction::Value { args, .. })
                | Code::Instruction(Instruction::Effect { args, .. }) = instr
                {
                    for var in args.iter() {
                        if unused_defs.contains_key(&var) {
                            unused_defs.remove(var);
                        }
                    }
                }
                // Check for variable definitions
                if let Code::Instruction(Instruction::Constant { dest, .. })
                | Code::Instruction(Instruction::Value { dest, .. }) = instr
                {
                    if unused_defs.contains_key(dest) {
                        block.remove(unused_defs[dest]);
                    }
                    unused_defs.insert(dest, i);
                }
                // Return
                (block, unused_defs)
            },
        );

        if block == last {
            break;
        }
        last = block;
    }
    return last;
}
