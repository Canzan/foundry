//! xtask — developer-only task runner. Slice 1 stub.
//!
//! Future subcommands (per architecture.md): `check-arch`, `check-probes`.

fn main() -> anyhow::Result<()> {
    let cmd = std::env::args().nth(1).unwrap_or_default();
    match cmd.as_str() {
        "" | "help" => {
            println!("foundry xtask — slice 1 stub");
            println!("subcommands: (none yet)");
        }
        other => {
            eprintln!("unknown subcommand: {other}");
            std::process::exit(2);
        }
    }
    Ok(())
}
