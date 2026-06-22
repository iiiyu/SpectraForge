use crate::analysis::{Features, SPECTRUM_BINS};
use anyhow::{Context, Result, bail};
use glow::HasContext;
use khronos_egl as egl;
use std::ffi::c_void;

/// Headless OpenGL renderer: off-screen framebuffers drawing a fullscreen quad
/// with the user's fragment shader(s). A shader file may declare multiple
/// passes (see `split_passes`); each renders to its own texture and later
/// passes sample earlier ones via `iPass1`, `iPass2`, … The final pass is the
/// image read out to the encoder.
pub struct Renderer {
    // The backend context must stay alive while GL objects exist.
    _backend: GlBackend,

    gl: glow::Context,
    passes: Vec<Pass>,
    // One render target per pass, in order. targets[i] is pass i's output.
    targets: Vec<(glow::Framebuffer, glow::Texture)>,
    vao: glow::VertexArray,
    spectrum_tex: glow::Texture,
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

struct Pass {
    program: glow::Program,
    u_resolution: Option<glow::UniformLocation>,
    u_time: Option<glow::UniformLocation>,
    u_rms: Option<glow::UniformLocation>,
    u_bass: Option<glow::UniformLocation>,
    u_mid: Option<glow::UniformLocation>,
    u_treble: Option<glow::UniformLocation>,
    u_spectrum: Option<glow::UniformLocation>,
    // iPass1.. locations; index j is iPass{j+1}, referring to targets[j].
    u_pass: Vec<Option<glow::UniformLocation>>,
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

/// Splits a shader source into ordered passes. Passes are separated by a line
/// whose trimmed content starts with `//---pass` (e.g. `//---pass---`). A file
/// with no such marker is a single pass, identical to the old behavior.
fn split_passes(src: &str) -> Vec<String> {
    let mut passes = vec![String::new()];
    for line in src.lines() {
        if line.trim_start().starts_with("//---pass") {
            passes.push(String::new());
        } else {
            let cur = passes.last_mut().unwrap();
            cur.push_str(line);
            cur.push('\n');
        }
    }
    passes
}

const FRAG_MAIN: &str = r#"
void main() {
    vec4 c = vec4(0.0, 0.0, 0.0, 1.0);
    mainImage(c, gl_FragCoord.xy);
    spectraforge_fragColor = c;
}
"#;

const EGL_PLATFORM_SURFACELESS_MESA: egl::Enum = 0x31DD;

impl Renderer {
    pub fn new(width: u32, height: u32, user_shader: &str) -> Result<Self> {
        let backend = GlBackend::create(width, height)?;

        let gl =
            unsafe { glow::Context::from_loader_function(|name| backend.get_proc_address(name)) };

        unsafe {
            let vendor = gl.get_parameter_string(glow::VENDOR);
            let renderer_name = gl.get_parameter_string(glow::RENDERER);
            let version = gl.get_parameter_string(glow::VERSION);
            eprintln!(
                "OpenGL backend: {} | vendor: {vendor} | renderer: {renderer_name} | version: {version}",
                backend.label()
            );

            gl.pixel_store_i32(glow::PACK_ALIGNMENT, 1);
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);

            let sources = split_passes(user_shader);
            let pass_count = sources.len();
            let vao = gl
                .create_vertex_array()
                .map_err(|e| anyhow::anyhow!("VAO: {e}"))?;

            // One RGBA8 render target per pass.
            let mut targets = Vec::with_capacity(pass_count);
            for _ in 0..pass_count {
                let framebuffer = gl
                    .create_framebuffer()
                    .map_err(|e| anyhow::anyhow!("framebuffer: {e}"))?;
                gl.bind_framebuffer(glow::FRAMEBUFFER, Some(framebuffer));

                let color_tex = gl
                    .create_texture()
                    .map_err(|e| anyhow::anyhow!("color texture: {e}"))?;
                gl.bind_texture(glow::TEXTURE_2D, Some(color_tex));
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
                gl.tex_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    glow::RGBA8 as i32,
                    width as i32,
                    height as i32,
                    0,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelUnpackData::Slice(None),
                );
                gl.framebuffer_texture_2d(
                    glow::FRAMEBUFFER,
                    glow::COLOR_ATTACHMENT0,
                    glow::TEXTURE_2D,
                    Some(color_tex),
                    0,
                );
                gl.draw_buffers(&[glow::COLOR_ATTACHMENT0]);
                gl.read_buffer(glow::COLOR_ATTACHMENT0);
                let framebuffer_status = gl.check_framebuffer_status(glow::FRAMEBUFFER);
                if framebuffer_status != glow::FRAMEBUFFER_COMPLETE {
                    bail!("off-screen framebuffer incomplete: 0x{framebuffer_status:x}");
                }
                targets.push((framebuffer, color_tex));
            }

