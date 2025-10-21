pub mod kdmapi {
    use kdmapi_rs::KDMAPI as KDMAPILib;
    use kdmapi_rs::KDMAPIStream;
    use crate::audio::midi_audio_engine::MIDIAudioEngine;

    pub struct KDMAPI {
        stream: Option<KDMAPIStream>,
    }

    impl KDMAPI {
        pub fn new() -> Self {
            match KDMAPILib.as_ref() {
                Ok(_) => {
                    println!("KDMAPI loaded!");
                }
                Err(e) => {
                    println!("[WARNING] KDMAPI needs to be installed in order to use it: {}", e);
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
                            println!("Failed to start KDMAPI streaming: {}", e);
                        }
                    }
                }
                Err(e) => {
                    println!("KDMAPI not found or installed: {}", e);
                }
            }
        }

        pub fn close(&mut self) {
            self.stream = None; // Drop the stream
        }
    }

    impl Drop for KDMAPI {
        fn drop(&mut self) {
            println!("Closing KDMAPI...");
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
                println!("[WARNING] KDMAPI stream was never initialized. Initializing automatically...");
                self.init_audio();
            }

            if let Some(stream) = &self.stream {
                let ev = (raw_event[0] as u32) |
                    ((raw_event[1] as u32) << 8) |
                    ((raw_event[2] as u32) << 16);
                stream.send_direct_data(ev);
            } else {
                println!("KDMAPI stream not available...");
            }

            Ok(())
        }
    }
}
