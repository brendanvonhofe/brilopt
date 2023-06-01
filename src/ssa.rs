use std::collections::{HashMap, HashSet};

use bril_rs::{Code, Function, Instruction, Type, ValueOps};

use crate::{
    analyze::{dominance_frontier, dominator_tree},
    parse::{basic_blocks, block_name_to_idx, control_flow_graph, get_block_name, BasicBlock},
    util::invert_hashset,
};

pub fn convert_to_ssa(func: &Function) {
    // Insert phi nodes
    let mut blocks = basic_blocks(func);
    let successors = control_flow_graph(func);
    let dom_tree = dominator_tree(func);
    let frontier = dominance_frontier(func);
    let inv_frontier = invert_hashset(&frontier);
    let block_map = block_name_to_idx(func);

    let orig_var_names: HashSet<String> = func
        .instrs
        .iter()
        .filter_map(|code| {
            if let Code::Instruction(instr) = code {
                if let Instruction::Constant { dest, .. } | Instruction::Value { dest, .. } = instr
                {
                    return Some(dest.clone());
                }
            }
            return None;
        })
        .collect::<HashSet<String>>();

    // map variable names to definitions (block name, block idx, line no.)
    let mut var_defs: HashMap<String, Vec<(String, Type)>> = orig_var_names
        .iter()
        .map(|var| {
            (
                var.clone(),
                blocks
                    .iter()
                    .enumerate()
                    .flat_map(|(block_idx, block)| {
                        block.iter().filter_map(move |code| {
                            if let Code::Instruction(instr) = code {
                                if let Instruction::Constant {
                                    dest, const_type, ..
                                } = instr
                                {
                                    if dest == var {
                                        return Some((
                                            get_block_name(block, block_idx, &func.name),
                                            const_type.clone(),
                                        ));
                                    }
                                }
                                if let Instruction::Value { dest, op_type, .. } = instr {
                                    if dest == var {
                                        return Some((
                                            get_block_name(block, block_idx, &func.name),
                                            op_type.clone(),
                                        ));
                                    }
                                }
                            }
                            return None;
                        })
                    })
                    .collect(),
            )
        })
        .collect();

    for var in &orig_var_names {
        for (def_block_name, op_type) in &var_defs[var].clone() {
            for sub_block_name in &frontier[def_block_name] {
                let sub_block_idx = block_map[sub_block_name];

                // label must always be first instruction in block
                let mut phi_idx = 0;
                if let Code::Label { .. } = blocks[sub_block_idx][0] {
                    if let Code::Instruction(Instruction::Value {
                        op: ValueOps::Phi, ..
                    }) = blocks[sub_block_idx][1]
                    {
                        continue;
                    }
                    phi_idx = 1;
                }

                // check for phi block of same var
                if let Some(_) = &blocks[sub_block_idx].iter().find(|&code| {
                    if let Code::Instruction(Instruction::Value {
                        op: ValueOps::Phi,
                        dest,
                        ..
                    }) = code
                    {
                        if dest == var {
                            return true;
                        }
                    }
                    return false;
                }) {
                    continue;
                }

                // insert phi block
                blocks[sub_block_idx].insert(
                    phi_idx,
                    Code::Instruction(Instruction::Value {
                        args: vec![],
                        dest: var.clone(),
                        funcs: vec![],
                        labels: vec![],
                        op: ValueOps::Phi,
                        pos: None,
                        op_type: op_type.clone(),
                    }),
                );

                // register phi block as variable definition
                var_defs
                    .get_mut(var)
                    .expect(&format!("Variable definition vec not found for {}", var))
                    .push((sub_block_name.clone(), op_type.clone()));
            }
        }
    }

    // Rename variables
    let mut var_names: HashMap<String, Vec<String>> = orig_var_names
        .iter()
        .map(|var| (var.clone(), vec![var.clone()]))
        .collect();

    let rename = |block_name: &String| {
        let mut block = &mut blocks[block_map[block_name]];
        for instr in block {
            // replace args in instr with top of stacks of respective vars
            if let Code::Instruction(Instruction::Effect { args, .. })
            | Code::Instruction(Instruction::Value { args, .. }) = instr
            {
                for i in 0..args.len() {
                    args[i] = var_names[&args[i]]
                        .last()
                        .expect(&format!("Stack for variable '{}' is empty", &args[i]))
                        .clone();
                }
            }

            // create new name for destination and push to stack
            if let Code::Instruction(Instruction::Constant { dest, .. })
            | Code::Instruction(Instruction::Value { dest, .. }) = instr
            {
                let mut new_name = format!(
                    "{}_{}",
                    var_names[dest]
                        .last()
                        .expect(&format!("Stack for variable '{}' is empty", dest)),
                    var_names[dest].len()
                );
                while orig_var_names.contains(&new_name) {
                    new_name = new_name + "_";
                }

                var_names
                    .get_mut(dest)
                    .expect(&format!("Stack for variable '{}' not found", dest))
                    .push(new_name);
            }
        }

        // get phi nodes in successor blocks
        let mut ancestors: Vec<BasicBlock>;
        for successor in &successors[block_name] {
            ancestors = inv_frontier[successor]
                .iter()
                .map(|ancestor_name| blocks[block_map[ancestor_name]].clone())
                .collect();
            let mut suc_block = &mut blocks[block_map[successor]];
            let mut phi_nodes = suc_block.iter_mut().filter_map(|code| {
                if let Code::Instruction(Instruction::Value {
                    op: ValueOps::Phi,
                    dest,
                    args,
                    labels,
                    ..
                }) = code
                {
                    return Some((dest, args, labels));
                }
                return None;
            });

            // for each dominant ancestor block, get the latest definition of the relevant variable and add info to phi node
            for (phi_dest, args, labels) in phi_nodes {
                for ancestor in &ancestors {
                    ancestor
                        .iter()
                        .rev()
                        .filter_map(|code| {
                            if let Code::Instruction(Instruction::Constant { dest, .. })
                            | Code::Instruction(Instruction::Value { dest, .. }) = code
                            {
                                return Some(dest);
                            }
                            return None;
                        })
                        .find(|&dest| {
                            if var_names[phi_dest].contains(dest) {
                                return true;
                            }
                            return false;
                        })
                        .map(|dest| {
                            args.push(dest.clone());
                            labels.push(successor.clone());
                        });
                }
            }
        }

        for sub_block in &dom_tree[block_name] {
            rename(sub_block);
        }

        var_names = orig_var_names
            .iter()
            .map(|var| (var.clone(), vec![var.clone()]))
            .collect();
    };
}
