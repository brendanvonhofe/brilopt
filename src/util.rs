use std::error::Error;
use std::fmt::Write;

use bril_rs::Function;

use crate::parse::control_flow_graph;

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
