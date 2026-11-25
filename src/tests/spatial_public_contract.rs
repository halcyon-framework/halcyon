use std::fs;
use std::path::Path;

const SPATIAL_SOURCE_FILE: &str = "src/model/spatial/mod.rs";
const PLOT_SPATIAL_SOURCE_FILE: &str = "src/plot/spatial.rs";
const DOMAIN_NOUNS: &[&str] = &[
    "Candle", "Canvas", "Chart", "Graph", "Interval", "Node", "Plot", "Port", "Price", "Probe",
    "Series", "Temporal", "Timeline", "Wire",
];
const PLOT_DOMAIN_AUTHORITY_WORDS: &[&str] = &[
    "Axis", "axis", "Series", "series", "Probe", "probe", "Tool", "tool", "Candle", "candle",
];
const SPATIAL_FORBIDDEN_AUTHORITY_WORDS: &[&str] = &[
    "crate::plot",
    "crate::widgets::timeline",
    "crate::model::time",
    "TimeGeometry",
    "Timestamp",
    "ViewState",
    "AxisAuthority",
    "AxisKind",
    "Series",
    "Candle",
    "Probe",
    "TemporalSubstrate",
    "TimelineCanvas",
    "NodeGraphDocument",
    "GraphTransaction",
    "PriceLevel",
];

#[test]
fn spatial_public_names_remain_domain_neutral() {
    let content = spatial_source();

    let mut violations = Vec::new();
    let mut enum_depth = 0usize;
    let mut current_enum: Option<String> = None;

    for (line_index, line) in content.lines().enumerate() {
        let code = line.split("//").next().unwrap_or("").trim();
        if code.is_empty() || code.starts_with("#[") {
            continue;
        }

        if let Some(enum_name) = &current_enum {
            if enum_depth == 1 {
                if let Some(variant) = upper_camel_token(code) {
                    if let Some(noun) = domain_noun_in(variant) {
                        violations.push(format!(
                            "{}:{}: {}::{} contains domain noun '{}'",
                            SPATIAL_SOURCE_FILE,
                            line_index + 1,
                            enum_name,
                            variant,
                            noun
                        ));
                    }
                }
            }
            enum_depth = enum_depth_after(enum_depth, code);
            if enum_depth == 0 {
                current_enum = None;
            }
            continue;
        }

        if let Some(name) = public_spatial_type_name(code) {
            if name != "SpatialAxisOrientation"
                && let Some(noun) = domain_noun_in(name)
            {
                violations.push(format!(
                    "{}:{}: public {} contains domain noun '{}'",
                    SPATIAL_SOURCE_FILE,
                    line_index + 1,
                    name,
                    noun
                ));
            }
            if code.starts_with("pub enum ") {
                enum_depth = enum_depth_after(0, code);
                current_enum = (enum_depth > 0).then(|| name.to_string());
            }
        }
    }

    assert!(
        violations.is_empty(),
        "SPEC-HAL-343 violation: public Spatial* names must stay domain-neutral\n{}",
        violations.join("\n")
    );
}

#[test]
fn spatial_projection_public_api_has_no_mutation_channel() {
    let content = spatial_source();
    let mut violations = Vec::new();
    let mut struct_depth = 0usize;
    let mut impl_depth = 0usize;
    let mut inside_projection_struct = false;
    let mut inside_projection_impl = false;
    let mut pending_signature: Option<(usize, String)> = None;

    for (line_index, line) in content.lines().enumerate() {
        let code = line.split("//").next().unwrap_or("").trim();
        if code.is_empty() || code.starts_with("#[") {
            continue;
        }

        if inside_projection_struct {
            if struct_depth == 1 && code.starts_with("pub ") {
                violations.push(format!(
                    "{}:{}: SpatialProjection field is public",
                    SPATIAL_SOURCE_FILE,
                    line_index + 1
                ));
            }
            struct_depth = enum_depth_after(struct_depth, code);
            if struct_depth == 0 {
                inside_projection_struct = false;
            }
            continue;
        }

        if inside_projection_impl {
            if let Some((start_line, signature)) = &mut pending_signature {
                signature.push(' ');
                signature.push_str(code);
                if code.contains('{') || code.ends_with(';') {
                    push_mutation_signature_violation(*start_line, signature, &mut violations);
                    pending_signature = None;
                }
            } else if impl_depth == 1 && code.starts_with("pub fn ") {
                if code.contains('{') || code.ends_with(';') {
                    push_mutation_signature_violation(line_index + 1, code, &mut violations);
                } else {
                    pending_signature = Some((line_index + 1, code.to_string()));
                }
            }

            impl_depth = enum_depth_after(impl_depth, code);
            if impl_depth == 0 {
                inside_projection_impl = false;
                pending_signature = None;
            }
            continue;
        }

        if code.starts_with("pub struct SpatialProjection") {
            struct_depth = enum_depth_after(0, code);
            inside_projection_struct = struct_depth > 0;
            continue;
        }

        if code.starts_with("impl SpatialProjection") {
            impl_depth = enum_depth_after(0, code);
            inside_projection_impl = impl_depth > 0;
        }
    }

    assert!(
        violations.is_empty(),
        "SPEC-HAL-343 violation: SpatialProjection must expose no mutation channel\n{}",
        violations.join("\n")
    );
}

