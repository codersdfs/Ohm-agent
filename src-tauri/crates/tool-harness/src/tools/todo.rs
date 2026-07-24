// Todo tool — in-memory + session-persisted task list

use crate::metadata::{
    CostCategory, CostHint, LatencyHint, ToolCategory, ToolErrorSpec, ToolExample, ToolMetadata,
    ToolSource,
};
use crate::schema::string_param;
use crate::{Tool, ToolError, ToolInput, ToolResult, ToolUseContext};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    pub id: String,
    pub task: String,
    pub status: TodoStatus,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
    Blocked,
}

impl TodoStatus {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
            Self::Blocked => "blocked",
        }
    }
}

/// Shared todo store — persists across tool calls within a session.
pub type TodoStore = Arc<Mutex<HashMap<String, TodoItem>>>;

pub fn new_todo_store() -> TodoStore {
    Arc::new(Mutex::new(HashMap::new()))
}

pub struct TodoTool {
    store: TodoStore,
}

impl TodoTool {
    pub fn new(store: TodoStore) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for TodoTool {
    fn name(&self) -> &str {
        "todo"
    }
    fn description(&self) -> &str {
        "Manage a task list for long-running work. Operations: set, update, list."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "op": {
                    "type": "string",
                    "description": "Operation: 'set' (create), 'update' (change status), 'list' (show all)",
                    "enum": ["set", "update", "list"]
                },
                "task": string_param("Task description (for 'set' operation)"),
                "id": string_param("Task ID to update (for 'update' operation)"),
                "status": {
                    "type": "string",
                    "description": "New status (for 'update' operation)",
                    "enum": ["pending", "in_progress", "completed", "blocked"]
                }
            },
            "required": ["op"]
        })
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = self.parameters_schema();
        ToolMetadata {
            name: "todo".into(),
            label: "Todo List".into(),
            description: "Manage a task list for long-running work.".into(),
            doc: Some("Manages a task list that persists for the session.
- op='set': Create a new task (returns task ID)
- op='update': Update a task's status by ID
- op='list': Show all tasks with their status".into()),
            category: ToolCategory::AgentManagement,
            subcategory: Some("todo".into()),
            tags: vec!["todo".into(), "task".into(), "list".into(), "planning".into()],
            parameters: schema.clone(),
            param_summaries: ToolMetadata::extract_param_summaries(&schema),
            read_only: false,
            concurrency_safe: false,
            latency_hint: LatencyHint::Instant,
            supports_streaming: false,
            max_result_chars: 5_000,
            errors: vec![
                ToolErrorSpec {
                    kind: "invalid_op".into(),
                    description: "Invalid operation. Use 'set', 'update', or 'list'.".into(),
                    recoverable: true,
                    retry_advice: Some("Check the op parameter value".into()),
                },
                ToolErrorSpec {
                    kind: "task_not_found".into(),
                    description: "The specified task ID does not exist".into(),
                    recoverable: true,
                    retry_advice: Some("Use op='list' to see available task IDs".into()),
                },
            ],
            examples: vec![
                ToolExample {
                    title: "Add a task".into(),
                    description: "Create a new pending task".into(),
                    arguments: serde_json::json!({
                        "op": "set",
                        "task": "Implement web_fetch tool"
                    }),
                    expected_result: Some("Created task T001: Implement web_fetch tool".into()),
                },
                ToolExample {
                    title: "Update task status".into(),
                    description: "Mark a task as in progress".into(),
                    arguments: serde_json::json!({
                        "op": "update",
                        "id": "T001",
                        "status": "in_progress"
                    }),
                    expected_result: None,
                },
                ToolExample {
                    title: "List all tasks".into(),
                    description: "Show all tasks".into(),
                    arguments: serde_json::json!({ "op": "list" }),
                    expected_result: None,
                },
            ],
            cost_hint: Some(CostHint { tokens_per_call: 10, category: CostCategory::Free }),
            version: "1.0.0".into(),
            deprecation: None,
            source: ToolSource::Builtin,
            source_name: None,
        }
    }

    async fn call(&self, input: ToolInput, _ctx: &ToolUseContext) -> Result<ToolResult, ToolError> {
        let op = input
            .args
            .get("op")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::new("Missing argument: op"))?;

        match op {
            "set" => {
                let task = input
                    .args
                    .get("task")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::new("Missing argument: task"))?;

                let mut store = self.store.lock().await;
                let id = format!("T{:03}", store.len() + 1);
                let item = TodoItem {
                    id: id.clone(),
                    task: task.to_string(),
                    status: TodoStatus::Pending,
                    created_at: chrono::Utc::now().to_rfc3339(),
                };
                store.insert(id.clone(), item);

                Ok(ToolResult::success(format!(
                    "Created task {}: {}",
                    id, task
                )))
            }
            "update" => {
                let id = input
                    .args
                    .get("id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::new("Missing argument: id"))?;

                let status_str = input
                    .args
                    .get("status")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::new("Missing argument: status"))?;

                let status = match status_str {
                    "pending" => TodoStatus::Pending,
                    "in_progress" => TodoStatus::InProgress,
                    "completed" => TodoStatus::Completed,
                    "blocked" => TodoStatus::Blocked,
                    _ => {
                        return Err(ToolError::new(format!(
                            "Invalid status: {}. Use pending, in_progress, completed, or blocked",
                            status_str
                        )));
                    }
                };

                let mut store = self.store.lock().await;
                if let Some(item) = store.get_mut(id) {
                    item.status = status;
                    Ok(ToolResult::success(format!(
                        "Updated task {} to {}",
                        id,
                        status.as_str()
                    )))
                } else {
                    Err(ToolError::with_kind(
                        crate::ToolErrorKind::NotFound,
                        format!("Task not found: {}", id),
                    ))
                }
            }
            "list" => {
                let store = self.store.lock().await;
                if store.is_empty() {
                    return Ok(ToolResult::success("No tasks in list".to_string()));
                }

                let mut items: Vec<_> = store.values().collect();
                items.sort_by(|a, b| a.id.cmp(&b.id));

                let mut output = String::new();
                for item in items {
                    output.push_str(&format!(
                        "[{}] {} — {}\n",
                        item.status.as_str(),
                        item.id,
                        item.task
                    ));
                }

                Ok(ToolResult::success(output.trim().to_string()))
            }
            _ => Err(ToolError::with_kind(
                crate::ToolErrorKind::SchemaValidation,
                format!("Invalid operation: {}. Use 'set', 'update', or 'list'", op),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_todo_set_and_list() {
        let store = new_todo_store();
        let tool = TodoTool::new(store);

        let input = ToolInput {
            tool: "todo".into(),
            args: serde_json::json!({ "op": "set", "task": "Test task" }),
        };
        let ctx = ToolUseContext::new("test");

        let result = tool.call(input, &ctx).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("T001"));

        let input = ToolInput {
            tool: "todo".into(),
            args: serde_json::json!({ "op": "list" }),
        };
        let result = tool.call(input, &ctx).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("Test task"));
    }

    #[tokio::test]
    async fn test_todo_update() {
        let store = new_todo_store();
        let tool = TodoTool::new(store);

        let input = ToolInput {
            tool: "todo".into(),
            args: serde_json::json!({ "op": "set", "task": "Test task" }),
        };
        let ctx = ToolUseContext::new("test");

        tool.call(input, &ctx).await.unwrap();

        let input = ToolInput {
            tool: "todo".into(),
            args: serde_json::json!({ "op": "update", "id": "T001", "status": "completed" }),
        };
        let result = tool.call(input, &ctx).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("completed"));
    }

    #[tokio::test]
    async fn test_todo_update_not_found() {
        let store = new_todo_store();
        let tool = TodoTool::new(store);

        let input = ToolInput {
            tool: "todo".into(),
            args: serde_json::json!({ "op": "update", "id": "T999", "status": "completed" }),
        };
        let ctx = ToolUseContext::new("test");

        let result = tool.call(input, &ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_todo_list_empty() {
        let store = new_todo_store();
        let tool = TodoTool::new(store);

        let input = ToolInput {
            tool: "todo".into(),
            args: serde_json::json!({ "op": "list" }),
        };
        let ctx = ToolUseContext::new("test");

        let result = tool.call(input, &ctx).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("No tasks"));
    }

    #[tokio::test]
    async fn test_todo_invalid_op() {
        let store = new_todo_store();
        let tool = TodoTool::new(store);

        let input = ToolInput {
            tool: "todo".into(),
            args: serde_json::json!({ "op": "delete" }),
        };
        let ctx = ToolUseContext::new("test");

        let result = tool.call(input, &ctx).await;
        assert!(result.is_err());
    }
}
