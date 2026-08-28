use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::str::FromStr;

use derivon_core::{
    Budget, Cost, Graph, PointId, PointSet, SolveError, blocking_frontier, closure,
    executable_order, solve_many,
};
use serde_json::{Map, Value, json};

use crate::args::*;
use crate::error::{CliError, Issue};
use crate::protocol::{
    DataField, GraphDoc, HyperedgeDoc, PointDoc, graph_from_value, parse_json, parse_weight,
    sorted_ids, valid_id, weight_value,
};

pub fn execute(
    command: &Command,
    graph: &mut GraphDoc,
    max_value_bytes: u64,
) -> Result<Value, CliError> {
    match command {
        Command::Validate => Ok(graph.to_value()),
        Command::Point(args) => point_command(graph, &args.command, max_value_bytes),
        Command::Hyperedge(args) => hyperedge_command(graph, &args.command, max_value_bytes),
        Command::Query(args) => query_command(graph, &args.command),
        Command::Subgraph(args) => subgraph_command(graph, &args.command),
        Command::Apply(args) => apply(graph, args),
    }
}

fn point_command(
    graph: &mut GraphDoc,
    command: &PointCommand,
    limit: u64,
) -> Result<Value, CliError> {
    match command {
        PointCommand::List => Ok(Value::Array(
            graph.points.iter().map(PointDoc::to_value).collect(),
        )),
        PointCommand::Get { id } => Ok(point(graph, id)?.to_value()),
        PointCommand::Add { id, data } => {
            ensure_new_id(graph, id)?;
            graph.points.push(PointDoc {
                id: id.clone(),
                data: resolve_optional_json(data, limit)?,
            });
            Ok(graph.to_value())
        }
        PointCommand::Remove {
            id,
            cascade,
            ignore_missing,
        } => {
            let Some(index) = graph.point_index(id) else {
                return ignored_or_unknown(*ignore_missing, "unknown_point", id, graph);
            };
            let referenced = graph
                .hyperedges
                .iter()
                .any(|edge| edge.head == *id || edge.tails.iter().any(|tail| tail == id));
            if referenced && !cascade {
                return Err(CliError::data(
                    "point_referenced",
                    format!("point `{id}` is referenced"),
                )
                .with_detail("id", id.clone()));
            }
            graph.points.remove(index);
            if *cascade {
                graph
                    .hyperedges
                    .retain(|edge| edge.head != *id && !edge.tails.iter().any(|tail| tail == id));
            }
            Ok(graph.to_value())
        }
        PointCommand::Rename { id, new_id } => {
            ensure_valid_id(new_id)?;
            let index = graph.point_index(id).ok_or_else(|| unknown_point(id))?;
            if graph.contains_id(new_id) {
                return Err(id_conflict(new_id));
            }
            graph.points[index].id = new_id.clone();
            for edge in &mut graph.hyperedges {
                if edge.head == *id {
                    edge.head = new_id.clone();
                }
                for tail in &mut edge.tails {
                    if tail == id {
                        *tail = new_id.clone();
                    }
                }
            }
            Ok(graph.to_value())
        }
        PointCommand::Data(args) => data_command(graph, EntityKind::Point, &args.command, limit),
    }
}

fn hyperedge_command(
    graph: &mut GraphDoc,
    command: &HyperedgeCommand,
    limit: u64,
) -> Result<Value, CliError> {
    match command {
        HyperedgeCommand::List => Ok(Value::Array(
            graph
                .hyperedges
                .iter()
                .map(HyperedgeDoc::to_value)
                .collect(),
        )),
        HyperedgeCommand::Get { id } => Ok(edge(graph, id)?.to_value()),
        HyperedgeCommand::Add {
            id,
            tails,
            head,
            weight,
            data,
        } => {
            ensure_new_id(graph, id)?;
            ensure_point_refs(graph, tails, head)?;
            ensure_unique(tails, "tail")?;
            graph.hyperedges.push(HyperedgeDoc {
                id: id.clone(),
                weight_units: parse_weight_text(weight)?,
                tails: tails.clone(),
                head: head.clone(),
                data: resolve_optional_json(data, limit)?,
            });
            Ok(graph.to_value())
        }
        HyperedgeCommand::Remove { id, ignore_missing } => {
            let Some(index) = graph.edge_index(id) else {
                return ignored_or_unknown(*ignore_missing, "unknown_hyperedge", id, graph);
            };
            graph.hyperedges.remove(index);
            Ok(graph.to_value())
        }
        HyperedgeCommand::Rename { id, new_id } => {
            ensure_valid_id(new_id)?;
            let index = graph.edge_index(id).ok_or_else(|| unknown_edge(id))?;
            if graph.contains_id(new_id) {
                return Err(id_conflict(new_id));
            }
            graph.hyperedges[index].id = new_id.clone();
            Ok(graph.to_value())
        }
        HyperedgeCommand::Set(args) => {
            match &args.command {
                HyperedgeSetCommand::Tails { id, tails } => {
                    ensure_unique(tails, "tail")?;
                    for tail in tails {
                        if graph.point_index(tail).is_none() {
                            return Err(unknown_point(tail));
                        }
                    }
                    edge_mut(graph, id)?.tails = tails.clone();
                }
                HyperedgeSetCommand::Head { id, head } => {
                    if graph.point_index(head).is_none() {
                        return Err(unknown_point(head));
                    }
                    edge_mut(graph, id)?.head = head.clone();
                }
                HyperedgeSetCommand::Weight { id, weight } => {
                    let units = parse_weight_text(weight)?;
                    edge_mut(graph, id)?.weight_units = units;
                }
            }
            Ok(graph.to_value())
        }
        HyperedgeCommand::Data(args) => {
            data_command(graph, EntityKind::Hyperedge, &args.command, limit)
        }
    }
}

