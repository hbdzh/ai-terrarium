use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_uint, c_void};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use napi_derive_ohos::napi;
use napi_ohos::threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode};
use tauri::Emitter;
use tokio::sync::oneshot;

#[link(name = "deviceinfo_ndk.z")]
extern "C" {
    fn OH_GetDeviceType() -> *const c_char;
}

/// Query HarmonyOS / OpenHarmony native device type via `<deviceinfo.h>` C NDK (`@since 10`).
/// Returns values such as `"phone"`, `"tablet"`, `"2in1"`, `"pc"`, `"wearable"`, `"tv"`, `"car"`.
pub fn get_device_type() -> String {
    unsafe {
        let ptr = OH_GetDeviceType();
        if ptr.is_null() {
            return "unknown".to_string();
        }
        CStr::from_ptr(ptr)
            .to_string_lossy()
            .trim()
            .to_ascii_lowercase()
    }
}

static FILE_BRIDGE_TSFN: OnceLock<Mutex<Option<Arc<ThreadsafeFunction<String>>>>> = OnceLock::new();
static NEXT_REQ_ID: AtomicU32 = AtomicU32::new(1);
static PENDING_FILE_REQS: OnceLock<Mutex<HashMap<u32, oneshot::Sender<Result<String, String>>>>> =
    OnceLock::new();

fn tsfn_slot() -> &'static Mutex<Option<Arc<ThreadsafeFunction<String>>>> {
    FILE_BRIDGE_TSFN.get_or_init(|| Mutex::new(None))
}

fn pending_map() -> &'static Mutex<HashMap<u32, oneshot::Sender<Result<String, String>>>> {
    PENDING_FILE_REQS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[napi(ts_args_type = "callback: (err: Error | null, payloadJson: string) => void")]
pub fn register_ohos_file_bridge(callback: ThreadsafeFunction<String>) {
    if let Ok(mut guard) = tsfn_slot().lock() {
        *guard = Some(Arc::new(callback));
    }
}

#[napi]
pub fn resolve_ohos_file_bridge(id: u32, ok: bool, payload: String) {
    if let Ok(mut map) = pending_map().lock() {
        if let Some(tx) = map.remove(&id) {
            let res = if ok { Ok(payload) } else { Err(payload) };
            let _ = tx.send(res);
        }
    }
}

pub async fn call_file_bridge(mut req: serde_json::Value) -> Result<String, String> {
    let tsfn = {
        let guard = tsfn_slot()
            .lock()
            .map_err(|_| "file bridge lock poisoned".to_string())?;
        guard
            .as_ref()
            .cloned()
            .ok_or_else(|| "HarmonyOS file bridge is not registered".to_string())?
    };

    let req_id = NEXT_REQ_ID.fetch_add(1, Ordering::Relaxed);
    if let Some(obj) = req.as_object_mut() {
        obj.insert("id".to_string(), serde_json::json!(req_id));
    }
    let payload = req.to_string();

    let (tx, rx) = oneshot::channel();
    {
        let mut map = pending_map()
            .lock()
            .map_err(|_| "pending file reqs lock poisoned".to_string())?;
        map.insert(req_id, tx);
    }

    let status = tsfn.call(Ok(payload), ThreadsafeFunctionCallMode::NonBlocking);
    if status != napi_ohos::Status::Ok {
        if let Ok(mut map) = pending_map().lock() {
            map.remove(&req_id);
        }
        return Err(format!("failed to invoke HarmonyOS file bridge: {status:?}"));
    }

    rx.await
        .map_err(|_| "HarmonyOS file bridge request dropped".to_string())?
}

static SANDBOX_FILES_DIR: OnceLock<String> = OnceLock::new();
static LATEST_INSETS_JSON: OnceLock<Mutex<String>> = OnceLock::new();
static UI_BRIDGE_TSFN: OnceLock<Mutex<Option<Arc<ThreadsafeFunction<String>>>>> = OnceLock::new();

#[napi]
pub fn set_ohos_files_dir(dir: String) {
    let _ = SANDBOX_FILES_DIR.set(dir);
}

pub fn sandbox_files_dir() -> std::path::PathBuf {
    SANDBOX_FILES_DIR
        .get()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("/data/storage/el2/base/haps/entry/files"))
}

fn insets_slot() -> &'static Mutex<String> {
    LATEST_INSETS_JSON.get_or_init(|| Mutex::new(String::new()))
}

fn ui_tsfn_slot() -> &'static Mutex<Option<Arc<ThreadsafeFunction<String>>>> {
    UI_BRIDGE_TSFN.get_or_init(|| Mutex::new(None))
}

#[napi]
pub fn update_ohos_insets(insets_json: String) {
    if let Ok(mut guard) = insets_slot().lock() {
        *guard = insets_json.clone();
    }
    if let Some(app) = APP_HANDLE.get() {
        let _ = app.emit("terrarium://native-insets", insets_json);
    }
}

pub fn get_ohos_insets() -> Option<String> {
    insets_slot()
        .lock()
        .ok()
        .and_then(|g| if g.is_empty() { None } else { Some(g.clone()) })
}

#[napi(ts_args_type = "callback: (err: Error | null, payloadJson: string) => void")]
pub fn register_ohos_ui_bridge(callback: ThreadsafeFunction<String>) {
    if let Ok(mut guard) = ui_tsfn_slot().lock() {
        *guard = Some(Arc::new(callback));
    }
}

pub fn call_ui_bridge(payload: serde_json::Value) {
    let tsfn = ui_tsfn_slot().lock().ok().and_then(|g| g.as_ref().cloned());
    if let Some(tsfn) = tsfn {
        let _ = tsfn.call(Ok(payload.to_string()), ThreadsafeFunctionCallMode::NonBlocking);
    }
}