#[test]
fn spatial_adapter_trait_accepts_only_immutable_inputs() {
    let content = spatial_source();
    let signatures = spatial_adapter_trait_signatures(&content);
    let mut violations = Vec::new();

    let build_projection = signatures
        .iter()
        .find(|(_, signature)| trait_fn_name(signature) == Some("build_projection"));
    let translate_intent = signatures
        .iter()
        .find(|(_, signature)| trait_fn_name(signature) == Some("translate_intent"));

    let Some((build_line, build_signature)) = build_projection else {
        panic!("SPEC-HAL-343: SpatialAdapter::build_projection signature not found");
    };
    let Some((translate_line, translate_signature)) = translate_intent else {
        panic!("SPEC-HAL-343: SpatialAdapter::translate_intent signature not found");
    };

    require_signature_fragments(
        *build_line,
        build_signature,
        &[
            "&self",
            "domain: &Self::DomainSnapshot",
            "session: &Self::SessionSnapshot",
            "viewport: Option<&SpatialViewportState>",
            "orientation: Option<&SpatialAxisOrientation>",
            "policy: &Self::PolicyInput",
            "-> Result<SpatialProjection, SpatialProjectionError>",
        ],
        &mut violations,
    );
    require_signature_fragments(
        *translate_line,
        translate_signature,
        &[
            "&self",
            "intent: &SpatialIntent",
            "context: &Self::Context",
            "-> Option<Self::DomainIntent>",
        ],
        &mut violations,
    );

    for (line_number, signature) in [
        (*build_line, build_signature),
        (*translate_line, translate_signature),
    ] {
        if signature.contains("&mut") {
            violations.push(format!(
                "{}:{}: SpatialAdapter signature exposes mutable input: {}",
                SPATIAL_SOURCE_FILE, line_number, signature
            ));
        }
    }

    assert!(
        violations.is_empty(),
        "SPEC-HAL-343 violation: SpatialAdapter must translate immutable domain state into spatial projections\n{}",
        violations.join("\n")
    );
}