#[derive(Clone, Copy)]
enum EntityKind {
    Point,
    Hyperedge,
}

fn data_command(
    graph: &mut GraphDoc,
    kind: EntityKind,
    command: &DataCommand,
    limit: u64,
) -> Result<Value, CliError> {
    match command {
        DataCommand::Get { id, pointer } => {
            let data = entity_data(graph, kind, id)?;
            match (data, pointer.as_deref().unwrap_or("")) {
                (DataField::Missing, "") => Ok(Value::Null),
                (DataField::Present(value), "") => Ok(value.clone()),
                (DataField::Missing, _) | (DataField::Present(Value::Null), _) => {
                    Err(pointer_type_mismatch())
                }
                (DataField::Present(value), pointer) => {
                    pointer_tokens(pointer)?;
                    value
                        .pointer(pointer)
                        .cloned()
                        .ok_or_else(pointer_not_found)
                }
            }
        }
        DataCommand::Set { id, pointer, value } => {
            let replacement = resolve_required_json(value, limit)?;
            let pointer = pointer.as_deref().unwrap_or("");
            let data = entity_data_mut(graph, kind, id)?;
            if pointer.is_empty() {
                *data = DataField::Present(replacement);
            } else {
                validate_pointer(pointer)?;
                let DataField::Present(root) = data else {
                    return Err(pointer_type_mismatch());
                };
                if root.is_null() {
                    return Err(pointer_type_mismatch());
                }
                pointer_set(root, pointer, replacement)?;
            }
            Ok(graph.to_value())
        }
        DataCommand::Remove {
            id,
            pointer,
            ignore_missing,
        } => {
            let pointer = pointer.as_deref().unwrap_or("");
            let data = entity_data_mut(graph, kind, id)?;
            if pointer.is_empty() {
                if matches!(data, DataField::Missing) && !ignore_missing {
                    return Err(pointer_not_found());
                }
                *data = DataField::Missing;
            } else {
                validate_pointer(pointer)?;
                let DataField::Present(root) = data else {
                    return Err(pointer_type_mismatch());
                };
                if root.is_null() {
                    return Err(pointer_type_mismatch());
                }
                match pointer_remove(root, pointer) {
                    Ok(()) => {}
                    Err(error) if *ignore_missing && error.code == "pointer_not_found" => {}
                    Err(error) => return Err(error),
                }
            }
            Ok(graph.to_value())
        }
    }
}

fn query_command(graph: &GraphDoc, command: &QueryCommand) -> Result<Value, CliError> {
    match command {
        QueryCommand::Closure { starts } => closure_result(graph, starts),
        QueryCommand::Route(args) => route_result(graph, args),
        QueryCommand::Diagnose(args) => diagnose_result(graph, &args.starts, &args.targets),
    }
}

fn subgraph_command(graph: &GraphDoc, command: &SubgraphCommand) -> Result<Value, CliError> {
    match command {
        SubgraphCommand::Induced { points } => {
            ensure_unique(points, "point")?;
            for id in points {
                if graph.point_index(id).is_none() {
                    return Err(unknown_point(id));
                }
            }
            let selected: BTreeSet<_> = points.iter().cloned().collect();
            let projected = induced(graph, &selected);
            Ok(json!({
                "graph": projected.to_value(),
                "selection": { "kind": "induced", "pointIds": sorted_ids(selected) }
            }))
        }
        SubgraphCommand::Reachable { starts } => {
            let closure = closure_ids(graph, starts)?;
            let selected: BTreeSet<_> = closure.iter().cloned().collect();
            Ok(json!({
                "graph": induced(graph, &selected).to_value(),
                "selection": {
                    "kind": "reachable",
                    "startPointIds": sorted_unique(starts)?,
                    "pointIds": closure
                }
            }))
        }
        SubgraphCommand::Route(args) => {
            let selection = route_result(graph, args)?;
            let graph_value = if selection["reachable"] == Value::Bool(true) {
                let points: BTreeSet<String> = selection["pointIds"]
                    .as_array()
                    .expect("route points")
                    .iter()
                    .map(|value| value.as_str().expect("point id").to_owned())
                    .collect();
                let edges: BTreeSet<String> = selection["hyperedgeIds"]
                    .as_array()
                    .expect("route edges")
                    .iter()
                    .map(|value| value.as_str().expect("edge id").to_owned())
                    .collect();
                route_projection(graph, &points, &edges).to_value()
            } else {
                Value::Null
            };
            Ok(json!({ "graph": graph_value, "selection": selection }))
        }
    }
}

