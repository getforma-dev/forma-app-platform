use anyhow::{Context, Result};
use parser_host::{job, Limits, Sandbox};
use std::io::Read;
fn read_bounded(path: &str, cap: usize) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(cap as u64 + 1)
        .read_to_end(&mut bytes)?;
    anyhow::ensure!(bytes.len() <= cap, "host_file_limit");
    Ok(bytes)
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--job") {
        // One framed job on stdin, then exit: one Engine per process.
        let code = job::main(
            &args[1..],
            &mut std::io::stdin().lock(),
            &mut std::io::stdout().lock(),
            &mut std::io::stderr().lock(),
        );
        std::process::exit(code);
    }
    anyhow::ensure!(
        args.len() == 4,
        "usage: parser-host <parser.wasm> <input> <config.json> | parser-host --job --wasm <parser.wasm> --wasm-sha256 <hex>"
    );
    // The module is an operator-selected, built artifact; never an upload.
    let wasm = read_bounded(&args[1], 16 * 1024 * 1024)?;
    let input = read_bounded(&args[2], 100 * 1024 * 1024)?;
    let config = serde_json::from_slice(&read_bounded(&args[3], 4096)?).context("config JSON")?;
    let result = Sandbox::new(&wasm)?.run(&input, &config, Limits::default())?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
