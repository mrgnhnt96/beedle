mod audit;
mod js;
mod review;

use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "beedle",
    version,
    about = "Audits a built marketing site and reviews it in a lean headless Claude"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Check a built site for what a script can decide. Exits 1 if there are errors
    Audit(audit::Args),
    /// Run one review as its own headless claude session. Exits 1 if the review failed
    Review(review::Args),
}

fn main() -> ExitCode {
    let code = match Cli::parse().command {
        Command::Audit(args) => audit::run(args),
        Command::Review(args) => review::run(args),
    };
    ExitCode::from(code)
}