fn apply(graph: &mut GraphDoc, args: &ApplyArgs) -> Result<Value, CliError> {
    let bytes = read_limited(&args.operations, args.max_operations_bytes)?;
    let value = parse_json(&bytes).map_err(|error| {
        CliError::invalid_operations(vec![Issue::new(
            error.code.as_str(),
            "/operations",
            error.message,
        )])
    })?;
    let Some(operations) = value.as_array() else {
        return Err(CliError::invalid_operations(vec![Issue::new(
            "invalid_type",
            "/operations",
            "operations must be an array",
        )]));
    };
    for (index, operation) in operations.iter().enumerate() {
        if let Err(error) = apply_one(graph, operation) {
            return Err(CliError::invalid_operations(vec![Issue::new(
                operation_issue_code(&error),
                operation_issue_path(index, operation, &error),
                error.message.clone(),
            )]));
        }
    }
    graph_from_value(&graph.to_value()).map_err(CliError::invalid_graph)?;
    Ok(graph.to_value())
}

fn apply_one(graph: &mut GraphDoc, value: &Value) -> Result<(), CliError> {
    let object = value
        .as_object()
        .ok_or_else(|| CliError::data("invalid_type", "operation must be an object"))?;
    let op = required_str(object, "op")?;
    match op {
        "point.add" => {
            check_fields(object, &["op", "id", "data"])?;
            let id = required_str(object, "id")?.to_owned();
            ensure_new_id(graph, &id)?;
            graph.points.push(PointDoc {
                id,
                data: operation_data(object),
            });
        }
        "point.remove" => {
            check_fields(object, &["op", "id", "cascade", "ignoreMissing"])?;
            let id = required_str(object, "id")?;
            let cascade = optional_bool(object, "cascade")?;
            let ignore = optional_bool(object, "ignoreMissing")?;
            let Some(index) = graph.point_index(id) else {
                if ignore {
                    return Ok(());
                }
                return Err(unknown_point(id));
            };
            if !cascade
                && graph
                    .hyperedges
                    .iter()
                    .any(|edge| edge.head == id || edge.tails.iter().any(|tail| tail == id))
            {
                return Err(CliError::data(
                    "point_referenced",
                    format!("point `{id}` is referenced"),
                )
                .with_detail("id", id.to_owned()));
            }
            graph.points.remove(index);
            if cascade {
                graph
                    .hyperedges
                    .retain(|edge| edge.head != id && !edge.tails.iter().any(|tail| tail == id));
            }
        }
        "point.rename" => {
            check_fields(object, &["op", "id", "newId"])?;
            rename_point(
                graph,
                required_str(object, "id")?,
                required_str(object, "newId")?,
            )?;
        }
        "point.data.set" | "point.data.remove" | "hyperedge.data.set" | "hyperedge.data.remove" => {
            let is_point = op.starts_with("point");
            let is_set = op.ends_with("set");
            if is_set {
                check_fields(object, &["op", "id", "pointer", "value"])?;
            } else {
                check_fields(object, &["op", "id", "pointer", "ignoreMissing"])?;
            }
            let kind = if is_point {
                EntityKind::Point
            } else {
                EntityKind::Hyperedge
            };
            let id = required_str(object, "id")?;
            let pointer = optional_str(object, "pointer")?.unwrap_or("");
            if is_set {
                let replacement = object.get("value").cloned().ok_or_else(|| {
                    operation_field_error("missing_field", "value", "missing value")
                })?;
                let data = entity_data_mut(graph, kind, id)?;
                if pointer.is_empty() {
                    *data = DataField::Present(replacement);
                } else {
                    validate_pointer(pointer)?;
                    let DataField::Present(root) = data else {
                        return Err(pointer_type_mismatch());
                    };
                    if root.is_null() {
                        return Err(pointer_type_mismatch());
                    }
                    pointer_set(root, pointer, replacement)?;
                }
            } else {
                let ignore = optional_bool(object, "ignoreMissing")?;
                let data = entity_data_mut(graph, kind, id)?;
                if pointer.is_empty() {
                    if matches!(data, DataField::Missing) && !ignore {
                        return Err(pointer_not_found());
                    }
                    *data = DataField::Missing;
                } else {
                    validate_pointer(pointer)?;
                    let DataField::Present(root) = data else {
                        return Err(pointer_type_mismatch());
                    };
                    if root.is_null() {
                        return Err(pointer_type_mismatch());
                    }
                    if let Err(error) = pointer_remove(root, pointer)
                        && !(ignore && error.code == "pointer_not_found")
                    {
                        return Err(error);
                    }
                }
            }
        }
        "hyperedge.add" => {
            check_fields(object, &["op", "id", "tails", "head", "weight", "data"])?;
            let id = required_str(object, "id")?.to_owned();
            ensure_new_id(graph, &id)?;
            let tails = optional_string_array(object, "tails")?.unwrap_or_default();
            let head = required_str(object, "head")?.to_owned();
            ensure_point_refs(graph, &tails, &head)?;
            ensure_unique_operation(&tails, "tails")?;
            let weight = object.get("weight").ok_or_else(|| {
                operation_field_error("missing_field", "weight", "missing weight")
            })?;
            graph.hyperedges.push(HyperedgeDoc {
                id,
                tails,
                head,
                weight_units: parse_weight(weight)
                    .map_err(|m| CliError::data("invalid_weight", m))?,
                data: operation_data(object),
            });
        }
        "hyperedge.remove" => {
            check_fields(object, &["op", "id", "ignoreMissing"])?;
            let id = required_str(object, "id")?;
            let ignore = optional_bool(object, "ignoreMissing")?;
            let Some(index) = graph.edge_index(id) else {
                if ignore {
                    return Ok(());
                }
                return Err(unknown_edge(id));
            };
            graph.hyperedges.remove(index);
        }
        "hyperedge.rename" => {
            check_fields(object, &["op", "id", "newId"])?;
            let id = required_str(object, "id")?;
            let new_id = required_str(object, "newId")?;
            ensure_valid_id(new_id)?;
            let index = graph.edge_index(id).ok_or_else(|| unknown_edge(id))?;
            if graph.contains_id(new_id) {
                return Err(id_conflict(new_id));
            }
            graph.hyperedges[index].id = new_id.to_owned();
        }
        "hyperedge.set.tails" => {
            check_fields(object, &["op", "id", "tails"])?;
            let id = required_str(object, "id")?;
            let tails = required_string_array(object, "tails")?;
            ensure_unique_operation(&tails, "tails")?;
            for tail in &tails {
                if graph.point_index(tail).is_none() {
                    return Err(unknown_point(tail));
                }
            }
            edge_mut(graph, id)?.tails = tails;
        }
        "hyperedge.set.head" => {
            check_fields(object, &["op", "id", "head"])?;
            let id = required_str(object, "id")?;
            let head = required_str(object, "head")?;
            if graph.point_index(head).is_none() {
                return Err(unknown_point(head));
            }
            edge_mut(graph, id)?.head = head.to_owned();
        }
        "hyperedge.set.weight" => {
            check_fields(object, &["op", "id", "weight"])?;
            let id = required_str(object, "id")?;
            let weight = object.get("weight").ok_or_else(|| {
                operation_field_error("missing_field", "weight", "missing weight")
            })?;
            edge_mut(graph, id)?.weight_units =
                parse_weight(weight).map_err(|m| CliError::data("invalid_weight", m))?;
        }
        _ => {
            return Err(operation_field_error(
                "unknown_field",
                "op",
                format!("unknown operation `{op}`"),
            ));
        }
    }
    Ok(())
}