            // Compile one program per pass. iPass1.. are declared up to
            // pass_count so any pass may sample any earlier pass.
            let mut passes = Vec::with_capacity(pass_count);
            for src in &sources {
                let program = compile_program(&gl, src, pass_count)?;
                let loc = |name: &str| gl.get_uniform_location(program, name);
                let u_pass = (0..pass_count)
                    .map(|j| loc(&format!("iPass{}", j + 1)))
                    .collect();
                passes.push(Pass {
                    u_resolution: loc("iResolution"),
                    u_time: loc("iTime"),
                    u_rms: loc("iRMS"),
                    u_bass: loc("iBass"),
                    u_mid: loc("iMid"),
                    u_treble: loc("iTreble"),
                    u_spectrum: loc("iSpectrum"),
                    u_pass,
                    program,
                });
            }

            let spectrum_tex = gl
                .create_texture()
                .map_err(|e| anyhow::anyhow!("spectrum texture: {e}"))?;
            gl.active_texture(glow::TEXTURE0);
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
            let empty_spectrum = [0.0f32; SPECTRUM_BINS];
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::R32F as i32,
                SPECTRUM_BINS as i32,
                1,
                0,
                glow::RED,
                glow::FLOAT,
                glow::PixelUnpackData::Slice(Some(bytemuck_cast(&empty_spectrum))),
            );

            gl.viewport(0, 0, width as i32, height as i32);

            Ok(Renderer {
                _backend: backend,
                gl,
                passes,
                targets,
                vao,
                spectrum_tex,
                width,
                height,
                pixels: vec![0u8; (width * height * 3) as usize],
            })
        }
    }

    /// Render one frame and return its rgb24 bytes (top-to-bottom).
    pub fn render_frame(&mut self, time: f32, f: &Features) -> &[u8] {
        let gl = &self.gl;
        unsafe {
            gl.bind_vertex_array(Some(self.vao));

            // Upload spectrum as a 1D (Nx1) R32F texture on unit 0. Storage is
            // allocated once in `new`; per-frame updates only replace contents.
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.spectrum_tex));
            gl.tex_sub_image_2d(
                glow::TEXTURE_2D,
                0,
                0,
                0,
                SPECTRUM_BINS as i32,
                1,
                glow::RED,
                glow::FLOAT,
                glow::PixelUnpackData::Slice(Some(bytemuck_cast(&f.spectrum))),
            );

            // Run each pass in order into its own target. Texture units:
            // 0 = spectrum, 1+j = targets[j] (iPass{j+1}).
            for (i, pass) in self.passes.iter().enumerate() {
                let (framebuffer, _) = self.targets[i];
                gl.bind_framebuffer(glow::FRAMEBUFFER, Some(framebuffer));
                gl.viewport(0, 0, self.width as i32, self.height as i32);
                gl.read_buffer(glow::COLOR_ATTACHMENT0);
                gl.use_program(Some(pass.program));

                if let Some(l) = &pass.u_resolution {
                    gl.uniform_2_f32(Some(l), self.width as f32, self.height as f32);
                }
                if let Some(l) = &pass.u_time {
                    gl.uniform_1_f32(Some(l), time);
                }
                if let Some(l) = &pass.u_rms {
                    gl.uniform_1_f32(Some(l), f.rms);
                }
                if let Some(l) = &pass.u_bass {
                    gl.uniform_1_f32(Some(l), f.bass);
                }
                if let Some(l) = &pass.u_mid {
                    gl.uniform_1_f32(Some(l), f.mid);
                }
                if let Some(l) = &pass.u_treble {
                    gl.uniform_1_f32(Some(l), f.treble);
                }
                if let Some(l) = &pass.u_spectrum {
                    gl.uniform_1_i32(Some(l), 0);
                }
                // Bind earlier passes' outputs. A pass sampling its own or a
                // later pass reads last frame's (or initially black) contents.
                for (j, loc) in pass.u_pass.iter().enumerate() {
                    gl.active_texture(glow::TEXTURE1 + j as u32);
                    gl.bind_texture(glow::TEXTURE_2D, Some(self.targets[j].1));
                    if let Some(l) = loc {
                        gl.uniform_1_i32(Some(l), 1 + j as i32);
                    }
                }

                gl.clear_color(0.0, 0.0, 0.0, 1.0);
                gl.clear(glow::COLOR_BUFFER_BIT);
                gl.draw_arrays(glow::TRIANGLES, 0, 3);
            }

            // Read out the final pass's target.
            let (final_fb, _) = self.targets[self.passes.len() - 1];
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(final_fb));
            gl.read_buffer(glow::COLOR_ATTACHMENT0);
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

