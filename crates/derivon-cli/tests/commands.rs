use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

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
    std::env::temp_dir().join(format!("derivon-commands-{label}-{unique}.json"))
}

#[test]
fn point_crud_data_and_no_op_contracts() {
    let graph = r#"{
      "points":[{"id":"A"},{"id":"B","data":{"a/b":{"~":7}}}],
      "hyperedges":[{"id":"h","weight":1,"tails":["A"],"head":"B"}]
    }"#;

    let listed = execute(&["point", "list"], graph).unwrap();
    assert_eq!(listed.as_array().unwrap().len(), 2);
    assert_eq!(listed[0], json!({"id":"A"}));
    assert_eq!(execute(&["point", "get", "B"], graph).unwrap()["id"], "B");

    let added = execute(&["point", "add", "C"], graph).unwrap();
    assert_eq!(added["points"][2], json!({"id":"C","data":{}}));
    let added = execute(&["point", "add", "C", "--data", "-1"], graph).unwrap();
    assert_eq!(added["points"][2]["data"], -1);

    let data_path = temp_path("point-data");
    fs::write(&data_path, r#"{"n":123456789012345678901234567890}"#).unwrap();
    let added = execute(
        &[
            "point",
            "add",
            "C",
            "--data-file",
            data_path.to_str().unwrap(),
        ],
        graph,
    )
    .unwrap();
    fs::remove_file(data_path).unwrap();
    assert_eq!(
        added["points"][2]["data"]["n"].to_string(),
        "123456789012345678901234567890"
    );

    assert_eq!(
        execute(&["point", "data", "get", "A"], graph).unwrap(),
        Value::Null
    );
    assert_eq!(
        execute(&["point", "data", "get", "B", "/a~1b/~0"], graph).unwrap(),
        7
    );

    let removed = execute(&["point", "data", "remove", "A", "--ignore-missing"], graph).unwrap();
    assert_eq!(removed, execute(&["validate"], graph).unwrap());

    let renamed = execute(&["point", "rename", "A", "X"], graph).unwrap();
    assert_eq!(renamed["points"][0]["id"], "X");
    assert_eq!(renamed["hyperedges"][0]["tails"], json!(["X"]));

    let error = execute(&["point", "rename", "A", "A"], graph).unwrap_err();
    assert_eq!(error.code, "id_conflict");

    let ignored = execute(
        &[
            "point",
            "remove",
            "missing",
            "--cascade",
            "--ignore-missing",
        ],
        graph,
    )
    .unwrap();
    assert_eq!(ignored, execute(&["validate"], graph).unwrap());
}

