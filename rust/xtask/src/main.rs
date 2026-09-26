#![allow(clippy::collapsible_if, clippy::needless_borrows_for_generic_args)]

use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, bail};
use openapiv3::OpenAPI;
use progenitor::{GenerationSettings, Generator, InterfaceStyle, TagStyle};
use quote::quote;
use serde_yaml::{Mapping, Value};

fn main() -> anyhow::Result<()> {
    let command = std::env::args().nth(1).unwrap_or_default();
    match command.as_str() {
        "generate-api" => generate_api(),
        _ => bail!("usage: cargo run -p xtask -- generate-api"),
    }
}

fn generate_api() -> anyhow::Result<()> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .context("could not locate repository root")?;
    let spec_path = repo_root.join("openapi/openapi.yaml");
    let output_path = repo_root.join("rust/trustify-client/src/api_generated.rs");

    let source = fs::read_to_string(&spec_path)
        .with_context(|| format!("reading {}", spec_path.display()))?;
    let mut document: Value = serde_yaml::from_str(&source).context("parsing OpenAPI YAML")?;
    normalize_openapi_31(&mut document)?;
    let spec: OpenAPI = serde_yaml::from_value(document).context("loading normalized OpenAPI")?;

    let mut settings = GenerationSettings::default();
    settings
        .with_interface(InterfaceStyle::Builder)
        .with_tag(TagStyle::Separate)
        .with_inner_type(quote! { crate::auth::ClientContext })
        .with_pre_hook_async(quote! { crate::client::prepare_request });

    let mut generator = Generator::new(&settings);
    let tokens = generator
        .generate_tokens(&spec)
        .context("generating Trustify API bindings")?;
    let syntax = syn::parse2(tokens).context("parsing generated Rust")?;
    let output = prettyplease::unparse(&syntax);
    fs::write(&output_path, output)
        .with_context(|| format!("writing {}", output_path.display()))?;

    println!("Generated {}", output_path.display());
    Ok(())
}

fn normalize_openapi_31(document: &mut Value) -> anyhow::Result<()> {
    let root = document
        .as_mapping_mut()
        .context("OpenAPI document must be a mapping")?;
    let version = get_string(root, "openapi")?;
    if !version.starts_with("3.1.") {
        bail!("expected an OpenAPI 3.1 document, found {version}");
    }

    let nullable_components = strip_nullable_component_schemas(document)?;
    normalize_value(document, &nullable_components, false)?;
    let root = document
        .as_mapping_mut()
        .context("normalized OpenAPI document must be a mapping")?;
    root.insert(key("openapi"), Value::String("3.0.3".to_owned()));

    if let Some(Value::Mapping(info)) = root.get_mut(&key("info")) {
        if let Some(Value::Mapping(license)) = info.get_mut(&key("license")) {
            license.remove(&key("identifier"));
        }
    }

    normalize_path_parameters(root)?;
    Ok(())
}

fn strip_nullable_component_schemas(document: &mut Value) -> anyhow::Result<HashSet<String>> {
    let root = document
        .as_mapping_mut()
        .context("OpenAPI document must be a mapping")?;
    let Some(Value::Mapping(components)) = root.get_mut(&key("components")) else {
        return Ok(HashSet::new());
    };
    let Some(Value::Mapping(schemas)) = components.get_mut(&key("schemas")) else {
        return Ok(HashSet::new());
    };

    let mut nullable_components = HashSet::new();
    for (name, schema) in schemas.iter_mut() {
        let Some(schema) = schema.as_mapping_mut() else {
            continue;
        };

        if let Some(Value::Sequence(types)) = schema.get(&key("type")) {
            if types.iter().any(|value| value.as_str() == Some("null")) {
                let concrete = types
                    .iter()
                    .filter(|value| value.as_str() != Some("null"))
                    .cloned()
                    .collect::<Vec<_>>();
                if concrete.len() != 1 {
                    bail!("unsupported nullable component type union for {name:?}");
                }
                schema.insert(key("type"), concrete[0].clone());
                nullable_components.insert(
                    name.as_str()
                        .context("component schema name must be a string")?
                        .to_owned(),
                );
            }
        }

        for union_name in ["oneOf", "anyOf"] {
            let Some(Value::Sequence(branches)) = schema.get(&key(union_name)) else {
                continue;
            };
            if branches.iter().any(is_null_schema) {
                let remaining = branches
                    .iter()
                    .filter(|branch| !is_null_schema(branch))
                    .cloned()
                    .collect::<Vec<_>>();
                if remaining.is_empty() {
                    bail!("nullable component {name:?} has no non-null schema branch");
                }
                schema.insert(key(union_name), Value::Sequence(remaining));
                nullable_components.insert(
                    name.as_str()
                        .context("component schema name must be a string")?
                        .to_owned(),
                );
            }
        }
    }
    Ok(nullable_components)
}

