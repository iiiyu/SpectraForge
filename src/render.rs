use crate::analysis::{Features, SPECTRUM_BINS};
use anyhow::{Context, Result, bail};
use glow::HasContext;
use khronos_egl as egl;

/// Headless OpenGL renderer: an EGL pbuffer context drawing a fullscreen quad
/// with the user's fragment shader into an off-screen framebuffer.
pub struct Renderer {
    // EGL handles are kept alive for the lifetime of the renderer.
    _egl: egl::DynamicInstance<egl::EGL1_5>,
    _display: egl::Display,
    _context: egl::Context,
    _surface: egl::Surface,

    gl: glow::Context,
    program: glow::Program,
    vao: glow::VertexArray,
    spectrum_tex: glow::Texture,
    width: u32,
    height: u32,
    pixels: Vec<u8>,

    u_resolution: Option<glow::UniformLocation>,
    u_time: Option<glow::UniformLocation>,
    u_rms: Option<glow::UniformLocation>,
    u_bass: Option<glow::UniformLocation>,
    u_mid: Option<glow::UniformLocation>,
    u_treble: Option<glow::UniformLocation>,
    u_spectrum: Option<glow::UniformLocation>,
}

const VERTEX_SHADER: &str = r#"#version 330 core
const vec2 verts[3] = vec2[3](vec2(-1.0,-1.0), vec2(3.0,-1.0), vec2(-1.0,3.0));
void main() { gl_Position = vec4(verts[gl_VertexID], 0.0, 1.0); }
"#;

const FRAG_PREAMBLE: &str = r#"#version 330 core
out vec4 spectraforge_fragColor;
uniform vec2  iResolution;
uniform float iTime;
uniform float iRMS;
uniform float iBass;
uniform float iMid;
uniform float iTreble;
uniform sampler2D iSpectrum;
"#;

const FRAG_MAIN: &str = r#"
void main() {
    vec4 c = vec4(0.0, 0.0, 0.0, 1.0);
    mainImage(c, gl_FragCoord.xy);
    spectraforge_fragColor = c;
}
"#;

