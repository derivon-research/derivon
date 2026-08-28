use std::collections::{BTreeSet, HashMap, HashSet};
use std::str::FromStr;

use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use serde_json::{Map, Number, Value};

use crate::error::{CliError, Issue};

pub const GRAPH_SCHEMA: &str = "derivon.graph/v1";
pub const MAX_WEIGHT_UNITS: u64 = 9_007_199_254_740_991;
pub const MAX_JSON_DEPTH: usize = 128;

#[derive(Clone, Debug, PartialEq)]
pub enum DataField {
    Missing,
    Present(Value),
}

#[derive(Clone, Debug, PartialEq)]
pub struct PointDoc {
    pub id: String,
    pub data: DataField,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HyperedgeDoc {
    pub id: String,
    pub weight_units: u64,
    pub tails: Vec<String>,
    pub head: String,
    pub data: DataField,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GraphDoc {
    pub schema: Option<String>,
    pub points: Vec<PointDoc>,
    pub hyperedges: Vec<HyperedgeDoc>,
}

impl GraphDoc {
    pub fn to_value(&self) -> Value {
        let mut root = Map::new();
        if let Some(schema) = &self.schema {
            root.insert("schema".to_owned(), Value::String(schema.clone()));
        }
        root.insert(
            "points".to_owned(),
            Value::Array(self.points.iter().map(PointDoc::to_value).collect()),
        );
        root.insert(
            "hyperedges".to_owned(),
            Value::Array(self.hyperedges.iter().map(HyperedgeDoc::to_value).collect()),
        );
        Value::Object(root)
    }

    pub fn point_index(&self, id: &str) -> Option<usize> {
        self.points.iter().position(|point| point.id == id)
    }

    pub fn edge_index(&self, id: &str) -> Option<usize> {
        self.hyperedges.iter().position(|edge| edge.id == id)
    }

    pub fn contains_id(&self, id: &str) -> bool {
        self.point_index(id).is_some() || self.edge_index(id).is_some()
    }
}

impl PointDoc {
    pub fn to_value(&self) -> Value {
        let mut value = Map::new();
        value.insert("id".to_owned(), Value::String(self.id.clone()));
        if let DataField::Present(data) = &self.data {
            value.insert("data".to_owned(), data.clone());
        }
        Value::Object(value)
    }
}

impl HyperedgeDoc {
    pub fn to_value(&self) -> Value {
        let mut value = Map::new();
        value.insert("id".to_owned(), Value::String(self.id.clone()));
        value.insert("weight".to_owned(), weight_value(self.weight_units));
        value.insert(
            "tails".to_owned(),
            Value::Array(self.tails.iter().cloned().map(Value::String).collect()),
        );
        value.insert("head".to_owned(), Value::String(self.head.clone()));
        if let DataField::Present(data) = &self.data {
            value.insert("data".to_owned(), data.clone());
        }
        Value::Object(value)
    }
}

pub fn parse_json(bytes: &[u8]) -> Result<Value, CliError> {
    scan_json(bytes).map_err(|issue| {
        let code = if issue.code == "duplicate_key" {
            "duplicate_key"
        } else if issue.code == "nesting_limit_exceeded" {
            "nesting_limit_exceeded"
        } else {
            "invalid_json"
        };
        CliError::data(code, issue.message).with_detail("path", issue.path)
    })?;
    serde_json::from_slice(bytes).map_err(|error| CliError::data("invalid_json", error.to_string()))
}

pub fn parse_graph(bytes: &[u8]) -> Result<GraphDoc, CliError> {
    let value = parse_json(bytes)?;
    if let Some(schema) = value.get("schema").and_then(Value::as_str)
        && schema != GRAPH_SCHEMA
    {
        return Err(CliError::data(
            "unsupported_schema",
            format!("unsupported graph schema `{schema}`"),
        )
        .with_detail("schema", schema.to_owned()));
    }
    graph_from_value(&value).map_err(CliError::invalid_graph)
}

pub fn graph_from_value(value: &Value) -> Result<GraphDoc, Vec<Issue>> {
    let Some(root) = value.as_object() else {
        return Err(vec![Issue::new(
            "invalid_type",
            "",
            "graph must be an object",
        )]);
    };
    let mut issues = Vec::new();
    for key in root.keys() {
        if !matches!(key.as_str(), "schema" | "points" | "hyperedges") {
            issues.push(Issue::new(
                "unknown_field",
                format!("/{}", escape_pointer(key)),
                format!("unknown graph field `{key}`"),
            ));
        }
    }

    let schema = match root.get("schema") {
        None => None,
        Some(Value::String(schema)) if schema == GRAPH_SCHEMA => Some(schema.clone()),
        Some(Value::String(schema)) => {
            issues.push(Issue::new(
                "unknown_field",
                "/schema",
                format!("unsupported schema `{schema}`"),
            ));
            None
        }
        Some(_) => {
            issues.push(Issue::new(
                "invalid_type",
                "/schema",
                "schema must be a string",
            ));
            None
        }
    };

    let point_values = required_array(root, "points", &mut issues);
    let edge_values = required_array(root, "hyperedges", &mut issues);
    let mut points = Vec::new();
    let mut point_ids = HashMap::new();
    let mut all_ids = HashMap::new();

    if let Some(values) = point_values {
        for (index, value) in values.iter().enumerate() {
            let path = format!("/points/{index}");
            let Some(object) = value.as_object() else {
                issues.push(Issue::new("invalid_type", path, "point must be an object"));
                continue;
            };
            report_unknown(object, &["id", "data"], &path, &mut issues);
            let Some(id) = string_field(object, "id", &path, &mut issues) else {
                continue;
            };
            if !valid_id(&id) {
                issues.push(Issue::new(
                    "invalid_id",
                    format!("{path}/id"),
                    "invalid point id",
                ));
            }
            if let Some(previous) = all_ids.insert(id.clone(), format!("{path}/id")) {
                issues.push(Issue::new(
                    "duplicate_id",
                    format!("{path}/id"),
                    format!("id `{id}` already appears at {previous}"),
                ));
            }
            point_ids.entry(id.clone()).or_insert(index);
            points.push(PointDoc {
                id,
                data: data_field(object),
            });
        }
    }

    let mut hyperedges = Vec::new();
    if let Some(values) = edge_values {
        for (index, value) in values.iter().enumerate() {
            let path = format!("/hyperedges/{index}");
            let Some(object) = value.as_object() else {
                issues.push(Issue::new(
                    "invalid_type",
                    path,
                    "hyperedge must be an object",
                ));
                continue;
            };
            report_unknown(
                object,
                &["id", "weight", "tails", "head", "data"],
                &path,
                &mut issues,
            );
            let id = string_field(object, "id", &path, &mut issues);
            let weight = match object.get("weight") {
                None => {
                    issues.push(Issue::new(
                        "missing_field",
                        format!("{path}/weight"),
                        "missing weight",
                    ));
                    None
                }
                Some(value) => match parse_weight(value) {
                    Ok(units) => Some(units),
                    Err(message) => {
                        issues.push(Issue::new(
                            "invalid_weight",
                            format!("{path}/weight"),
                            message,
                        ));
                        None
                    }
                },
            };
            let tails = parse_string_array(object, "tails", &path, &mut issues);
            let head = string_field(object, "head", &path, &mut issues);
            let (Some(id), Some(weight_units), Some(tails), Some(head)) = (id, weight, tails, head)
            else {
                continue;
            };
            if !valid_id(&id) {
                issues.push(Issue::new(
                    "invalid_id",
                    format!("{path}/id"),
                    "invalid hyperedge id",
                ));
            }
            if let Some(previous) = all_ids.insert(id.clone(), format!("{path}/id")) {
                issues.push(Issue::new(
                    "duplicate_id",
                    format!("{path}/id"),
                    format!("id `{id}` already appears at {previous}"),
                ));
            }
            let mut seen = HashSet::new();
            for (tail_index, tail) in tails.iter().enumerate() {
                if !seen.insert(tail) {
                    issues.push(Issue::new(
                        "duplicate_tail",
                        format!("{path}/tails/{tail_index}"),
                        format!("duplicate tail `{tail}`"),
                    ));
                }
            }
            hyperedges.push(HyperedgeDoc {
                id,
                weight_units,
                tails,
                head,
                data: data_field(object),
            });
        }
    }

    for (index, edge) in hyperedges.iter().enumerate() {
        for (tail_index, tail) in edge.tails.iter().enumerate() {
            if !point_ids.contains_key(tail) {
                issues.push(Issue::new(
                    "unknown_point",
                    format!("/hyperedges/{index}/tails/{tail_index}"),
                    format!("unknown point `{tail}`"),
                ));
            }
        }
        if !point_ids.contains_key(&edge.head) {
            issues.push(Issue::new(
                "unknown_point",
                format!("/hyperedges/{index}/head"),
                format!("unknown point `{}`", edge.head),
            ));
        }
    }

    if issues.is_empty() {
        Ok(GraphDoc {
            schema,
            points,
            hyperedges,
        })
    } else {
        Err(issues)
    }
}

pub fn valid_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 128
        && bytes[0].is_ascii_alphanumeric()
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

pub fn parse_weight(value: &Value) -> Result<u64, String> {
    let number = value
        .as_number()
        .ok_or_else(|| "weight must be a JSON number".to_owned())?;
    let text = number.to_string();
    let decimal = Decimal::from_scientific(&text)
        .or_else(|_| Decimal::from_str(&text))
        .map_err(|_| "weight is not an exact decimal".to_owned())?;
    if decimal.is_sign_negative() && !decimal.is_zero() {
        return Err("weight must be non-negative".to_owned());
    }
    let scaled = decimal
        .checked_mul(Decimal::TEN)
        .ok_or_else(|| "weight is too large".to_owned())?;
    if !scaled.fract().is_zero() {
        return Err("weight must be a multiple of 0.1".to_owned());
    }
    let units = scaled
        .to_u64()
        .ok_or_else(|| "weight is outside the supported range".to_owned())?;
    if units > MAX_WEIGHT_UNITS {
        return Err("weight exceeds 900719925474099.1".to_owned());
    }
    Ok(units)
}

pub fn weight_value(units: u64) -> Value {
    let text = if units.is_multiple_of(10) {
        (units / 10).to_string()
    } else {
        format!("{}.{}", units / 10, units % 10)
    };
    Value::Number(Number::from_str(&text).expect("valid weight number"))
}

pub fn sorted_ids(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut values: Vec<_> = values.into_iter().collect();
    values.sort_unstable();
    values
}

fn required_array<'a>(
    root: &'a Map<String, Value>,
    key: &str,
    issues: &mut Vec<Issue>,
) -> Option<&'a Vec<Value>> {
    match root.get(key) {
        Some(Value::Array(values)) => Some(values),
        Some(_) => {
            issues.push(Issue::new(
                "invalid_type",
                format!("/{key}"),
                format!("{key} must be an array"),
            ));
            None
        }
        None => {
            issues.push(Issue::new(
                "missing_field",
                format!("/{key}"),
                format!("missing {key}"),
            ));
            None
        }
    }
}

