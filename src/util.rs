use std::error::Error;
use std::fmt::Write;

use bril_rs::Function;

use crate::parse::{control_flow_graph, ControlFlowGraph};

pub fn graphviz(func: &Function) -> Result<String, Box<dyn Error>> {
    let mut s = String::new();
    write!(s, "digraph {} {{\n", func.name)?;
    let cfg = control_flow_graph(func);

    // Sort to make output deterministic
    let mut sorted_keys: Vec<&String> = cfg.keys().collect();
    sorted_keys.sort();

    for &key in &sorted_keys {
        write!(s, "  {};\n", key)?;
    }
    for &key in &sorted_keys {
        for succ in cfg[key].iter() {
            write!(s, "  {key} -> {succ};\n")?;
        }
    }
    write!(s, "}}")?;
    return Ok(s);
}

pub fn invert_digraph(graph: &ControlFlowGraph) -> ControlFlowGraph {
    graph
        .keys()
        .map(|node| {
            (
                node.clone(),
                graph
                    .keys()
                    .cloned()
                    .filter(|key| graph[key].contains(node))
                    .collect(),
            )
        })
        .collect()
}

// e.g. postorder_traversal(&control_flow_graph(func), "entry", vec![]);
pub fn postorder_traversal(
    graph: &ControlFlowGraph,
    cur_block: String,
    postorder: Vec<String>,
) -> Vec<String> {
    let mut new_postorder = vec![];
    for child_block in graph[&cur_block].iter() {
        for block in postorder_traversal(graph, child_block.clone(), postorder.clone()) {
            if !new_postorder.contains(&block) {
                new_postorder.push(block);
            }
        }
    }
    if !new_postorder.contains(&cur_block) {
        new_postorder.push(cur_block.clone());
    }
    return new_postorder;
}
