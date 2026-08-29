use std::io::Write;

use clap::error::ErrorKind;
use derivon_cli::args::Cli;
use derivon_cli::error::CliError;

fn main() {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) =>
        {
            print!("{error}");
            return;
        }
        Err(error) => exit_error(CliError::new(64, "invalid_arguments", error.to_string())),
    };

    let result = derivon_cli::read_graph(&cli)
        .and_then(|bytes| derivon_cli::run(&cli, &bytes))
        .and_then(|value| derivon_cli::serialize(&value, cli.pretty));

    match result {
        Ok(bytes) => {
            if let Err(error) = std::io::stdout().write_all(&bytes) {
                exit_error(CliError::new(74, "io", error.to_string()));
            }
        }
        Err(error) => exit_error(error),
    }
}

fn exit_error(error: CliError) -> ! {
    let bytes = derivon_cli::serialize(&error.json(), false).unwrap_or_else(|_| {
        b"{\"error\":{\"code\":\"internal\",\"message\":\"failed to serialize error\"}}\n".to_vec()
    });
    let _ = std::io::stderr().write_all(&bytes);
    std::process::exit(i32::from(error.exit));
}
