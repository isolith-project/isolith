//! A cleaner sketch<->feature mapping (replaces the earlier draft with the
//! payload under the stable `sketch` parameter key).

use crate::sketch::Sketch;
use crate::varset::VariableSet;
use oep_geo::Feature;

/// Feature kind tag for a sketch.
pub const SKETCH_KIND: &str = "Sketch";
/// Feature kind tag for a variable set.
pub const VARSET_KIND: &str = "VariableSet";

/// Deterministic feature id for a sketch feature.
pub fn sketch_feature_id(sketch_id: &str) -> String {
    format!("oep-feature-sketch-{sketch_id}")
}

/// Deterministic feature id for a variable-set feature.
pub fn varset_feature_id(tag: &str) -> String {
    format!("oep-feature-varset-{tag}")
}

/// Encode a sketch as an `oep-geo` `Feature` (kind `Sketch`).
///
/// The whole sketch (our own serde model) rides in the `sketch` parameter as
/// canonical JSON; the `parameters` map is a `BTreeMap`, so serialization is
/// deterministic. Feature id: `oep-feature-sketch-<id>`.
pub fn sketch_to_feature(sketch: &Sketch) -> Feature {
    let value = serde_json::to_value(sketch).unwrap_or_else(|_| serde_json::json!({}));
    Feature::new(
        sketch_feature_id(&sketch.id),
        sketch.id.clone(),
        SKETCH_KIND,
    )
    .with_param("sketch", value)
}

/// Decode a `Feature` back into a sketch, exact inverse of [`sketch_to_feature`].
pub fn sketch_from_feature(feature: &Feature) -> Result<Sketch, String> {
    if feature.kind != SKETCH_KIND {
        return Err(format!("feature kind {} is not a sketch", feature.kind));
    }
    let value = feature
        .parameters
        .get("sketch")
        .ok_or_else(|| "sketch feature has no sketch payload".to_string())?;
    serde_json::from_value(value.clone()).map_err(|e| format!("bad sketch payload: {e}"))
}

/// Encode a variable set as an `oep-geo` `Feature` (kind `VariableSet`).
///
/// The sorted `name -> value` map is stored under `variables`. Feature id:
/// `oep-feature-varset-<first-name>` (or `-empty`).
pub fn varset_to_feature(vars: &VariableSet) -> Feature {
    let value = serde_json::to_value(&vars.variables).unwrap_or_else(|_| serde_json::json!({}));
    let tag = vars
        .names()
        .next()
        .cloned()
        .unwrap_or_else(|| "empty".to_string());
    Feature::new(varset_feature_id(&tag), tag, VARSET_KIND).with_param("variables", value)
}

/// Decode a `Feature` back into a variable set.
pub fn varset_from_feature(feature: &Feature) -> Result<VariableSet, String> {
    if feature.kind != VARSET_KIND {
        return Err(format!(
            "feature kind {} is not a variable set",
            feature.kind
        ));
    }
    let value = feature
        .parameters
        .get("variables")
        .ok_or_else(|| "variable-set feature has no variables payload".to_string())?;
    let variables: std::collections::BTreeMap<String, f64> =
        serde_json::from_value(value.clone()).map_err(|e| format!("bad varset payload: {e}"))?;
    Ok(VariableSet { variables })
}