fn normalize_value(
    value: &mut Value,
    nullable_components: &HashSet<String>,
    within_nullable_schema: bool,
) -> anyhow::Result<()> {
    match value {
        Value::Sequence(items) => {
            for item in items {
                normalize_value(item, nullable_components, within_nullable_schema)?;
            }
        }
        Value::Mapping(mapping) => {
            let children_within_nullable = within_nullable_schema
                || mapping.get(&key("nullable")).and_then(Value::as_bool) == Some(true)
                || has_nullable_type_union(mapping)
                || has_nullable_union(mapping);
            for item in mapping.values_mut() {
                normalize_value(item, nullable_components, children_within_nullable)?;
            }
            normalize_type_union(mapping)?;
            normalize_nullable_unions(mapping)?;
            normalize_media_type(mapping)?;
            normalize_nullable_header_parameter(mapping);
            normalize_nullable_component_ref(mapping, nullable_components, within_nullable_schema);
        }
        _ => {}
    }
    Ok(())
}

fn has_nullable_type_union(mapping: &Mapping) -> bool {
    mapping
        .get(&key("type"))
        .and_then(Value::as_sequence)
        .is_some_and(|types| types.iter().any(|value| value.as_str() == Some("null")))
}

fn has_nullable_union(mapping: &Mapping) -> bool {
    ["oneOf", "anyOf"].iter().any(|name| {
        mapping
            .get(&key(name))
            .and_then(Value::as_sequence)
            .is_some_and(|branches| branches.iter().any(is_null_schema))
    })
}

fn normalize_nullable_component_ref(
    mapping: &mut Mapping,
    nullable_components: &HashSet<String>,
    within_nullable_schema: bool,
) {
    if within_nullable_schema
        || mapping.get(&key("nullable")).and_then(Value::as_bool) == Some(true)
    {
        return;
    }
    let Some(reference) = mapping.get(&key("$ref")).and_then(Value::as_str) else {
        return;
    };
    let Some(name) = reference.strip_prefix("#/components/schemas/") else {
        return;
    };
    if !nullable_components.contains(name) {
        return;
    }

    let reference = mapping.remove(&key("$ref")).expect("reference exists");
    let mut ref_schema = Mapping::new();
    ref_schema.insert(key("$ref"), reference);
    let mut wrapper = Mapping::new();
    wrapper.insert(
        key("allOf"),
        Value::Sequence(vec![Value::Mapping(ref_schema)]),
    );
    for (name, value) in std::mem::take(mapping) {
        wrapper.entry(name).or_insert(value);
    }
    wrapper.insert(key("nullable"), Value::Bool(true));
    *mapping = wrapper;
}

fn normalize_type_union(mapping: &mut Mapping) -> anyhow::Result<()> {
    let type_key = key("type");
    let Some(Value::Sequence(types)) = mapping.get(&type_key) else {
        return Ok(());
    };

    let nullable = types.iter().any(|value| value.as_str() == Some("null"));
    let concrete_types = types
        .iter()
        .filter(|value| value.as_str() != Some("null"))
        .cloned()
        .collect::<Vec<_>>();
    if concrete_types.len() != 1 {
        bail!("unsupported OpenAPI 3.1 type union: {types:?}");
    }

    mapping.insert(type_key, concrete_types[0].clone());
    if nullable {
        mapping.insert(key("nullable"), Value::Bool(true));
    }
    Ok(())
}

