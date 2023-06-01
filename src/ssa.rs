use std::collections::{HashMap, HashSet};

use bril_rs::{Code, Function, Instruction, Type, ValueOps};

use crate::{
    analyze::{dominance_frontier, dominator_tree},
    parse::{block_name_to_idx, control_flow_graph, expanded_basic_blocks, get_block_name},
    util::{invert_digraph, invert_hashset},
};

pub fn convert_to_ssa(func: &Function) -> Function {
    // Insert phi nodes
    let mut blocks = expanded_basic_blocks(func);
    let successors = control_flow_graph(func);
    let predecessors = invert_digraph(&successors);
    let dom_tree = dominator_tree(func);
    let inv_dom_tree = invert_digraph(&dom_tree);
    let frontier = dominance_frontier(func);
    let inv_frontier = invert_hashset(&frontier);
    let block_map = block_name_to_idx(func);

    // set of var definitions (block name, var name)
    let orig_var_block_names: HashSet<(String, String)> = blocks
        .iter()
        .enumerate()
        .flat_map(|(block_idx, block)| {
            block.iter().filter_map(move |code| {
                if let Code::Instruction(instr) = code {
                    if let Instruction::Constant { dest, .. } | Instruction::Value { dest, .. } =
                        instr
                    {
                        return Some((get_block_name(block, block_idx, &func.name), dest.clone()));
                    }
                }
                return None;
            })
        })
        .chain(
            func.args
                .iter()
                .map(|arg| (String::from("<func_arg>"), arg.name.clone())),
        )
        .collect();

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
        .chain(func.args.iter().map(|arg| arg.name.clone()))
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

    // add phi blocks
    for var in &orig_var_names {
        for (def_block_name, op_type) in &var_defs[var].clone() {
            for sub_block_name in &frontier[def_block_name] {
                let sub_block_idx = block_map[sub_block_name];

                // label must always be first instruction in block
                let mut phi_idx = 0;
                if let Code::Label { .. } = blocks[sub_block_idx][0] {
                    if blocks[sub_block_idx].len() < 2 {
                        continue;
                    }
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

    // Map from old var names to vector of definitions (block name, new var name)
    let mut var_names: HashMap<String, Vec<(String, String)>> = orig_var_block_names
        .iter()
        .map(|(block, var)| (var.clone(), vec![(block.clone(), var.clone())]))
        .collect();

    let mut name_counter: HashMap<String, usize> =
        orig_var_names.iter().map(|s| (s.clone(), 1)).collect();

    // Rename variables
    fn rename(
        block_name: &String,
        var_names: &mut HashMap<String, Vec<(String, String)>>,
        blocks: &mut Vec<Vec<Code>>,
        block_map: &HashMap<String, usize>,
        orig_var_names: &HashSet<String>,
        successors: &HashMap<String, Vec<String>>,
        predecessors: &HashMap<String, Vec<String>>,
        inv_frontier: &HashMap<String, HashSet<String>>,
        dom_tree: &HashMap<String, Vec<String>>,
        inv_dom_tree: &HashMap<String, Vec<String>>,
        name_counter: &mut HashMap<String, usize>,
    ) {
        let init_var_stacks = var_names.clone();

        let block = &mut blocks[block_map[block_name]];
        for instr in block {
            // replace args in instr with top of stacks of respective vars
            if let Code::Instruction(Instruction::Value {
                op: ValueOps::Phi, ..
            }) = &instr
            {
                // do nothing
            } else if let Code::Instruction(Instruction::Effect { args, .. })
            | Code::Instruction(Instruction::Value { args, .. }) = instr
            {
                for i in 0..args.len() {
                    args[i] = var_names[&args[i]]
                        .last()
                        .expect(&format!("Stack for variable '{}' is empty", &args[i]))
                        .1
                        .clone();
                }
            }

            // create new name for destination and push to stack
            if let Code::Instruction(Instruction::Constant { dest, .. })
            | Code::Instruction(Instruction::Value { dest, .. }) = instr
            {
                let mut new_name = format!("{}.{}", dest, name_counter[dest]);
                while orig_var_names.contains(&new_name) {
                    new_name = new_name + "_";
                }

                var_names
                    .get_mut(dest)
                    .expect(&format!("Stack for variable '{}' not found", dest))
                    .push((block_name.clone(), new_name.clone()));
                *name_counter
                    .get_mut(dest)
                    .expect(&format!("Name counter for variable '{}' not found", dest)) += 1;

                *dest = new_name;
            }
        }

        // get phi nodes in successor blocks
        for successor in &successors[block_name] {
            let suc_block = &mut blocks[block_map[successor]];
            let phi_nodes = suc_block.iter_mut().filter_map(|code| {
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

            // add info to phi nodes in successor block
            for (phi_dest, args, labels) in phi_nodes {
                let canonical_name = var_names
                    .iter()
                    .map(|(key, v)| {
                        (
                            key,
                            v.iter().map(|(_, y)| y.clone()).collect::<Vec<String>>(),
                        )
                    })
                    .find_map(|(old_name, names)| {
                        if names.contains(phi_dest) {
                            return Some(old_name);
                        }
                        return None;
                    })
                    .expect("Cannot find canonical name");
                let (bname, vname) = var_names[canonical_name]
                    .last()
                    .expect(&format!("Stack for variable '{}' is empty", &phi_dest));
                args.push(vname.clone());
                labels.push(bname.clone());
            }
        }

        for sub_block in &dom_tree[block_name] {
            rename(
                sub_block,
                var_names,
                blocks,
                block_map,
                orig_var_names,
                successors,
                predecessors,
                inv_frontier,
                dom_tree,
                inv_dom_tree,
                name_counter,
            );
        }

        var_names.clear();
        for (key, val) in init_var_stacks {
            var_names.insert(key, val);
        }
    }

    rename(
        &String::from("entry"),
        &mut var_names,
        &mut blocks,
        &block_map,
        &orig_var_names,
        &successors,
        &predecessors,
        &inv_frontier,
        &dom_tree,
        &inv_dom_tree,
        &mut name_counter,
    );

    Function {
        args: func.args.clone(),
        instrs: blocks[1..blocks.len() - 1]
            .into_iter()
            .flatten()
            .cloned()
            .collect(),
        name: func.name.clone(),
        pos: func.pos.clone(),
        return_type: func.return_type.clone(),
    }
}
