use std::fs;
use std::io::{ErrorKind, Write};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use clap::Parser;
use derivon_cli::args::Cli;
use serde_json::{Value, json};

fn execute(args: &[&str], graph: &str) -> Result<Value, derivon_cli::error::CliError> {
    let cli = Cli::try_parse_from(std::iter::once("derivon").chain(args.iter().copied())).unwrap();
    derivon_cli::run(&cli, graph.as_bytes())
}

fn empty_graph() -> &'static str {
    r#"{"points":[],"hyperedges":[]}"#
}

fn temp_path(label: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("derivon-{label}-{unique}.json"))
}

fn process(args: &[&str], stdin: &str) -> std::process::Output {
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

#[test]
fn validate_preserves_schema_presence_and_arbitrary_precision_data() {
    let input = r#"{"schema":"derivon.graph/v1","points":[{"id":"A","data":{"n":123456789012345678901234567890}}],"hyperedges":[]}"#;
    let output = execute(&["validate"], input).unwrap();
    assert_eq!(output["schema"], "derivon.graph/v1");
    assert_eq!(
        output["points"][0]["data"]["n"].to_string(),
        "123456789012345678901234567890"
    );

    let output = execute(&["validate"], empty_graph()).unwrap();
    assert!(output.get("schema").is_none());
}

#[test]
fn strict_ids_and_global_namespace_are_enforced() {
    let error = execute(&["point", "add", "bad/id"], empty_graph()).unwrap_err();
    assert_eq!(error.code, "invalid_id");

    let graph = r#"{"points":[{"id":"A"}],"hyperedges":[]}"#;
    let error = execute(
        &["hyperedge", "add", "A", "--head", "A", "--weight", "1"],
        graph,
    )
    .unwrap_err();
    assert_eq!(error.code, "id_conflict");
}

#[test]
fn point_rename_rewrites_references_and_cascade_is_explicit() {
    let graph = r#"{"points":[{"id":"A"},{"id":"B"}],"hyperedges":[{"id":"h","weight":1,"tails":["A"],"head":"B"}]}"#;
    let renamed = execute(&["point", "rename", "A", "X"], graph).unwrap();
    assert_eq!(renamed["hyperedges"][0]["tails"], json!(["X"]));

    let error = execute(&["point", "remove", "A"], graph).unwrap_err();
    assert_eq!(error.code, "point_referenced");

    let removed = execute(&["point", "remove", "A", "--cascade"], graph).unwrap();
    assert_eq!(removed["points"], json!([{"id":"B"}]));
    assert_eq!(removed["hyperedges"], json!([]));
}

#[test]
fn null_data_never_panics_or_promotes_for_subpaths() {
    let graph = r#"{"points":[{"id":"A","data":null}],"hyperedges":[]}"#;
    let error = execute(
        &["point", "data", "set", "A", "/label", "--value", "\"A\""],
        graph,
    )
    .unwrap_err();
    assert_eq!(error.code, "pointer_type_mismatch");

    let replaced = execute(
        &["point", "data", "set", "A", "--value", "{\"label\":\"A\"}"],
        graph,
    )
    .unwrap();
    assert_eq!(replaced["points"][0]["data"]["label"], "A");
}

#[test]
fn data_set_and_remove_follow_pointer_rules() {
    let graph = r#"{"points":[{"id":"A","data":{"items":[1,2],"old":true}}],"hyperedges":[]}"#;
    let appended = execute(
        &["point", "data", "set", "A", "/items/-", "--value", "3"],
        graph,
    )
    .unwrap();
    assert_eq!(appended["points"][0]["data"]["items"], json!([1, 2, 3]));

    let removed = execute(&["point", "data", "remove", "A", "/items/0"], graph).unwrap();
    assert_eq!(removed["points"][0]["data"]["items"], json!([2]));
}

#[test]
fn empty_tail_closure_and_route_use_core_semantics() {
    let graph = r#"{"points":[{"id":"A"},{"id":"B"}],"hyperedges":[{"id":"entry","weight":1,"tails":[],"head":"A"},{"id":"ab","weight":2.5,"tails":["A"],"head":"B"}]}"#;
    let closure = execute(&["query", "closure"], graph).unwrap();
    assert_eq!(closure["pointIds"], json!(["A", "B"]));

    let route = execute(&["query", "route", "--target", "B"], graph).unwrap();
    assert_eq!(route["reachable"], true);
    assert_eq!(route["cost"], 3.5);
    assert_eq!(route["executableOrder"], json!(["entry", "ab"]));
}

#[test]
fn unreachable_route_is_a_success_union_with_diagnosis() {
    let graph = r#"{"points":[{"id":"A"},{"id":"B"}],"hyperedges":[]}"#;
    let route = execute(&["query", "route", "--start", "A", "--target", "B"], graph).unwrap();
    assert_eq!(route["reachable"], false);
    assert!(route.get("cost").is_none());
    assert_eq!(route["targetDiagnoses"][0]["targetPointId"], "B");
}

#[test]
fn subgraphs_return_envelopes_and_preserve_order() {
    let graph = r#"{"points":[{"id":"B"},{"id":"A"},{"id":"X"}],"hyperedges":[{"id":"h","weight":1,"tails":["A"],"head":"B"}]}"#;
    let result = execute(
        &["subgraph", "induced", "--point", "A", "--point", "B"],
        graph,
    )
    .unwrap();
    assert_eq!(result["graph"]["points"], json!([{"id":"B"},{"id":"A"}]));
    assert_eq!(result["graph"]["hyperedges"][0]["id"], "h");
}

#[test]
fn apply_executes_typed_operations_in_order() {
    let path = temp_path("operations");
    fs::write(
        &path,
        r#"[
          {"op":"point.add","id":"A"},
          {"op":"point.add","id":"B","data":null},
          {"op":"hyperedge.add","id":"h","tails":["A"],"head":"B","weight":1.5}
        ]"#,
    )
    .unwrap();
    let output = execute(
        &["apply", "--operations", path.to_str().unwrap()],
        empty_graph(),
    )
    .unwrap();
    fs::remove_file(path).unwrap();
    assert_eq!(output["points"].as_array().unwrap().len(), 2);
    assert_eq!(output["points"][1]["data"], Value::Null);
    assert_eq!(output["hyperedges"][0]["weight"], 1.5);
}

