use anyhow::{Context, Result, bail, ensure};
use serde_json::Value;
use std::collections::BTreeSet;

pub fn validate_schema(schema: &Value) -> Result<()> {
    ensure!(schema["type"] == "object", "tool parameters must be an object schema");
    walk(schema, 0, &mut 0)
}
fn walk(schema: &Value, depth: usize, nodes: &mut usize) -> Result<()> {
    *nodes += 1;
    ensure!(depth <= 16 && *nodes <= 1024, "schema complexity limit exceeded");
    let object = schema.as_object().context("schema must be an object")?;
    let kind = schema["type"].as_str().context("schema type required")?;
    let extra: &[&str] = match kind {
        "object" => &["properties", "required", "additionalProperties"],
        "array" => &["items", "minItems", "maxItems"],
        "string" => &["minLength", "maxLength"],
        "integer" | "number" => &["minimum", "maximum"],
        "boolean" => &[],
        _ => bail!("unsupported schema type"),
    };
    ensure!(
        object
            .keys()
            .all(|k| ["type", "description", "title", "enum"].contains(&k.as_str()) || extra.contains(&k.as_str())),
        "unsupported schema keyword"
    );
    for field in ["description", "title"] {
        if let Some(v) = object.get(field) {
            ensure!(v.as_str().is_some_and(|v| v.len() <= 4096), "invalid schema description/title");
        }
    }
    if let Some(values) = object.get("enum") {
        let values = values.as_array().context("enum must be array")?;
        ensure!(!values.is_empty() && values.len() <= 256, "enum exceeds limit");
        let mut seen = BTreeSet::new();
        for value in values {
            ensure!(seen.insert(value.to_string()), "duplicate enum value");
            check(schema, value, depth, false)?;
        }
    }
    match kind {
        "object" => {
            if let Some(props) = object.get("properties") {
                for (name, child) in props.as_object().context("properties must be object")? {
                    ensure!(!name.is_empty() && name.len() <= 128, "invalid property name");
                    walk(child, depth + 1, nodes)?;
                }
            }
            if let Some(required) = object.get("required") {
                let mut seen = BTreeSet::new();
                for value in required.as_array().context("required must be array")? {
                    let name = value.as_str().context("required name must be string")?;
                    ensure!(
                        schema["properties"].get(name).is_some() && seen.insert(name),
                        "invalid/duplicate required property"
                    );
                }
            }
            if let Some(v) = object.get("additionalProperties") {
                ensure!(v.is_boolean(), "additionalProperties must be boolean");
            }
        }
        "array" => {
            walk(object.get("items").context("array items required")?, depth + 1, nodes)?;
            bounds(schema, "minItems", "maxItems", true)?;
        }
        "string" => bounds(schema, "minLength", "maxLength", true)?,
        "number" | "integer" => bounds(schema, "minimum", "maximum", false)?,
        _ => {}
    }
    Ok(())
}
fn bounds(schema: &Value, min: &str, max: &str, unsigned: bool) -> Result<()> {
    for key in [min, max] {
        if let Some(v) = schema.get(key) {
            ensure!(
                v.as_f64().is_some_and(f64::is_finite) && (!unsigned || v.as_u64().is_some()),
                "invalid schema bound"
            );
        }
    }
    if let (Some(a), Some(b)) = (schema[min].as_f64(), schema[max].as_f64()) {
        ensure!(a <= b, "reversed schema bounds");
    }
    Ok(())
}
pub fn validate_arguments(schema: &Value, value: &Value) -> Result<()> {
    validate_schema(schema)?;
    ensure!(serde_json::to_vec(value)?.len() <= crate::FRAME_LIMIT, "arguments exceed frame limit");
    check(schema, value, 0, true)
}
fn check(schema: &Value, value: &Value, depth: usize, enumeration: bool) -> Result<()> {
    ensure!(depth <= 16, "argument depth limit exceeded");
    if enumeration && let Some(values) = schema["enum"].as_array() {
        ensure!(values.contains(value), "value not in enum");
    }
    match schema["type"].as_str().context("schema type required")? {
        "object" => {
            let object = value.as_object().context("expected object")?;
            if let Some(required) = schema["required"].as_array() {
                for name in required {
                    ensure!(
                        object.contains_key(name.as_str().context("invalid required name")?),
                        "required argument missing"
                    );
                }
            }
            for (name, v) in object {
                if let Some(child) = schema["properties"].get(name) {
                    check(child, v, depth + 1, true)?;
                } else {
                    ensure!(schema["additionalProperties"] != false, "unexpected argument");
                }
            }
        }
        "array" => {
            let array = value.as_array().context("expected array")?;
            length(schema, array.len(), "minItems", "maxItems")?;
            for v in array {
                check(&schema["items"], v, depth + 1, true)?;
            }
        }
        "string" => {
            length(schema, value.as_str().context("expected string")?.chars().count(), "minLength", "maxLength")?
        }
        "boolean" => ensure!(value.is_boolean(), "expected boolean"),
        "number" | "integer" => {
            let n = value.as_f64().context("expected number")?;
            ensure!(
                n.is_finite() && (schema["type"] != "integer" || n.fract() == 0.0),
                "expected finite number/integer"
            );
            if let Some(min) = schema["minimum"].as_f64() {
                ensure!(n >= min, "number below minimum");
            }
            if let Some(max) = schema["maximum"].as_f64() {
                ensure!(n <= max, "number above maximum");
            }
        }
        _ => bail!("unsupported schema type"),
    }
    Ok(())
}
fn length(schema: &Value, n: usize, min: &str, max: &str) -> Result<()> {
    if let Some(v) = schema[min].as_u64() {
        ensure!(n as u64 >= v, "value too short");
    }
    if let Some(v) = schema[max].as_u64() {
        ensure!(n as u64 <= v, "value too long");
    }
    Ok(())
}