enum GlBackend {
    #[cfg(target_os = "macos")]
    NativeMac(macos_gl::NativeContext),
    Egl(Box<EglBackend>),
}

impl GlBackend {
    fn create(width: u32, height: u32) -> Result<Self> {
        #[cfg(target_os = "macos")]
        {
            let requested = std::env::var("SPECTRAFORGE_RENDER_BACKEND")
                .ok()
                .map(|value| value.to_ascii_lowercase());

            match requested.as_deref() {
                Some("egl") | Some("mesa") => {
                    return Ok(Self::Egl(Box::new(EglBackend::new(width, height)?)));
                }
                Some("cgl") | Some("macos") | Some("metal") | Some("native") => {
                    return macos_gl::NativeContext::new().map(Self::NativeMac);
                }
                Some(other) => bail!(
                    "unknown SPECTRAFORGE_RENDER_BACKEND={other}; expected native, metal, cgl, macos, egl, or mesa"
                ),
                None => {}
            }

            match macos_gl::NativeContext::new() {
                Ok(context) => return Ok(Self::NativeMac(context)),
                Err(error) => {
                    eprintln!("native macOS OpenGL failed ({error:#}); falling back to Mesa EGL");
                }
            }
        }

        Ok(Self::Egl(Box::new(EglBackend::new(width, height)?)))
    }

    fn get_proc_address(&self, name: &str) -> *const c_void {
        match self {
            #[cfg(target_os = "macos")]
            Self::NativeMac(context) => context.get_proc_address(name),
            Self::Egl(context) => context.get_proc_address(name),
        }
    }

    fn make_current(&self) -> Result<()> {
        match self {
            #[cfg(target_os = "macos")]
            Self::NativeMac(context) => context.make_current(),
            Self::Egl(context) => context.make_current(),
        }
    }

    fn label(&self) -> &'static str {
        match self {
            #[cfg(target_os = "macos")]
            Self::NativeMac(_) => "macOS CGL/OpenGL (Metal-backed)",
            Self::Egl(_) => "Mesa EGL/OpenGL",
        }
    }
}

struct EglBackend {
    egl: egl::DynamicInstance<egl::EGL1_5>,
    display: egl::Display,
    context: egl::Context,
    surface: egl::Surface,
}

impl EglBackend {
    fn new(width: u32, height: u32) -> Result<Self> {
        let egl = load_egl()?;
        let display = initialize_display(&egl)?;

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

        let backend = Self {
            egl,
            display,
            context,
            surface,
        };
        backend.make_current()?;
        Ok(backend)
    }

    fn get_proc_address(&self, name: &str) -> *const c_void {
        self.egl
            .get_proc_address(name)
            .map_or(std::ptr::null(), |proc| proc as *const c_void)
    }