#[test]
fn unknown_schema_and_duplicate_keys_have_specific_codes() {
    let error = execute(
        &["validate"],
        r#"{"schema":"derivon.graph/v2","points":[],"hyperedges":[]}"#,
    )
    .unwrap_err();
    assert_eq!(error.code, "unsupported_schema");

    let error = execute(
        &["validate"],
        r#"{"points":[],"points":[],"hyperedges":[]}"#,
    )
    .unwrap_err();
    assert_eq!(error.code, "duplicate_key");
}

#[test]
fn process_errors_are_structured_and_use_stable_exit_codes() {
    let output = process(&["point", "get"], empty_graph());
    assert_eq!(output.status.code(), Some(64));
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["error"]["code"], "invalid_arguments");

    let output = process(&["validate"], r#"{"points":[],"hyperedges":[],"extra":1}"#);
    assert_eq!(output.status.code(), Some(65));
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["error"]["code"], "invalid_graph");
}

#[test]
fn failed_apply_writes_no_stdout() {
    let path = temp_path("atomic-operations");
    fs::write(
        &path,
        r#"[{"op":"point.add","id":"A"},{"op":"hyperedge.add","id":"h","head":"missing","weight":1}]"#,
    )
    .unwrap();
    let output = process(
        &["apply", "--operations", path.to_str().unwrap()],
        empty_graph(),
    );
    fs::remove_file(path).unwrap();
    assert_eq!(output.status.code(), Some(65));
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["error"]["code"], "invalid_operations");
    assert_eq!(error["error"]["issues"][0]["code"], "unknown_point");
    assert_eq!(error["error"]["issues"][0]["path"], "/operations/1/head");
}

#[test]
fn apply_reports_typed_field_paths() {
    let path = temp_path("invalid-fields");
    fs::write(&path, r#"[{"op":"point.add","id":42}]"#).unwrap();
    let error = execute(
        &["apply", "--operations", path.to_str().unwrap()],
        empty_graph(),
    )
    .unwrap_err();
    fs::remove_file(path).unwrap();
    assert_eq!(error.code, "invalid_operations");
    assert_eq!(error.issues[0].code, "invalid_type");
    assert_eq!(error.issues[0].path, "/operations/0/id");
}

#[test]
fn generated_five_thousand_point_graph_is_a_validation_smoke_test() {
    let points: Vec<_> = (0..5_000)
        .map(|index| json!({ "id": format!("p-{index}") }))
        .collect();
    let hyperedges: Vec<_> = (1..5_000)
        .map(|index| {
            json!({
                "id": format!("h-{index}"),
                "weight": 1,
                "tails": [format!("p-{}", index - 1)],
                "head": format!("p-{index}")
            })
        })
        .collect();
    let graph = serde_json::to_string(&json!({
        "points": points,
        "hyperedges": hyperedges
    }))
    .unwrap();
    let output = execute(&["validate"], &graph).unwrap();
    assert_eq!(output["points"].as_array().unwrap().len(), 5_000);
    assert_eq!(output["hyperedges"].as_array().unwrap().len(), 4_999);
}
