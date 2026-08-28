use serde::Serialize;
use serde_json::{Map, Value};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub code: String,
    pub path: String,
    pub message: String,
}

impl Issue {
    pub fn new(code: &str, path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            path: path.into(),
            message: message.into(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct CliError {
    pub exit: u8,
    pub code: String,
    pub message: String,
    pub issues: Vec<Issue>,
    pub details: Map<String, Value>,
}

impl CliError {
    pub fn new(exit: u8, code: &str, message: impl Into<String>) -> Self {
        Self {
            exit,
            code: code.to_owned(),
            message: message.into(),
            issues: Vec::new(),
            details: Map::new(),
        }
    }

    pub fn data(code: &str, message: impl Into<String>) -> Self {
        Self::new(65, code, message)
    }

    pub fn invalid_graph(issues: Vec<Issue>) -> Self {
        Self {
            issues,
            ..Self::data("invalid_graph", "graph validation failed")
        }
    }

    pub fn invalid_operations(issues: Vec<Issue>) -> Self {
        Self {
            issues,
            ..Self::data("invalid_operations", "operation batch failed")
        }
    }

    pub fn with_detail(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.details.insert(key.to_owned(), value.into());
        self
    }

    pub fn json(&self) -> Value {
        let mut error = Map::new();
        error.insert("code".to_owned(), Value::String(self.code.clone()));
        error.insert("message".to_owned(), Value::String(self.message.clone()));
        if !self.issues.is_empty() {
            error.insert(
                "issues".to_owned(),
                serde_json::to_value(&self.issues).expect("issues serialize"),
            );
        }
        if !self.details.is_empty() {
            error.insert("details".to_owned(), Value::Object(self.details.clone()));
        }
        let mut root = Map::new();
        root.insert("error".to_owned(), Value::Object(error));
        Value::Object(root)
    }
}
