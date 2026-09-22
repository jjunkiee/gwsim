use std::io::Write;
use std::process::ExitCode;

use clap::Parser;
use gwsim_cli::{Cli, Command, DataCommand, TemplateCommand};

fn main() -> ExitCode {
    let cli = Cli::parse();

    let code = match cli.command {
        // `--version` and `--help` are handled by clap before we get here.
        None => {
            eprintln!("gwsim: no command given. Try `gwsim --help`.");
            gwsim_cli::data::FAILED
        }
        Some(Command::Data(args)) => match args.command {
            DataCommand::Validate(args) => run(|out| gwsim_cli::data::validate(&args, out)),
            DataCommand::Describe(args) => run(|out| gwsim_cli::describe::run(&args, out)),
            DataCommand::Coverage(args) => run(|out| gwsim_cli::coverage::run(&args, out)),
            DataCommand::Info(args) => run(|out| gwsim_cli::info::run(&args, out)),
        },
        Some(Command::Template(args)) => match args.command {
            TemplateCommand::Decode(args) => run(|out| gwsim_cli::template::decode(&args, out)),
            TemplateCommand::Encode(args) => run(|out| gwsim_cli::template::encode(&args, out)),
        },
    };

    ExitCode::from(code as u8)
}

/// Runs a command against stdout, turning an I/O failure into an exit code.
fn run<F>(command: F) -> i32
where
    F: FnOnce(&mut std::io::StdoutLock<'_>) -> std::io::Result<i32>,
{
    let mut out = std::io::stdout().lock();
    match command(&mut out) {
        Ok(code) => code,
        Err(error) => {
            let _ = writeln!(std::io::stderr(), "gwsim: {error}");
            gwsim_cli::data::FAILED
        }
    }
}