#[test]
fn point_data_set_remove_and_pointer_failures() {
    let graph = r#"{"points":[{"id":"A","data":{"items":[1,2]}}],"hyperedges":[]}"#;
    let value_path = temp_path("point-value");
    fs::write(&value_path, r#"{"fresh":true}"#).unwrap();
    let replaced = execute(
        &[
            "point",
            "data",
            "set",
            "A",
            "--value-file",
            value_path.to_str().unwrap(),
        ],
        graph,
    )
    .unwrap();
    fs::remove_file(value_path).unwrap();
    assert_eq!(replaced["points"][0]["data"], json!({"fresh":true}));

    let removed = execute(&["point", "data", "remove", "A"], graph).unwrap();
    assert!(removed["points"][0].get("data").is_none());

    let negative = execute(&["point", "data", "set", "A", "--value", "-2.5"], graph).unwrap();
    assert_eq!(negative["points"][0]["data"], -2.5);

    for (pointer, code) in [
        ("items/0", "invalid_pointer"),
        ("/items/01", "invalid_pointer"),
        ("/items/-", "invalid_pointer"),
        ("/missing", "pointer_not_found"),
        ("/~2", "invalid_pointer"),
    ] {
        let error = execute(&["point", "data", "remove", "A", pointer], graph).unwrap_err();
        assert_eq!(error.code, code, "pointer: {pointer}");
    }
}

#[test]
fn hyperedge_crud_set_data_and_weight_contracts() {
    let graph = r#"{
      "points":[{"id":"A"},{"id":"B"},{"id":"C"}],
      "hyperedges":[{"id":"h","weight":1,"tails":["A"],"head":"B","data":{"x":1}}]
    }"#;

    assert_eq!(
        execute(&["hyperedge", "list"], graph).unwrap()[0]["id"],
        "h"
    );
    assert_eq!(
        execute(&["hyperedge", "get", "h"], graph).unwrap()["head"],
        "B"
    );

    let added = execute(
        &[
            "hyperedge",
            "add",
            "entry",
            "--head",
            "A",
            "--weight",
            "1e1",
        ],
        graph,
    )
    .unwrap();
    assert_eq!(
        added["hyperedges"][1],
        json!({"id":"entry","weight":10,"tails":[],"head":"A","data":{}})
    );

    let tails = execute(
        &[
            "hyperedge",
            "set",
            "tails",
            "h",
            "--tail",
            "C",
            "--tail",
            "A",
        ],
        graph,
    )
    .unwrap();
    assert_eq!(tails["hyperedges"][0]["tails"], json!(["C", "A"]));

    let head = execute(&["hyperedge", "set", "head", "h", "C"], graph).unwrap();
    assert_eq!(head["hyperedges"][0]["head"], "C");

    let weight = execute(&["hyperedge", "set", "weight", "h", "-0.0"], graph).unwrap();
    assert_eq!(weight["hyperedges"][0]["weight"], 0);

    assert_eq!(
        execute(&["hyperedge", "data", "get", "h", "/x"], graph).unwrap(),
        1
    );
    let data = execute(
        &["hyperedge", "data", "set", "h", "/new", "--value", "true"],
        graph,
    )
    .unwrap();
    assert_eq!(data["hyperedges"][0]["data"], json!({"x":1,"new":true}));

    let renamed = execute(&["hyperedge", "rename", "h", "renamed"], graph).unwrap();
    assert_eq!(renamed["hyperedges"][0]["id"], "renamed");
    let error = execute(&["hyperedge", "rename", "h", "h"], graph).unwrap_err();
    assert_eq!(error.code, "id_conflict");

    let ignored = execute(
        &["hyperedge", "remove", "missing", "--ignore-missing"],
        graph,
    )
    .unwrap();
    assert_eq!(ignored, execute(&["validate"], graph).unwrap());
    assert!(
        execute(&["hyperedge", "remove", "h"], graph).unwrap()["hyperedges"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn hyperedge_references_tails_and_weight_boundaries_are_strict() {
    let graph = r#"{"points":[{"id":"A"},{"id":"B"}],"hyperedges":[]}"#;
    let max = execute(
        &[
            "hyperedge",
            "add",
            "h",
            "--tail",
            "A",
            "--head",
            "B",
            "--weight",
            "900719925474099.1",
        ],
        graph,
    )
    .unwrap();
    assert_eq!(
        max["hyperedges"][0]["weight"].to_string(),
        "900719925474099.1"
    );

    for weight in ["-0.1", "1.25", "900719925474099.2", "null", "NaN"] {
        let error = execute(
            &["hyperedge", "add", "h", "--head", "A", "--weight", weight],
            graph,
        )
        .unwrap_err();
        assert_eq!(error.code, "invalid_weight", "weight: {weight}");
    }

    let error = execute(
        &[
            "hyperedge",
            "add",
            "h",
            "--tail",
            "A",
            "--tail",
            "A",
            "--head",
            "B",
            "--weight",
            "1",
        ],
        graph,
    )
    .unwrap_err();
    assert_eq!(error.code, "invalid_arguments");

    let error = execute(
        &[
            "hyperedge",
            "add",
            "h",
            "--head",
            "missing",
            "--weight",
            "1",
        ],
        graph,
    )
    .unwrap_err();
    assert_eq!(error.code, "unknown_point");
}

#[test]
fn query_and_all_subgraph_variants_return_documented_shapes() {
    let graph = r#"{
      "schema":"derivon.graph/v1",
      "points":[{"id":"A"},{"id":"B"},{"id":"C"}],
      "hyperedges":[{"id":"ab","weight":1,"tails":["A"],"head":"B"}]
    }"#;

    let closure = execute(&["query", "closure", "--start", "A"], graph).unwrap();
    assert_eq!(closure, json!({"startPointIds":["A"],"pointIds":["A","B"]}));

    let diagnosis = execute(
        &[
            "query", "diagnose", "--start", "A", "--target", "B", "--target", "C",
        ],
        graph,
    )
    .unwrap();
    assert_eq!(diagnosis["reachable"], false);
    assert_eq!(diagnosis["targetDiagnoses"].as_array().unwrap().len(), 2);

    let reachable = execute(&["subgraph", "reachable", "--start", "A"], graph).unwrap();
    assert_eq!(reachable["graph"]["schema"], "derivon.graph/v1");
    assert_eq!(reachable["selection"]["pointIds"], json!(["A", "B"]));
    assert_eq!(reachable["graph"]["hyperedges"][0]["id"], "ab");

    let route = execute(
        &["subgraph", "route", "--start", "A", "--target", "B"],
        graph,
    )
    .unwrap();
    assert_eq!(route["selection"]["reachable"], true);
    assert_eq!(route["graph"]["hyperedges"][0]["id"], "ab");

    let unreachable = execute(
        &["subgraph", "route", "--start", "A", "--target", "C"],
        graph,
    )
    .unwrap();
    assert_eq!(unreachable["selection"]["reachable"], false);
    assert_eq!(unreachable["graph"], Value::Null);

    let empty = execute(&["subgraph", "induced"], graph).unwrap();
    assert_eq!(empty["graph"]["points"], json!([]));
    assert_eq!(empty["graph"]["hyperedges"], json!([]));
    assert_eq!(empty["graph"]["schema"], "derivon.graph/v1");
}

#[test]
fn apply_supports_every_documented_mutation_type_in_order() {
    let graph = r#"{
      "points":[{"id":"A"},{"id":"B"}],
      "hyperedges":[{"id":"h","weight":1,"tails":["A"],"head":"B"}]
    }"#;
    let path = temp_path("all-operations");
    fs::write(
        &path,
        r#"[
          {"op":"point.add","id":"C","data":null},
          {"op":"point.rename","id":"C","newId":"D"},
          {"op":"point.data.set","id":"D","value":{"items":[1]}},
          {"op":"point.data.set","id":"D","pointer":"/items/-","value":2},
          {"op":"hyperedge.add","id":"e","tails":["D"],"head":"B","weight":1,"data":{"old":true}},
          {"op":"hyperedge.rename","id":"e","newId":"f"},
          {"op":"hyperedge.set.tails","id":"f","tails":["A","D"]},
          {"op":"hyperedge.set.head","id":"f","head":"A"},
          {"op":"hyperedge.set.weight","id":"f","weight":2.5},
          {"op":"hyperedge.data.set","id":"f","value":{"remove":true,"keep":1}},
          {"op":"hyperedge.data.remove","id":"f","pointer":"/remove"},
          {"op":"hyperedge.remove","id":"h"},
          {"op":"point.remove","id":"B"},
          {"op":"point.data.remove","id":"D"}
        ]"#,
    )
    .unwrap();
    let output = execute(&["apply", "--operations", path.to_str().unwrap()], graph).unwrap();
    fs::remove_file(path).unwrap();

    assert_eq!(output["points"], json!([{"id":"A"},{"id":"D"}]));
    assert_eq!(
        output["hyperedges"],
        json!([{
            "id":"f",
            "weight":2.5,
            "tails":["A","D"],
            "head":"A",
            "data":{"keep":1}
        }])
    );
}

