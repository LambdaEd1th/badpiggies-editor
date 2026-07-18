#[cfg(not(target_arch = "wasm32"))]
use badpiggies_editor_core::worker_protocol::perform_worker_request;
use badpiggies_editor_core::worker_protocol::{WorkerRequest, WorkerResponse};

#[cfg(not(target_arch = "wasm32"))]
use std::panic::{AssertUnwindSafe, catch_unwind};

#[cfg(target_arch = "wasm32")]
use js_sys::{Array, Function, Object, Promise, Reflect, Uint8Array};
#[cfg(target_arch = "wasm32")]
use serde::Serialize;
#[cfg(target_arch = "wasm32")]
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::JsFuture;
#[cfg(target_arch = "wasm32")]
use web_sys::{ErrorEvent, MessageEvent, Worker, WorkerOptions, WorkerType};

#[cfg(target_arch = "wasm32")]
const PROCESSING_WORKER_URL: &str = "assets/worker/badpiggies-worker.js?v=20260716-pool-1";

#[cfg(target_arch = "wasm32")]
struct PendingRequest {
    resolve: Function,
    reject: Function,
}

#[cfg(target_arch = "wasm32")]
struct WorkerClient {
    worker: Worker,
    next_id: u64,
    pending: Rc<RefCell<HashMap<u64, PendingRequest>>>,
    failed: Rc<Cell<bool>>,
    _onmessage: Closure<dyn FnMut(MessageEvent)>,
    _onerror: Closure<dyn FnMut(ErrorEvent)>,
}

#[cfg(target_arch = "wasm32")]
struct WorkerPool {
    workers: Vec<WorkerClient>,
    next_worker: usize,
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    static WORKER_POOL: RefCell<Option<WorkerPool>> = const { RefCell::new(None) };
}

#[cfg(target_arch = "wasm32")]
impl WorkerClient {
    fn new() -> Result<Self, String> {
        let options = WorkerOptions::new();
        options.set_type(WorkerType::Module);
        let worker =
            Worker::new_with_options(PROCESSING_WORKER_URL, &options).map_err(js_error_string)?;
        let pending = Rc::new(RefCell::new(HashMap::<u64, PendingRequest>::new()));
        let failed = Rc::new(Cell::new(false));

        let message_pending = Rc::clone(&pending);
        let onmessage = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
            let response = event.data();
            let Some(id) = message_id(&response) else {
                return;
            };
            if let Some(request) = message_pending.borrow_mut().remove(&id) {
                let _ = request.resolve.call1(&JsValue::UNDEFINED, &response);
            }
        });
        worker.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));

        let error_pending = Rc::clone(&pending);
        let error_failed = Rc::clone(&failed);
        let onerror = Closure::<dyn FnMut(ErrorEvent)>::new(move |event: ErrorEvent| {
            error_failed.set(true);
            let message = if event.message().is_empty() {
                "Processing Worker failed".to_string()
            } else {
                event.message()
            };
            log::error!("Processing Worker failed: {message}");
            let error = JsValue::from_str(&message);
            for (_, request) in error_pending.borrow_mut().drain() {
                let _ = request.reject.call1(&JsValue::UNDEFINED, &error);
            }
        });
        worker.set_onerror(Some(onerror.as_ref().unchecked_ref()));

        Ok(Self {
            worker,
            next_id: 1,
            pending,
            failed,
            _onmessage: onmessage,
            _onerror: onerror,
        })
    }

    fn request(&mut self, request: JsValue) -> Result<Promise, String> {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        let message = Object::new();
        Reflect::set(
            message.as_ref(),
            &JsValue::from_str("id"),
            &JsValue::from_f64(id as f64),
        )
        .map_err(js_error_string)?;
        Reflect::set(message.as_ref(), &JsValue::from_str("request"), &request)
            .map_err(js_error_string)?;

        let worker = self.worker.clone();
        let pending = Rc::clone(&self.pending);
        let failed = Rc::clone(&self.failed);
        let transfer = request_transfer_list(&request);
        Ok(Promise::new(
            &mut move |resolve: Function, reject: Function| {
                pending.borrow_mut().insert(
                    id,
                    PendingRequest {
                        resolve,
                        reject: reject.clone(),
                    },
                );
                let posted = if transfer.length() == 0 {
                    worker.post_message(message.as_ref())
                } else {
                    worker.post_message_with_transfer(message.as_ref(), transfer.as_ref())
                };
                if let Err(error) = posted {
                    failed.set(true);
                    pending.borrow_mut().remove(&id);
                    let _ = reject.call1(&JsValue::UNDEFINED, &error);
                }
            },
        ))
    }
}

#[cfg(target_arch = "wasm32")]
impl Drop for WorkerClient {
    fn drop(&mut self) {
        self.worker.terminate();
    }
}

#[cfg(target_arch = "wasm32")]
impl WorkerPool {
    fn new() -> Result<Self, String> {
        let worker_count = web_worker_count();
        let workers = (0..worker_count)
            .map(|_| WorkerClient::new())
            .collect::<Result<Vec<_>, _>>()?;
        record_pool_runtime(worker_count);
        log::info!("Processing Worker pool initialized with {worker_count} worker(s)");
        Ok(Self {
            workers,
            next_worker: 0,
        })
    }

    fn has_usable_worker(&self) -> bool {
        self.workers.iter().any(|worker| !worker.failed.get())
    }

