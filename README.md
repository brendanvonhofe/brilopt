# brilopt

## Examples:
Generate control flow graph of Bril program as Graphviz PDF
	`cd mycfg`
	`bril2json < ../bril/test/interp/core/add-overflow.bril | cargo run | dot -Tpdf -o add-overflow-cfg.pdf` 
