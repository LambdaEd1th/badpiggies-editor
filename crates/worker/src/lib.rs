#![forbid(unsafe_code)]

#[cfg(target_arch = "wasm32")]
use badpiggies_editor_core::worker_protocol::{WorkerRequest, perform_worker_request};
#[cfg(target_arch = "wasm32")]
use serde::Serialize;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn perform(request: JsValue) -> Result<JsValue, JsValue> {
    let request: WorkerRequest = serde_wasm_bindgen::from_value(request)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let serializer =
        serde_wasm_bindgen::Serializer::new().serialize_large_number_types_as_bigints(true);
    perform_worker_request(request)
        .serialize(&serializer)
        .map_err(|error| JsValue::from_str(&error.to_string()))
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use badpiggies_editor_core::io::unity3d::Unity3dTextAssetEntry;
    use badpiggies_editor_core::worker_protocol::WorkerResponse;
    use serde::Serialize;
    use wasm_bindgen_test::wasm_bindgen_test;

    #[wasm_bindgen_test]
    fn large_unity_path_ids_round_trip_as_bigints() {
        const PATH_ID: i64 = -1_805_451_283_445_223_269;
        let response = WorkerResponse::UnityEntries {
            entries: vec![Unity3dTextAssetEntry {
                asset_path: "episode_6_level_10_data.bytes".to_string(),
                display_name: "episode_6_level_10_data.bytes".to_string(),
                asset_index: 1,
                path_id: PATH_ID,
                bundle_asset_name: "level_data".to_string(),
            }],
        };
        let serializer =
            serde_wasm_bindgen::Serializer::new().serialize_large_number_types_as_bigints(true);
        let value = response
            .serialize(&serializer)
            .expect("serialize response containing an unsafe JavaScript integer");
        let round_trip: WorkerResponse =
            serde_wasm_bindgen::from_value(value).expect("deserialize BigInt response");

        let WorkerResponse::UnityEntries { entries } = round_trip else {
            panic!("expected UnityEntries response");
        };
        assert_eq!(entries[0].path_id, PATH_ID);
    }
}