fn closure_result(graph: &GraphDoc, starts: &[String]) -> Result<Value, CliError> {
    Ok(json!({ "startPointIds": sorted_unique(starts)?, "pointIds": closure_ids(graph, starts)? }))
}

fn closure_ids(graph: &GraphDoc, starts: &[String]) -> Result<Vec<String>, CliError> {
    let core = CoreView::new(graph)?;
    let start = core.point_set(starts)?;
    Ok(sorted_ids(
        closure(&core.graph, &start)
            .iter()
            .map(|id| core.point_name(id)),
    ))
}

fn diagnose_result(
    graph: &GraphDoc,
    starts: &[String],
    targets: &[String],
) -> Result<Value, CliError> {
    let core = CoreView::new(graph)?;
    let start_ids = sorted_unique(starts)?;
    let target_ids = sorted_unique(targets)?;
    let start = core.point_set(&start_ids)?;
    let reached = closure(&core.graph, &start);
    let diagnoses: Vec<_> = target_ids.iter().map(|target| {
        let id = core.point_id(target)?;
        let diagnosis = blocking_frontier(&core.graph, &start, id);
        let mut cycles: Vec<Vec<String>> = diagnosis.cycles.into_iter().map(|cycle| sorted_ids(cycle.into_iter().map(|id| core.point_name(id)))).collect();
        cycles.sort();
        Ok(json!({
            "targetPointId": target,
            "blockingPointIds": sorted_ids(diagnosis.blocking.into_iter().map(|id| core.point_name(id))),
            "cycles": cycles
        }))
    }).collect::<Result<_, CliError>>()?;
    Ok(json!({
        "reachable": target_ids.iter().all(|target| reached.contains(core.point_id(target).expect("validated"))),
        "startPointIds": start_ids,
        "targetPointIds": target_ids,
        "targetDiagnoses": diagnoses
    }))
}

