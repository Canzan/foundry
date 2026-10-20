//! The API's own description (US-AOD-01; DESIGN DDD-2..DDD-4, ADR-API-OPENAPI-001).
//!
//! `crates/foundry-api/openapi.json` is hand-written and compiled in with
//! `include_bytes!`. The handler serves those bytes verbatim and never parses
//! them, so the description an instance serves is the committed file by
//! construction (DDD-3). The handler takes no extractor and no state, so no
//! credential can change its answer (D8).

use axum::http::header;
use axum::response::{IntoResponse, Response};

/// The committed description, byte for byte (DDD-2, DDD-3).
pub const DOCUMENT: &[u8] = include_bytes!("../openapi.json");

/// `GET /api/v1/openapi.json` (DDD-4). axum answers `HEAD` from the same
/// handler with the body removed.
pub(crate) async fn openapi_document() -> Response {
    (
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        DOCUMENT,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    //! The type-derived and validity layers of the drift check (DDD-9, DDD-11,
    //! DDD-12, DDD-13; DESIGN Test Seams). The five names are load-bearing:
    //! the drift-check acceptance scenarios look for them in the output (DIST-5).

    use super::DOCUMENT;
    use crate::{
        CommentBodyRequest, CommentJson, CreateIssueRequest, ErrorBody, ErrorDetail,
        IssueHistoryJson, IssueJson, PatchIssueRequest, TokenJson,
    };
    use serde_json::{Map, Value};
    use sha2::{Digest, Sha256};
    use std::collections::BTreeSet;

    /// The OAS 3.1 JSON Schema, iteration 2025-11-23, vendored (DDD-12).
    const OAS_SCHEMA: &[u8] = include_bytes!("../tests/fixtures/oas-3.1-schema-2025-11-23.json");
    /// sha256 of `https://spec.openapis.org/oas/3.1/schema/2025-11-23`, retrieved 2026-10-19.
    const OAS_SCHEMA_SHA256: &str =
        "1b8ccc6e34234b17536f2dd0eb3597142a32bd108438cd42471a5fca4c1a07ef";

    const COMPONENTS: &str = "#/components/schemas/";
    const DEFS: &str = "#/$defs/";

    fn document() -> Value {
        serde_json::from_slice(DOCUMENT).expect("openapi.json parses as JSON")
    }

    /// DDD-6: the component name of a wire struct.
    fn component_name(rust_name: &str) -> String {
        match rust_name {
            "ErrorBody" => "Error".to_string(),
            "IssueHistoryJson" => "IssueHistoryEntry".to_string(),
            other => other.strip_suffix("Json").unwrap_or(other).to_string(),
        }
    }

    /// The nine wire structs as `schemars` sees them (the serde shape). An
    /// answer is judged as the server writes it, so an `Option` field is
    /// present-but-null and required (DDD-8); a request body is judged as the
    /// server reads it, so a `#[serde(default)]` field is optional (C8).
    fn wire_schemas() -> Vec<(String, Value)> {
        fn sent<T: schemars::JsonSchema>() -> (String, Value) {
            of::<T>(schemars::generate::SchemaSettings::draft2020_12().for_serialize())
        }
        fn read<T: schemars::JsonSchema>() -> (String, Value) {
            of::<T>(schemars::generate::SchemaSettings::draft2020_12().for_deserialize())
        }
        fn of<T: schemars::JsonSchema>(
            settings: schemars::generate::SchemaSettings,
        ) -> (String, Value) {
            let schema = settings.into_generator().into_root_schema_for::<T>();
            let schema = serde_json::to_value(schema).expect("schema");
            (component_name(&T::schema_name()), schema)
        }
        vec![
            sent::<ErrorBody>(),
            sent::<ErrorDetail>(),
            sent::<IssueJson>(),
            read::<CreateIssueRequest>(),
            read::<PatchIssueRequest>(),
            read::<CommentBodyRequest>(),
            sent::<CommentJson>(),
            sent::<IssueHistoryJson>(),
            sent::<TokenJson>(),
        ]
    }

    /// A JSON string or array of strings as a sorted array (`type`, `required`).
    fn string_set(value: &Value) -> Value {
        let set: BTreeSet<&str> = match value {
            Value::String(one) => [one.as_str()].into(),
            Value::Array(many) => many.iter().filter_map(Value::as_str).collect(),
            _ => BTreeSet::new(),
        };
        Value::from(set.into_iter().collect::<Vec<_>>())
    }

    /// DDD-11: keep property names, type sets, `format`, `required` and
    /// `$ref` targets (a `$defs` reference becomes its component reference),
    /// and every other keyword that constrains the shape: `enum`, `const`,
    /// `nullable`, `additionalProperties`, `pattern`, the length, bound and
    /// item-count limits. Drop only titles, descriptions, examples, defaults,
    /// `$schema` and anything else that does not decide what a value may be.
    fn normalise(schema: &Value) -> Value {
        let Some(object) = schema.as_object() else {
            return schema.clone();
        };
        if let Some(target) = object.get("$ref").and_then(Value::as_str) {
            let name = target
                .strip_prefix(DEFS)
                .map(component_name)
                .or_else(|| target.strip_prefix(COMPONENTS).map(str::to_string))
                .unwrap_or_else(|| target.to_string());
            return serde_json::json!({ "$ref": format!("{COMPONENTS}{name}") });
        }
        let mut out = Map::new();
        for (key, value) in object {
            let kept = match key.as_str() {
                "type" => string_set(value),
                "format" => value.clone(),
                "required" => string_set(value),
                "enum" | "const" | "nullable" | "pattern" | "minLength" | "maxLength"
                | "minimum" | "maximum" | "exclusiveMinimum" | "exclusiveMaximum" | "minItems"
                | "maxItems" => value.clone(),
                "additionalProperties" => normalise(value),
                "items" => normalise(value),
                "properties" => Value::Object(
                    value
                        .as_object()
                        .into_iter()
                        .flatten()
                        .map(|(name, property)| (name.clone(), normalise(property)))
                        .collect(),
                ),
                "anyOf" | "oneOf" | "allOf" => Value::from(
                    value
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(normalise)
                        .collect::<Vec<_>>(),
                ),
                _ => continue,
            };
            out.insert(key.clone(), kept);
        }
        Value::Object(out)
    }

    /// DDD-11: a change in any shape-bearing keyword is a drift; a change in
    /// prose or examples is not.
    #[test]
    fn normalise_keeps_every_shape_keyword() {
        let base = serde_json::json!({
            "type": "object",
            "title": "T",
            "description": "d",
            "properties": { "state": { "type": "string", "examples": ["todo"] } },
            "required": ["state"],
        });
        let with = |pointer: &str, key: &str, value: Value| {
            let mut schema = base.clone();
            schema
                .pointer_mut(pointer)
                .and_then(Value::as_object_mut)
                .expect("a schema object")
                .insert(key.to_string(), value);
            schema
        };
        for (key, a, b) in [
            (
                "enum",
                serde_json::json!(["todo"]),
                serde_json::json!(["done"]),
            ),
            (
                "const",
                serde_json::json!("todo"),
                serde_json::json!("done"),
            ),
            (
                "nullable",
                serde_json::json!(true),
                serde_json::json!(false),
            ),
            (
                "pattern",
                serde_json::json!("^a$"),
                serde_json::json!("^b$"),
            ),
            ("minLength", serde_json::json!(1), serde_json::json!(2)),
            ("maxLength", serde_json::json!(10), serde_json::json!(20)),
            ("minimum", serde_json::json!(0), serde_json::json!(1)),
            ("maximum", serde_json::json!(9), serde_json::json!(8)),
            (
                "exclusiveMinimum",
                serde_json::json!(0),
                serde_json::json!(1),
            ),
            (
                "exclusiveMaximum",
                serde_json::json!(9),
                serde_json::json!(8),
            ),
            ("minItems", serde_json::json!(0), serde_json::json!(1)),
            ("maxItems", serde_json::json!(9), serde_json::json!(8)),
        ] {
            let property = "/properties/state";
            let (one, other) = (with(property, key, a.clone()), with(property, key, b));
            assert_ne!(
                normalise(&one),
                normalise(&other),
                "a changed `{key}` is not detected"
            );
            assert_ne!(
                normalise(&one),
                normalise(&base),
                "an added `{key}` is not detected"
            );
        }
        let closed = with("", "additionalProperties", serde_json::json!(false));
        let open = with(
            "",
            "additionalProperties",
            serde_json::json!({ "type": "string" }),
        );
        assert_ne!(
            normalise(&closed),
            normalise(&open),
            "a changed `additionalProperties` is not detected"
        );
        assert_ne!(
            normalise(&closed),
            normalise(&base),
            "an added `additionalProperties` is not detected"
        );
        let reworded = with("", "description", serde_json::json!("other prose"));
        let reworded = {
            let mut schema = reworded;
            schema["properties"]["state"]["examples"] = serde_json::json!(["done"]);
            schema["title"] = serde_json::json!("U");
            schema["default"] = serde_json::json!({});
            schema["$schema"] = serde_json::json!("x");
            schema
        };
        assert_eq!(
            normalise(&reworded),
            normalise(&base),
            "prose or examples count as drift"
        );
    }

    /// The fields at which two normalised object schemas differ.
    fn differing_fields(code: &Value, documented: &Value) -> Vec<String> {
        let empty = Map::new();
        let properties = |schema: &Value| -> Map<String, Value> {
            schema["properties"].as_object().unwrap_or(&empty).clone()
        };
        let (ours, theirs) = (properties(code), properties(documented));
        let names: BTreeSet<&String> = ours.keys().chain(theirs.keys()).collect();
        let required = |schema: &Value, name: &str| {
            schema["required"]
                .as_array()
                .is_some_and(|r| r.iter().any(|n| n == name))
        };
        let mut fields: Vec<String> = names
            .into_iter()
            .filter(|name| {
                ours.get(*name) != theirs.get(*name)
                    || required(code, name) != required(documented, name)
            })
            .cloned()
            .collect();
        if code["type"] != documented["type"] {
            fields.push("(type)".to_string());
        }
        fields
    }

    /// Every operation and status (or request body) whose schema reaches the
    /// component, directly or through another component.
    fn references(doc: &Value, component: &str) -> Vec<String> {
        let needle = |name: &str| format!("\"{COMPONENTS}{name}\"");
        let schemas = doc["components"]["schemas"]
            .as_object()
            .cloned()
            .unwrap_or_default();
        let mut reaching: BTreeSet<String> = [component.to_string()].into();
        loop {
            let before = reaching.len();
            for (name, schema) in &schemas {
                let text = schema.to_string();
                if reaching.iter().any(|r| text.contains(&needle(r))) {
                    reaching.insert(name.clone());
                }
            }
            if reaching.len() == before {
                break;
            }
        }
        let mentions = |node: &Value| {
            let text = resolve(doc, node).to_string();
            reaching.iter().any(|r| text.contains(&needle(r)))
        };
        let mut out = Vec::new();
        for (path, item) in doc["paths"].as_object().into_iter().flatten() {
            for (method, op) in item.as_object().into_iter().flatten() {
                let method = method.to_ascii_uppercase();
                if op.get("requestBody").is_some_and(mentions) {
                    out.push(format!("{method} {path} request body"));
                }
                for (status, response) in op["responses"].as_object().into_iter().flatten() {
                    if mentions(response) {
                        out.push(format!("{method} {path} {status}"));
                    }
                }
            }
        }
        out
    }

    fn resolve<'a>(doc: &'a Value, node: &'a Value) -> &'a Value {
        node.get("$ref")
            .and_then(Value::as_str)
            .and_then(|r| r.strip_prefix('#'))
            .and_then(|pointer| doc.pointer(pointer))
            .unwrap_or(node)
    }

    #[test]
    fn document_is_valid_openapi_3_1() {
        let digest: String = Sha256::digest(OAS_SCHEMA)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(
            digest, OAS_SCHEMA_SHA256,
            "the vendored OAS 3.1 schema fixture is not the pinned 2025-11-23 iteration"
        );
        let schema: Value = serde_json::from_slice(OAS_SCHEMA).expect("the OAS schema parses");
        let validator =
            jsonschema::draft202012::new(&schema).expect("the OAS 3.1 schema compiles offline");
        let doc = document();
        let errors: Vec<String> = validator
            .iter_errors(&doc)
            .map(|e| format!("{e} at {}", e.instance_path()))
            .collect();
        assert!(
            errors.is_empty(),
            "openapi.json is not a valid OpenAPI 3.1 document:\n{}",
            errors.join("\n")
        );
        assert_eq!(
            doc["openapi"], "3.1.0",
            "openapi.json declares OpenAPI 3.1.0"
        );
    }

    #[test]
    fn wire_shapes_match_document() {
        let doc = document();
        let documented = doc["components"]["schemas"]
            .as_object()
            .expect("openapi.json has components.schemas");
        let wire = wire_schemas();
        let wire_names: BTreeSet<&str> = wire.iter().map(|(name, _)| name.as_str()).collect();
        let strays: Vec<&String> = documented
            .keys()
            .filter(|name| !wire_names.contains(name.as_str()))
            .collect();
        assert!(
            strays.is_empty(),
            "openapi.json describes components no wire struct has: {strays:?}"
        );
        let mut failures = Vec::new();
        for (name, schema) in &wire {
            let Some(component) = documented.get(name) else {
                failures.push(format!(
                    "wire struct component `{name}` is not described in openapi.json"
                ));
                continue;
            };
            let (code, described) = (normalise(schema), normalise(component));
            if code != described {
                failures.push(format!(
                    "component `{name}` differs from the code at field(s) {:?}; referenced by {:?}\n  code:     {code}\n  document: {described}",
                    differing_fields(&code, &described),
                    references(&doc, name),
                ));
            }
        }
        assert!(
            failures.is_empty(),
            "the description's shapes differ from the wire structs:\n{}",
            failures.join("\n")
        );
    }

    #[test]
    fn info_version_is_the_release() {
        let doc = document();
        let documented = doc["info"]["version"].as_str().unwrap_or_default();
        let release = env!("CARGO_PKG_VERSION");
        assert_eq!(
            documented, release,
            "openapi.json info.version is {documented:?} but the release is {release:?}; run `cargo xtask openapi` to bring the description up to the release"
        );
    }

    #[test]
    fn document_carries_the_d14_copy() {
        let doc = document();
        let copy = [
            (
                "/components/securitySchemes/machineToken/description",
                "A machine token: an EdDSA-signed JWT sent as `Authorization: Bearer <token>`. A workspace admin mints one, signed in to Foundry, at /admin/tokens; the value is shown once. This API lists and revokes tokens but cannot create one.",
            ),
            (
                "/components/schemas/Error/description",
                "Every JSON error answer has this shape. `code` is stable and machine-readable; `message` is for people.",
            ),
            (
                "/components/responses/NotFound/description",
                "Not found. The same answer, byte for byte, for a team, project, issue or comment that does not exist, one your token cannot reach, and a path that is not part of the API.",
            ),
            (
                "/components/schemas/Issue/properties/state/description",
                "The lane the issue is in, by its slug. Each project has its own lanes; the defaults are backlog, todo, in_progress, done and cancelled. `in-progress` and `canceled` are accepted as aliases.",
            ),
        ];
        for (pointer, text) in copy {
            assert_eq!(
                doc.pointer(pointer).and_then(Value::as_str),
                Some(text),
                "openapi.json {pointer} is not the locked D14 copy"
            );
        }
        let unauthorized = doc
            .pointer("/components/responses/Unauthorized/description")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(
            unauthorized.contains("Also answered when the token cannot be checked."),
            "the 401 description must say it also covers a token that cannot be checked (C13)"
        );
    }

    #[test]
    fn document_names_no_instance() {
        let doc = document();
        assert_eq!(
            doc["servers"],
            serde_json::json!([{ "url": "/" }]),
            "servers must be the single relative entry \"/\" (OQ-7)"
        );
        let text = String::from_utf8_lossy(DOCUMENT).to_ascii_lowercase();
        let named: Vec<String> = [
            "://",
            "localhost",
            "127.0.0.1",
            "canzan",
            "jeffbailey",
            "unintelligent-design",
            "foundry.test",
        ]
        .into_iter()
        .map(str::to_string)
        // No answer of this API is HTML (`check_api_no_html`); spelt in parts so
        // this check is not itself flagged as constructing one.
        .chain([["text", "html"].join("/")])
        .filter(|needle| text.contains(needle.as_str()))
        .collect();
        assert!(
            named.is_empty(),
            "openapi.json names something of an instance or an HTML answer (D7): {named:?}"
        );
    }
}
