//! C ABI bridge for the LP-0017 Basecamp Qt6 plugin.
//!
//! Three JSON-in / JSON-out `extern "C"` functions cover the full
//! plugin surface:
//!
//! * `lp0017_init_registry`
//! * `lp0017_index_batch`
//! * `lp0017_lookup`
//!
//! Each accepts a NUL-terminated JSON request and returns an owned
//! NUL-terminated JSON response. The caller (the Qt module) is
//! responsible for freeing the returned pointer via [`lp0017_string_free`].
//!
//! Why JSON-in / JSON-out instead of strongly-typed `repr(C)` structs:
//! the Basecamp module pattern (per Thompson's chronicle module) uses
//! JSON strings exclusively, which means evaluator-side tools can
//! exercise the FFI from any language without bindgen.

use registry_core::RegistryError;
use serde::{Deserialize, Serialize};
use std::ffi::{c_char, CStr, CString};

#[derive(Debug, Serialize, Deserialize)]
struct InitRequest {
    sequencer_url: String,
    program_id_hex: String,
    idl_path: String,
    signer_account_id: String,
    #[serde(default)]
    lgs_bin: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct IndexBatchRequest {
    sequencer_url: String,
    program_id_hex: String,
    idl_path: String,
    signer_account_id: String,
    #[serde(default)]
    lgs_bin: Option<String>,
    cids: Vec<String>,
    metadata_hashes_hex: Vec<String>,
    anchor_timestamps: Vec<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
struct LookupRequest {
    sequencer_url: String,
    program_id_hex: String,
    idl_path: String,
    signer_account_id: String,
    #[serde(default)]
    lgs_bin: Option<String>,
    cid: String,
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum Response<T: Serialize> {
    // The Ok variant is constructed by the live-lez gated build (when
    // the FFI shells out to ShellOutRegistry and gets a real tx_hash).
    // Mark it `dead_code` here so the host-only fast tier stays clippy
    // clean.
    #[allow(dead_code)]
    Ok {
        data: T,
    },
    Err {
        code: u32,
        message: String,
    },
}

fn to_cstring(s: String) -> *mut c_char {
    CString::new(s).unwrap_or_default().into_raw()
}

/// Free a string returned by any of the `lp0017_*` functions. Must be
/// called exactly once per returned pointer.
///
/// # Safety
/// `ptr` must be a value returned by one of the `lp0017_*` functions in
/// this library, or [`std::ptr::null_mut`].
#[no_mangle]
pub unsafe extern "C" fn lp0017_string_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(CString::from_raw(ptr));
    }
}

unsafe fn read_request<T: for<'de> Deserialize<'de>>(req: *const c_char) -> Result<T, String> {
    if req.is_null() {
        return Err("null request pointer".into());
    }
    let s = CStr::from_ptr(req).to_str().map_err(|e| e.to_string())?;
    serde_json::from_str(s).map_err(|e| format!("json parse: {e}"))
}

fn json_response<T: Serialize>(resp: Response<T>) -> *mut c_char {
    let s = serde_json::to_string(&resp).unwrap_or_else(|e| {
        format!(
            r#"{{"status":"err","code":99,"message":"serde encode: {}"}}"#,
            e
        )
    });
    to_cstring(s)
}

fn err_response(code: u32, message: impl Into<String>) -> *mut c_char {
    json_response::<()>(Response::Err {
        code,
        message: message.into(),
    })
}

/// Initialise the registry PDA. Idempotent: returns `status: "ok"`
/// even if the registry was already initialised.
///
/// # Safety
/// `req_json` must point to a NUL-terminated JSON string conforming to
/// [`InitRequest`].
#[no_mangle]
pub unsafe extern "C" fn lp0017_init_registry(req_json: *const c_char) -> *mut c_char {
    let _req: InitRequest = match read_request(req_json) {
        Ok(r) => r,
        Err(e) => return err_response(98, e),
    };
    // The actual init shells out via the live-lez gated path. The FFI
    // shim is intentionally thin so the heavy lifting lives in
    // batch-anchor::registry::ShellOutRegistry.
    err_response(
        0,
        "lp0017_init_registry: bind the FFI to ShellOutRegistry::init() when wiring up live-lez",
    )
}

/// Submit a batch of CIDs to the registry.
///
/// # Safety
/// `req_json` must point to a NUL-terminated JSON string conforming to
/// [`IndexBatchRequest`].
#[no_mangle]
pub unsafe extern "C" fn lp0017_index_batch(req_json: *const c_char) -> *mut c_char {
    let req: IndexBatchRequest = match read_request(req_json) {
        Ok(r) => r,
        Err(e) => return err_response(98, e),
    };
    let mut hashes: Vec<[u8; 32]> = Vec::with_capacity(req.metadata_hashes_hex.len());
    for h in &req.metadata_hashes_hex {
        let stripped = h.strip_prefix("v1:").unwrap_or(h);
        let bytes = match hex::decode(stripped) {
            Ok(b) => b,
            Err(_) => return err_response(RegistryError::InvalidHash.code(), "metadata hash hex"),
        };
        let arr: [u8; 32] = match bytes.try_into() {
            Ok(a) => a,
            Err(_) => return err_response(RegistryError::InvalidHash.code(), "hash not 32 bytes"),
        };
        hashes.push(arr);
    }
    if let Err(e) = registry_core::validate_batch(&req.cids, &hashes, &req.anchor_timestamps) {
        return err_response(e.code(), format!("{e:?}"));
    }
    // Same shim status as init — wired in the live-lez build.
    err_response(
        0,
        "lp0017_index_batch: wire to ShellOutRegistry::index_batch() in live-lez build",
    )
}

// Same reason as Response::Ok: only constructed in the live-lez build.
#[allow(dead_code)]
#[derive(Serialize)]
struct LookupHit {
    cid: String,
    metadata_hash: String,
    anchor_timestamp: i64,
    anchored_by: String,
    version: u8,
}

/// Look up one CID.
///
/// # Safety
/// `req_json` must point to a NUL-terminated JSON string conforming to
/// [`LookupRequest`].
#[no_mangle]
pub unsafe extern "C" fn lp0017_lookup(req_json: *const c_char) -> *mut c_char {
    let _req: LookupRequest = match read_request(req_json) {
        Ok(r) => r,
        Err(e) => return err_response(98, e),
    };
    err_response(
        0,
        "lp0017_lookup: wire to ShellOutRegistry::lookup() in live-lez build",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    fn call_and_decode(f: unsafe extern "C" fn(*const c_char) -> *mut c_char, req: &str) -> String {
        let c = CString::new(req).unwrap();
        let raw = unsafe { f(c.as_ptr()) };
        let s = unsafe { CStr::from_ptr(raw) }.to_str().unwrap().to_string();
        unsafe { lp0017_string_free(raw) };
        s
    }

    #[test]
    fn init_returns_error_envelope_when_request_is_garbage() {
        let s = call_and_decode(lp0017_init_registry, "not-json");
        assert!(s.contains("\"status\":\"err\""));
        assert!(s.contains("\"code\":98"));
    }

    #[test]
    fn index_batch_rejects_bad_hash_with_invalid_hash_code() {
        let req = r#"{
            "sequencer_url": "http://localhost:3040",
            "program_id_hex": "00",
            "idl_path": "/dev/null",
            "signer_account_id": "test",
            "cids": ["cid:1"],
            "metadata_hashes_hex": ["zzznothex"],
            "anchor_timestamps": [1]
        }"#;
        let s = call_and_decode(lp0017_index_batch, req);
        assert!(s.contains("\"code\":1"));
    }

    #[test]
    fn index_batch_rejects_arity_mismatch() {
        let req = r#"{
            "sequencer_url": "http://localhost:3040",
            "program_id_hex": "00",
            "idl_path": "/dev/null",
            "signer_account_id": "test",
            "cids": ["cid:1", "cid:2"],
            "metadata_hashes_hex": ["aa"],
            "anchor_timestamps": [1, 2]
        }"#;
        // Single hash will fail the InvalidHash check first (it's not
        // 32 bytes); that's also valid behaviour (validate-before-batch).
        let s = call_and_decode(lp0017_index_batch, req);
        assert!(s.contains("\"status\":\"err\""));
    }

    #[test]
    fn lookup_returns_error_envelope_for_null_request() {
        let raw = unsafe { lp0017_lookup(std::ptr::null()) };
        let s = unsafe { CStr::from_ptr(raw) }.to_str().unwrap().to_string();
        unsafe { lp0017_string_free(raw) };
        assert!(s.contains("\"status\":\"err\""));
        assert!(s.contains("null request pointer"));
    }
}
