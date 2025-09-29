use eframe::glow::{self, NativeBuffer, NativeTexture, NativeVertexArray};
use eframe::glow::HasContext;
use image::ImageReader;
use std::fs::File;
use std::io::Read;
use std::sync::Arc;

pub struct Buffer {
    pub buffer: NativeBuffer,
    target: u32,
    gl: Arc<glow::Context>
}

impl Buffer {
    pub fn new(gl: Arc<glow::Context>, target: u32) -> Self {
        unsafe {
            let buffer = gl.create_buffer().unwrap();
            Self {
                buffer,
                target,
                gl
            }
        }
    }

    pub fn bind(&self) {
        unsafe {
            self.gl.bind_buffer(self.target, Some(self.buffer));
        }
    }

    pub fn set_data<D>(&self, data: &[D], usage: u32) {
        unsafe { 
            self.bind();
            let (_, data_bytes, _) = data.align_to::<u8>();
            self.gl.buffer_data_u8_slice(
                self.target, 
                data_bytes, 
                usage
            );
        }
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        unsafe {
            self.gl.delete_buffer(self.buffer);
        }
    }
}

pub struct VertexArray {
    pub array: NativeVertexArray,
    gl: Arc<glow::Context>
}

impl VertexArray {
    pub fn new(gl: Arc<glow::Context>) -> Self {
        unsafe {
            Self {
                array: gl.create_vertex_array().unwrap(),
                gl
            }
        }
    }

    pub fn bind(&self) {
        unsafe {
            self.gl.bind_vertex_array(Some(self.array));
        }
    }

    pub fn set_attribute<V: Sized>(
        &self,
        type_: u32,
        attrib_pos: u32,
        components: i32,
        offset: i32
    ) {
        unsafe {
            self.bind();
            match type_ {
                glow::FLOAT => {
                    self.gl.vertex_attrib_pointer_f32(
                        attrib_pos,
                        components,
                        type_,
                        false, 
                        std::mem::size_of::<V>() as i32,
                        offset
                    );
                }
                _ => {
                    self.gl.vertex_attrib_pointer_i32(
                        attrib_pos,
                        components,
                        type_,
                        std::mem::size_of::<V>() as i32,
                        offset
                    );
                }
            }
            /*if type_ == glow::FLOAT {
                self.gl.vertex_attrib_pointer_f32(
                    attrib_pos,
                    components,
                    type_,
                    false, 
                    std::mem::size_of::<V>() as i32,
                    offset
                );
            } else if type_ == glow::INT {
                self.gl.vertex_attrib_pointer_i32(
                    attrib_pos,
                    components,
                    type_,
                    std::mem::size_of::<V>() as i32,
                    offset
                );
            } else if type_ ==*/

            self.gl.enable_vertex_attrib_array(attrib_pos);
        }
    }
}

impl Drop for VertexArray {
    fn drop(&mut self) {
        unsafe {
            self.gl.delete_vertex_array(self.array);
        }
    }
}

pub struct Texture {
    tex: NativeTexture,
    target: u32,
    gl: Arc<glow::Context>,

    width: i32,
    height: i32,
}

impl Texture {
    pub fn new(gl: Arc<glow::Context>, target: u32) -> Self {
        unsafe {
            let tex = gl.create_texture().unwrap();
            Self {
                tex,
                target,
                gl,
                width: 1,
                height: 1
            }
        }
    }

    pub fn set_wrapping(&mut self, wrapping: u32) {
        unsafe {
            self.gl.tex_parameter_i32(self.target, glow::TEXTURE_WRAP_S, wrapping as i32);
            self.gl.tex_parameter_i32(self.target, glow::TEXTURE_WRAP_T, wrapping as i32);
        }
    }

    pub fn set_filtering(&mut self, filter: u32) {
        unsafe {
            self.gl.tex_parameter_i32(self.target, glow::TEXTURE_MIN_FILTER, filter as i32);
            self.gl.tex_parameter_i32(self.target, glow::TEXTURE_MAG_FILTER, filter as i32);
        }
    }

    pub fn load_texture(&mut self, path: &str, width: i32, height: i32) {
        let img = ImageReader::open(path).unwrap().decode().unwrap().to_rgb8();
        let data = img.as_raw();
        self.load_raw(data.as_slice(), width, height);
    }

    pub fn load_raw(&mut self, data: &[u8], width: i32, height: i32) {
        unsafe {
            self.gl.tex_image_2d(self.target, 0, glow::RGB8 as i32, width, height, 0, glow::RGB, glow::UNSIGNED_BYTE, glow::PixelUnpackData::Slice(Some(data)));
            self.gl.generate_mipmap(self.target);
        }

        self.width = width;
        self.height = height;
    }

    pub fn update_texture(&mut self, path: &str) {
        let img = ImageReader::open(path).unwrap().decode().unwrap().to_rgb8();
        let data = img.as_raw();
        self.update_texture_raw(data.as_slice());
    }

    pub fn update_texture_raw(&mut self, data: &[u8]) {
        assert!(data.len() == (self.width * self.height * 3) as usize);

        unsafe {
            self.bind();
            self.gl.tex_sub_image_2d(self.target, 0, 0, 0, self.width, self.height, glow::RGB, glow::UNSIGNED_BYTE, glow::PixelUnpackData::Slice(Some(data)));
        }
    }

    pub fn bind(&mut self) {
        unsafe {
            self.gl.bind_texture(self.target, Some(self.tex));
        }
    }
}

impl Drop for Texture {
    fn drop(&mut self) {
        unsafe {
            self.gl.delete_texture(self.tex);
        }
    }
}

#[macro_export]
macro_rules! set_attribute {
    ($type_:ident :: $tfield:tt, $vbo:ident, $pos:tt, $t:ident :: $field:tt) => {{
        let dummy = core::mem::MaybeUninit::<$t>::uninit();
        let dummy_ptr = dummy.as_ptr();
        let member_ptr = core::ptr::addr_of!((*dummy_ptr).$field);
        const fn size_of_raw<T>(_: *const T) -> usize {
            core::mem::size_of::<T>()
        }
        let member_offset = member_ptr as i32 - dummy_ptr as i32;
        $vbo.set_attribute::<$t>(
            $type_::$tfield,
            $pos,
            (size_of_raw(member_ptr) / 4) as i32,
            member_offset,
        )
    }};
}