impl Renderer {
    pub fn new(width: u32, height: u32, user_shader: &str) -> Result<Self> {
        // --- EGL: headless pbuffer context ---
        let egl = unsafe { egl::DynamicInstance::<egl::EGL1_5>::load_required() }
            .map_err(|e| anyhow::anyhow!("loading libEGL: {e}"))?;

        let display = unsafe {
            egl.get_display(egl::DEFAULT_DISPLAY)
                .context("no EGL default display")?
        };
        egl.initialize(display).context("eglInitialize failed")?;

        let config_attrs = [
            egl::SURFACE_TYPE,
            egl::PBUFFER_BIT,
            egl::RENDERABLE_TYPE,
            egl::OPENGL_BIT,
            egl::RED_SIZE,
            8,
            egl::GREEN_SIZE,
            8,
            egl::BLUE_SIZE,
            8,
            egl::NONE,
        ];
        let config = egl
            .choose_first_config(display, &config_attrs)
            .context("eglChooseConfig failed")?
            .context("no matching EGL config")?;

        egl.bind_api(egl::OPENGL_API)
            .context("eglBindAPI(OpenGL) failed")?;

        let surface_attrs = [
            egl::WIDTH,
            width as i32,
            egl::HEIGHT,
            height as i32,
            egl::NONE,
        ];
        let surface = egl
            .create_pbuffer_surface(display, config, &surface_attrs)
            .context("creating pbuffer surface")?;

        let context_attrs = [
            egl::CONTEXT_MAJOR_VERSION,
            3,
            egl::CONTEXT_MINOR_VERSION,
            3,
            egl::NONE,
        ];
        let context = egl
            .create_context(display, config, None, &context_attrs)
            .context("creating GL 3.3 context")?;
        egl.make_current(display, Some(surface), Some(surface), Some(context))
            .context("eglMakeCurrent failed")?;

        // --- glow: load GL functions via EGL ---
        let gl = unsafe {
            glow::Context::from_loader_function(|s| {
                egl.get_proc_address(s)
                    .map_or(std::ptr::null(), |p| p as *const _)
            })
        };

        unsafe {
            let program = compile_program(&gl, user_shader)?;
            let vao = gl
                .create_vertex_array()
                .map_err(|e| anyhow::anyhow!("VAO: {e}"))?;

            let spectrum_tex = gl
                .create_texture()
                .map_err(|e| anyhow::anyhow!("texture: {e}"))?;
            gl.bind_texture(glow::TEXTURE_2D, Some(spectrum_tex));
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::LINEAR as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::LINEAR as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_S,
                glow::CLAMP_TO_EDGE as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_T,
                glow::CLAMP_TO_EDGE as i32,
            );

            gl.viewport(0, 0, width as i32, height as i32);

            let loc = |name: &str| gl.get_uniform_location(program, name);
            let renderer = Renderer {
                u_resolution: loc("iResolution"),
                u_time: loc("iTime"),
                u_rms: loc("iRMS"),
                u_bass: loc("iBass"),
                u_mid: loc("iMid"),
                u_treble: loc("iTreble"),
                u_spectrum: loc("iSpectrum"),
                _egl: egl,
                _display: display,
                _context: context,
                _surface: surface,
                gl,
                program,
                vao,
                spectrum_tex,
                width,
                height,
                pixels: vec![0u8; (width * height * 3) as usize],
            };
            Ok(renderer)
        }
    }

    /// Render one frame and return its rgb24 bytes (top-to-bottom).
    pub fn render_frame(&mut self, time: f32, f: &Features) -> &[u8] {
        let gl = &self.gl;
        unsafe {
            gl.use_program(Some(self.program));
            gl.bind_vertex_array(Some(self.vao));

            // Upload spectrum as a 1D (Nx1) R32F texture on unit 0.
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.spectrum_tex));
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::R32F as i32,
                SPECTRUM_BINS as i32,
                1,
                0,
                glow::RED,
                glow::FLOAT,
                glow::PixelUnpackData::Slice(Some(bytemuck_cast(&f.spectrum))),
            );

            if let Some(l) = &self.u_resolution {
                gl.uniform_2_f32(Some(l), self.width as f32, self.height as f32);
            }
            if let Some(l) = &self.u_time {
                gl.uniform_1_f32(Some(l), time);
            }
            if let Some(l) = &self.u_rms {
                gl.uniform_1_f32(Some(l), f.rms);
            }
            if let Some(l) = &self.u_bass {
                gl.uniform_1_f32(Some(l), f.bass);
            }
            if let Some(l) = &self.u_mid {
                gl.uniform_1_f32(Some(l), f.mid);
            }
            if let Some(l) = &self.u_treble {
                gl.uniform_1_f32(Some(l), f.treble);
            }
            if let Some(l) = &self.u_spectrum {
                gl.uniform_1_i32(Some(l), 0);
            }

            gl.clear_color(0.0, 0.0, 0.0, 1.0);
            gl.clear(glow::COLOR_BUFFER_BIT);
            gl.draw_arrays(glow::TRIANGLES, 0, 3);

            gl.read_pixels(
                0,
                0,
                self.width as i32,
                self.height as i32,
                glow::RGB,
                glow::UNSIGNED_BYTE,
                glow::PixelPackData::Slice(Some(&mut self.pixels)),
            );
        }
        // GL gives bottom-to-top rows; ffmpeg wants top-to-bottom. Flip.
        flip_vertical(&mut self.pixels, self.width, self.height);
        &self.pixels
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            self.gl.delete_program(self.program);
            self.gl.delete_vertex_array(self.vao);
            self.gl.delete_texture(self.spectrum_tex);
        }
    }
}

/// Reinterpret an `f32` slice as bytes for texture upload.
fn bytemuck_cast(data: &[f32]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(data.as_ptr() as *const u8, std::mem::size_of_val(data)) }
}

fn flip_vertical(pixels: &mut [u8], width: u32, height: u32) {
    let row = (width * 3) as usize;
    let h = height as usize;
    let mut rows: Vec<&mut [u8]> = pixels.chunks_mut(row).collect();
    for y in 0..h / 2 {
        let (a, b) = rows.split_at_mut(y + 1);
        a[y].swap_with_slice(b[h - 1 - 2 * y - 1]);
    }
}

unsafe fn compile_program(gl: &glow::Context, user_shader: &str) -> Result<glow::Program> {
    let frag_src = format!("{FRAG_PREAMBLE}\n{user_shader}\n{FRAG_MAIN}");
    unsafe {
        let program = gl
            .create_program()
            .map_err(|e| anyhow::anyhow!("create_program: {e}"))?;

        for (kind, src) in [
            (glow::VERTEX_SHADER, VERTEX_SHADER),
            (glow::FRAGMENT_SHADER, frag_src.as_str()),
        ] {
            let shader = gl
                .create_shader(kind)
                .map_err(|e| anyhow::anyhow!("create_shader: {e}"))?;
            gl.shader_source(shader, src);
            gl.compile_shader(shader);
            if !gl.get_shader_compile_status(shader) {
                let log = gl.get_shader_info_log(shader);
                let what = if kind == glow::VERTEX_SHADER {
                    "vertex"
                } else {
                    "fragment"
                };
                bail!("{what} shader compile failed:\n{log}");
            }
            gl.attach_shader(program, shader);
            gl.delete_shader(shader);
        }

        gl.link_program(program);
        if !gl.get_program_link_status(program) {
            let log = gl.get_program_info_log(program);
            bail!("shader link failed:\n{log}");
        }
        Ok(program)
    }
}