static APP_HANDLE: OnceLock<tauri::AppHandle> = OnceLock::new();
static BACK_INTERCEPT_ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn set_app_handle(app: tauri::AppHandle) {
    let _ = APP_HANDLE.set(app);
}

pub fn set_back_intercept_active(active: bool) {
    BACK_INTERCEPT_ACTIVE.store(active, Ordering::Relaxed);
}

#[napi]
pub fn on_ohos_back_press() -> bool {
    if !BACK_INTERCEPT_ACTIVE.load(Ordering::Relaxed) {
        return false;
    }
    if let Some(app) = APP_HANDLE.get() {
        let _ = app.emit("terrarium://native-back", ());
        return true;
    }
    false
}

#[link(name = "pasteboard")]
extern "C" {
    fn OH_Pasteboard_Create() -> *mut c_void;
    fn OH_Pasteboard_Destroy(pasteboard: *mut c_void);
    fn OH_Pasteboard_GetData(pasteboard: *mut c_void, status: *mut c_int) -> *mut c_void;
    fn OH_Pasteboard_SetData(pasteboard: *mut c_void, data: *mut c_void) -> c_int;
}

#[link(name = "udmf")]
extern "C" {
    fn OH_UdmfData_Create() -> *mut c_void;
    fn OH_UdmfData_Destroy(data: *mut c_void);
    fn OH_UdmfData_AddRecord(data: *mut c_void, record: *mut c_void) -> c_int;
    fn OH_UdmfData_GetRecords(data: *mut c_void, count: *mut c_uint) -> *mut *mut c_void;
    fn OH_UdmfRecord_Create() -> *mut c_void;
    fn OH_UdmfRecord_Destroy(record: *mut c_void);
    fn OH_UdmfRecord_AddPlainText(record: *mut c_void, plain_text: *mut c_void) -> c_int;
    fn OH_UdmfRecord_GetPlainText(record: *mut c_void, plain_text: *mut c_void) -> c_int;
    fn OH_UdsPlainText_Create() -> *mut c_void;
    fn OH_UdsPlainText_Destroy(plain_text: *mut c_void);
    fn OH_UdsPlainText_GetContent(plain_text: *mut c_void) -> *const c_char;
    fn OH_UdsPlainText_SetContent(plain_text: *mut c_void, content: *const c_char) -> c_int;
}

pub fn write_clipboard_text(text: &str) -> Result<(), String> {
    let c_text = CString::new(text).map_err(|_| "clipboard text contains interior NUL".to_string())?;
    unsafe {
        let pb = OH_Pasteboard_Create();
        if pb.is_null() {
            return Err("OH_Pasteboard_Create returned null".to_string());
        }
        let data = OH_UdmfData_Create();
        let record = OH_UdmfRecord_Create();
        let plain = OH_UdsPlainText_Create();
        if data.is_null() || record.is_null() || plain.is_null() {
            if !plain.is_null() { OH_UdsPlainText_Destroy(plain); }
            if !record.is_null() { OH_UdmfRecord_Destroy(record); }
            if !data.is_null() { OH_UdmfData_Destroy(data); }
            OH_Pasteboard_Destroy(pb);
            return Err("failed to allocate UDMF clipboard objects".to_string());
        }

        let res = (|| -> Result<(), String> {
            let rc1 = OH_UdsPlainText_SetContent(plain, c_text.as_ptr());
            if rc1 != 0 {
                return Err(format!("OH_UdsPlainText_SetContent failed: {rc1}"));
            }
            let rc2 = OH_UdmfRecord_AddPlainText(record, plain);
            if rc2 != 0 {
                return Err(format!("OH_UdmfRecord_AddPlainText failed: {rc2}"));
            }
            let rc3 = OH_UdmfData_AddRecord(data, record);
            if rc3 != 0 {
                return Err(format!("OH_UdmfData_AddRecord failed: {rc3}"));
            }
            let rc4 = OH_Pasteboard_SetData(pb, data);
            if rc4 != 0 {
                return Err(format!("OH_Pasteboard_SetData failed: {rc4}"));
            }
            Ok(())
        })();

        OH_UdsPlainText_Destroy(plain);
        OH_UdmfRecord_Destroy(record);
        OH_UdmfData_Destroy(data);
        OH_Pasteboard_Destroy(pb);
        res
    }
}

pub fn read_clipboard_text() -> Result<String, String> {
    unsafe {
        let pb = OH_Pasteboard_Create();
        if pb.is_null() {
            return Err("OH_Pasteboard_Create returned null".to_string());
        }
        let mut status: c_int = 0;
        let data = OH_Pasteboard_GetData(pb, &mut status);
        if data.is_null() {
            OH_Pasteboard_Destroy(pb);
            return if status == 0 {
                Ok(String::new())
            } else {
                Err(format!("OH_Pasteboard_GetData failed: {status}"))
            };
        }

        let mut out = String::new();
        let mut count: c_uint = 0;
        let records = OH_UdmfData_GetRecords(data, &mut count);
        if !records.is_null() && count > 0 {
            let plain = OH_UdsPlainText_Create();
            if !plain.is_null() {
                for i in 0..(count as usize) {
                    let rec = *records.add(i);
                    if !rec.is_null() && OH_UdmfRecord_GetPlainText(rec, plain) == 0 {
                        let c_ptr = OH_UdsPlainText_GetContent(plain);
                        if !c_ptr.is_null() {
                            out = CStr::from_ptr(c_ptr).to_string_lossy().into_owned();
                            break;
                        }
                    }
                }
                OH_UdsPlainText_Destroy(plain);
            }
        }

        OH_UdmfData_Destroy(data);
        OH_Pasteboard_Destroy(pb);
        Ok(out)
    }
}