#[test]
fn apply_error_codes_and_paths_cover_typed_operation_fields() {
    let graph = r#"{"points":[{"id":"A"}],"hyperedges":[]}"#;
    let cases = [
        ("42", "invalid_type", "/operations/1"),
        (r#"{}"#, "missing_field", "/operations/1/op"),
        (r#"{"op":"unknown"}"#, "unknown_field", "/operations/1/op"),
        (
            r#"{"op":"point.add","id":"B","extra":true}"#,
            "unknown_field",
            "/operations/1/extra",
        ),
        (
            r#"{"op":"point.remove","id":"A","ignoreMissing":1}"#,
            "invalid_type",
            "/operations/1/ignoreMissing",
        ),
        (
            r#"{"op":"point.rename","id":"A","newId":1}"#,
            "invalid_type",
            "/operations/1/newId",
        ),
        (
            r#"{"op":"point.data.set","id":"A","pointer":1,"value":0}"#,
            "invalid_type",
            "/operations/1/pointer",
        ),
        (
            r#"{"op":"hyperedge.add","id":"h","tails":["A",1],"head":"A","weight":1}"#,
            "invalid_type",
            "/operations/1/tails/1",
        ),
        (
            r#"{"op":"hyperedge.add","id":"h","head":"A"}"#,
            "missing_field",
            "/operations/1/weight",
        ),
        (
            r#"{"op":"hyperedge.add","id":"h","tails":["A","A"],"head":"A","weight":1}"#,
            "duplicate_tail",
            "/operations/1/tails/1",
        ),
        (
            r#"{"op":"hyperedge.add","id":"h","head":"missing","weight":1}"#,
            "unknown_point",
            "/operations/1/head",
        ),
    ];

    for (index, (operation, code, path)) in cases.into_iter().enumerate() {
        let operations_path = temp_path(&format!("invalid-operation-{index}"));
        fs::write(
            &operations_path,
            format!(r#"[{{"op":"point.add","id":"B"}},{operation}]"#),
        )
        .unwrap();
        let error = execute(
            &["apply", "--operations", operations_path.to_str().unwrap()],
            graph,
        )
        .unwrap_err();
        fs::remove_file(operations_path).unwrap();
        assert_eq!(error.code, "invalid_operations");
        assert_eq!(error.issues.len(), 1);
        assert_eq!(error.issues[0].code, code, "case {index}");
        assert_eq!(error.issues[0].path, path, "case {index}");
    }
}

#[test]
fn empty_apply_is_a_no_op_and_operation_limits_are_enforced() {
    let path = temp_path("empty-operations");
    fs::write(&path, "[]").unwrap();
    let output = execute(
        &["apply", "--operations", path.to_str().unwrap()],
        empty_graph(),
    )
    .unwrap();
    assert_eq!(output, json!({"points":[],"hyperedges":[]}));

    let error = execute(
        &[
            "apply",
            "--operations",
            path.to_str().unwrap(),
            "--max-operations-bytes",
            "1",
        ],
        empty_graph(),
    )
    .unwrap_err();
    fs::remove_file(path).unwrap();
    assert_eq!(error.code, "input_limit_exceeded");
}