#[test]
fn plot_spatial_adapter_excludes_plot_domain_rendering_authority() {
    let content = source_file(PLOT_SPATIAL_SOURCE_FILE);
    let mut violations = Vec::new();

    for (line_index, line) in content.lines().enumerate() {
        let code = line
            .split("//")
            .next()
            .unwrap_or("")
            .trim()
            .replace("SpatialAxisOrientation", "");
        if code.is_empty() || code.starts_with("#[") {
            continue;
        }

        for word in PLOT_DOMAIN_AUTHORITY_WORDS {
            if code.contains(word) {
                violations.push(format!(
                    "{}:{}: Plot spatial adapter contains plot-owned rendering authority word '{}': {}",
                    PLOT_SPATIAL_SOURCE_FILE,
                    line_index + 1,
                    word,
                    code
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "SPEC-HAL-343 violation: Plot axes, series, probes, and tools must remain outside the spatial adapter\n{}",
        violations.join("\n")
    );
}

#[test]
fn spatial_substrate_excludes_domain_coordinate_authorities() {
    let content = spatial_source();
    let mut violations = Vec::new();

    for (line_index, line) in content.lines().enumerate() {
        let code = line.split("//").next().unwrap_or("").trim();
        if code.is_empty() || code.starts_with("#[") {
            continue;
        }

        for word in SPATIAL_FORBIDDEN_AUTHORITY_WORDS {
            if code.contains(word) {
                violations.push(format!(
                    "{}:{}: spatial substrate contains forbidden domain authority word '{}': {}",
                    SPATIAL_SOURCE_FILE,
                    line_index + 1,
                    word,
                    code
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "SPEC-HAL-343 violation: spatial substrate must not own plot, timeline, or domain coordinate authority\n{}",
        violations.join("\n")
    );
}

fn spatial_source() -> String {
    source_file(SPATIAL_SOURCE_FILE)
}

fn source_file(file_path: &str) -> String {
    let path = Path::new(file_path);
    assert!(
        path.exists(),
        "SPEC-HAL-343: spatial source file not found: {}",
        file_path
    );
    fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("SPEC-HAL-343: failed to read {}: {}", file_path, err))
}

fn public_spatial_type_name(code: &str) -> Option<&str> {
    ["pub struct ", "pub enum ", "pub trait "]
        .iter()
        .find_map(|prefix| code.strip_prefix(prefix))
        .and_then(|rest| {
            rest.split(|ch: char| ch == '<' || ch == '{' || ch.is_whitespace())
                .next()
        })
        .filter(|name| name.starts_with("Spatial"))
}

fn upper_camel_token(code: &str) -> Option<&str> {
    let first = code.chars().next()?;
    if !first.is_ascii_uppercase() {
        return None;
    }
    code.split(|ch: char| ch == '(' || ch == '{' || ch == ',' || ch.is_whitespace())
        .next()
        .filter(|token| !token.is_empty())
}

fn domain_noun_in(name: &str) -> Option<&'static str> {
    DOMAIN_NOUNS
        .iter()
        .copied()
        .find(|noun| name.contains(noun))
}

fn enum_depth_after(depth: usize, code: &str) -> usize {
    let opens = code.bytes().filter(|byte| *byte == b'{').count();
    let closes = code.bytes().filter(|byte| *byte == b'}').count();
    depth.saturating_add(opens).saturating_sub(closes)
}

fn push_mutation_signature_violation(
    line_number: usize,
    signature: &str,
    violations: &mut Vec<String>,
) {
    if signature.contains("&mut self") || signature.contains("-> &mut") {
        violations.push(format!(
            "{}:{}: mutable SpatialProjection accessor: {}",
            SPATIAL_SOURCE_FILE, line_number, signature
        ));
        return;
    }

    if public_fn_name(signature).is_some_and(|name| name.ends_with("_mut")) {
        violations.push(format!(
            "{}:{}: mutable-style SpatialProjection accessor name: {}",
            SPATIAL_SOURCE_FILE, line_number, signature
        ));
    }
}

fn spatial_adapter_trait_signatures(content: &str) -> Vec<(usize, String)> {
    let mut signatures = Vec::new();
    let mut trait_depth = 0usize;
    let mut inside_adapter_trait = false;
    let mut pending_signature: Option<(usize, String)> = None;

    for (line_index, line) in content.lines().enumerate() {
        let code = line.split("//").next().unwrap_or("").trim();
        if code.is_empty() || code.starts_with("#[") {
            continue;
        }

        if inside_adapter_trait {
            if let Some((start_line, signature)) = &mut pending_signature {
                signature.push(' ');
                signature.push_str(code);
                if code.contains('{') || code.ends_with(';') {
                    signatures.push((*start_line, signature.clone()));
                    pending_signature = None;
                }
            } else if trait_depth == 1 && code.starts_with("fn ") {
                if code.contains('{') || code.ends_with(';') {
                    signatures.push((line_index + 1, code.to_string()));
                } else {
                    pending_signature = Some((line_index + 1, code.to_string()));
                }
            }

            trait_depth = enum_depth_after(trait_depth, code);
            if trait_depth == 0 {
                inside_adapter_trait = false;
                pending_signature = None;
            }
            continue;
        }

        if code.starts_with("pub trait SpatialAdapter") {
            trait_depth = enum_depth_after(0, code);
            inside_adapter_trait = trait_depth > 0;
        }
    }

    signatures
}

fn require_signature_fragments(
    line_number: usize,
    signature: &str,
    required: &[&str],
    violations: &mut Vec<String>,
) {
    for fragment in required {
        if !signature.contains(fragment) {
            violations.push(format!(
                "{}:{}: SpatialAdapter signature missing '{}': {}",
                SPATIAL_SOURCE_FILE, line_number, fragment, signature
            ));
        }
    }
}

fn trait_fn_name(signature: &str) -> Option<&str> {
    signature
        .strip_prefix("fn ")
        .and_then(|rest| {
            rest.split(|ch: char| ch == '(' || ch.is_whitespace())
                .next()
        })
        .filter(|name| !name.is_empty())
}

fn public_fn_name(signature: &str) -> Option<&str> {
    signature
        .strip_prefix("pub fn ")
        .and_then(|rest| {
            rest.split(|ch: char| ch == '(' || ch.is_whitespace())
                .next()
        })
        .filter(|name| !name.is_empty())
}
