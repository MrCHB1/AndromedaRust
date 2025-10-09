use midir::{MidiInputPort, MidiInputConnection, MidiOutputConnection, MidiOutputPort};

use crate::audio::midi_audio_engine::MIDIAudioEngine;

pub struct MIDIDevices {
    midi_in_ports: Vec<MidiInputPort>,
    midi_in_port_names: Vec<String>,
    midi_out_ports: Vec<MidiOutputPort>,
    midi_out_port_names: Vec<String>,

    curr_midi_in_port: Option<usize>,
    curr_midi_out_port: Option<usize>,

    in_connection: Option<MidiInputConnection<()>>,
    out_connection: Option<MidiOutputConnection>,
}

impl MIDIDevices {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let midi_in = midir::MidiInput::new("list_input")?;
        let midi_out = midir::MidiOutput::new("list_output")?;

        let in_ports = midi_in.ports();
        let out_ports = midi_out.ports();

        let mut s = MIDIDevices {
            midi_in_ports: Vec::new(),
            midi_in_port_names: Vec::new(),
            midi_out_ports: Vec::new(),
            midi_out_port_names: Vec::new(),
            curr_midi_in_port: None,
            curr_midi_out_port: None,
            in_connection: None,
            out_connection: None,
        };

        for (i, port) in in_ports.iter().enumerate() {
            s.midi_in_port_names.push(midi_in.port_name(port)?);
            s.midi_in_ports.push(port.clone());
            println!("IN({}): {}", i, s.midi_in_port_names[i]);
        }

        for (i, port) in out_ports.iter().enumerate() {
            s.midi_out_port_names.push(midi_out.port_name(port)?);
            s.midi_out_ports.push(port.clone());
            println!("OUT({}): {}", i, s.midi_out_port_names[i]);
        }

        s.connect_in_port(0)?;
        s.connect_out_port(0)?;

        Ok(s)
    }

    pub fn connect_out_port(&mut self, idx: usize) -> Result<(), Box<dyn std::error::Error>> {
        self.out_connection = None;
        self.curr_midi_out_port = None;

        if self.midi_out_ports.len() == 0 {
            println!("No MIDI outputs to connect to.");
            return Ok(());
        }

        if idx >= self.midi_out_ports.len() {
            return Err("Invalid output port index".into());
        }

        let midi_out = midir::MidiOutput::new("andromeda out")?;
        let conn_out = midi_out.connect(&self.midi_out_ports[idx], "Andromeda out")?;
        println!("Connected to OUT({}): {}", idx, self.midi_out_port_names[idx]);

        self.out_connection = Some(conn_out);
        self.curr_midi_out_port = Some(idx);
        Ok(())
    }

    pub fn connect_in_port(&mut self, idx: usize) -> Result<(), Box<dyn std::error::Error>> {
        if self.midi_in_ports.len() == 0 {
            println!("No MIDI inputs to connect to.");
            return Ok(());
        }

        if idx >= self.midi_in_ports.len() {
            return Err("Invalid input port index".into());
        }

        let mut midi_in = midir::MidiInput::new("andromeda in")?;
        midi_in.ignore(midir::Ignore::None);

        let conn_in = midi_in.connect(
            &self.midi_in_ports[idx],
            "Andromeda in",
            move |stamp, message, _| {
                println!("At {}ms: {:?}", stamp, message);
            },
            (),
        )?;
        println!("Connected to IN({}): {}", idx, self.midi_in_port_names[idx]);

        self.in_connection = Some(conn_in);
        self.curr_midi_in_port = Some(idx);
        Ok(())
    }

    pub fn send_event(&mut self, raw_event: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(conn) = self.out_connection.as_mut() {
            conn.send(raw_event)?;
        }

        Ok(())
    }

    pub fn get_midi_in_port_names(&self) -> &Vec<String> {
        &self.midi_in_port_names
    }

    pub fn get_midi_out_port_names(&self) -> &Vec<String> {
        &self.midi_out_port_names
    }
}

impl MIDIAudioEngine for MIDIDevices {
    /// This will open the current MIDI port for sending events.
    fn init_audio(&mut self) {
        self.connect_out_port(self.curr_midi_out_port.unwrap()).unwrap();
    }

    fn close_stream(&mut self) {
        self.out_connection = None;
        self.curr_midi_out_port = None;
    }

    fn send_event(&mut self, raw_event: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        self.send_event(raw_event)
    }
}