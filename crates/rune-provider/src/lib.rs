//! Minimal helper API for Rune WASM guests.
//!
//! Provides ergonomic wrappers for emitting host mutations and parsing inbound payloads.
//! Import as `use rune_provider as rune;` and call:
//! - `rune::prevent_default(form_id)`
//! - `rune::augment_form_kv(form_id, &[("k","v")])`
//! - `rune::http_post_json(url, &json_value)`
//! - `rune::dispatch_json(&json_value)`
//! - `rune::open_overlay(kind)` / `rune::open_overlay_with(...)`
//! - `rune::close_overlay()`
//! - `rune::parse_payload(ptr, len)`

#[link(wasm_import_module = "rune")]
extern "C" {
    // Canonical import name; host also provides compatible aliases.
    #[link_name = "rune_execute_mutation"]
    fn rune_execute_mutation(ptr: i32, len: i32);
}

/// Send a JSON mutation to the host.
pub fn dispatch_json(msg: &serde_json::Value) {
    let s = msg.to_string();
    unsafe { rune_execute_mutation(s.as_ptr() as i32, s.len() as i32) };
}

/// Convenience: build a mutation from a kind + object fields.
pub fn exec_mutation(kind: &str, mut fields: serde_json::Map<String, serde_json::Value>) {
    fields.insert(
        "type".to_string(),
        serde_json::Value::String(kind.to_string()),
    );
    dispatch_json(&serde_json::Value::Object(fields));
}

/// Cancel the next pending native submit for a form.
pub fn prevent_default(form_id: &str) {
    dispatch_json(&serde_json::json!({
        "type": "cancel_form_submit",
        "form_id": form_id,
    }));
}

/// Augment/overwrite fields before native submit.
pub fn augment_form_kv(form_id: &str, extra: &[(&str, &str)]) {
    let mut map = serde_json::Map::new();
    for (k, v) in extra {
        map.insert(
            (*k).to_string(),
            serde_json::Value::String((*v).to_string()),
        );
    }
    dispatch_json(&serde_json::json!({
        "type": "augment_pending_form",
        "form_id": form_id,
        "extra": serde_json::Value::Object(map),
    }));
}

/// Request the host to send a JSON POST to a URL using the async HttpService path.
/// Fire-and-forget: uses a no-op callback export so guests don't need to handle it.
pub fn http_post_json(url: &str, body: &serde_json::Value) {
    dispatch_json(&serde_json::json!({
        "type": "http_fetch",
        "url": url,
        "method": "post",
        "content_type": "application/json",
        "body": body.to_string(),
        "callback": "__rune_http_cb_nop",
    }));
}

/// Request the host to send a GET to a URL via the async HttpService path (fire-and-forget).
pub fn http_get(url: &str) {
    dispatch_json(&serde_json::json!({
        "type": "http_fetch",
        "url": url,
        "method": "get",
        "callback": "__rune_http_cb_nop",
    }));
}

/// Convenience: ask host to GET a URL and replace a text node by `node_id` using a JSON field
/// (or "message"/"text" heuristics when field is None).
pub fn http_get_replace_text_by_node_id(url: &str, node_id: &str, field: Option<&str>) {
    let mut map = serde_json::Map::new();
    map.insert(
        "type".into(),
        serde_json::Value::String("http_get_replace_text_by_node_id".into()),
    );
    map.insert("url".into(), serde_json::Value::String(url.to_string()));
    map.insert(
        "node_id".into(),
        serde_json::Value::String(node_id.to_string()),
    );
    if let Some(f) = field {
        map.insert("field".into(), serde_json::Value::String(f.to_string()));
    }
    dispatch_json(&serde_json::Value::Object(map));
}

/// Issue a generic HTTP request and ask host to invoke a WASM export with the response payload.
pub fn http_fetch_then(url: &str, callback_export: &str) {
    let mut map = serde_json::Map::new();
    map.insert(
        "type".into(),
        serde_json::Value::String("http_fetch".into()),
    );
    map.insert("url".into(), serde_json::Value::String(url.to_string()));
    map.insert(
        "callback".into(),
        serde_json::Value::String(callback_export.to_string()),
    );
    dispatch_json(&serde_json::Value::Object(map));
}

/// Dispatch a minimal IR diff that replaces text at a target.
pub fn ir_diff_replace_text(target: &str, text: &str) {
    dispatch_json(&serde_json::json!({
        "type": "ir_diff",
        "ops": [
            { "op": "replace_text", "target": target, "text": text }
        ]
    }));
}

/// Dispatch an IR diff that targets a data node by `node_id`.
pub fn ir_diff_replace_text_by_node_id(node_id: &str, text: &str) {
    dispatch_json(&serde_json::json!({
        "type": "ir_diff",
        "ops": [
            { "op": "replace_text_by_node_id", "node_id": node_id, "text": text }
        ]
    }));
}

/// Convenience: request the host to open a runtime overlay with explicit fields.
#[allow(clippy::too_many_arguments)]
pub fn open_overlay_with(
    kind: &str,
    title: &str,
    description: &str,
    left_label: Option<&str>,
    right_label: Option<&str>,
    width: Option<f64>,
    height: Option<f64>,
    dismissible: Option<bool>,
    show_close: Option<bool>,
) {
    let mut map = serde_json::Map::new();
    map.insert(
        "type".into(),
        serde_json::Value::String("open_overlay".into()),
    );
    map.insert("kind".into(), serde_json::Value::String(kind.to_string()));
    if !title.is_empty() {
        map.insert("title".into(), serde_json::Value::String(title.to_string()));
    }
    if !description.is_empty() {
        map.insert(
            "description".into(),
            serde_json::Value::String(description.to_string()),
        );
    }
    if let Some(l) = left_label {
        map.insert(
            "left_label".into(),
            serde_json::Value::String(l.to_string()),
        );
    }
    if let Some(r) = right_label {
        map.insert(
            "right_label".into(),
            serde_json::Value::String(r.to_string()),
        );
    }
    if let Some(w) = width {
        map.insert("width".into(), serde_json::Value::from(w));
    }
    if let Some(h) = height {
        map.insert("height".into(), serde_json::Value::from(h));
    }
    if let Some(d) = dismissible {
        map.insert("dismissible".into(), serde_json::Value::from(d));
    }
    if let Some(sc) = show_close {
        map.insert("show_close".into(), serde_json::Value::from(sc));
    }
    dispatch_json(&serde_json::Value::Object(map));
}

/// Convenience: request the host to open a runtime overlay.
/// `kind` must be one of: "alert", "modal", or "confirm".
/// Provides sensible defaults for content so overlays are not empty.
pub fn open_overlay(kind: &str) {
    // Provider does not set defaults; host will populate from IR or built-ins.
    open_overlay_with(kind, "", "", None, None, None, None, None, None)
}

/// Parse the (ptr,len) JSON payload provided by the host into a Value.
pub fn parse_payload(ptr: i32, len: i32) -> serde_json::Value {
    let bytes = unsafe { core::slice::from_raw_parts(ptr as *const u8, len as usize) };
    let s = core::str::from_utf8(bytes).unwrap_or("{}\n");
    serde_json::from_str(s).unwrap_or(serde_json::json!({}))
}

/// Close any currently displayed overlay.
pub fn close_overlay() {
    dispatch_json(&serde_json::json!({
        "type": "close_overlay",
    }));
}
