#![allow(non_snake_case)]

// Host import: rune.rune_execute_mutation(ptr, len)
// (Aliases also supported: execute_mutation, core_dispatch_mutation)
// Pass JSON bytes in guest memory at (ptr,len) to host.
use rune_provider as rune;

#[no_mangle]
pub extern "C" fn overlay_open_modal(_ptr: i32, _len: i32) { rune::open_overlay("modal"); }

#[no_mangle]
pub extern "C" fn overlay_open_confirm(_ptr: i32, _len: i32) { rune::open_overlay("confirm"); }

#[no_mangle]
pub extern "C" fn overlay_open_alert(_ptr: i32, _len: i32) { rune::open_overlay("alert"); }

// Receive confirm results from host
#[no_mangle]
pub extern "C" fn overlay_confirm_result(ptr: i32, len: i32) {
    let v = rune::parse_payload(ptr, len);
    let _accepted = v["result"].as_bool().unwrap_or(false);
    // In a real guest, you would branch logic here; this sample just no-ops.
}

#[no_mangle]
pub extern "C" fn on_form_submit(ptr: i32, len: i32) {
    // Global listener: route per-form to specific helpers
    let v = rune::parse_payload(ptr, len);
    let form_id = v["form_id"].as_str().unwrap_or("");
    match form_id {
        "intercept" => handle_intercept_submit(&v),
        _ => {}
    }
}

#[no_mangle]
pub extern "C" fn submit_contact_form(ptr: i32, len: i32) {
    // Named handler (via onsubmit_intent) delegates to the same helper.
    let v = rune::parse_payload(ptr, len);
    let form_id = v["form_id"].as_str().unwrap_or("");
    let first = v["data"]["first-name"].as_str().unwrap_or("");
    let last = v["data"]["last-name"].as_str().unwrap_or("");
    let token = format!("tok_{}{}", first.to_lowercase(), last.to_lowercase());
    rune::augment_form_kv(
        form_id,
        &[
            ("first-name", "OverriddenName"),
            ("computed_token", &token),
            ("added_by", "wasm_guest"),
        ],
    );
}

fn handle_intercept_submit(v: &serde_json::Value) {
    // Example: cancel native submit, then post custom payload to the form action.
    rune::prevent_default("intercept");
    let url = v["action"].as_str().unwrap_or("http://localhost:3000/log");
    let body = serde_json::json!({
        "form_id": v["form_id"],
        "data": v["data"],
        "note": "sent by wasm_guest via http_fetch"
    });
    rune::http_post_json(url, &body);
}

// Intent handler: fetch.ir_diff → normalized export name: fetch_ir_diff
// Guest requests data from server, and host uses that data to replace text by node_id.
#[no_mangle]
pub extern "C" fn fetch_ir_diff(_ptr: i32, _len: i32) {
    // Standard pattern: fetch, then handle in a guest callback.
    rune::http_fetch_then("http://localhost:3000/api/hello", "on_http_hello");
}

// Guest callback: receives { url, status, content_type, body }
// Parse JSON, extract message, and patch via IR diff by node_id.
#[no_mangle]
pub extern "C" fn on_http_hello(ptr: i32, len: i32) {
    let v = rune::parse_payload(ptr, len);
    let body = v["body"].as_str().unwrap_or("");
    let msg = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|j| j["message"].as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| body.to_string());
    rune::ir_diff_replace_text_by_node_id("P5Q9LkD2", &msg);
}
