use std::collections::HashMap;

use bril_rs::{Code, EffectOps, Function, Instruction};

pub type ControlFlowGraph = HashMap<String, Vec<String>>;
pub type BasicBlock = Vec<Code>;

const TERMINATORS: [EffectOps; 3] = [EffectOps::Jump, EffectOps::Branch, EffectOps::Return];

pub fn basic_blocks(func: &Function) -> Vec<BasicBlock> {
    let mut blocks: Vec<BasicBlock> = Vec::new();
    let mut block: BasicBlock = BasicBlock::new();

    for line in func.instrs.iter() {
        match &line {
            Code::Label { .. } => {
                if !block.is_empty() {
                    blocks.push(block.clone());
                    block.clear();
                }
                block.push(line.clone());
            }
            Code::Instruction(instr) => {
                block.push(line.clone());
                if let Instruction::Effect { op, .. } = instr {
                    if TERMINATORS.contains(&op) && !block.is_empty() {
                        blocks.push(block.clone());
                        block.clear();
                    }
                }
            }
        }
    }
    if !block.is_empty() {
        blocks.push(block.clone());
        block.clear();
    }

    blocks
}

pub fn get_block_name(block: &BasicBlock, block_idx: usize, func_name: &String) -> String {
    let from: String;
    if let Code::Label { label, .. } = &block[0] {
        from = label.clone();
    } else {
        from = func_name.clone() + &block_idx.to_string()
    }
    from
}

pub fn expanded_basic_blocks(func: &Function) -> Vec<BasicBlock> {
    let mut blocks = basic_blocks(func);
    blocks.insert(
        0,
        vec![Code::Label {
            label: String::from("entry"),
            pos: None,
        }],
    );
    blocks.push(vec![Code::Label {
        label: String::from("exit"),
        pos: None,
    }]);
    blocks
}

pub fn block_name_to_idx(func: &Function) -> HashMap<String, usize> {
    expanded_basic_blocks(func)
        .iter()
        .enumerate()
        .map(|(idx, block)| (get_block_name(block, idx, &func.name), idx))
        .collect()
}

pub fn control_flow_graph(func: &Function) -> ControlFlowGraph {
    let fname = &func.name;
    let mut cfg = ControlFlowGraph::new();
    let blocks = expanded_basic_blocks(&func);
    assert!(blocks.len() > 0);

    for i in 0..blocks.len() - 1 {
        let block = &blocks[i];
        let last = &block[block.len() - 1];
        let from = get_block_name(block, i, fname);

        match &last {
            Code::Instruction(instr) => match &instr {
                Instruction::Effect { op, labels, .. }
                    if op == &EffectOps::Jump || op == &EffectOps::Branch =>
                {
                    // Last instruction in block is a jump or branch
                    cfg.insert(from, labels.clone());
                }
                _ => {
                    // Successor is just the next block
                    cfg.insert(from, vec![get_block_name(&blocks[i + 1], i + 1, fname)]);
                }
            },
            Code::Label { .. } => {
                cfg.insert(from, vec![get_block_name(&blocks[i + 1], i + 1, fname)]);
            }
        }
    }
    cfg.insert(
        get_block_name(&blocks[blocks.len() - 1], blocks.len() - 1, fname),
        vec![],
    );

    return cfg;
}
