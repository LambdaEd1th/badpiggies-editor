#[cfg(not(target_arch = "wasm32"))]
use futures_timer::Delay;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::JsFuture;

#[cfg(not(target_arch = "wasm32"))]
pub async fn sleep_ms(milliseconds: u64) {
    Delay::new(Duration::from_millis(milliseconds)).await;
}

#[cfg(target_arch = "wasm32")]
pub async fn sleep_ms(milliseconds: u64) {
    let promise = js_sys::Promise::new(&mut |resolve, reject| {
        let Some(window) = web_sys::window() else {
            let _ = resolve.call0(&JsValue::UNDEFINED);
            return;
        };
        let callback = Closure::once(move || {
            let _ = resolve.call0(&JsValue::UNDEFINED);
        });
        if let Err(error) = window.set_timeout_with_callback_and_timeout_and_arguments_0(
            callback.as_ref().unchecked_ref(),
            milliseconds.min(i32::MAX as u64) as i32,
        ) {
            let _ = reject.call1(&JsValue::UNDEFINED, &error);
            return;
        }
        callback.forget();
    });
    let _ = JsFuture::from(promise).await;
}
