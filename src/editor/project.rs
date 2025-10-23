pub mod project_data;
pub mod project_manager;

use std::{fs::File, io::{self, Write}, path::PathBuf};

use crate::editor::project::project_manager::ProjectManager;

pub struct ProjectWriter<'a> {
    stream: File,
    project_manager: &'a ProjectManager,

    buffer: Vec<u8>,
}

impl<'a> ProjectWriter<'a> {
    pub fn new(project_manager: &'a ProjectManager, path: PathBuf) -> Self {
        let stream = File::create(path).unwrap();
        
        Self {
            stream,
            project_manager: project_manager,
            buffer: Vec::new()
        }
    }

    /// Writes the header of an Andromeda Project file.
    /// 1st 4 bytes are "AnHd".
    pub fn write_header(&mut self) -> io::Result<()> {
        self.write_text(b"AnHd")?;

        {
            let project_data = self.project_manager.get_project_data();
            let project_info = self.project_manager.get_project_info();

            let ppq = project_data.ppq;
            self.buffer.extend(Self::u16_to_bytes(ppq));

            // write project's name
            let (name_len, name_bytes) = Self::text_to_bytes(project_info.name.as_str());
            self.buffer.extend(Self::u32_to_bytes(name_len));
            self.buffer.extend(name_bytes);

            // write project's author
            let (author_len, author_bytes) = Self::text_to_bytes(project_info.author.as_str());
            self.buffer.extend(Self::u32_to_bytes(author_len));
            self.buffer.extend(author_bytes);

            // write project's description
            let (desc_len, desc_bytes) = Self::text_to_bytes(project_info.description.as_str());
            self.buffer.extend(Self::u32_to_bytes(desc_len));
            self.buffer.extend(desc_bytes);
        }

        let buf_len = Self::u32_to_bytes(self.buffer.len() as u32);
        self.write(&buf_len)?;
        self.flush_buffer()?;

        Ok(())
    }

    pub fn finalize(&mut self) -> io::Result<()> {
        self.stream.flush()?;
        Ok(())
    }

    fn u16_to_bytes(num: u16) -> [u8; 2] {
        [
            ((num & 0xFF00) >> 8) as u8,
            (num & 0xFF) as u8
        ]
    }

    fn u32_to_bytes(num: u32) -> [u8; 4] {
        [
            ((num & 0xFF000000) >> 24) as u8,
            ((num & 0xFF0000) >> 16) as u8,
            ((num & 0xFF00) >> 8) as u8,
            (num & 0xFF) as u8,
        ]
    }

    fn flush_buffer(&mut self) -> io::Result<()> {
        let stream = &mut self.stream;
        stream.write(std::mem::take(&mut &*self.buffer))?;
        Ok(())
    }

    fn write_text(&mut self, txt: &'static [u8]) -> io::Result<()> {
        let stream = &mut self.stream;
        stream.write(txt)?;
        Ok(())
    }

    fn write(&mut self, data: &[u8]) -> io::Result<()> {
        let stream = &mut self.stream;
        stream.write(data)?;
        Ok(())
    }

    fn text_to_bytes(text: &str) -> (u32, Vec<u8>) {
        let utf16: Vec<u16> = text.encode_utf16().collect();

        let len = utf16.len() * 2;
        let ptr = utf16.as_ptr() as *const u8;
        let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };

        (len as u32, bytes.to_vec())
    }
}