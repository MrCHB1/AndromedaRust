pub mod kdmapi {
    use kdmapi_rs::KDMAPI as KDMAPILib;
    use kdmapi_rs::KDMAPIStream;
    use crate::audio::midi_audio_engine::MIDIAudioEngine;
    use crate::util::debugger::Debugger;

    pub struct KDMAPI {
        stream: Option<KDMAPIStream>,
    }

    impl KDMAPI {
        pub fn new() -> Self {
            match KDMAPILib.as_ref() {
                Ok(_) => {
                    Debugger::log("KDMAPI loaded!");
                }
                Err(e) => {
                    Debugger::log_error(format!("KDMAPI needs to be installed in order to use it. Details: {}", e));
                }
            }

            Self {
                stream: None,
            }
        }

        pub fn init(&mut self) {
            if self.stream.is_some() { return; }

            match KDMAPILib.as_ref() {
                Ok(kdmapi) => {
                    match kdmapi.open_stream() {
                        Ok(stream) => {
                            self.stream = Some(stream);
                        }
                        Err(e) => {
                            Debugger::log_error(format!("Failed to start KDMAPI streaming! Details: {}", e));
                        }
                    }
                }
                Err(e) => {
                    Debugger::log_error(format!("KDMAPI not found or installed! Details: {}", e));
                }
            }
        }

        pub fn close(&mut self) {
            self.stream = None; // Drop the stream
        }
    }

    impl Drop for KDMAPI {
        fn drop(&mut self) {
            Debugger::log("Closing KDMAPI...");
            self.close();
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
            if self.stream.is_none() {
                Debugger::log_warning("KDMAPI stream was never initialized. Initializing automatically...");
                self.init_audio();
            }

            if let Some(stream) = &self.stream {
                let ev = (raw_event[0] as u32) |
                    ((raw_event[1] as u32) << 8) |
                    ((raw_event[2] as u32) << 16);
                stream.send_direct_data(ev);
            } else {
                Debugger::log_error("KDMAPI Stream is not available.");
            }

            Ok(())
        }
    }
}