fn route_result(graph: &GraphDoc, args: &RouteArgs) -> Result<Value, CliError> {
    let core = CoreView::new(graph)?;
    let start_names = sorted_unique(&args.starts)?;
    let target_names = sorted_unique(&args.targets)?;
    let start = core.point_set(&start_names)?;
    let target_ids: Vec<_> = target_names
        .iter()
        .map(|id| core.point_id(id))
        .collect::<Result<_, _>>()?;
    let targets = PointSet::from_ids(&core.graph, target_ids.iter().copied())
        .map_err(|e| CliError::new(70, "internal", e.to_string()))?;
    let solution = solve_many(
        &core.graph,
        &start,
        &targets,
        &Budget {
            max_nodes: args.max_nodes,
            max_millis: args.max_millis,
        },
    )
    .map_err(|error| match error {
        SolveError::CostOverflow => CliError::data("invalid_weight", error.to_string()),
        _ => CliError::new(70, "internal", error.to_string()),
    })?;
    if !solution.cost.is_finite() {
        return diagnose_result(graph, &start_names, &target_names);
    }
    let order = executable_order(&core.graph, &start, &solution.derivation)
        .map_err(|error| CliError::new(70, "internal", error.to_string()))?;
    let hyperedge_ids = sorted_ids(solution.derivation.iter().map(|id| core.edge_name(*id)));
    let mut point_names: BTreeSet<String> =
        start_names.iter().chain(&target_names).cloned().collect();
    for edge_id in &solution.derivation {
        let edge = core.graph.hyperedge(*edge_id).expect("solution edge");
        point_names.insert(core.point_name(edge.head()));
        point_names.extend(edge.tail().iter().map(|id| core.point_name(*id)));
    }
    Ok(json!({
        "reachable": true,
        "startPointIds": start_names,
        "targetPointIds": target_names,
        "pointIds": sorted_ids(point_names),
        "hyperedgeIds": hyperedge_ids,
        "executableOrder": order.into_iter().map(|id| core.edge_name(id)).collect::<Vec<_>>(),
        "cost": core_cost(solution.cost),
        "lower": core_cost(solution.lower),
        "upper": core_cost(solution.upper),
        "provenOptimal": solution.proven_optimal,
        "nodes": solution.nodes,
        "pruned": solution.pruned,
        "millis": solution.millis
    }))
}

struct CoreView {
    graph: Graph,
    points: HashMap<String, PointId>,
}

impl CoreView {
    fn new(document: &GraphDoc) -> Result<Self, CliError> {
        let mut graph = Graph::new();
        let mut points = HashMap::new();
        for point in &document.points {
            let id = graph
                .add_point(&point.id, ())
                .map_err(|e| CliError::new(70, "internal", e.to_string()))?;
            points.insert(point.id.clone(), id);
        }
        for edge in &document.hyperedges {
            let tails: Vec<_> = edge.tails.iter().map(|id| points[id]).collect();
            graph
                .add_hyperedge(
                    &edge.id,
                    tails,
                    points[&edge.head],
                    Cost::from_units(edge.weight_units),
                    (),
                )
                .map_err(|e| CliError::new(70, "internal", e.to_string()))?;
        }
        Ok(Self { graph, points })
    }

    fn point_id(&self, id: &str) -> Result<PointId, CliError> {
        self.points
            .get(id)
            .copied()
            .ok_or_else(|| unknown_point(id))
    }

    fn point_set(&self, ids: &[String]) -> Result<PointSet, CliError> {
        ensure_unique(ids, "point")?;
        let ids: Vec<_> = ids
            .iter()
            .map(|id| self.point_id(id))
            .collect::<Result<_, _>>()?;
        PointSet::from_ids(&self.graph, ids)
            .map_err(|e| CliError::new(70, "internal", e.to_string()))
    }

    fn point_name(&self, id: PointId) -> String {
        self.graph.point(id).expect("core point").name().to_owned()
    }

    fn edge_name(&self, id: derivon_core::HyperedgeId) -> String {
        self.graph
            .hyperedge(id)
            .expect("core edge")
            .name()
            .to_owned()
    }
}

fn induced(graph: &GraphDoc, points: &BTreeSet<String>) -> GraphDoc {
    GraphDoc {
        schema: graph.schema.clone(),
        points: graph
            .points
            .iter()
            .filter(|point| points.contains(&point.id))
            .cloned()
            .collect(),
        hyperedges: graph
            .hyperedges
            .iter()
            .filter(|edge| {
                points.contains(&edge.head) && edge.tails.iter().all(|tail| points.contains(tail))
            })
            .cloned()
            .collect(),
    }
}

fn route_projection(
    graph: &GraphDoc,
    points: &BTreeSet<String>,
    edges: &BTreeSet<String>,
) -> GraphDoc {
    GraphDoc {
        schema: graph.schema.clone(),
        points: graph
            .points
            .iter()
            .filter(|point| points.contains(&point.id))
            .cloned()
            .collect(),
        hyperedges: graph
            .hyperedges
            .iter()
            .filter(|edge| edges.contains(&edge.id))
            .cloned()
            .collect(),
    }
}

fn core_cost(cost: Cost) -> Value {
    cost.units().map(weight_value).unwrap_or(Value::Null)
}

fn point<'a>(graph: &'a GraphDoc, id: &str) -> Result<&'a PointDoc, CliError> {
    graph
        .point_index(id)
        .map(|index| &graph.points[index])
        .ok_or_else(|| unknown_point(id))
}