    fn make_current(&self) -> Result<()> {
        self.egl
            .make_current(
                self.display,
                Some(self.surface),
                Some(self.surface),
                Some(self.context),
            )
            .context("eglMakeCurrent failed")
    }
}

fn initialize_display(egl: &egl::DynamicInstance<egl::EGL1_5>) -> Result<egl::Display> {
    let mut errors = Vec::new();

    match unsafe { egl.get_display(egl::DEFAULT_DISPLAY) } {
        Some(display) => match egl.initialize(display) {
            Ok(_) => return Ok(display),
            Err(error) => errors.push(format!("default display: {error}")),
        },
        None => errors.push("default display: no EGL display".to_string()),
    }

    match unsafe {
        egl.get_platform_display(
            EGL_PLATFORM_SURFACELESS_MESA,
            egl::DEFAULT_DISPLAY,
            &[egl::ATTRIB_NONE],
        )
    } {
        Ok(display) => match egl.initialize(display) {
            Ok(_) => return Ok(display),
            Err(error) => errors.push(format!("surfaceless display: {error}")),
        },
        Err(error) => errors.push(format!("surfaceless display: {error}")),
    }

    bail!("eglInitialize failed; tried {}", errors.join("; "))
}

fn load_egl() -> Result<egl::DynamicInstance<egl::EGL1_5>> {
    if let Ok(path) = std::env::var("SPECTRAFORGE_EGL_LIBRARY") {
        return unsafe { egl::DynamicInstance::<egl::EGL1_5>::load_required_from_filename(&path) }
            .map_err(|e| anyhow::anyhow!("loading EGL from SPECTRAFORGE_EGL_LIBRARY={path}: {e}"));
    }

    #[cfg(target_os = "macos")]
    {
        let candidates = [
            "libEGL.dylib",
            "/opt/homebrew/lib/libEGL.dylib",
            "/opt/homebrew/opt/mesa/lib/libEGL.dylib",
            "/usr/local/lib/libEGL.dylib",
            "/usr/local/opt/mesa/lib/libEGL.dylib",
        ];
        let mut errors = Vec::new();

        for candidate in candidates {
            match unsafe {
                egl::DynamicInstance::<egl::EGL1_5>::load_required_from_filename(candidate)
            } {
                Ok(egl) => return Ok(egl),
                Err(error) => errors.push(format!("{candidate}: {error}")),
            }
        }

        bail!(
            "loading libEGL for macOS failed. Install Mesa with `brew install mesa`, \
             or set SPECTRAFORGE_EGL_LIBRARY to the full libEGL.dylib path. Tried: {}",
            errors.join("; ")
        );
    }

    #[cfg(not(target_os = "macos"))]
    unsafe {
        egl::DynamicInstance::<egl::EGL1_5>::load_required()
            .map_err(|e| anyhow::anyhow!("loading libEGL: {e}"))
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        let _ = self._backend.make_current();
        unsafe {
            for pass in &self.passes {
                self.gl.delete_program(pass.program);
            }
            self.gl.delete_vertex_array(self.vao);
            for (framebuffer, color_tex) in &self.targets {
                self.gl.delete_framebuffer(*framebuffer);
                self.gl.delete_texture(*color_tex);
            }
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

unsafe fn compile_program(
    gl: &glow::Context,
    user_shader: &str,
    pass_count: usize,
) -> Result<glow::Program> {
    // Declare iPass1..iPassN so any pass can sample any pass's output texture.
    let mut pass_uniforms = String::new();
    for i in 1..=pass_count {
        pass_uniforms.push_str(&format!("uniform sampler2D iPass{i};\n"));
    }
    let frag_src = format!("{FRAG_PREAMBLE}{pass_uniforms}\n{user_shader}\n{FRAG_MAIN}");
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

#[cfg(test)]
mod tests {
    use super::split_passes;

    #[test]
    fn no_marker_is_single_pass() {
        let passes = split_passes("void mainImage(){}\n");
        assert_eq!(passes.len(), 1);
        assert!(passes[0].contains("mainImage"));
    }

    #[test]
    fn marker_splits_into_ordered_passes() {
        let src = "A\n//---pass---\nB\n//---pass\nC\n";
        let passes = split_passes(src);
        assert_eq!(passes.len(), 3);
        assert!(passes[0].contains("A") && !passes[0].contains("B"));
        assert!(passes[1].contains("B"));
        assert!(passes[2].contains("C"));
        // The marker lines themselves are dropped.
        assert!(!passes.iter().any(|p| p.contains("---pass")));
    }
}

#[cfg(target_os = "macos")]
mod macos_gl {
    use anyhow::{Context, Result, bail};
    use libloading::Library;
    use std::ffi::{CStr, CString, c_char, c_int, c_void};
    use std::ptr;

    type CglContextObj = *mut c_void;
    type CglPixelFormatObj = *mut c_void;
    type CglPixelFormatAttribute = c_int;

    type CglChoosePixelFormatFn = unsafe extern "C" fn(
        *const CglPixelFormatAttribute,
        *mut CglPixelFormatObj,
        *mut c_int,
    ) -> c_int;
    type CglCreateContextFn =
        unsafe extern "C" fn(CglPixelFormatObj, CglContextObj, *mut CglContextObj) -> c_int;
    type CglDestroyPixelFormatFn = unsafe extern "C" fn(CglPixelFormatObj) -> c_int;
    type CglSetCurrentContextFn = unsafe extern "C" fn(CglContextObj) -> c_int;
    type CglDestroyContextFn = unsafe extern "C" fn(CglContextObj) -> c_int;
    type CglErrorStringFn = unsafe extern "C" fn(c_int) -> *const c_char;

    const CGL_PFA_COLOR_SIZE: CglPixelFormatAttribute = 8;
    const CGL_PFA_ALPHA_SIZE: CglPixelFormatAttribute = 11;
    const CGL_PFA_ACCELERATED: CglPixelFormatAttribute = 73;
    const CGL_PFA_OPENGL_PROFILE: CglPixelFormatAttribute = 99;
    const CGL_OGLP_VERSION_3_2_CORE: CglPixelFormatAttribute = 0x3200;
    const CGL_OGLP_VERSION_GL4_CORE: CglPixelFormatAttribute = 0x4100;

    pub struct NativeContext {
        library: Library,
        context: CglContextObj,
        cgl_set_current_context: CglSetCurrentContextFn,
        cgl_destroy_context: CglDestroyContextFn,
        cgl_error_string: CglErrorStringFn,
    }

    impl NativeContext {
        pub fn new() -> Result<Self> {
            unsafe {
                let library = Library::new("/System/Library/Frameworks/OpenGL.framework/OpenGL")
                    .context("loading OpenGL.framework")?;

                let cgl_choose_pixel_format =
                    load_symbol::<CglChoosePixelFormatFn>(&library, b"CGLChoosePixelFormat\0")?;
                let cgl_create_context =
                    load_symbol::<CglCreateContextFn>(&library, b"CGLCreateContext\0")?;
                let cgl_destroy_pixel_format =
                    load_symbol::<CglDestroyPixelFormatFn>(&library, b"CGLDestroyPixelFormat\0")?;
                let cgl_set_current_context =
                    load_symbol::<CglSetCurrentContextFn>(&library, b"CGLSetCurrentContext\0")?;
                let cgl_destroy_context =
                    load_symbol::<CglDestroyContextFn>(&library, b"CGLDestroyContext\0")?;
                let cgl_error_string =
                    load_symbol::<CglErrorStringFn>(&library, b"CGLErrorString\0")?;

                let pixel_format = choose_pixel_format(
                    cgl_choose_pixel_format,
                    cgl_error_string,
                    &[
                        &[
                            CGL_PFA_ACCELERATED,
                            CGL_PFA_OPENGL_PROFILE,
                            CGL_OGLP_VERSION_GL4_CORE,
                            CGL_PFA_COLOR_SIZE,
                            24,
                            CGL_PFA_ALPHA_SIZE,
                            8,
                            0,
                        ],
                        &[
                            CGL_PFA_ACCELERATED,
                            CGL_PFA_OPENGL_PROFILE,
                            CGL_OGLP_VERSION_3_2_CORE,
                            CGL_PFA_COLOR_SIZE,
                            24,
                            CGL_PFA_ALPHA_SIZE,
                            8,
                            0,
                        ],
                    ],
                )?;

                let mut context = ptr::null_mut();
                let create_result = cgl_create_context(pixel_format, ptr::null_mut(), &mut context);
                let destroy_pixel_format_result = cgl_destroy_pixel_format(pixel_format);
                check_error(
                    destroy_pixel_format_result,
                    cgl_error_string,
                    "CGLDestroyPixelFormat",
                )?;
                check_error(create_result, cgl_error_string, "CGLCreateContext")?;
                if context.is_null() {
                    bail!("CGLCreateContext returned a null context");
                }

                let native = Self {
                    library,
                    context,
                    cgl_set_current_context,
                    cgl_destroy_context,
                    cgl_error_string,
                };
                native.make_current()?;
                Ok(native)
            }
        }

        pub fn get_proc_address(&self, name: &str) -> *const c_void {
            let symbol_name = match CString::new(name) {
                Ok(symbol_name) => symbol_name,
                Err(_) => return ptr::null(),
            };

            unsafe {
                self.library
                    .get::<*const c_void>(symbol_name.as_bytes_with_nul())
                    .map(|symbol| *symbol)
                    .unwrap_or(ptr::null())
            }
        }

        pub fn make_current(&self) -> Result<()> {
            unsafe {
                check_error(
                    (self.cgl_set_current_context)(self.context),
                    self.cgl_error_string,
                    "CGLSetCurrentContext",
                )
            }
        }
    }

    impl Drop for NativeContext {
        fn drop(&mut self) {
            unsafe {
                let _ = (self.cgl_set_current_context)(ptr::null_mut());
                let _ = (self.cgl_destroy_context)(self.context);
            }
        }
    }

    unsafe fn load_symbol<T: Copy>(library: &Library, name: &[u8]) -> Result<T> {
        unsafe {
            library
                .get::<T>(name)
                .map(|symbol| *symbol)
                .with_context(|| format!("loading {}", String::from_utf8_lossy(name)))
        }
    }

    unsafe fn choose_pixel_format(
        cgl_choose_pixel_format: CglChoosePixelFormatFn,
        cgl_error_string: CglErrorStringFn,
        attempts: &[&[CglPixelFormatAttribute]],
    ) -> Result<CglPixelFormatObj> {
        let mut errors = Vec::new();

        for attribs in attempts {
            let mut pixel_format = ptr::null_mut();
            let mut pixel_format_count = 0;
            let error = unsafe {
                cgl_choose_pixel_format(
                    attribs.as_ptr(),
                    &mut pixel_format,
                    &mut pixel_format_count,
                )
            };

            if error == 0 && !pixel_format.is_null() && pixel_format_count > 0 {
                return Ok(pixel_format);
            }

            errors.push(unsafe { cgl_error_message(error, cgl_error_string) });
        }

        bail!(
            "CGLChoosePixelFormat returned no accelerated pixel formats; tried {}",
            errors.join("; ")
        )
    }

    unsafe fn check_error(
        error: c_int,
        cgl_error_string: CglErrorStringFn,
        action: &str,
    ) -> Result<()> {
        if error == 0 {
            return Ok(());
        }

        let message = unsafe { cgl_error_message(error, cgl_error_string) };
        bail!("{action} failed: {message} ({error})")
    }

    unsafe fn cgl_error_message(error: c_int, cgl_error_string: CglErrorStringFn) -> String {
        if error == 0 {
            return "success".to_string();
        }

        let ptr = unsafe { cgl_error_string(error) };
        if ptr.is_null() {
            format!("CGL error {error}")
        } else {
            unsafe { CStr::from_ptr(ptr) }
                .to_string_lossy()
                .into_owned()
        }
    }
}
