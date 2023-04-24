use std::fs::File;

use bril_rs::{load_program, load_program_from_read, Function};

use brilopt::{
    dataflow::{dominators, reaching_definitions},
    optimize::{dead_store_elim, dead_variable_elim, lvn_block},
    parse::{basic_blocks, block_name_to_idx, expanded_basic_blocks, get_block_name},
    util::graphviz,
};

const DEBUG_FILEPATH: &str = "/Users/bvonhofe/Desktop/bril/bril-rs/brilopt/test/fib2seven.json";

fn main() {
    let mut args = std::env::args();
    args.next();
    let mode = args.next().unwrap_or(String::from("dbg")).to_lowercase();

    match mode.as_str() {
        "main" => {
            let prog = load_program();
            println!("{}", &prog);
        }
        "cfg" => {
            let prog = load_program();
            for func in prog.functions.iter() {
                println!("{}", graphviz(&func).unwrap());
                break;
            }
        }
        "opt" => {
            let prog = load_program();

            let mut opt_prog = prog.clone();
            opt_prog.functions = opt_prog
                .functions
                .iter()
                .map(|func| Function {
                    args: func.args.clone(),
                    instrs: basic_blocks(&func)
                        .iter()
                        .flat_map(|block| lvn_block(block, false))
                        .collect(),
                    name: func.name.clone(),
                    pos: func.pos.clone(),
                    return_type: func.return_type.clone(),
                })
                .map(|func| dead_variable_elim(&func))
                .map(|func| Function {
                    args: func.args.clone(),
                    instrs: basic_blocks(&func)
                        .iter()
                        .flat_map(|block| dead_store_elim(block))
                        .collect(),
                    name: func.name.clone(),
                    pos: func.pos.clone(),
                    return_type: func.return_type.clone(),
                })
                .collect();

            println!("[original] {}\n[optimized] {}", &prog, &opt_prog);
        }
        "fold" => {
            let prog = load_program();

            let mut opt_prog = prog.clone();
            opt_prog.functions = opt_prog
                .functions
                .iter()
                .map(|func| Function {
                    args: func.args.clone(),
                    instrs: basic_blocks(&func)
                        .iter()
                        .flat_map(|block| lvn_block(block, true))
                        .collect(),
                    name: func.name.clone(),
                    pos: func.pos.clone(),
                    return_type: func.return_type.clone(),
                })
                .collect();

            println!("[original] {}\n[folded] {}", &prog, &opt_prog);
        }
        "foldopt" => {
            let prog = load_program();

            let mut opt_prog = prog.clone();
            opt_prog.functions = opt_prog
                .functions
                .iter()
                .map(|func| Function {
                    args: func.args.clone(),
                    instrs: basic_blocks(&func)
                        .iter()
                        .flat_map(|block| lvn_block(block, true))
                        .collect(),
                    name: func.name.clone(),
                    pos: func.pos.clone(),
                    return_type: func.return_type.clone(),
                })
                .map(|func| dead_variable_elim(&func))
                .map(|func| Function {
                    args: func.args.clone(),
                    instrs: basic_blocks(&func)
                        .iter()
                        .flat_map(|block| dead_store_elim(block))
                        .collect(),
                    name: func.name.clone(),
                    pos: func.pos.clone(),
                    return_type: func.return_type.clone(),
                })
                .collect();

            println!("[original] {}\n[optimized] {}", &prog, &opt_prog);
        }
        "reach" => {
            let prog = load_program();

            for func in prog.functions.iter() {
                let reaching = reaching_definitions(func);
                for (i, b) in expanded_basic_blocks(func).iter().enumerate() {
                    let block = get_block_name(&b, i, &func.name);
                    let (inputs, outputs) = &reaching[&block];

                    let mut inputs_str = inputs
                        .iter()
                        .map(|def| {
                            def.name.clone() + "_" + &def.block + "_" + &def.line.to_string()
                        })
                        .collect::<Vec<String>>();
                    inputs_str.sort();

                    let mut outputs_str = outputs
                        .iter()
                        .map(|def| {
                            def.name.clone() + "_" + &def.block + "_" + &def.line.to_string()
                        })
                        .collect::<Vec<String>>();
                    outputs_str.sort();

                    println!(
                        "{}:\n  in:  {}\n  out: {}",
                        block,
                        inputs_str.join(" "),
                        outputs_str.join(" ")
                    );
                }
                println!("");
            }
        }
        "dom" => {
            let prog = load_program();

            for func in prog.functions.iter() {
                let name2idx = block_name_to_idx(func);

                println!("{}", &func.name);
                let dom_map = dominators(func);
                let mut blocks: Vec<String> = dom_map.keys().cloned().collect();
                blocks.sort_by(|a, b| name2idx[a].cmp(&name2idx[b]));

                for block in blocks.iter() {
                    let mut doms: Vec<String> = dom_map[block].clone().into_iter().collect();
                    doms.sort_by(|a, b| name2idx[a].cmp(&name2idx[b]));
                    println!("  {}: {:?}", block, doms);
                }
                println!("");
            }
        }
        _ => {
            println!("[DEBUG MODE] Reading program from {}\n", DEBUG_FILEPATH);
            let debug_file = File::open(DEBUG_FILEPATH).unwrap();
            let prog = load_program_from_read(debug_file);

            for func in prog.functions.iter() {
                println!("{:?}", reaching_definitions(func));
            }
        }
    }
}
