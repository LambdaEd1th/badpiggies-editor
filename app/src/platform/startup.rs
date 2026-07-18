use js_sys::{Function, Reflect};
use wasm_bindgen::{JsCast, JsValue};

fn controller() -> Option<JsValue> {
    Reflect::get(&js_sys::global(), &JsValue::from_str("bpStartup")).ok()
}

fn method(controller: &JsValue, name: &str) -> Option<Function> {
    Reflect::get(controller, &JsValue::from_str(name))
        .ok()?
        .dyn_into::<Function>()
        .ok()
}

pub fn set_stage(label: &str, detail: &str, percent: Option<f64>) {
    let Some(controller) = controller() else {
        return;
    };
    let Some(method) = method(&controller, "setStage") else {
        return;
    };
    let progress = percent.map_or(JsValue::UNDEFINED, JsValue::from_f64);
    let _ = method.call3(
        &controller,
        &JsValue::from_str(label),
        &JsValue::from_str(detail),
        &progress,
    );
}

pub fn update_assets(loaded: usize, total: usize, relative: &str) {
    let Some(controller) = controller() else {
        return;
    };
    let Some(method) = method(&controller, "updateAssets") else {
        return;
    };
    let _ = method.call3(
        &controller,
        &JsValue::from_f64(loaded as f64),
        &JsValue::from_f64(total as f64),
        &JsValue::from_str(relative),
    );
}

pub fn fail(message: &str) {
    let Some(controller) = controller() else {
        return;
    };
    let Some(method) = method(&controller, "fail") else {
        return;
    };
    let _ = method.call1(&controller, &JsValue::from_str(message));
}

pub fn finish() {
    let Some(controller) = controller() else {
        return;
    };
    let Some(method) = method(&controller, "finish") else {
        return;
    };
    let _ = method.call0(&controller);
}
