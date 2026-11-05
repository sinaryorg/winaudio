use std::sync::{Arc, Mutex};
use windows::core::{implement, Interface, HRESULT, PCWSTR, PROPVARIANT};
use windows::Win32::Foundation::{CloseHandle, HANDLE, S_OK};
use windows::Win32::Media::Audio::*;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
use windows::Win32::System::Threading::{CreateEventW, SetEvent, WaitForSingleObject, INFINITE};
use crate::errors::{Result, WinAudioError};

pub const VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK: PCWSTR = windows::core::w!("VAD\\Process_Loopback");
pub const VT_BLOB: u16 = 65;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessLoopbackMode {
    IncludeTargetProcessTree = 0,
    ExcludeTargetProcessTree = 1,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AudioClientProcessLoopbackParams {
    pub target_process_id: u32,
    pub process_loopback_mode: ProcessLoopbackMode,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioClientActivationType {
    Default = 0,
    ProcessLoopback = 1,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AudioClientActivationParams {
    pub activation_type: AudioClientActivationType,
    pub process_loopback_params: AudioClientProcessLoopbackParams,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Win32Blob {
    cb_size: u32,
    #[cfg(target_pointer_width = "64")]
    _padding: u32,
    p_blob_data: *mut u8,
}

#[repr(C)]
struct PropVariantWithBlob {
    vt: u16,
    w_reserved1: u16,
    w_reserved2: u16,
    w_reserved3: u16,
    blob: Win32Blob,
}

/// Ensures COM is initialized on the current thread in MTA (Multithreaded Apartment) mode.
pub fn ensure_mta() -> Result<()> {
    unsafe {
        let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
        // S_OK (0) or S_FALSE (1, already initialized on this thread) are both acceptable
        if hr.is_ok() || hr == windows::Win32::Foundation::S_FALSE {
            Ok(())
        } else {
            Err(WinAudioError::Com(windows::core::Error::from(hr)))
        }
    }
}

#[implement(IActivateAudioInterfaceCompletionHandler)]
struct ActivationHandler {
    event: HANDLE,
    result: Arc<Mutex<Option<std::result::Result<windows::core::IUnknown, HRESULT>>>>,
}

impl IActivateAudioInterfaceCompletionHandler_Impl for ActivationHandler_Impl {
    fn ActivateCompleted(
        &self,
        operation: Option<&IActivateAudioInterfaceAsyncOperation>,
    ) -> windows::core::Result<()> {
        let mut hr = HRESULT(0);
        let mut activated: Option<windows::core::IUnknown> = None;

        if let Some(op) = operation {
            unsafe {
                let _ = op.GetActivateResult(&mut hr, &mut activated);
            }
        }

        let mut lock = self.result.lock().unwrap();
        if hr == S_OK {
            if let Some(punk) = activated {
                *lock = Some(Ok(punk));
            } else {
                *lock = Some(Err(windows::core::Error::from_win32().code()));
            }
        } else {
            *lock = Some(Err(hr));
        }

        unsafe {
            let _ = SetEvent(self.event);
        }

        Ok(())
    }
}

struct AutoHandle(HANDLE);
impl Drop for AutoHandle {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

/// Asynchronously activates an `IAudioClient` configured for process-specific loopback capture.
pub fn activate_process_loopback_client(
    target_pid: u32,
    include_tree: bool,
) -> Result<IAudioClient> {
    ensure_mta()?;

    unsafe {
        let event = CreateEventW(None, false, false, None)?;
        let _auto_event = AutoHandle(event);

        let result_store = Arc::new(Mutex::new(None));
        let handler: IActivateAudioInterfaceCompletionHandler = ActivationHandler {
            event,
            result: Arc::clone(&result_store),
        }
        .into();

        let mut activation_params = AudioClientActivationParams {
            activation_type: AudioClientActivationType::ProcessLoopback,
            process_loopback_params: AudioClientProcessLoopbackParams {
                target_process_id: target_pid,
                process_loopback_mode: if include_tree {
                    ProcessLoopbackMode::IncludeTargetProcessTree
                } else {
                    ProcessLoopbackMode::ExcludeTargetProcessTree
                },
            },
        };

        let raw_propvariant = PropVariantWithBlob {
            vt: VT_BLOB,
            w_reserved1: 0,
            w_reserved2: 0,
            w_reserved3: 0,
            blob: Win32Blob {
                cb_size: std::mem::size_of::<AudioClientActivationParams>() as u32,
                #[cfg(target_pointer_width = "64")]
                _padding: 0,
                p_blob_data: &mut activation_params as *mut _ as *mut u8,
            },
        };

        let prop_variant_ptr: *const PROPVARIANT =
            &raw_propvariant as *const _ as *const PROPVARIANT;

        let _async_op = ActivateAudioInterfaceAsync(
            VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK,
            &IAudioClient::IID,
            Some(prop_variant_ptr),
            &handler,
        )?;

        // Wait for COM activation callback to signal event
        let wait_res = WaitForSingleObject(event, INFINITE);
        if wait_res.0 != 0 {
            return Err(WinAudioError::ActivationFailed(format!(
                "Wait failed with code: {}",
                wait_res.0
            )));
        }

        let res = result_store.lock().unwrap().take();
        match res {
            Some(Ok(punk)) => {
                let audio_client: IAudioClient = punk.cast()?;
                Ok(audio_client)
            }
            Some(Err(hr)) => Err(WinAudioError::ActivationFailed(format!(
                "COM activation returned HRESULT: 0x{:08X}",
                hr.0
            ))),
            None => Err(WinAudioError::ActivationFailed(
                "Activation completed without result".into(),
            )),
        }
    }
}