fn report_unknown(
    object: &Map<String, Value>,
    allowed: &[&str],
    path: &str,
    issues: &mut Vec<Issue>,
) {
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            issues.push(Issue::new(
                "unknown_field",
                format!("{path}/{}", escape_pointer(key)),
                format!("unknown field `{key}`"),
            ));
        }
    }
}

fn string_field(
    object: &Map<String, Value>,
    field: &str,
    path: &str,
    issues: &mut Vec<Issue>,
) -> Option<String> {
    match object.get(field) {
        Some(Value::String(value)) => Some(value.clone()),
        Some(_) => {
            issues.push(Issue::new(
                "invalid_type",
                format!("{path}/{field}"),
                format!("{field} must be a string"),
            ));
            None
        }
        None => {
            issues.push(Issue::new(
                "missing_field",
                format!("{path}/{field}"),
                format!("missing {field}"),
            ));
            None
        }
    }
}

fn parse_string_array(
    object: &Map<String, Value>,
    field: &str,
    path: &str,
    issues: &mut Vec<Issue>,
) -> Option<Vec<String>> {
    let Some(value) = object.get(field) else {
        issues.push(Issue::new(
            "missing_field",
            format!("{path}/{field}"),
            format!("missing {field}"),
        ));
        return None;
    };
    let Some(array) = value.as_array() else {
        issues.push(Issue::new(
            "invalid_type",
            format!("{path}/{field}"),
            format!("{field} must be an array"),
        ));
        return None;
    };
    let mut result = Vec::new();
    for (index, value) in array.iter().enumerate() {
        if let Some(value) = value.as_str() {
            result.push(value.to_owned());
        } else {
            issues.push(Issue::new(
                "invalid_type",
                format!("{path}/{field}/{index}"),
                "point id must be a string",
            ));
        }
    }
    Some(result)
}

