// JSON schema validation for tool inputs

use crate::ToolError;
use serde_json::Value;

/// Validate input JSON against a JSON schema
pub fn validate_input(schema: &Value, input: &Value) -> Result<(), ToolError> {
    if let Value::Object(schema_obj) = schema {
        if let Some(expected_type) = schema_obj.get("type") {
            match expected_type.as_str() {
                Some("object") => validate_object_input(schema_obj, input),
                Some("string") => validate_string_input(schema_obj, input),
                Some("array") => validate_array_input(schema_obj, input),
                Some("number") => validate_number_input(schema_obj, input),
                Some("boolean") => validate_boolean_input(schema_obj, input),
                Some("null") => validate_null_input(input),
                _ => Ok(()), // Schema unknown type, skip validation
            }
        } else {
            Ok(()) // No type defined, skip validation
        }
    } else {
        Ok(()) // Schema is not an object, skip validation
    }
}

fn validate_object_input(
    schema: &serde_json::Map<String, Value>,
    input: &Value,
) -> Result<(), ToolError> {
    if let Value::Object(obj) = input {
        // Check required fields — "required" is at the schema root, not under "properties"
        if let Some(required) = schema.get("required") {
            if let Some(req_arr) = required.as_array() {
                for req_field in req_arr {
                    if let Some(field_name) = req_field.as_str() {
                        if !obj.contains_key(field_name) {
                            return Err(ToolError::new(format!(
                                "Missing required field: {}",
                                field_name
                            )));
                        }
                    }
                }
            }
        }

        // Validate property types
        if let Some(properties) = schema.get("properties") {
            if let Some(props_obj) = properties.as_object() {
                for (field_name, field_schema) in props_obj {
                    if let Some(value) = obj.get(field_name) {
                        validate_field_value(field_schema, value)?;
                    }
                }
            }
        }

        // Validate additional properties
        if let Some(additional) = schema.get("additionalProperties") {
            if let Some(false_val) = additional.as_bool() {
                if false_val {
                    // Disallow additional properties
                    if let Some(props) = schema.get("properties") {
                        if let Some(props_obj) = props.as_object() {
                            for field_name in obj.keys() {
                                if !props_obj.contains_key(field_name) {
                                    return Err(ToolError::new(format!(
                                        "Unexpected field: {}",
                                        field_name
                                    )));
                                }
                            }
                        }
                    }
                }
            }
        }
    } else {
        return Err(ToolError::new("Expected object but got other type"));
    }
    Ok(())
}

fn validate_string_input(
    schema: &serde_json::Map<String, Value>,
    input: &Value,
) -> Result<(), ToolError> {
    if let Value::String(_) = input {
        // Validate length constraints if specified
        if let Some(min_len) = schema.get("minLength") {
            if let Some(len) = input.as_str().map(|s| s.len()) {
                if let Some(min) = min_len.as_u64() {
                    if len < min as usize {
                        return Err(ToolError::new(format!(
                            "String length {} is less than minimum {}",
                            len, min
                        )));
                    }
                }
            }
        }
        if let Some(max_len) = schema.get("maxLength") {
            if let Some(len) = input.as_str().map(|s| s.len()) {
                if let Some(max) = max_len.as_u64() {
                    if len > max as usize {
                        return Err(ToolError::new(format!(
                            "String length {} exceeds maximum {}",
                            len, max
                        )));
                    }
                }
            }
        }
        // Validate pattern if specified
        if let Some(pattern) = schema.get("pattern") {
            if let Some(pattern_str) = pattern.as_str() {
                if let Ok(regex) = regex::Regex::new(pattern_str) {
                    if let Some(s) = input.as_str() {
                        if !regex.is_match(s) {
                            return Err(ToolError::new(format!(
                                "String '{}' does not match pattern '{}'",
                                s, pattern_str
                            )));
                        }
                    }
                }
            }
        }
    } else {
        return Err(ToolError::new("Expected string but got other type"));
    }
    Ok(())
}

fn validate_array_input(
    schema: &serde_json::Map<String, Value>,
    input: &Value,
) -> Result<(), ToolError> {
    if let Value::Array(arr) = input {
        // Validate item count
        if let Some(min_items) = schema.get("minItems") {
            if let Some(min) = min_items.as_u64() {
                if arr.len() < min as usize {
                    return Err(ToolError::new(format!(
                        "Array has {} items, less than minimum {}",
                        arr.len(),
                        min
                    )));
                }
            }
        }
        if let Some(max_items) = schema.get("maxItems") {
            if let Some(max) = max_items.as_u64() {
                if arr.len() > max as usize {
                    return Err(ToolError::new(format!(
                        "Array has {} items, exceeds maximum {}",
                        arr.len(),
                        max
                    )));
                }
            }
        }
        // Validate item schemas if items schema is specified
        if let Some(items_schema) = schema.get("items") {
            for (i, item) in arr.iter().enumerate() {
                validate_field_value(items_schema, item).map_err(|e| {
                    ToolError::with_details(
                        format!("Item at index {} validation failed", i),
                        e.to_string(),
                    )
                })?
            }
        }
    } else {
        return Err(ToolError::new("Expected array but got other type"));
    }
    Ok(())
}

