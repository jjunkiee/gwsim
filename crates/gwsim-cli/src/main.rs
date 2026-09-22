use clap::Parser;
use gwsim_cli::Cli;

fn main() {
    let cli = Cli::parse();

    match cli.command {
        // No subcommands exist yet; `--version` and `--help` are handled
        // by clap before we get here.
        None => {
            eprintln!("gwsim: no command given. Try `gwsim --help`.");
        }
        Some(command) => match command {},
    }
}