fn edge<'a>(graph: &'a GraphDoc, id: &str) -> Result<&'a HyperedgeDoc, CliError> {
    graph
        .edge_index(id)
        .map(|index| &graph.hyperedges[index])
        .ok_or_else(|| unknown_edge(id))
}

fn edge_mut<'a>(graph: &'a mut GraphDoc, id: &str) -> Result<&'a mut HyperedgeDoc, CliError> {
    let index = graph.edge_index(id).ok_or_else(|| unknown_edge(id))?;
    Ok(&mut graph.hyperedges[index])
}

fn entity_data<'a>(
    graph: &'a GraphDoc,
    kind: EntityKind,
    id: &str,
) -> Result<&'a DataField, CliError> {
    match kind {
        EntityKind::Point => Ok(&point(graph, id)?.data),
        EntityKind::Hyperedge => Ok(&edge(graph, id)?.data),
    }
}

fn entity_data_mut<'a>(
    graph: &'a mut GraphDoc,
    kind: EntityKind,
    id: &str,
) -> Result<&'a mut DataField, CliError> {
    match kind {
        EntityKind::Point => {
            let index = graph.point_index(id).ok_or_else(|| unknown_point(id))?;
            Ok(&mut graph.points[index].data)
        }
        EntityKind::Hyperedge => {
            let index = graph.edge_index(id).ok_or_else(|| unknown_edge(id))?;
            Ok(&mut graph.hyperedges[index].data)
        }
    }
}

fn ensure_new_id(graph: &GraphDoc, id: &str) -> Result<(), CliError> {
    ensure_valid_id(id)?;
    if graph.contains_id(id) {
        Err(id_conflict(id))
    } else {
        Ok(())
    }
}

fn ensure_valid_id(id: &str) -> Result<(), CliError> {
    if valid_id(id) {
        Ok(())
    } else {
        Err(CliError::data("invalid_id", format!("invalid id `{id}`"))
            .with_detail("id", id.to_owned()))
    }
}

fn ensure_point_refs(graph: &GraphDoc, tails: &[String], head: &str) -> Result<(), CliError> {
    for id in tails {
        if graph.point_index(id).is_none() {
            return Err(unknown_point(id));
        }
    }
    if graph.point_index(head).is_none() {
        return Err(unknown_point(head));
    }
    Ok(())
}

fn ensure_unique(values: &[String], label: &str) -> Result<(), CliError> {
    let mut seen = HashSet::new();
    for value in values {
        if !seen.insert(value) {
            return Err(CliError::new(
                64,
                "invalid_arguments",
                format!("duplicate {label} `{value}`"),
            ));
        }
    }
    Ok(())
}

fn ensure_unique_operation(values: &[String], field: &str) -> Result<(), CliError> {
    let mut seen = HashSet::new();
    for (index, value) in values.iter().enumerate() {
        if !seen.insert(value) {
            return Err(operation_field_error(
                "duplicate_tail",
                &format!("{field}/{index}"),
                format!("duplicate tail `{value}`"),
            ));
        }
    }
    Ok(())
}

fn sorted_unique(values: &[String]) -> Result<Vec<String>, CliError> {
    ensure_unique(values, "point")?;
    Ok(sorted_ids(values.iter().cloned()))
}

fn parse_weight_text(text: &str) -> Result<u64, CliError> {
    let value: Value = serde_json::from_str(text)
        .map_err(|_| CliError::data("invalid_weight", "weight must be a JSON number"))?;
    parse_weight(&value).map_err(|message| CliError::data("invalid_weight", message))
}

fn resolve_optional_json(value: &OptionalJson, limit: u64) -> Result<DataField, CliError> {
    if let Some(text) = &value.data {
        ensure_value_limit(text.len(), limit)?;
        return Ok(DataField::Present(parse_json(text.as_bytes())?));
    }
    if let Some(path) = &value.data_file {
        return Ok(DataField::Present(parse_json(&read_limited(path, limit)?)?));
    }
    Ok(DataField::Present(json!({})))
}

fn resolve_required_json(value: &RequiredJson, limit: u64) -> Result<Value, CliError> {
    if let Some(text) = &value.value {
        ensure_value_limit(text.len(), limit)?;
        return parse_json(text.as_bytes());
    }
    parse_json(&read_limited(
        value.value_file.as_ref().expect("clap requires source"),
        limit,
    )?)
}

fn ensure_value_limit(length: usize, limit: u64) -> Result<(), CliError> {
    if length as u64 > limit {
        Err(CliError::data(
            "input_limit_exceeded",
            "JSON value exceeds configured byte limit",
        )
        .with_detail("limit", limit))
    } else {
        Ok(())
    }
}

