use std::collections::HashSet;
use std::fmt::Write;
use std::{collections::HashMap, error::Error};

pub type DiGraph = HashMap<String, Vec<String>>;

pub fn graphviz(digraph: &DiGraph, name: &String) -> Result<String, Box<dyn Error>> {
    let mut s = String::new();
    write!(s, "digraph {} {{\n", name)?;

    // Sort to make output deterministic
    let mut sorted_keys: Vec<&String> = digraph.keys().collect();
    sorted_keys.sort();

    for &key in &sorted_keys {
        write!(s, "  {};\n", key)?;
    }
    for &key in &sorted_keys {
        for succ in digraph[key].iter() {
            write!(s, "  {key} -> {succ};\n")?;
        }
    }
    write!(s, "}}")?;
    return Ok(s);
}

// probably not correct nomenclature and algorithmically slow
// reverses the direction of the edges of the graph
// e.g. takes a graph that represents a "successor" relation and produces a graph that represents a "predecessor" relation
pub fn invert_digraph(graph: &DiGraph) -> DiGraph {
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

pub fn invert_hashset(
    graph: &HashMap<String, HashSet<String>>,
) -> HashMap<String, HashSet<String>> {
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
// will panic if `cur_block` is not a key of `graph`
// will cause a stack overflow if there are loops
pub fn postorder_traversal(
    graph: &DiGraph,
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
