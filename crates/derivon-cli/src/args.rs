use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

pub const DEFAULT_MAX_INPUT_BYTES: u64 = 256 * 1024 * 1024;
pub const DEFAULT_MAX_VALUE_BYTES: u64 = 64 * 1024 * 1024;
pub const DEFAULT_MAX_NODES: u64 = 200_000;
pub const DEFAULT_MAX_MILLIS: u64 = 10_000;

#[derive(Debug, Parser)]
#[command(
    name = "derivon",
    version,
    long_version = concat!(env!("CARGO_PKG_VERSION"), " (default graph schema: derivon.graph/v1)"),
    about = "Stateless operations on Derivon weighted directed B-hypergraphs",
    disable_help_subcommand = true
)]
pub struct Cli {
    #[arg(long, global = true)]
    pub input: Option<PathBuf>,
    #[arg(long, global = true)]
    pub pretty: bool,
    #[arg(long, global = true, default_value_t = DEFAULT_MAX_INPUT_BYTES, value_parser = clap::value_parser!(u64).range(1..))]
    pub max_input_bytes: u64,
    #[arg(long, global = true, default_value_t = DEFAULT_MAX_VALUE_BYTES, value_parser = clap::value_parser!(u64).range(1..))]
    pub max_value_bytes: u64,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Validate,
    Point(PointArgs),
    Hyperedge(HyperedgeArgs),
    Query(QueryArgs),
    Subgraph(SubgraphArgs),
    Apply(ApplyArgs),
}

#[derive(Debug, Args)]
pub struct PointArgs {
    #[command(subcommand)]
    pub command: PointCommand,
}

#[derive(Debug, Subcommand)]
pub enum PointCommand {
    List,
    Get {
        id: String,
    },
    Add {
        id: String,
        #[command(flatten)]
        data: OptionalJson,
    },
    Remove {
        id: String,
        #[arg(long)]
        cascade: bool,
        #[arg(long)]
        ignore_missing: bool,
    },
    Rename {
        id: String,
        new_id: String,
    },
    Data(DataArgs),
}

#[derive(Debug, Args)]
pub struct HyperedgeArgs {
    #[command(subcommand)]
    pub command: HyperedgeCommand,
}

#[derive(Debug, Subcommand)]
pub enum HyperedgeCommand {
    List,
    Get {
        id: String,
    },
    Add {
        id: String,
        #[arg(long = "tail")]
        tails: Vec<String>,
        #[arg(long)]
        head: String,
        #[arg(long)]
        weight: String,
        #[command(flatten)]
        data: OptionalJson,
    },
    Remove {
        id: String,
        #[arg(long)]
        ignore_missing: bool,
    },
    Rename {
        id: String,
        new_id: String,
    },
    Set(HyperedgeSetArgs),
    Data(DataArgs),
}

#[derive(Debug, Args)]
pub struct HyperedgeSetArgs {
    #[command(subcommand)]
    pub command: HyperedgeSetCommand,
}

#[derive(Debug, Subcommand)]
pub enum HyperedgeSetCommand {
    Tails {
        id: String,
        #[arg(long = "tail")]
        tails: Vec<String>,
    },
    Head {
        id: String,
        head: String,
    },
    Weight {
        id: String,
        weight: String,
    },
}

#[derive(Debug, Args)]
pub struct DataArgs {
    #[command(subcommand)]
    pub command: DataCommand,
}

#[derive(Debug, Subcommand)]
pub enum DataCommand {
    Get {
        id: String,
        pointer: Option<String>,
    },
    Set {
        id: String,
        pointer: Option<String>,
        #[command(flatten)]
        value: RequiredJson,
    },
    Remove {
        id: String,
        pointer: Option<String>,
        #[arg(long)]
        ignore_missing: bool,
    },
}

#[derive(Debug, Args)]
#[group(id = "optional_json", multiple = false)]
pub struct OptionalJson {
    #[arg(long, group = "optional_json")]
    pub data: Option<String>,
    #[arg(long, group = "optional_json")]
    pub data_file: Option<PathBuf>,
}

#[derive(Debug, Args)]
#[group(id = "required_json", required = true, multiple = false)]
pub struct RequiredJson {
    #[arg(long, group = "required_json")]
    pub value: Option<String>,
    #[arg(long, group = "required_json")]
    pub value_file: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct QueryArgs {
    #[command(subcommand)]
    pub command: QueryCommand,
}

#[derive(Debug, Subcommand)]
pub enum QueryCommand {
    Closure {
        #[arg(long = "start")]
        starts: Vec<String>,
    },
    Route(RouteArgs),
    Diagnose(TargetArgs),
}

#[derive(Debug, Args)]
pub struct RouteArgs {
    #[arg(long = "start")]
    pub starts: Vec<String>,
    #[arg(long = "target", required = true)]
    pub targets: Vec<String>,
    #[arg(long, default_value_t = DEFAULT_MAX_NODES)]
    pub max_nodes: u64,
    #[arg(long, default_value_t = DEFAULT_MAX_MILLIS)]
    pub max_millis: u64,
}

#[derive(Debug, Args)]
pub struct TargetArgs {
    #[arg(long = "start")]
    pub starts: Vec<String>,
    #[arg(long = "target", required = true)]
    pub targets: Vec<String>,
}

#[derive(Debug, Args)]
pub struct SubgraphArgs {
    #[command(subcommand)]
    pub command: SubgraphCommand,
}

#[derive(Debug, Subcommand)]
pub enum SubgraphCommand {
    Induced {
        #[arg(long = "point")]
        points: Vec<String>,
    },
    Reachable {
        #[arg(long = "start")]
        starts: Vec<String>,
    },
    Route(RouteArgs),
}

#[derive(Debug, Args)]
pub struct ApplyArgs {
    #[arg(long)]
    pub operations: PathBuf,
    #[arg(long, default_value_t = DEFAULT_MAX_VALUE_BYTES, value_parser = clap::value_parser!(u64).range(1..))]
    pub max_operations_bytes: u64,
}