pub fn read_limited(path: &Path, limit: u64) -> Result<Vec<u8>, CliError> {
    let metadata = fs::metadata(path).map_err(|error| file_error(path, error))?;
    if metadata.len() > limit {
        return Err(CliError::data(
            "input_limit_exceeded",
            "input exceeds configured byte limit",
        )
        .with_detail("source", path.display().to_string())
        .with_detail("limit", limit));
    }
    let bytes = fs::read(path).map_err(|error| file_error(path, error))?;
    if bytes.len() as u64 > limit {
        return Err(CliError::data(
            "input_limit_exceeded",
            "input exceeds configured byte limit",
        ));
    }
    Ok(bytes)
}

fn file_error(path: &Path, error: std::io::Error) -> CliError {
    let code = if error.kind() == std::io::ErrorKind::NotFound {
        "file_not_found"
    } else {
        "file_unreadable"
    };
    CliError::new(66, code, error.to_string()).with_detail("source", path.display().to_string())
}

fn unknown_point(id: &str) -> CliError {
    CliError::data("unknown_point", format!("unknown point `{id}`"))
        .with_detail("id", id.to_owned())
}
fn unknown_edge(id: &str) -> CliError {
    CliError::data("unknown_hyperedge", format!("unknown hyperedge `{id}`"))
        .with_detail("id", id.to_owned())
}
fn id_conflict(id: &str) -> CliError {
    CliError::data("id_conflict", format!("id `{id}` already exists"))
        .with_detail("id", id.to_owned())
}
fn pointer_not_found() -> CliError {
    CliError::data("pointer_not_found", "JSON Pointer does not exist")
}
fn pointer_type_mismatch() -> CliError {
    CliError::data(
        "pointer_type_mismatch",
        "JSON Pointer traverses a non-container value",
    )
}

fn ignored_or_unknown(
    ignore: bool,
    code: &str,
    id: &str,
    graph: &GraphDoc,
) -> Result<Value, CliError> {
    if ignore {
        Ok(graph.to_value())
    } else if code == "unknown_point" {
        Err(unknown_point(id))
    } else {
        Err(unknown_edge(id))
    }
}

fn validate_pointer(pointer: &str) -> Result<(), CliError> {
    if pointer.is_empty() || pointer.starts_with('/') {
        Ok(())
    } else {
        Err(CliError::data(
            "invalid_pointer",
            "JSON Pointer must be empty or start with `/`",
        ))
    }
}

fn pointer_tokens(pointer: &str) -> Result<Vec<String>, CliError> {
    validate_pointer(pointer)?;
    pointer
        .split('/')
        .skip(1)
        .map(|token| {
            let mut result = String::new();
            let mut chars = token.chars();
            while let Some(ch) = chars.next() {
                if ch == '~' {
                    match chars.next() {
                        Some('0') => result.push('~'),
                        Some('1') => result.push('/'),
                        _ => {
                            return Err(CliError::data(
                                "invalid_pointer",
                                "invalid JSON Pointer escape",
                            ));
                        }
                    }
                } else {
                    result.push(ch);
                }
            }
            Ok(result)
        })
        .collect()
}

fn pointer_set(root: &mut Value, pointer: &str, replacement: Value) -> Result<(), CliError> {
    let tokens = pointer_tokens(pointer)?;
    let (last, parents) = tokens.split_last().expect("non-root pointer");
    let mut current = root;
    for token in parents {
        current = descend_mut(current, token)?;
    }
    match current {
        Value::Object(object) => {
            object.insert(last.clone(), replacement);
            Ok(())
        }
        Value::Array(array) if last == "-" => {
            array.push(replacement);
            Ok(())
        }
        Value::Array(array) => {
            let index = array_index(last)?;
            let slot = array.get_mut(index).ok_or_else(pointer_not_found)?;
            *slot = replacement;
            Ok(())
        }
        _ => Err(pointer_type_mismatch()),
    }
}

fn pointer_remove(root: &mut Value, pointer: &str) -> Result<(), CliError> {
    let tokens = pointer_tokens(pointer)?;
    let (last, parents) = tokens.split_last().expect("non-root pointer");
    let mut current = root;
    for token in parents {
        current = descend_mut(current, token)?;
    }
    match current {
        Value::Object(object) => object
            .remove(last)
            .map(|_| ())
            .ok_or_else(pointer_not_found),
        Value::Array(_) if last == "-" => Err(CliError::data(
            "invalid_pointer",
            "`-` is invalid for remove",
        )),
        Value::Array(array) => {
            let index = array_index(last)?;
            if index < array.len() {
                array.remove(index);
                Ok(())
            } else {
                Err(pointer_not_found())
            }
        }
        _ => Err(pointer_type_mismatch()),
    }
}

fn descend_mut<'a>(value: &'a mut Value, token: &str) -> Result<&'a mut Value, CliError> {
    match value {
        Value::Object(object) => object.get_mut(token).ok_or_else(pointer_not_found),
        Value::Array(array) => array
            .get_mut(array_index(token)?)
            .ok_or_else(pointer_not_found),
        _ => Err(pointer_type_mismatch()),
    }
}

fn array_index(token: &str) -> Result<usize, CliError> {
    if token.is_empty() || (token.len() > 1 && token.starts_with('0')) {
        return Err(CliError::data("invalid_pointer", "invalid array index"));
    }
    usize::from_str(token).map_err(|_| CliError::data("invalid_pointer", "invalid array index"))
}