fn data_field(object: &Map<String, Value>) -> DataField {
    object
        .get("data")
        .cloned()
        .map(DataField::Present)
        .unwrap_or(DataField::Missing)
}

fn escape_pointer(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

struct Scanner<'a> {
    bytes: &'a [u8],
    index: usize,
}

fn scan_json(bytes: &[u8]) -> Result<(), Issue> {
    let mut scanner = Scanner { bytes, index: 0 };
    scanner.skip_ws();
    scanner.value(0, "")?;
    scanner.skip_ws();
    if scanner.index != bytes.len() {
        return Err(Issue::new("invalid_json", "", "trailing JSON data"));
    }
    Ok(())
}

impl Scanner<'_> {
    fn value(&mut self, depth: usize, path: &str) -> Result<(), Issue> {
        if depth > MAX_JSON_DEPTH {
            return Err(Issue::new(
                "nesting_limit_exceeded",
                path,
                "JSON nesting exceeds 128 levels",
            ));
        }
        self.skip_ws();
        match self.peek() {
            Some(b'{') => self.object(depth + 1, path),
            Some(b'[') => self.array(depth + 1, path),
            Some(b'"') => self.string().map(|_| ()),
            Some(_) => self.primitive(),
            None => Err(Issue::new("invalid_json", path, "unexpected end of JSON")),
        }
    }

    fn object(&mut self, depth: usize, path: &str) -> Result<(), Issue> {
        self.index += 1;
        self.skip_ws();
        let mut keys = BTreeSet::new();
        if self.consume(b'}') {
            return Ok(());
        }
        loop {
            self.skip_ws();
            let key = self.string()?;
            let key_path = format!("{path}/{}", escape_pointer(&key));
            if !keys.insert(key.clone()) {
                return Err(Issue::new(
                    "duplicate_key",
                    key_path,
                    format!("duplicate object key `{key}`"),
                ));
            }
            self.skip_ws();
            if !self.consume(b':') {
                return Err(Issue::new("invalid_json", path, "expected `:`"));
            }
            self.value(depth, &key_path)?;
            self.skip_ws();
            if self.consume(b'}') {
                return Ok(());
            }
            if !self.consume(b',') {
                return Err(Issue::new("invalid_json", path, "expected `,` or `}`"));
            }
        }
    }

    fn array(&mut self, depth: usize, path: &str) -> Result<(), Issue> {
        self.index += 1;
        self.skip_ws();
        if self.consume(b']') {
            return Ok(());
        }
        let mut index = 0;
        loop {
            self.value(depth, &format!("{path}/{index}"))?;
            index += 1;
            self.skip_ws();
            if self.consume(b']') {
                return Ok(());
            }
            if !self.consume(b',') {
                return Err(Issue::new("invalid_json", path, "expected `,` or `]`"));
            }
        }
    }

    fn string(&mut self) -> Result<String, Issue> {
        let start = self.index;
        if !self.consume(b'"') {
            return Err(Issue::new("invalid_json", "", "expected string"));
        }
        let mut escaped = false;
        while let Some(byte) = self.peek() {
            self.index += 1;
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                return serde_json::from_slice(&self.bytes[start..self.index])
                    .map_err(|error| Issue::new("invalid_json", "", error.to_string()));
            } else if byte < 0x20 {
                return Err(Issue::new(
                    "invalid_json",
                    "",
                    "control character in string",
                ));
            }
        }
        Err(Issue::new("invalid_json", "", "unterminated string"))
    }

    fn primitive(&mut self) -> Result<(), Issue> {
        let start = self.index;
        while let Some(byte) = self.peek() {
            if byte.is_ascii_whitespace() || matches!(byte, b',' | b']' | b'}') {
                break;
            }
            self.index += 1;
        }
        if start == self.index {
            Err(Issue::new("invalid_json", "", "expected JSON value"))
        } else {
            Ok(())
        }
    }

    fn skip_ws(&mut self) {
        while self.peek().is_some_and(|byte| byte.is_ascii_whitespace()) {
            self.index += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.index).copied()
    }

    fn consume(&mut self, expected: u8) -> bool {
        if self.peek() == Some(expected) {
            self.index += 1;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_keys_are_rejected_inside_data() {
        let error =
            parse_json(br#"{"points":[],"hyperedges":[],"data":{"x":1,"x":2}}"#).unwrap_err();
        assert_eq!(error.code, "duplicate_key");
    }

    #[test]
    fn weights_are_exact_safe_tenths() {
        assert_eq!(parse_weight(&serde_json::json!(1.5)).unwrap(), 15);
        assert_eq!(
            parse_weight(&serde_json::from_str("1e2").unwrap()).unwrap(),
            1000
        );
        assert!(parse_weight(&serde_json::json!(1.25)).is_err());
        assert!(parse_weight(&serde_json::from_str("900719925474099.2").unwrap()).is_err());
    }
}