fn validate_number_input(
    schema: &serde_json::Map<String, Value>,
    input: &Value,
) -> Result<(), ToolError> {
    if let Value::Number(num) = input {
        if let Some(min) = schema.get("minimum") {
            if let Some(min_val) = min.as_f64() {
                if let Some(n) = num.as_f64() {
                    if n < min_val {
                        return Err(ToolError::new(format!(
                            "Number {} is less than minimum {}",
                            n, min_val
                        )));
                    }
                }
            }
        }
        if let Some(max) = schema.get("maximum") {
            if let Some(max_val) = max.as_f64() {
                if let Some(n) = num.as_f64() {
                    if n > max_val {
                        return Err(ToolError::new(format!(
                            "Number {} exceeds maximum {}",
                            n, max_val
                        )));
                    }
                }
            }
        }
    } else {
        return Err(ToolError::new("Expected number but got other type"));
    }
    Ok(())
}

fn validate_boolean_input(
    _schema: &serde_json::Map<String, Value>,
    input: &Value,
) -> Result<(), ToolError> {
    if !input.is_boolean() {
        return Err(ToolError::new("Expected boolean but got other type"));
    }
    Ok(())
}

fn validate_null_input(input: &Value) -> Result<(), ToolError> {
    if !input.is_null() {
        return Err(ToolError::new("Expected null but got other type"));
    }
    Ok(())
}

fn validate_field_value(schema: &Value, value: &Value) -> Result<(), ToolError> {
    if let Value::Object(schema_obj) = schema {
        if let Some(expected_type) = schema_obj.get("type") {
            match expected_type.as_str() {
                Some("object") => validate_object_input(schema_obj, value),
                Some("string") => validate_string_input(schema_obj, value),
                Some("array") => validate_array_input(schema_obj, value),
                Some("number") => validate_number_input(schema_obj, value),
                Some("boolean") => validate_boolean_input(schema_obj, value),
                Some("null") => validate_null_input(value),
                _ => Ok(()),
            }
        } else {
            Ok(())
        }
    } else {
        // Non-object schema, direct validation
        match value {
            Value::String(_) if schema.is_string() => Ok(()),
            Value::Number(_) if schema.is_number() => Ok(()),
            Value::Bool(_) if schema.is_boolean() => Ok(()),
            Value::Null if schema.is_null() => Ok(()),
            _ => Err(ToolError::new("Value type does not match schema")),
        }
    }
}

pub fn string_param(description: &str) -> Value {
    serde_json::json!({
        "type": "string",
        "description": description
    })
}

pub fn boolean_param(description: &str) -> Value {
    serde_json::json!({
        "type": "boolean",
        "description": description
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_param() {
        let param = string_param("test field");
        assert!(param.is_object());
        assert_eq!(param["type"], "string");
        assert_eq!(param["description"], "test field");
    }

    #[test]
    fn test_validate_object() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "maxLength": 10 },
                "age": { "type": "number", "minimum": 0 }
            },
            "required": ["name", "age"]
        });
        let valid_input = serde_json::json!({
            "name": "test",
            "age": 25
        });
        assert!(validate_input(&schema, &valid_input).is_ok());

        let invalid_input = serde_json::json!({
            "name": "this is way too long",
            "age": -5
        });
        assert!(validate_input(&schema, &invalid_input).is_err());
    }

    #[test]
    fn test_validate_missing_required_field() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "filePath": { "type": "string" },
                "content": { "type": "string" }
            },
            "required": ["filePath", "content"]
        });
        // Missing "content" field
        let invalid_input = serde_json::json!({
            "filePath": "/some/path"
        });
        let result = validate_input(&schema, &invalid_input);
        assert!(result.is_err(), "Should error on missing required field");
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Missing required field: content"),
            "Error should name the missing field"
        );

        // Happy path — all required fields present
        let valid_input = serde_json::json!({
            "filePath": "/some/path",
            "content": "hello"
        });
        assert!(validate_input(&schema, &valid_input).is_ok());
    }

    #[test]
    fn test_validate_string_constraints() {
        let schema = serde_json::json!({
            "type": "string",
            "minLength": 3,
            "maxLength": 10,
            "pattern": "^[a-z]+$"
        });
        let valid = serde_json::json!("hello");
        assert!(validate_input(&schema, &valid).is_ok());

        let invalid = serde_json::json!("Hello123");
        assert!(validate_input(&schema, &invalid).is_err());
    }
}