fn rename_point(graph: &mut GraphDoc, id: &str, new_id: &str) -> Result<(), CliError> {
    ensure_valid_id(new_id)?;
    let index = graph.point_index(id).ok_or_else(|| unknown_point(id))?;
    if graph.contains_id(new_id) {
        return Err(id_conflict(new_id));
    }
    graph.points[index].id = new_id.to_owned();
    for edge in &mut graph.hyperedges {
        if edge.head == id {
            edge.head = new_id.to_owned();
        }
        for tail in &mut edge.tails {
            if tail == id {
                *tail = new_id.to_owned();
            }
        }
    }
    Ok(())
}

fn required_str<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, CliError> {
    match object.get(key) {
        Some(Value::String(value)) => Ok(value),
        Some(_) => Err(operation_field_error(
            "invalid_type",
            key,
            format!("`{key}` must be a string"),
        )),
        None => Err(operation_field_error(
            "missing_field",
            key,
            format!("missing `{key}`"),
        )),
    }
}

fn optional_str<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<Option<&'a str>, CliError> {
    match object.get(key) {
        Some(Value::String(value)) => Ok(Some(value)),
        Some(_) => Err(operation_field_error(
            "invalid_type",
            key,
            format!("`{key}` must be a string"),
        )),
        None => Ok(None),
    }
}

fn optional_bool(object: &Map<String, Value>, key: &str) -> Result<bool, CliError> {
    match object.get(key) {
        Some(Value::Bool(value)) => Ok(*value),
        Some(_) => Err(operation_field_error(
            "invalid_type",
            key,
            format!("`{key}` must be a boolean"),
        )),
        None => Ok(false),
    }
}

fn required_string_array(object: &Map<String, Value>, key: &str) -> Result<Vec<String>, CliError> {
    optional_string_array(object, key)?
        .ok_or_else(|| operation_field_error("missing_field", key, format!("missing `{key}`")))
}

fn optional_string_array(
    object: &Map<String, Value>,
    key: &str,
) -> Result<Option<Vec<String>>, CliError> {
    let Some(value) = object.get(key) else {
        return Ok(None);
    };
    let array = value.as_array().ok_or_else(|| {
        operation_field_error("invalid_type", key, format!("`{key}` must be an array"))
    })?;
    array
        .iter()
        .enumerate()
        .map(|(index, value)| {
            value.as_str().map(str::to_owned).ok_or_else(|| {
                operation_field_error(
                    "invalid_type",
                    &format!("{key}/{index}"),
                    format!("`{key}` entries must be strings"),
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

fn operation_data(object: &Map<String, Value>) -> DataField {
    DataField::Present(object.get("data").cloned().unwrap_or_else(|| json!({})))
}

fn check_fields(object: &Map<String, Value>, allowed: &[&str]) -> Result<(), CliError> {
    if let Some(key) = object.keys().find(|key| !allowed.contains(&key.as_str())) {
        Err(operation_field_error(
            "unknown_field",
            key,
            format!("unknown field `{key}`"),
        ))
    } else {
        Ok(())
    }
}

fn operation_field_error(code: &str, field: &str, message: impl Into<String>) -> CliError {
    CliError::data(code, message).with_detail("field", field.to_owned())
}

fn operation_issue_code(error: &CliError) -> &str {
    match error.code.as_str() {
        "invalid_operations" => "invalid_type",
        code => code,
    }
}

fn operation_issue_path(index: usize, operation: &Value, error: &CliError) -> String {
    let base = format!("/operations/{index}");
    if let Some(field) = error.details.get("field").and_then(Value::as_str) {
        return format!("{base}/{field}");
    }
    let object = operation.as_object();
    let detail_id = error.details.get("id").and_then(Value::as_str);
    let inferred = match error.code.as_str() {
        "invalid_weight" => Some("weight".to_owned()),
        "invalid_pointer" | "pointer_not_found" | "pointer_type_mismatch" => {
            Some("pointer".to_owned())
        }
        "unknown_hyperedge" | "point_referenced" => Some("id".to_owned()),
        "id_conflict" | "invalid_id" => object.and_then(|object| {
            let id = detail_id?;
            if object.get("newId").and_then(Value::as_str) == Some(id) {
                Some("newId".to_owned())
            } else {
                Some("id".to_owned())
            }
        }),
        "unknown_point" => object.and_then(|object| {
            let id = detail_id?;
            if object.get("head").and_then(Value::as_str) == Some(id) {
                return Some("head".to_owned());
            }
            object
                .get("tails")
                .and_then(Value::as_array)
                .and_then(|tails| {
                    tails
                        .iter()
                        .position(|tail| tail.as_str() == Some(id))
                        .map(|tail_index| format!("tails/{tail_index}"))
                })
                .or_else(|| Some("id".to_owned()))
        }),
        _ => None,
    };
    inferred.map_or(base.clone(), |field| format!("{base}/{field}"))
}
