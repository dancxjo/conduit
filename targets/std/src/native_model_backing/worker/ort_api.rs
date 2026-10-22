//! Optional API23 bindings loaded from one caller-identified installed artifact.
use super::super::{NativeBackingRefusal, NativeCpuProfile};
use ort_sys::*;
use sha2::{Digest, Sha256};
use std::{
    ffi::{CStr, CString},
    io::Read,
    ptr,
};
pub struct NativeSession {
    api: &'static OrtApi,
    session: *mut OrtSession,
    input: *mut OrtValue,
    output: *mut OrtValue,
    io: *mut f32,
}
pub fn file_digest(path: &std::path::Path) -> Result<[u8; 32], NativeBackingRefusal> {
    let mut file = std::fs::File::open(path).map_err(|_| NativeBackingRefusal::Io)?;
    let mut digest = Sha256::new();
    let mut buf = [0u8; 8192];
    let mut total = 0u64;
    loop {
        let n = file.read(&mut buf).map_err(|_| NativeBackingRefusal::Io)?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > 128 * 1024 * 1024 {
            return Err(NativeBackingRefusal::UnsupportedProfile);
        }
        digest.update(&buf[..n]);
    }
    Ok(digest.finalize().into())
}
impl NativeSession {
    pub unsafe fn prepare(
        profile: &NativeCpuProfile,
        graph: &[u8],
        io: *mut f32,
    ) -> Result<Self, NativeBackingRefusal> {
        if file_digest(std::path::Path::new(&profile.runtime_library))? != profile.runtime_sha256 {
            return Err(NativeBackingRefusal::IdentityMismatch);
        }
        let path = CString::new(profile.runtime_library.as_bytes())
            .map_err(|_| NativeBackingRefusal::UnsupportedProfile)?;
        let library = unsafe { libc::dlopen(path.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
        if library.is_null() {
            return Err(NativeBackingRefusal::UnsupportedProfile);
        }
        let symbol = unsafe { libc::dlsym(library, c"OrtGetApiBase".as_ptr()) };
        if symbol.is_null() {
            return Err(NativeBackingRefusal::UnsupportedProfile);
        }
        let entry: unsafe extern "system" fn() -> *const OrtApiBase =
            unsafe { std::mem::transmute(symbol) };
        let base = unsafe { entry().as_ref() }.ok_or(NativeBackingRefusal::UnsupportedProfile)?;
        let version = unsafe { (base.GetVersionString)() };
        if version.is_null() || unsafe { CStr::from_ptr(version) }.to_bytes() != b"1.23.2" {
            return Err(NativeBackingRefusal::UnsupportedProfile);
        }
        let api = unsafe { (base.GetApi)(23).as_ref() }
            .ok_or(NativeBackingRefusal::UnsupportedProfile)?;
        let mut env = ptr::null_mut();
        let mut options = ptr::null_mut();
        let mut session = ptr::null_mut();
        let mut memory = ptr::null_mut();
        let mut input = ptr::null_mut();
        let mut output = ptr::null_mut();
        unsafe {
            check(
                api,
                (api.CreateEnv)(
                    OrtLoggingLevel::ORT_LOGGING_LEVEL_ERROR,
                    c"conduit-native-backing".as_ptr(),
                    &mut env,
                ),
            )?;
            check(api, (api.CreateSessionOptions)(&mut options))?;
            check(api, (api.SetIntraOpNumThreads)(options, 1))?;
            check(api, (api.SetInterOpNumThreads)(options, 1))?;
            check(
                api,
                (api.SetSessionExecutionMode)(options, ExecutionMode::ORT_SEQUENTIAL),
            )?;
            check(
                api,
                (api.SetSessionGraphOptimizationLevel)(
                    options,
                    GraphOptimizationLevel::ORT_DISABLE_ALL,
                ),
            )?;
            check(api, (api.DisableCpuMemArena)(options))?;
            check(api, (api.DisableMemPattern)(options))?;
            check(
                api,
                (api.CreateSessionFromArray)(
                    env,
                    graph.as_ptr().cast(),
                    graph.len(),
                    options,
                    &mut session,
                ),
            )?;
            check(
                api,
                (api.CreateCpuMemoryInfo)(
                    OrtAllocatorType::OrtArenaAllocator,
                    OrtMemType::OrtMemTypeDefault,
                    &mut memory,
                ),
            )?;
            check(
                api,
                (api.CreateTensorWithDataAsOrtValue)(
                    memory,
                    io.cast(),
                    4,
                    ptr::null(),
                    0,
                    ONNXTensorElementDataType::ONNX_TENSOR_ELEMENT_DATA_TYPE_FLOAT,
                    &mut input,
                ),
            )?;
            check(
                api,
                (api.CreateTensorWithDataAsOrtValue)(
                    memory,
                    io.add(1).cast(),
                    4,
                    ptr::null(),
                    0,
                    ONNXTensorElementDataType::ONNX_TENSOR_ELEMENT_DATA_TYPE_FLOAT,
                    &mut output,
                ),
            )?;
        }
        // The disposable process owns these permanent prepared resources. OS teardown
        // releases them; graceful ORT session retirement is a separate contract.
        Ok(Self {
            api,
            session,
            input,
            output,
            io,
        })
    }
    pub fn infer(&mut self, value: f32) -> Result<f32, NativeBackingRefusal> {
        unsafe {
            self.io.write(value);
            self.io.add(1).write(f32::NAN);
        }
        let inputs = [self.input.cast_const()];
        let mut outputs = [self.output];
        let input_names = [c"x".as_ptr()];
        let output_names = [c"y".as_ptr()];
        unsafe {
            check(
                self.api,
                (self.api.Run)(
                    self.session,
                    ptr::null(),
                    input_names.as_ptr(),
                    inputs.as_ptr(),
                    1,
                    output_names.as_ptr(),
                    1,
                    outputs.as_mut_ptr(),
                ),
            )?;
        }
        if outputs[0] != self.output {
            return Err(NativeBackingRefusal::InvalidOutput);
        }
        Ok(unsafe { self.io.add(1).read() })
    }
}
unsafe fn check(api: &OrtApi, status: OrtStatusPtr) -> Result<(), NativeBackingRefusal> {
    if status.0.is_null() {
        Ok(())
    } else {
        unsafe { (api.ReleaseStatus)(status.0) };
        Err(NativeBackingRefusal::BackingPreparation)
    }
}