fn normalize_nullable_unions(mapping: &mut Mapping) -> anyhow::Result<()> {
    for union_name in ["oneOf", "anyOf"] {
        let union_key = key(union_name);
        let Some(Value::Sequence(branches)) = mapping.get(&union_key) else {
            continue;
        };
        let null_indexes = branches
            .iter()
            .enumerate()
            .filter_map(|(index, branch)| is_null_schema(branch).then_some(index))
            .collect::<Vec<_>>();
        if null_indexes.is_empty() {
            continue;
        }
        if null_indexes.len() != 1 {
            bail!("unsupported nullable {union_name} with multiple null branches");
        }

        let concrete = branches
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != null_indexes[0])
            .map(|(_, branch)| branch.clone())
            .collect::<Vec<_>>();
        mapping.remove(&union_key);

        if concrete.len() == 1 {
            let mut branch = concrete.into_iter().next().expect("one concrete branch");
            if let Value::Mapping(branch_map) = &mut branch {
                if branch_map.contains_key(&key("$ref")) {
                    if let Some(description) = branch_map.remove(&key("description")) {
                        mapping.entry(key("description")).or_insert(description);
                    }
                    mapping.insert(key("allOf"), Value::Sequence(vec![branch]));
                } else {
                    for (branch_key, branch_value) in branch_map.iter() {
                        mapping
                            .entry(branch_key.clone())
                            .or_insert_with(|| branch_value.clone());
                    }
                }
            } else {
                bail!("nullable {union_name} branch must be a schema mapping");
            }
        } else {
            let mut wrapper = Mapping::new();
            wrapper.insert(union_key, Value::Sequence(concrete));
            mapping.insert(key("allOf"), Value::Sequence(vec![Value::Mapping(wrapper)]));
        }
        mapping.insert(key("nullable"), Value::Bool(true));
    }
    Ok(())
}

fn normalize_media_type(mapping: &mut Mapping) -> anyhow::Result<()> {
    if let Some(value) = mapping.remove(&key("application/merge-patch+json")) {
        mapping.insert(key("application/json"), value);
    }

    if let Some(Value::Mapping(media)) = mapping.get_mut(&key("text/plain")) {
        if let Some(Value::Mapping(schema)) = media.get_mut(&key("schema")) {
            if schema.get(&key("type")).and_then(Value::as_str) == Some("boolean") {
                // Plain-text booleans are sent as the unchanged strings
                // `true` and `false`; Progenitor models text bodies as String.
                schema.insert(key("type"), Value::String("string".to_owned()));
            }
        }
    }

    if let Some(Value::Mapping(media)) = mapping.get_mut(&key("application/octet-stream")) {
        if let Some(Value::Mapping(schema)) = media.get_mut(&key("schema")) {
            schema.insert(key("type"), Value::String("string".to_owned()));
            schema.insert(key("format"), Value::String("binary".to_owned()));
            schema.remove(&key("items"));
        }
    }

    Ok(())
}

fn normalize_nullable_header_parameter(mapping: &mut Mapping) {
    if mapping.get(&key("in")).and_then(Value::as_str) != Some("header") {
        return;
    }
    if let Some(Value::Mapping(schema)) = mapping.get_mut(&key("schema")) {
        // Progenitor already represents a non-required header as Option<T>;
        // retaining schema nullability creates an unsupported Option<Option<T>>.
        schema.remove(&key("nullable"));
    }
}