    fn request(&mut self, request: JsValue) -> Result<Promise, String> {
        let worker_count = self.workers.len();
        let selected = (0..worker_count)
            .map(|offset| (self.next_worker + offset) % worker_count)
            .filter(|&index| !self.workers[index].failed.get())
            .min_by_key(|&index| self.workers[index].pending.borrow().len())
            .ok_or_else(|| "All Processing Workers have failed".to_string())?;
        self.next_worker = (selected + 1) % worker_count;
        self.workers[selected].request(request)
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn perform(request: WorkerRequest) -> Result<WorkerResponse, String> {
    let (sender, receiver) = futures_channel::oneshot::channel();
    rayon::spawn(move || {
        let response = catch_unwind(AssertUnwindSafe(|| perform_worker_request(request)))
            .unwrap_or_else(|_| WorkerResponse::Error {
                message: "Native processing task panicked".to_string(),
            });
        let _ = sender.send(response);
    });
    let response = receiver
        .await
        .map_err(|_| "Native processing task was cancelled".to_string())?;
    normalize(response)
}

#[cfg(target_arch = "wasm32")]
pub async fn warm_up() -> Result<(), String> {
    let result = match perform(WorkerRequest::Batch {
        requests: (0..web_worker_count())
            .map(|_| WorkerRequest::Ping)
            .collect(),
    })
    .await
    {
        Ok(WorkerResponse::Batch { responses })
            if responses
                .iter()
                .all(|response| matches!(response, WorkerResponse::Pong)) =>
        {
            Ok(())
        }
        Ok(_) => Err("Unexpected Processing Worker response".to_string()),
        Err(error) => Err(error),
    };
    #[cfg(target_arch = "wasm32")]
    record_status(if result.is_ok() { "ready" } else { "error" });
    result
}

#[cfg(target_arch = "wasm32")]
pub async fn perform(request: WorkerRequest) -> Result<WorkerResponse, String> {
    let response = match request {
        WorkerRequest::Batch { requests } => {
            let promises = requests
                .into_iter()
                .map(submit_web_request)
                .collect::<Result<Vec<_>, _>>()?;
            let mut responses = Vec::with_capacity(promises.len());
            for promise in promises {
                responses.push(decode_web_response(promise).await?);
            }
            WorkerResponse::Batch { responses }
        }
        request => decode_web_response(submit_web_request(request)?).await?,
    };
    normalize(response)
}

#[cfg(target_arch = "wasm32")]
fn submit_web_request(request: WorkerRequest) -> Result<Promise, String> {
    let serializer =
        serde_wasm_bindgen::Serializer::new().serialize_large_number_types_as_bigints(true);
    let request = request
        .serialize(&serializer)
        .map_err(|error| error.to_string())?;
    WORKER_POOL.with(|slot| {
        let mut slot = slot.borrow_mut();
        let recreate = slot.as_ref().is_none_or(|pool| !pool.has_usable_worker());
        if recreate {
            *slot = Some(WorkerPool::new()?);
        }
        slot.as_mut()
            .expect("processing worker pool was initialized")
            .request(request)
    })
}

#[cfg(target_arch = "wasm32")]
async fn decode_web_response(promise: Promise) -> Result<WorkerResponse, String> {
    let envelope = JsFuture::from(promise).await.map_err(js_error_string)?;
    let ok = Reflect::get(&envelope, &JsValue::from_str("ok"))
        .ok()
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    if !ok {
        return Err(Reflect::get(&envelope, &JsValue::from_str("error"))
            .ok()
            .and_then(|value| value.as_string())
            .unwrap_or_else(|| "Processing Worker failed".to_string()));
    }
    let response =
        Reflect::get(&envelope, &JsValue::from_str("response")).map_err(js_error_string)?;
    serde_wasm_bindgen::from_value(response).map_err(|error| error.to_string())
}

fn normalize(response: WorkerResponse) -> Result<WorkerResponse, String> {
    match response {
        WorkerResponse::Error { message } => Err(message),
        response => Ok(response),
    }
}

#[cfg(target_arch = "wasm32")]
fn message_id(value: &JsValue) -> Option<u64> {
    Reflect::get(value, &JsValue::from_str("id"))
        .ok()?
        .as_f64()
        .map(|value| value as u64)
}

#[cfg(target_arch = "wasm32")]
fn request_transfer_list(request: &JsValue) -> Array {
    let transfer = Array::new();
    collect_transferables(request, &transfer);
    transfer
}

#[cfg(target_arch = "wasm32")]
fn collect_transferables(value: &JsValue, transfer: &Array) {
    if value.is_instance_of::<Uint8Array>() {
        transfer.push(Uint8Array::new(value).buffer().as_ref());
        return;
    }
    if Array::is_array(value) {
        for item in Array::from(value) {
            collect_transferables(&item, transfer);
        }
        return;
    }
    if !value.is_object() {
        return;
    }
    for key in Object::keys(&Object::from(value.clone())) {
        if let Ok(item) = Reflect::get(value, &key) {
            collect_transferables(&item, transfer);
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn record_status(status: &str) {
    if let Some(root) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.document_element())
    {
        let _ = root.set_attribute("data-processing-worker", status);
    }
}

#[cfg(target_arch = "wasm32")]
fn record_pool_runtime(worker_count: usize) {
    let Some(root) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.document_element())
    else {
        return;
    };
    let _ = root.set_attribute("data-processing-worker-backend", "worker-pool");
    let _ = root.set_attribute("data-processing-worker-threads", &worker_count.to_string());
}

#[cfg(target_arch = "wasm32")]
fn web_worker_count() -> usize {
    let hardware_concurrency = web_sys::window()
        .map(|window| window.navigator().hardware_concurrency() as usize)
        .unwrap_or(1);
    (hardware_concurrency.saturating_sub(1) / 2).clamp(1, 4)
}

#[cfg(target_arch = "wasm32")]
fn js_error_string(error: JsValue) -> String {
    error
        .as_string()
        .unwrap_or_else(|| format!("JavaScript error: {error:?}"))
}
