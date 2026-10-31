//! `genui-dev` (ADR-012 D12.1): the local dev binary, built only with the
//! `dev-stand` feature (`required-features`); see `genui::dev`. Started by
//! `scripts/dev/server.sh` under process-compose (`just dev`).
fn main() -> std::process::ExitCode {
    genui::dev::main()
}
