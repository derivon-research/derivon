use std::fs;
use std::io::{ErrorKind, Write};
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

fn process(args: &[&str], stdin: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_derivon"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Err(error) = child.stdin.take().unwrap().write_all(stdin.as_bytes()) {
        assert_eq!(error.kind(), ErrorKind::BrokenPipe);
    }
    child.wait_with_output().unwrap()
}

fn temp_path(label: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("derivon-cli-{label}-{unique}"))
}

fn assert_success(output: &Output) {
    assert_eq!(output.status.code(), Some(0));
    assert!(
        output.stderr.is_empty(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn error_code(output: &Output, exit: i32) -> String {
    assert_eq!(output.status.code(), Some(exit));
    assert!(output.stdout.is_empty());
    let value: Value = serde_json::from_slice(&output.stderr).unwrap();
    value["error"]["code"].as_str().unwrap().to_owned()
}

#[test]
fn root_help_has_complete_commands_options_and_lowercase_version_flag() {
    let short = process(&["-h"], "not graph JSON");
    let long = process(&["--help"], "not graph JSON");
    assert_success(&short);
    assert_success(&long);
    assert_eq!(short.stdout, long.stdout);

    let help = String::from_utf8(short.stdout).unwrap();
    for expected in [
        "validate   Validate and serialize a graph",
        "point      Manage points and point data",
        "hyperedge  Manage hyperedges and hyperedge data",
        "query      Run closure, route, and diagnosis queries",
        "subgraph   Extract induced, reachable, and route subgraphs",
        "apply      Apply an atomic batch of typed mutations",
        "--input <FILE>",
        "--pretty",
        "--max-input-bytes <N>",
        "--max-value-bytes <N>",
        "-v, --version",
        "-h, --help",
    ] {
        assert!(help.contains(expected), "missing help text: {expected}");
    }
    assert!(!help.contains("-V"));
}

#[test]
fn every_command_help_is_plain_text_success_without_reading_graph() {
    let commands: &[&[&str]] = &[
        &["validate", "--help"],
        &["point", "--help"],
        &["point", "list", "--help"],
        &["point", "get", "--help"],
        &["point", "add", "--help"],
        &["point", "remove", "--help"],
        &["point", "rename", "--help"],
        &["point", "data", "get", "--help"],
        &["point", "data", "set", "--help"],
        &["point", "data", "remove", "--help"],
        &["hyperedge", "--help"],
        &["hyperedge", "list", "--help"],
        &["hyperedge", "get", "--help"],
        &["hyperedge", "add", "--help"],
        &["hyperedge", "remove", "--help"],
        &["hyperedge", "rename", "--help"],
        &["hyperedge", "set", "tails", "--help"],
        &["hyperedge", "set", "head", "--help"],
        &["hyperedge", "set", "weight", "--help"],
        &["hyperedge", "data", "get", "--help"],
        &["hyperedge", "data", "set", "--help"],
        &["hyperedge", "data", "remove", "--help"],
        &["query", "--help"],
        &["query", "closure", "--help"],
        &["query", "route", "--help"],
        &["query", "diagnose", "--help"],
        &["subgraph", "--help"],
        &["subgraph", "induced", "--help"],
        &["subgraph", "reachable", "--help"],
        &["subgraph", "route", "--help"],
        &["apply", "--help"],
    ];
    for args in commands {
        let output = process(args, "not graph JSON");
        assert_success(&output);
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains("Usage:"), "missing usage for {args:?}");
        assert!(stdout.contains("--help"), "missing help flag for {args:?}");
    }
}

#[test]
fn version_flags_are_compatible_and_do_not_read_graph() {
    let short_expected = format!("derivon {}\n", env!("CARGO_PKG_VERSION"));
    for flag in ["-v", "-V"] {
        let output = process(&[flag], "not graph JSON");
        assert_success(&output);
        assert_eq!(String::from_utf8(output.stdout).unwrap(), short_expected);
    }

    let output = process(&["--version"], "not graph JSON");
    assert_success(&output);
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!(
            "derivon {} (default graph schema: derivon.graph/v1)\n",
            env!("CARGO_PKG_VERSION")
        )
    );
}

#[test]
fn malformed_command_lines_are_structured_exit_64_errors() {
    let cases: &[&[&str]] = &[
        &[],
        &["--unknown"],
        &["point", "get"],
        &["--max-input-bytes", "0", "validate"],
        &["--max-value-bytes", "0", "validate"],
        &["query", "route", "--start", "A"],
        &[
            "apply",
            "--operations",
            "ops.json",
            "--max-operations-bytes",
            "0",
        ],
        &["point", "data", "set", "A"],
        &["point", "add", "A", "--data", "--unknown"],
        &[
            "hyperedge",
            "add",
            "h",
            "--head",
            "A",
            "--weight",
            "--unknown",
        ],
        &[
            "point",
            "add",
            "A",
            "--data",
            "{}",
            "--data-file",
            "value.json",
        ],
    ];
    for args in cases {
        let output = process(args, r#"{"points":[],"hyperedges":[]}"#);
        assert_eq!(
            error_code(&output, 64),
            "invalid_arguments",
            "args: {args:?}"
        );
    }
}

#[test]
fn global_options_work_before_and_after_subcommands() {
    let graph = r#"{"points":[],"hyperedges":[]}"#;
    let before = process(&["--pretty", "validate"], graph);
    let after = process(&["validate", "--pretty"], graph);
    assert_success(&before);
    assert_success(&after);
    assert_eq!(before.stdout, after.stdout);
    assert_eq!(
        String::from_utf8(before.stdout).unwrap(),
        "{\n  \"hyperedges\": [],\n  \"points\": []\n}\n"
    );

    let compact = process(&["validate"], graph);
    assert_success(&compact);
    assert_eq!(compact.stdout, b"{\"hyperedges\":[],\"points\":[]}\n");
}

#[test]
fn input_file_takes_precedence_over_stdin() {
    let path = temp_path("graph.json");
    fs::write(&path, r#"{"points":[{"id":"A"}],"hyperedges":[]}"#).unwrap();
    let output = process(
        &["--input", path.to_str().unwrap(), "point", "get", "A"],
        "not graph JSON",
    );
    fs::remove_file(path).unwrap();
    assert_success(&output);
    assert_eq!(output.stdout, b"{\"id\":\"A\"}\n");
}

#[test]
fn file_and_input_limit_errors_use_documented_codes() {
    let missing = temp_path("missing.json");
    let output = process(&["--input", missing.to_str().unwrap(), "validate"], "");
    assert_eq!(error_code(&output, 66), "file_not_found");

    let directory = temp_path("directory");
    fs::create_dir(&directory).unwrap();
    let output = process(&["--input", directory.to_str().unwrap(), "validate"], "");
    fs::remove_dir(directory).unwrap();
    assert_eq!(error_code(&output, 66), "file_unreadable");

    let output = process(&["--max-input-bytes", "1", "validate"], "{}");
    assert_eq!(error_code(&output, 65), "input_limit_exceeded");

    let output = process(
        &[
            "point",
            "add",
            "A",
            "--data",
            "{}",
            "--max-value-bytes",
            "1",
        ],
        r#"{"points":[],"hyperedges":[]}"#,
    );
    assert_eq!(error_code(&output, 65), "input_limit_exceeded");
}

#[cfg(unix)]
#[test]
fn closed_stdout_is_a_structured_exit_74_io_error() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_derivon"))
        .arg("validate")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    child
        .stdin
        .take()
        .unwrap()
        .write_all(br#"{"points":[],"hyperedges":[]}"#)
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(error_code(&output, 74), "io");
}
