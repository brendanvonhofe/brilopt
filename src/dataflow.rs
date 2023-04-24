use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
};

use bril_rs::{Code, Function, Instruction};

use crate::parse::{
    block_name_to_idx, control_flow_graph, expanded_basic_blocks, ControlFlowGraph,
};

#[derive(Debug, Eq, Hash, PartialEq, Clone)]
pub struct Definition {
    pub name: String,
    pub block: String,
    pub line: usize,
}

// maps block name to in/out sets for that block
pub type DataFlowAnalysis = HashMap<String, (HashSet<Definition>, HashSet<Definition>)>;

fn invert_digraph(g: &ControlFlowGraph) -> ControlFlowGraph {
    let mut inv: ControlFlowGraph = HashMap::new();

    for node in g.keys() {
        inv.insert(
            node.clone(),
            g.keys().fold(vec![], |mut acc, key| {
                if g[key].contains(node) {
                    acc.push(key.clone());
                }
                return acc;
            }),
        );
    }

    inv
}

pub fn reaching_definitions(func: &Function) -> DataFlowAnalysis {
    let successors = control_flow_graph(func);
    let predecessors = invert_digraph(&successors);
    let blocks = expanded_basic_blocks(func);
    let block_names_to_idx: HashMap<String, usize> = block_name_to_idx(func);
    let block_names: Vec<String> = block_names_to_idx.keys().cloned().collect();

    let transfer = |b: &String, input: &HashSet<Definition>| -> HashSet<Definition> {
        let block = &blocks[block_names_to_idx[b]];
        let mut defined: HashSet<Definition> = HashSet::new();
        let mut in_minus_killed: HashSet<Definition> = input.clone();

        for (line, instr) in block.iter().enumerate() {
            if let Code::Instruction(Instruction::Constant { dest, .. })
            | Code::Instruction(Instruction::Value { dest, .. }) = instr
            {
                if let Some(def) = input.iter().find(|d| &d.name == dest) {
                    in_minus_killed.remove(def);
                }
                defined.insert(Definition {
                    name: dest.clone(),
                    block: b.clone(),
                    line,
                });
            }
        }

        defined.union(&in_minus_killed).cloned().collect()
    };

    let mut inputs: HashMap<String, HashSet<Definition>> = HashMap::new();
    let mut outputs: HashMap<String, HashSet<Definition>> = HashMap::new();

    // initialize
    inputs.insert(String::from("entry"), HashSet::new());
    for key in block_names.iter() {
        outputs.insert(key.clone(), HashSet::new());
    }

    let mut worklist = block_names.clone();
    while !worklist.is_empty() {
        let b = worklist.pop().unwrap();

        // merge
        inputs.insert(
            b.clone(),
            predecessors[&b].iter().fold(HashSet::new(), |acc, p| {
                acc.union(&outputs[p]).cloned().collect()
            }),
        );

        // transfer
        let new_output = transfer(&b, &inputs[&b]);
        if new_output != outputs[&b] {
            worklist.append(&mut successors[&b].clone());
            outputs.insert(b, new_output);
        }
    }

    block_names
        .iter()
        .map(|block_name| {
            (
                block_name.clone(),
                (inputs[block_name].clone(), outputs[block_name].clone()),
            )
        })
        .collect()
}