fn normalize_path_parameters(root: &mut Mapping) -> anyhow::Result<()> {
    let Some(Value::Mapping(paths)) = root.get_mut(&key("paths")) else {
        bail!("OpenAPI document is missing paths");
    };

    for (path_value, path_item) in paths.iter_mut() {
        let path = path_value
            .as_str()
            .context("OpenAPI path key must be a string")?;
        let placeholders = path_placeholders(path);
        let Some(path_item) = path_item.as_mapping_mut() else {
            continue;
        };

        if let Some(Value::Sequence(parameters)) = path_item.get_mut(&key("parameters")) {
            normalize_parameter_list(parameters, &placeholders, false);
        }

        for (method, operation) in path_item.iter_mut() {
            if !matches!(
                method.as_str(),
                Some("get" | "put" | "post" | "delete" | "patch" | "head" | "options")
            ) {
                continue;
            }
            let Some(operation) = operation.as_mapping_mut() else {
                continue;
            };
            let is_upload_sbom = path == "/api/v3/sbom"
                && operation.get(&key("operationId")).and_then(Value::as_str) == Some("uploadSbom");
            if let Some(Value::Sequence(parameters)) = operation.get_mut(&key("parameters")) {
                normalize_parameter_list(parameters, &placeholders, is_upload_sbom);
            }
        }
    }
    Ok(())
}

fn normalize_parameter_list(
    parameters: &mut [Value],
    placeholders: &HashSet<String>,
    upload_sbom: bool,
) {
    for parameter in parameters {
        let Some(parameter) = parameter.as_mapping_mut() else {
            continue;
        };
        let is_unmatched_path_parameter = parameter.get(&key("in")).and_then(Value::as_str)
            == Some("path")
            && parameter
                .get(&key("name"))
                .and_then(Value::as_str)
                .is_some_and(|name| !placeholders.contains(name));
        if is_unmatched_path_parameter {
            parameter.insert(key("in"), Value::String("query".to_owned()));
        }
        if upload_sbom && parameter.get(&key("in")).and_then(Value::as_str) == Some("query") {
            parameter.insert(key("required"), Value::Bool(false));
        }
    }
}

fn path_placeholders(path: &str) -> HashSet<String> {
    let mut placeholders = HashSet::new();
    let mut rest = path;
    while let Some(open) = rest.find('{') {
        let after_open = &rest[open + 1..];
        let Some(close) = after_open.find('}') else {
            break;
        };
        placeholders.insert(after_open[..close].to_owned());
        rest = &after_open[close + 1..];
    }
    placeholders
}

fn is_null_schema(value: &Value) -> bool {
    value
        .as_mapping()
        .and_then(|mapping| mapping.get(&key("type")))
        .and_then(Value::as_str)
        == Some("null")
}

fn get_string<'a>(mapping: &'a Mapping, name: &str) -> anyhow::Result<&'a str> {
    mapping
        .get(&key(name))
        .and_then(Value::as_str)
        .with_context(|| format!("OpenAPI document is missing string field `{name}`"))
}

fn key(name: &str) -> Value {
    Value::String(name.to_owned())
}

#[cfg(test)]
mod tests {
    use serde_yaml::Value;

    use super::{normalize_openapi_31, normalize_value};

    #[test]
    fn translates_nullable_type_arrays_and_refs_for_openapi_30() {
        let mut document: Value = serde_yaml::from_str(
            "openapi: 3.1.0\ninfo: {title: Test, version: '1'}\npaths: {}\ncomponents:\n  schemas:\n    MaybeText:\n      type: [string, 'null']\n    Envelope:\n      type: object\n      properties:\n        value:\n          $ref: '#/components/schemas/MaybeText'\n",
        )
        .unwrap();

        normalize_openapi_31(&mut document).unwrap();
        let schemas = document["components"]["schemas"].as_mapping().unwrap();
        assert_eq!(schemas["MaybeText"]["type"], "string");
        assert!(schemas["MaybeText"].get("nullable").is_none());
        assert_eq!(schemas["Envelope"]["properties"]["value"]["nullable"], true);
        assert_eq!(
            schemas["Envelope"]["properties"]["value"]["allOf"][0]["$ref"],
            "#/components/schemas/MaybeText"
        );
    }

    #[test]
    fn nullable_multi_branch_union_is_wrapped_in_all_of() {
        let mut value: Value = serde_yaml::from_str(
            "oneOf:\n  - {type: 'null'}\n  - {type: string}\n  - {type: integer}\n",
        )
        .unwrap();
        normalize_value(&mut value, &std::collections::HashSet::new(), false).unwrap();
        assert_eq!(value["nullable"], true);
        assert_eq!(value["allOf"][0]["oneOf"].as_sequence().unwrap().len(), 2);
    }
}
