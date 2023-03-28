use std::fs::File;

use bril_rs::{load_program, load_program_from_read, Function};

use brilopt::{
    optimize::{dead_store_elim, dead_variable_elim, lvn_block},
    parse::basic_blocks,
    util::graphviz,
};

const DEBUG_FILEPATH: &str = "/Users/bvonhofe/Desktop/bril/bril-rs/brilopt/test/opt/commute.json";

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
        _ => {
            println!("[DEBUG MODE] Reading program from {}\n", DEBUG_FILEPATH);
            let debug_file = File::open(DEBUG_FILEPATH).unwrap();
            let prog = load_program_from_read(debug_file);

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
    }
}
