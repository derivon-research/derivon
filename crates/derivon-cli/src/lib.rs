pub mod args;
mod engine;
pub mod error;
mod protocol;

use args::Cli;
use error::CliError;
use serde_json::Value;

pub use protocol::GRAPH_SCHEMA;

pub fn run(cli: &Cli, graph_bytes: &[u8]) -> Result<Value, CliError> {
    if graph_bytes.len() as u64 > cli.max_input_bytes {
        return Err(CliError::data(
            "input_limit_exceeded",
            "graph input exceeds configured byte limit",
        )
        .with_detail("limit", cli.max_input_bytes));
    }
    let mut graph = protocol::parse_graph(graph_bytes)?;
    engine::execute(&cli.command, &mut graph, cli.max_value_bytes)
}

pub fn read_graph(cli: &Cli) -> Result<Vec<u8>, CliError> {
    if let Some(path) = &cli.input {
        engine::read_limited(path, cli.max_input_bytes)
    } else {
        use std::io::Read;
        let mut bytes = Vec::new();
        std::io::stdin()
            .take(cli.max_input_bytes.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|error| CliError::new(74, "io", error.to_string()))?;
        if bytes.len() as u64 > cli.max_input_bytes {
            return Err(CliError::data(
                "input_limit_exceeded",
                "graph input exceeds configured byte limit",
            )
            .with_detail("limit", cli.max_input_bytes));
        }
        Ok(bytes)
    }
}

pub fn serialize(value: &Value, pretty: bool) -> Result<Vec<u8>, CliError> {
    let mut bytes = if pretty {
        serde_json::to_vec_pretty(value)
    } else {
        serde_json::to_vec(value)
    }
    .map_err(|error| CliError::new(70, "internal", error.to_string()))?;
    bytes.push(b'\n');
    Ok(bytes)
}
