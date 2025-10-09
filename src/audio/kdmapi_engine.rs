#[cfg(windows)]
pub mod kdmapi {
    use winapi::um::libloaderapi::{GetModuleHandleA, GetProcAddress};
    use winapi::shared::minwindef::{BOOL, DWORD};
    use winapi::um::mmsystem::MMRESULT;

    use crate::audio::midi_audio_engine::MIDIAudioEngine;

    type InitializeKDMAPIStream = unsafe extern "system" fn() -> BOOL;
    type TerminateKDMAPIStream = unsafe extern "system" fn() -> BOOL;
    type SendDirectDataNoBuf = unsafe extern "system" fn(DWORD) -> MMRESULT;

    pub struct KDMAPI {
        supports_kdmapi: bool,
        kdmapi_initialized: bool,

        init_ptr: Option<InitializeKDMAPIStream>,
        term_ptr: Option<TerminateKDMAPIStream>,
        send_ptr: Option<SendDirectDataNoBuf>,
    }

    impl KDMAPI {
        pub fn new() -> Self {
            
            let init_ptr = unsafe {
                let module = GetModuleHandleA("OmniMIDI\0".as_ptr() as *const i8);
                let func = GetProcAddress(module, "InitializeKDMAPIStream\0".as_ptr() as *const i8);
                if !(module.is_null() || func.is_null()) {
                    Some(std::mem::transmute::<_, InitializeKDMAPIStream>(func))
                } else {
                    None
                }
            };

            let term_ptr = unsafe {
                let module = GetModuleHandleA("OmniMIDI\0".as_ptr() as *const i8);
                let func = GetProcAddress(module, "TerminateKDMAPIStream\0".as_ptr() as *const i8);
                if !(module.is_null() || func.is_null()) {
                    Some(std::mem::transmute::<_, TerminateKDMAPIStream>(func))
                } else {
                    None
                }
            };

            let send_ptr = unsafe {
                let module = GetModuleHandleA("OmniMIDI\0".as_ptr() as *const i8);
                let func = GetProcAddress(module, "SendDirectDataNoBuf\0".as_ptr() as *const i8);
                if !(module.is_null() || func.is_null()) {
                    Some(std::mem::transmute::<_, SendDirectDataNoBuf>(func))
                } else {
                    None
                }
            };

            let supports_kdmapi = init_ptr.is_some() && term_ptr.is_some() && send_ptr.is_some();

            if supports_kdmapi {
                println!("KDMAPI loaded!");
            } else {
                println!("[WARNING] KDMAPI needs to be installed in order to use it.");
            }

            Self {
                supports_kdmapi,
                kdmapi_initialized: false,

                init_ptr,
                term_ptr,
                send_ptr
            }
        }

        pub fn init(&mut self) {
            if self.kdmapi_initialized { return; }

            if let Some(initialize_kdmapi_stream) = self.init_ptr.as_ref() {
                println!("{:?}", initialize_kdmapi_stream);
                unsafe { initialize_kdmapi_stream(); }
                self.kdmapi_initialized = true;
            } else {
                println!("KDMAPI not found or installed...");
            }
        }

        pub fn close(&mut self) {
            if !self.kdmapi_initialized { return; }
            if !self.supports_kdmapi { return; }

            if let Some(terminate_kdmapi_stream) = self.term_ptr.as_ref() {
                unsafe { terminate_kdmapi_stream(); }
            } else {
                println!("KDMAPI not found or installed...");
            }

            self.kdmapi_initialized = false;
        }
    }

    // i forgot that doing unsafe pointer stuff requires drop, else the app may crash lmfao
    impl Drop for KDMAPI {
        fn drop(&mut self) {
            println!("Closing KDMAPI...");
            self.close();

            self.init_ptr = None;
            self.send_ptr = None;
            self.term_ptr = None;
        }
    }

    impl MIDIAudioEngine for KDMAPI {
        fn init_audio(&mut self) {
            self.init();
        }

        fn close_stream(&mut self) {
            self.close();
        }

        fn send_event(&mut self, raw_event: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
            if !self.supports_kdmapi { return Ok(()); }
            if !self.kdmapi_initialized { 
                println!("[WARNING] KDMAPI stream was never initialized. Initializing automatically...");
                self.init_audio();
                self.kdmapi_initialized = true;
            }

            if let Some(send_direct_data) = self.send_ptr.as_ref() {
                let ev = (raw_event[0] as u32) | 
                    ((raw_event[1] as u32) << 8) |
                    ((raw_event[2] as u32) << 16);
                unsafe { send_direct_data(ev as DWORD); }
            } else {
                println!("KDMAPI not found or installed...");
            }

            Ok(())
        }
    }
}


#[cfg(not(windows))]
pub mod kdmapi {
    use crate::audio::midi_audio_engine::MIDIAudioEngine;

    pub struct KDMAPI {
        supports_kdmapi: bool
    }

    impl KDMAPI {
        pub fn new() -> Self {
            println!("KDMAPI is not supported on the current OS.");

            Self {
                supports_kdmapi: false
            }
        }
    }

    impl MIDIAudioEngine for KDMAPI {
        fn init_audio(&mut self) { }
        fn close_stream(&mut self) { }
        fn send_event(&mut self, raw_event: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
            Ok(())
        }
    }
}