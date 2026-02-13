use std::ffi::CString;
use std::fs;
use std::os::raw::{c_char, c_int, c_uchar, c_uint, c_void};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Supported image extensions (lowercase).
const SUPPORTED_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "gif", "webp"];

/// Simple RGBA image buffer.
#[derive(Clone)]
pub struct RgbaImage {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

impl RgbaImage {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            data: vec![0u8; (width * height * 4) as usize],
            width,
            height,
        }
    }

    pub fn from_raw(width: u32, height: u32, data: Vec<u8>) -> Option<Self> {
        if data.len() == (width * height * 4) as usize {
            Some(Self {
                data,
                width,
                height,
            })
        } else {
            None
        }
    }

    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub fn as_raw(&self) -> &[u8] {
        &self.data
    }
}

/// A loaded image — either static or animated.
pub enum LoadedImage {
    Static(RgbaImage),
    Animated { frames: Vec<(RgbaImage, Duration)> },
}

impl LoadedImage {
    pub fn first_frame(&self) -> &RgbaImage {
        match self {
            LoadedImage::Static(img) => img,
            LoadedImage::Animated { frames, .. } => &frames[0].0,
        }
    }
}

/// Collect image paths from CLI arguments.
pub fn collect_paths(args: &[String]) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for arg in args {
        let p = PathBuf::from(arg);
        if p.is_dir() {
            scan_directory(&p, &mut paths);
        } else if is_supported_image(&p) {
            paths.push(p);
        }
    }
    paths.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    paths
}

fn scan_directory(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_directory(&path, out);
        } else if is_supported_image(&path) {
            out.push(path);
        }
    }
}

fn ascii_lower(s: &str) -> String {
    s.bytes().map(|b| b.to_ascii_lowercase() as char).collect()
}

fn is_supported_image(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| SUPPORTED_EXTENSIONS.contains(&ascii_lower(ext).as_str()))
        .unwrap_or(false)
}

/// Load an image from disk.
pub fn load_image(path: &Path) -> Result<LoadedImage, String> {
    let ext = ascii_lower(path.extension().and_then(|e| e.to_str()).unwrap_or(""));

    match ext.as_str() {
        "jpg" | "jpeg" => load_jpeg(path),
        "png" => load_png(path),
        "webp" => load_webp(path),
        "gif" => load_gif(path),
        _ => Err(format!("Unsupported format: {}", ext)),
    }
}

// ============================================================
// JPEG via system libturbojpeg
// ============================================================

fn load_jpeg(path: &Path) -> Result<LoadedImage, String> {
    let data = fs::read(path).map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

    let image = turbojpeg::decompress(&data, turbojpeg::PixelFormat::RGBA)
        .map_err(|e| format!("Failed to decode JPEG {}: {}", path.display(), e))?;

    let mut img = RgbaImage::from_raw(image.width as u32, image.height as u32, image.pixels)
        .ok_or_else(|| "JPEG pixel buffer size mismatch".to_string())?;

    // Apply EXIF orientation
    if let Some(orientation) = read_exif_orientation(&data) {
        img = apply_orientation(img, orientation);
    }

    Ok(LoadedImage::Static(img))
}

// ============================================================
// PNG via system libpng16
// ============================================================

// libpng FFI declarations
#[allow(non_camel_case_types)]
mod libpng {
    use std::os::raw::{c_char, c_int, c_uchar, c_uint, c_void};

    pub type png_structp = *mut c_void;
    pub type png_infop = *mut c_void;
    pub type png_bytep = *mut c_uchar;
    pub type png_bytepp = *mut png_bytep;
    // jmp_buf is 200 bytes on x86_64 Linux
    pub type jmp_buf = [u8; 200];

    pub const PNG_COLOR_TYPE_PALETTE: c_uchar = 3;
    pub const PNG_COLOR_TYPE_GRAY: c_uchar = 0;
    pub const PNG_COLOR_TYPE_GRAY_ALPHA: c_uchar = 4;
    pub const PNG_COLOR_TYPE_RGB: c_uchar = 2;

    extern "C" {
        pub fn setjmp(buf: *mut jmp_buf) -> c_int;
        pub fn longjmp(buf: *mut jmp_buf, val: c_int) -> !;
    }

    #[link(name = "png16")]
    extern "C" {
        pub fn png_create_read_struct(
            ver: *const c_char,
            error_ptr: *mut c_void,
            error_fn: Option<unsafe extern "C" fn(png_structp, *const c_char)>,
            warn_fn: Option<unsafe extern "C" fn(png_structp, *const c_char)>,
        ) -> png_structp;
        pub fn png_create_info_struct(png_ptr: png_structp) -> png_infop;
        pub fn png_destroy_read_struct(
            png_ptr: *mut png_structp,
            info_ptr: *mut png_infop,
            end_info: *mut png_infop,
        );

        pub fn png_read_info(png_ptr: png_structp, info_ptr: png_infop);
        pub fn png_get_IHDR(
            png_ptr: png_structp,
            info_ptr: png_infop,
            width: *mut c_uint,
            height: *mut c_uint,
            bit_depth: *mut c_int,
            color_type: *mut c_int,
            interlace: *mut c_int,
            compression: *mut c_int,
            filter: *mut c_int,
        ) -> c_uint;
        pub fn png_set_expand(png_ptr: png_structp);
        pub fn png_set_gray_to_rgb(png_ptr: png_structp);
        pub fn png_set_add_alpha(png_ptr: png_structp, filler: c_uint, flags: c_int);
        pub fn png_set_strip_16(png_ptr: png_structp);
        pub fn png_set_palette_to_rgb(png_ptr: png_structp);
        pub fn png_set_tRNS_to_alpha(png_ptr: png_structp);
        pub fn png_read_update_info(png_ptr: png_structp, info_ptr: png_infop);
        pub fn png_read_image(png_ptr: png_structp, row_pointers: png_bytepp);
        pub fn png_read_end(png_ptr: png_structp, info_ptr: png_infop);
        pub fn png_set_longjmp_fn(
            png_ptr: png_structp,
            longjmp_fn: unsafe extern "C" fn(*mut jmp_buf, c_int) -> !,
            jmp_buf_size: usize,
        ) -> *mut jmp_buf;
        pub fn png_set_read_fn(
            png_ptr: png_structp,
            io_ptr: *mut c_void,
            read_fn: unsafe extern "C" fn(png_structp, png_bytep, usize),
        );
        pub fn png_get_io_ptr(png_ptr: png_structp) -> *mut c_void;
    }
}

/// State for reading PNG from memory.
struct PngReadState {
    data: *const u8,
    len: usize,
    offset: usize,
}

unsafe extern "C" fn png_read_callback(
    png_ptr: libpng::png_structp,
    out: libpng::png_bytep,
    length: usize,
) {
    let state = &mut *(libpng::png_get_io_ptr(png_ptr) as *mut PngReadState);
    let remaining = state.len - state.offset;
    let to_read = length.min(remaining);
    std::ptr::copy_nonoverlapping(state.data.add(state.offset), out, to_read);
    state.offset += to_read;
}

fn load_png(path: &Path) -> Result<LoadedImage, String> {
    let data = fs::read(path).map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

    // Check PNG signature
    if data.len() < 8 || &data[0..4] != b"\x89PNG" {
        return Err(format!("Not a valid PNG: {}", path.display()));
    }

    unsafe {
        let ver = b"1.6.0\0".as_ptr() as *const c_char;
        let png_ptr = libpng::png_create_read_struct(ver, std::ptr::null_mut(), None, None);
        if png_ptr.is_null() {
            return Err("png_create_read_struct failed".to_string());
        }

        let info_ptr = libpng::png_create_info_struct(png_ptr);
        if info_ptr.is_null() {
            let mut pp = png_ptr;
            libpng::png_destroy_read_struct(&mut pp, std::ptr::null_mut(), std::ptr::null_mut());
            return Err("png_create_info_struct failed".to_string());
        }

        // Set up error handling via setjmp
        let jmpbuf = libpng::png_set_longjmp_fn(
            png_ptr,
            libpng::longjmp,
            std::mem::size_of::<libpng::jmp_buf>(),
        );
        if jmpbuf.is_null() {
            let mut pp = png_ptr;
            let mut ip = info_ptr;
            libpng::png_destroy_read_struct(&mut pp, &mut ip, std::ptr::null_mut());
            return Err("png_set_longjmp_fn failed".to_string());
        }

        if libpng::setjmp(jmpbuf) != 0 {
            let mut pp = png_ptr;
            let mut ip = info_ptr;
            libpng::png_destroy_read_struct(&mut pp, &mut ip, std::ptr::null_mut());
            return Err(format!("PNG decode error: {}", path.display()));
        }

        // Set up memory read
        let mut read_state = PngReadState {
            data: data.as_ptr(),
            len: data.len(),
            offset: 0,
        };
        libpng::png_set_read_fn(
            png_ptr,
            &mut read_state as *mut PngReadState as *mut c_void,
            png_read_callback,
        );

        // Read header
        libpng::png_read_info(png_ptr, info_ptr);

        let mut width: c_uint = 0;
        let mut height: c_uint = 0;
        let mut bit_depth: c_int = 0;
        let mut color_type: c_int = 0;
        libpng::png_get_IHDR(
            png_ptr,
            info_ptr,
            &mut width,
            &mut height,
            &mut bit_depth,
            &mut color_type,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );

        // Set transforms to get RGBA output
        let ct = color_type as c_uchar;
        if ct == libpng::PNG_COLOR_TYPE_PALETTE {
            libpng::png_set_palette_to_rgb(png_ptr);
        }
        if ct == libpng::PNG_COLOR_TYPE_GRAY && bit_depth < 8 {
            libpng::png_set_expand(png_ptr);
        }
        if ct == libpng::PNG_COLOR_TYPE_GRAY || ct == libpng::PNG_COLOR_TYPE_GRAY_ALPHA {
            libpng::png_set_gray_to_rgb(png_ptr);
        }
        // Expand tRNS chunk to alpha
        libpng::png_set_tRNS_to_alpha(png_ptr);
        // Add alpha channel if missing
        if ct == libpng::PNG_COLOR_TYPE_RGB
            || ct == libpng::PNG_COLOR_TYPE_GRAY
            || ct == libpng::PNG_COLOR_TYPE_PALETTE
        {
            libpng::png_set_add_alpha(png_ptr, 0xFF, 1); // filler after RGB
        }
        if bit_depth == 16 {
            libpng::png_set_strip_16(png_ptr);
        }

        libpng::png_read_update_info(png_ptr, info_ptr);

        // Allocate row pointers
        let stride = (width * 4) as usize;
        let mut rgba_data = vec![0u8; stride * height as usize];
        let mut row_ptrs: Vec<*mut c_uchar> = (0..height as usize)
            .map(|row| rgba_data.as_mut_ptr().add(row * stride))
            .collect();

        libpng::png_read_image(png_ptr, row_ptrs.as_mut_ptr());
        libpng::png_read_end(png_ptr, info_ptr);

        let mut pp = png_ptr;
        let mut ip = info_ptr;
        libpng::png_destroy_read_struct(&mut pp, &mut ip, std::ptr::null_mut());

        let img = RgbaImage::from_raw(width, height, rgba_data)
            .ok_or_else(|| "PNG pixel buffer size mismatch".to_string())?;

        Ok(LoadedImage::Static(img))
    }
}

// ============================================================
// WebP via system libwebp
// ============================================================

fn load_webp(path: &Path) -> Result<LoadedImage, String> {
    let data = fs::read(path).map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

    let mut width: std::ffi::c_int = 0;
    let mut height: std::ffi::c_int = 0;

    let ptr =
        unsafe { libwebp_sys::WebPDecodeRGBA(data.as_ptr(), data.len(), &mut width, &mut height) };

    if ptr.is_null() {
        return Err(format!("Failed to decode WebP {}", path.display()));
    }

    let w = width as u32;
    let h = height as u32;
    let len = (w * h * 4) as usize;

    let rgba_data = unsafe { std::slice::from_raw_parts(ptr, len).to_vec() };
    unsafe {
        libwebp_sys::WebPFree(ptr as *mut std::ffi::c_void);
    }

    let img = RgbaImage::from_raw(w, h, rgba_data)
        .ok_or_else(|| "WebP pixel buffer size mismatch".to_string())?;

    Ok(LoadedImage::Static(img))
}

// ============================================================
// GIF via system libgif
// ============================================================

#[allow(non_camel_case_types, non_snake_case, dead_code)]
mod libgif {
    use std::os::raw::{c_char, c_int, c_uchar, c_void};

    pub type GifWord = c_int;
    pub type GifByteType = c_uchar;
    pub type GifPixelType = c_uchar;

    #[repr(C)]
    pub struct GifColorType {
        pub Red: GifByteType,
        pub Green: GifByteType,
        pub Blue: GifByteType,
    }

    #[repr(C)]
    pub struct ColorMapObject {
        pub ColorCount: c_int,
        pub BitsPerPixel: c_int,
        pub SortFlag: bool,
        pub Colors: *mut GifColorType,
    }

    #[repr(C)]
    pub struct GifImageDesc {
        pub Left: GifWord,
        pub Top: GifWord,
        pub Width: GifWord,
        pub Height: GifWord,
        pub Interlace: bool,
        pub ColorMap: *mut ColorMapObject,
    }

    #[repr(C)]
    pub struct ExtensionBlock {
        pub ByteCount: c_int,
        pub Bytes: *mut GifByteType,
        pub Function: c_int,
    }

    #[repr(C)]
    pub struct SavedImage {
        pub ImageDesc: GifImageDesc,
        pub RasterBits: *mut GifByteType,
        pub ExtensionBlockCount: c_int,
        pub ExtensionBlocks: *mut ExtensionBlock,
    }

    #[repr(C)]
    pub struct GifFileType {
        pub SWidth: GifWord,
        pub SHeight: GifWord,
        pub SColorResolution: GifWord,
        pub SBackGroundColor: GifWord,
        pub AspectByte: GifByteType,
        pub SColorMap: *mut ColorMapObject,
        pub ImageCount: c_int,
        pub Image: GifImageDesc,
        pub SavedImages: *mut SavedImage,
        pub ExtensionBlockCount: c_int,
        pub ExtensionBlocks: *mut ExtensionBlock,
        pub Error: c_int,
        pub UserData: *mut c_void,
        pub Private: *mut c_void,
    }

    #[repr(C)]
    pub struct GraphicsControlBlock {
        pub DisposalMode: c_int,
        pub UserInputFlag: bool,
        pub DelayTime: c_int,
        pub TransparentColor: c_int,
    }

    pub const GIF_OK: c_int = 1;

    #[link(name = "gif")]
    extern "C" {
        pub fn DGifOpenFileName(filename: *const c_char, error: *mut c_int) -> *mut GifFileType;
        pub fn DGifSlurp(gif: *mut GifFileType) -> c_int;
        pub fn DGifCloseFile(gif: *mut GifFileType, error: *mut c_int) -> c_int;
        pub fn DGifSavedExtensionToGCB(
            gif: *mut GifFileType,
            image_index: c_int,
            gcb: *mut GraphicsControlBlock,
        ) -> c_int;
    }
}

fn load_gif(path: &Path) -> Result<LoadedImage, String> {
    let c_path = CString::new(path.to_str().ok_or_else(|| "Invalid path".to_string())?)
        .map_err(|_| "Path contains null byte".to_string())?;

    unsafe {
        let mut error: c_int = 0;
        let gif = libgif::DGifOpenFileName(c_path.as_ptr(), &mut error);
        if gif.is_null() {
            return Err(format!(
                "Failed to open GIF {}: error {}",
                path.display(),
                error
            ));
        }

        if libgif::DGifSlurp(gif) != libgif::GIF_OK {
            let err = (*gif).Error;
            libgif::DGifCloseFile(gif, std::ptr::null_mut());
            return Err(format!(
                "Failed to decode GIF {}: error {}",
                path.display(),
                err
            ));
        }

        let canvas_w = (*gif).SWidth as u32;
        let canvas_h = (*gif).SHeight as u32;
        let image_count = (*gif).ImageCount as usize;

        if image_count == 0 || canvas_w == 0 || canvas_h == 0 {
            libgif::DGifCloseFile(gif, std::ptr::null_mut());
            return Err(format!("Empty GIF: {}", path.display()));
        }

        let mut frames: Vec<(RgbaImage, Duration)> = Vec::with_capacity(image_count);
        let mut canvas = vec![0u8; (canvas_w * canvas_h * 4) as usize];

        for i in 0..image_count {
            let saved = &*(*gif).SavedImages.add(i);
            let desc = &saved.ImageDesc;
            let fw = desc.Width as u32;
            let fh = desc.Height as u32;
            let fx = desc.Left as u32;
            let fy = desc.Top as u32;

            // Get color map (local or global)
            let cmap = if !desc.ColorMap.is_null() {
                desc.ColorMap
            } else {
                (*gif).SColorMap
            };
            if cmap.is_null() || saved.RasterBits.is_null() {
                continue;
            }
            let colors = (*cmap).Colors;
            let color_count = (*cmap).ColorCount;

            // Get graphics control block for timing and transparency
            let mut gcb = libgif::GraphicsControlBlock {
                DisposalMode: 0,
                UserInputFlag: false,
                DelayTime: 0,
                TransparentColor: -1,
            };
            libgif::DGifSavedExtensionToGCB(gif, i as c_int, &mut gcb);

            let transparent = gcb.TransparentColor;
            let delay_ms = ((gcb.DelayTime as u64) * 10).max(10);

            // Map palette indices to RGBA and composite onto canvas
            for row in 0..fh {
                for col in 0..fw {
                    let src_idx = (row * fw + col) as usize;
                    let pixel_idx = *saved.RasterBits.add(src_idx) as c_int;

                    let dx = fx + col;
                    let dy = fy + row;
                    if dx >= canvas_w || dy >= canvas_h {
                        continue;
                    }

                    if pixel_idx == transparent {
                        continue; // transparent pixel, keep canvas
                    }

                    if pixel_idx < color_count {
                        let color = &*colors.add(pixel_idx as usize);
                        let dst = ((dy * canvas_w + dx) * 4) as usize;
                        canvas[dst] = color.Red;
                        canvas[dst + 1] = color.Green;
                        canvas[dst + 2] = color.Blue;
                        canvas[dst + 3] = 255;
                    }
                }
            }

            let img = RgbaImage {
                data: canvas.clone(),
                width: canvas_w,
                height: canvas_h,
            };
            frames.push((img, Duration::from_millis(delay_ms)));
        }

        libgif::DGifCloseFile(gif, std::ptr::null_mut());

        if frames.is_empty() {
            return Err(format!("No frames decoded from GIF: {}", path.display()));
        }

        if frames.len() == 1 {
            let (img, _) = frames.into_iter().next().unwrap();
            return Ok(LoadedImage::Static(img));
        }

        Ok(LoadedImage::Animated { frames })
    }
}

// ============================================================
// Thumbnail-optimized loading (JPEG DCT scaling)
// ============================================================

/// Load an image and return a thumbnail-sized RgbaImage.
/// For JPEG: uses turbojpeg DCT scaling to decode at reduced resolution.
/// For other formats: decodes at full resolution and resizes.
pub fn load_image_thumbnail(path: &Path, thumb_size: u32) -> Result<RgbaImage, String> {
    let ext = ascii_lower(path.extension().and_then(|e| e.to_str()).unwrap_or(""));

    match ext.as_str() {
        "jpg" | "jpeg" => load_jpeg_thumbnail(path, thumb_size),
        _ => {
            // Non-JPEG: full decode + resize
            let loaded = load_image(path)?;
            let frame = loaded.first_frame();
            Ok(crate::render::generate_thumbnail(frame, thumb_size))
        }
    }
}

/// Load a JPEG at reduced resolution using DCT scaling, then resize to thumbnail.
fn load_jpeg_thumbnail(path: &Path, thumb_size: u32) -> Result<RgbaImage, String> {
    let data = fs::read(path).map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

    let mut decompressor = turbojpeg::Decompressor::new()
        .map_err(|e| format!("Failed to create decompressor: {}", e))?;

    let header = decompressor
        .read_header(&data)
        .map_err(|e| format!("Failed to read JPEG header {}: {}", path.display(), e))?;

    // Pick the best DCT scaling factor: smallest where both scaled dims >= thumb_size
    let scaling_factors = [
        turbojpeg::ScalingFactor::ONE_EIGHTH,
        turbojpeg::ScalingFactor::ONE_QUARTER,
        turbojpeg::ScalingFactor::ONE_HALF,
        turbojpeg::ScalingFactor::ONE,
    ];

    let mut best = turbojpeg::ScalingFactor::ONE;
    for &sf in &scaling_factors {
        let sw = sf.scale(header.width);
        let sh = sf.scale(header.height);
        if sw >= thumb_size as usize && sh >= thumb_size as usize {
            best = sf;
            break;
        }
    }

    if best != turbojpeg::ScalingFactor::ONE {
        decompressor
            .set_scaling_factor(best)
            .map_err(|e| format!("Failed to set scaling factor: {}", e))?;
    }

    let scaled_header = header.scaled(best);
    let w = scaled_header.width;
    let h = scaled_header.height;
    let pitch = w * 4;

    let mut image = turbojpeg::Image {
        pixels: vec![0u8; h * pitch],
        width: w,
        pitch,
        height: h,
        format: turbojpeg::PixelFormat::RGBA,
    };

    decompressor
        .decompress(&data, image.as_deref_mut())
        .map_err(|e| format!("Failed to decode JPEG {}: {}", path.display(), e))?;

    let mut img = RgbaImage::from_raw(w as u32, h as u32, image.pixels)
        .ok_or_else(|| "JPEG pixel buffer size mismatch".to_string())?;

    // Apply EXIF orientation
    if let Some(orientation) = read_exif_orientation(&data) {
        img = apply_orientation(img, orientation);
    }

    Ok(crate::render::generate_thumbnail(&img, thumb_size))
}

// ============================================================
// Manual EXIF orientation parser
// ============================================================

/// Parse EXIF orientation tag from raw JPEG data.
/// Looks for APP1 marker, parses TIFF header, walks IFD0 for tag 0x0112.
fn read_exif_orientation(data: &[u8]) -> Option<u32> {
    // JPEG must start with SOI (0xFFD8)
    if data.len() < 4 || data[0] != 0xFF || data[1] != 0xD8 {
        return None;
    }

    let mut pos = 2;
    // Scan for APP1 marker (0xFFE1)
    while pos + 4 < data.len() {
        if data[pos] != 0xFF {
            return None;
        }
        let marker = data[pos + 1];
        let seg_len = u16::from_be_bytes([data[pos + 2], data[pos + 3]]) as usize;
        if marker == 0xE1 {
            // APP1 found — check for "Exif\0\0" header
            let seg_start = pos + 4;
            if seg_start + 6 > data.len() {
                return None;
            }
            if &data[seg_start..seg_start + 6] != b"Exif\0\0" {
                pos += 2 + seg_len;
                continue;
            }
            let tiff_start = seg_start + 6;
            return parse_tiff_orientation(data, tiff_start);
        }
        if marker == 0xDA {
            break; // SOS — no more markers before image data
        }
        pos += 2 + seg_len;
    }
    None
}

fn parse_tiff_orientation(data: &[u8], tiff_offset: usize) -> Option<u32> {
    if tiff_offset + 8 > data.len() {
        return None;
    }

    let d = &data[tiff_offset..];
    // Byte order: "II" = little-endian, "MM" = big-endian
    let le = match (d[0], d[1]) {
        (b'I', b'I') => true,
        (b'M', b'M') => false,
        _ => return None,
    };

    let read_u16 = |off: usize| -> Option<u16> {
        if off + 2 > d.len() {
            return None;
        }
        Some(if le {
            u16::from_le_bytes([d[off], d[off + 1]])
        } else {
            u16::from_be_bytes([d[off], d[off + 1]])
        })
    };

    let read_u32 = |off: usize| -> Option<u32> {
        if off + 4 > d.len() {
            return None;
        }
        Some(if le {
            u32::from_le_bytes([d[off], d[off + 1], d[off + 2], d[off + 3]])
        } else {
            u32::from_be_bytes([d[off], d[off + 1], d[off + 2], d[off + 3]])
        })
    };

    // Check TIFF magic (42)
    if read_u16(2)? != 42 {
        return None;
    }

    // IFD0 offset
    let ifd_offset = read_u32(4)? as usize;
    if ifd_offset + 2 > d.len() {
        return None;
    }

    let entry_count = read_u16(ifd_offset)? as usize;
    let entries_start = ifd_offset + 2;

    for i in 0..entry_count {
        let entry_off = entries_start + i * 12;
        if entry_off + 12 > d.len() {
            break;
        }
        let tag = read_u16(entry_off)?;
        if tag == 0x0112 {
            // Orientation tag — value is in offset field for SHORT type
            let value = read_u16(entry_off + 8)?;
            return Some(value as u32);
        }
    }
    None
}

// ============================================================
// EXIF orientation transforms
// ============================================================

fn apply_orientation(img: RgbaImage, orientation: u32) -> RgbaImage {
    match orientation {
        2 => flip_h(img),
        3 => rotate_180(img),
        4 => flip_v(img),
        5 => flip_h(rotate_90(img)),
        6 => rotate_90(img),
        7 => flip_h(rotate_270(img)),
        8 => rotate_270(img),
        _ => img,
    }
}

pub fn rotate_90(img: RgbaImage) -> RgbaImage {
    let (w, h) = (img.width, img.height);
    let mut out = RgbaImage::new(h, w);
    for y in 0..h {
        for x in 0..w {
            let src = ((y * w + x) * 4) as usize;
            let dst_x = h - 1 - y;
            let dst_y = x;
            let dst = ((dst_y * h + dst_x) * 4) as usize;
            out.data[dst..dst + 4].copy_from_slice(&img.data[src..src + 4]);
        }
    }
    out
}

fn rotate_180(img: RgbaImage) -> RgbaImage {
    let (w, h) = (img.width, img.height);
    let mut out = RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let src = ((y * w + x) * 4) as usize;
            let dst = (((h - 1 - y) * w + (w - 1 - x)) * 4) as usize;
            out.data[dst..dst + 4].copy_from_slice(&img.data[src..src + 4]);
        }
    }
    out
}

pub fn rotate_270(img: RgbaImage) -> RgbaImage {
    let (w, h) = (img.width, img.height);
    let mut out = RgbaImage::new(h, w);
    for y in 0..h {
        for x in 0..w {
            let src = ((y * w + x) * 4) as usize;
            let dst_x = y;
            let dst_y = w - 1 - x;
            let dst = ((dst_y * h + dst_x) * 4) as usize;
            out.data[dst..dst + 4].copy_from_slice(&img.data[src..src + 4]);
        }
    }
    out
}

fn flip_h(img: RgbaImage) -> RgbaImage {
    let (w, h) = (img.width, img.height);
    let mut out = RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let src = ((y * w + x) * 4) as usize;
            let dst = ((y * w + (w - 1 - x)) * 4) as usize;
            out.data[dst..dst + 4].copy_from_slice(&img.data[src..src + 4]);
        }
    }
    out
}

// ============================================================
// Full EXIF tag reader
// ============================================================

/// Read all available EXIF tags from raw JPEG data.
/// Returns a list of (label, value) pairs for display.
pub fn read_exif_tags(data: &[u8]) -> Vec<(String, String)> {
    // JPEG must start with SOI (0xFFD8)
    if data.len() < 4 || data[0] != 0xFF || data[1] != 0xD8 {
        return Vec::new();
    }

    let mut pos = 2;
    while pos + 4 < data.len() {
        if data[pos] != 0xFF {
            return Vec::new();
        }
        let marker = data[pos + 1];
        let seg_len = u16::from_be_bytes([data[pos + 2], data[pos + 3]]) as usize;
        if marker == 0xE1 {
            let seg_start = pos + 4;
            if seg_start + 6 > data.len() {
                return Vec::new();
            }
            if &data[seg_start..seg_start + 6] != b"Exif\0\0" {
                pos += 2 + seg_len;
                continue;
            }
            let tiff_start = seg_start + 6;
            return parse_all_exif_tags(data, tiff_start);
        }
        if marker == 0xDA {
            break;
        }
        pos += 2 + seg_len;
    }
    Vec::new()
}

fn parse_all_exif_tags(data: &[u8], tiff_offset: usize) -> Vec<(String, String)> {
    if tiff_offset + 8 > data.len() {
        return Vec::new();
    }

    let d = &data[tiff_offset..];
    let le = match (d[0], d[1]) {
        (b'I', b'I') => true,
        (b'M', b'M') => false,
        _ => return Vec::new(),
    };

    let read_u16 = |off: usize| -> Option<u16> {
        if off + 2 > d.len() {
            return None;
        }
        Some(if le {
            u16::from_le_bytes([d[off], d[off + 1]])
        } else {
            u16::from_be_bytes([d[off], d[off + 1]])
        })
    };

    let read_u32 = |off: usize| -> Option<u32> {
        if off + 4 > d.len() {
            return None;
        }
        Some(if le {
            u32::from_le_bytes([d[off], d[off + 1], d[off + 2], d[off + 3]])
        } else {
            u32::from_be_bytes([d[off], d[off + 1], d[off + 2], d[off + 3]])
        })
    };

    if read_u16(2) != Some(42) {
        return Vec::new();
    }

    let ifd_offset = match read_u32(4) {
        Some(v) => v as usize,
        None => return Vec::new(),
    };

    let mut tags = Vec::new();
    let mut exif_ifd_offset: Option<usize> = None;
    let mut gps_ifd_offset: Option<usize> = None;

    // Parse IFD0
    parse_ifd_tags(
        d,
        ifd_offset,
        le,
        &IFD0_TAGS,
        &mut tags,
        &mut exif_ifd_offset,
        &mut gps_ifd_offset,
    );

    // Parse EXIF sub-IFD
    if let Some(offset) = exif_ifd_offset {
        parse_ifd_tags(d, offset, le, &EXIF_TAGS, &mut tags, &mut None, &mut None);
    }

    // Parse GPS IFD
    if let Some(offset) = gps_ifd_offset {
        parse_gps_tags(d, offset, le, &mut tags);
    }

    tags
}

/// Known IFD0 tags
const IFD0_TAGS: &[(u16, &str)] = &[
    (0x010F, "Make"),
    (0x0110, "Model"),
    (0x0112, "Orientation"),
    (0x011A, "X Resolution"),
    (0x011B, "Y Resolution"),
    (0x0131, "Software"),
    (0x0132, "Date/Time"),
    (0x013B, "Artist"),
    (0x8298, "Copyright"),
];

/// Known EXIF sub-IFD tags
const EXIF_TAGS: &[(u16, &str)] = &[
    (0x829A, "Exposure Time"),
    (0x829D, "F-Number"),
    (0x8827, "ISO"),
    (0x9003, "Date Original"),
    (0x9004, "Date Digitized"),
    (0x9204, "Exposure Bias"),
    (0x9207, "Metering Mode"),
    (0x9209, "Flash"),
    (0x920A, "Focal Length"),
    (0xA001, "Color Space"),
    (0xA002, "Width"),
    (0xA003, "Height"),
    (0xA402, "Exposure Mode"),
    (0xA403, "White Balance"),
    (0xA434, "Lens Model"),
];

/// TIFF data type sizes: 0=unused, 1=BYTE, 2=ASCII, 3=SHORT, 4=LONG, 5=RATIONAL,
/// 6=SBYTE, 7=UNDEFINED, 8=SSHORT, 9=SLONG, 10=SRATIONAL
const TYPE_SIZES: &[usize] = &[0, 1, 1, 2, 4, 8, 1, 1, 2, 4, 8];

fn parse_ifd_tags(
    d: &[u8],
    ifd_offset: usize,
    le: bool,
    known_tags: &[(u16, &str)],
    tags: &mut Vec<(String, String)>,
    exif_ifd: &mut Option<usize>,
    gps_ifd: &mut Option<usize>,
) {
    let read_u16 = |off: usize| -> Option<u16> {
        if off + 2 > d.len() {
            return None;
        }
        Some(if le {
            u16::from_le_bytes([d[off], d[off + 1]])
        } else {
            u16::from_be_bytes([d[off], d[off + 1]])
        })
    };

    let read_u32 = |off: usize| -> Option<u32> {
        if off + 4 > d.len() {
            return None;
        }
        Some(if le {
            u32::from_le_bytes([d[off], d[off + 1], d[off + 2], d[off + 3]])
        } else {
            u32::from_be_bytes([d[off], d[off + 1], d[off + 2], d[off + 3]])
        })
    };

    if ifd_offset + 2 > d.len() {
        return;
    }
    let entry_count = match read_u16(ifd_offset) {
        Some(v) => v as usize,
        None => return,
    };
    let entries_start = ifd_offset + 2;

    for i in 0..entry_count {
        let entry_off = entries_start + i * 12;
        if entry_off + 12 > d.len() {
            break;
        }
        let tag = match read_u16(entry_off) {
            Some(v) => v,
            None => continue,
        };
        let dtype = match read_u16(entry_off + 2) {
            Some(v) => v as usize,
            None => continue,
        };
        let count = match read_u32(entry_off + 4) {
            Some(v) => v as usize,
            None => continue,
        };

        // EXIF sub-IFD pointer
        if tag == 0x8769 {
            if let Some(offset) = read_u32(entry_off + 8) {
                *exif_ifd = Some(offset as usize);
            }
            continue;
        }
        // GPS IFD pointer
        if tag == 0x8825 {
            if let Some(offset) = read_u32(entry_off + 8) {
                *gps_ifd = Some(offset as usize);
            }
            continue;
        }

        // Check if this is a known tag
        let label = match known_tags.iter().find(|(t, _)| *t == tag) {
            Some((_, name)) => *name,
            None => continue,
        };

        let value = read_tag_value(d, entry_off + 8, dtype, count, le, tag);
        if let Some(v) = value {
            if !v.is_empty() {
                tags.push((label.to_string(), v));
            }
        }
    }
}

fn read_tag_value(
    d: &[u8],
    value_off: usize,
    dtype: usize,
    count: usize,
    le: bool,
    tag: u16,
) -> Option<String> {
    let read_u16_at = |off: usize| -> Option<u16> {
        if off + 2 > d.len() {
            return None;
        }
        Some(if le {
            u16::from_le_bytes([d[off], d[off + 1]])
        } else {
            u16::from_be_bytes([d[off], d[off + 1]])
        })
    };

    let read_u32_at = |off: usize| -> Option<u32> {
        if off + 4 > d.len() {
            return None;
        }
        Some(if le {
            u32::from_le_bytes([d[off], d[off + 1], d[off + 2], d[off + 3]])
        } else {
            u32::from_be_bytes([d[off], d[off + 1], d[off + 2], d[off + 3]])
        })
    };

    let read_i32_at = |off: usize| -> Option<i32> {
        if off + 4 > d.len() {
            return None;
        }
        Some(if le {
            i32::from_le_bytes([d[off], d[off + 1], d[off + 2], d[off + 3]])
        } else {
            i32::from_be_bytes([d[off], d[off + 1], d[off + 2], d[off + 3]])
        })
    };

    // Determine if value is inline or at an offset
    let type_size = if dtype < TYPE_SIZES.len() {
        TYPE_SIZES[dtype]
    } else {
        return None;
    };
    let total_bytes = type_size * count;
    let data_off = if total_bytes <= 4 {
        value_off // inline
    } else {
        read_u32_at(value_off)? as usize // offset into TIFF data
    };

    match dtype {
        // ASCII
        2 => {
            if data_off + count > d.len() {
                return None;
            }
            let bytes = &d[data_off..data_off + count];
            // Trim trailing null bytes
            let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
            let s: String = bytes[..end]
                .iter()
                .map(|&b| {
                    if b >= 0x20 && b <= 0x7E {
                        b as char
                    } else {
                        '?'
                    }
                })
                .collect();
            Some(s)
        }
        // SHORT
        3 => {
            let val = read_u16_at(data_off)? as u32;
            Some(format_tag_short(tag, val))
        }
        // LONG
        4 => {
            let val = read_u32_at(data_off)?;
            Some(format!("{}", val))
        }
        // RATIONAL (unsigned)
        5 => {
            let num = read_u32_at(data_off)?;
            let den = read_u32_at(data_off + 4)?;
            Some(format_rational(tag, num, den))
        }
        // SRATIONAL (signed)
        10 => {
            let num = read_i32_at(data_off)?;
            let den = read_i32_at(data_off + 4)?;
            Some(format_srational(tag, num, den))
        }
        _ => None,
    }
}

fn format_tag_short(tag: u16, val: u32) -> String {
    match tag {
        // Orientation
        0x0112 => match val {
            1 => "Normal".to_string(),
            2 => "Flipped horizontally".to_string(),
            3 => "Rotated 180".to_string(),
            4 => "Flipped vertically".to_string(),
            5 => "Transposed".to_string(),
            6 => "Rotated 90 CW".to_string(),
            7 => "Transversed".to_string(),
            8 => "Rotated 270 CW".to_string(),
            _ => format!("{}", val),
        },
        // MeteringMode
        0x9207 => match val {
            0 => "Unknown".to_string(),
            1 => "Average".to_string(),
            2 => "Center-weighted".to_string(),
            3 => "Spot".to_string(),
            4 => "Multi-spot".to_string(),
            5 => "Pattern".to_string(),
            6 => "Partial".to_string(),
            _ => format!("{}", val),
        },
        // Flash
        0x9209 => {
            if val & 1 == 0 {
                "No flash".to_string()
            } else {
                "Flash fired".to_string()
            }
        }
        // ColorSpace
        0xA001 => match val {
            1 => "sRGB".to_string(),
            0xFFFF => "Uncalibrated".to_string(),
            _ => format!("{}", val),
        },
        // ExposureMode
        0xA402 => match val {
            0 => "Auto".to_string(),
            1 => "Manual".to_string(),
            2 => "Auto bracket".to_string(),
            _ => format!("{}", val),
        },
        // WhiteBalance
        0xA403 => match val {
            0 => "Auto".to_string(),
            1 => "Manual".to_string(),
            _ => format!("{}", val),
        },
        _ => format!("{}", val),
    }
}

fn format_rational(tag: u16, num: u32, den: u32) -> String {
    if den == 0 {
        return "0".to_string();
    }
    match tag {
        // ExposureTime: show as fraction if < 1s
        0x829A => {
            if num == 0 {
                "0s".to_string()
            } else if num >= den {
                let secs = num as f64 / den as f64;
                format!("{}s", format_decimal(secs))
            } else {
                // Simplify fraction
                let ratio = den / num;
                format!("1/{}s", ratio)
            }
        }
        // FNumber
        0x829D => {
            let f = num as f64 / den as f64;
            format!("f/{}", format_decimal(f))
        }
        // FocalLength
        0x920A => {
            let fl = num as f64 / den as f64;
            format!("{}mm", format_decimal(fl))
        }
        // XResolution, YResolution
        0x011A | 0x011B => {
            let dpi = num / den;
            format!("{} dpi", dpi)
        }
        _ => {
            if den == 1 {
                format!("{}", num)
            } else {
                format!("{}/{}", num, den)
            }
        }
    }
}

fn format_srational(tag: u16, num: i32, den: i32) -> String {
    if den == 0 {
        return "0".to_string();
    }
    match tag {
        // ExposureBias
        0x9204 => {
            let ev = num as f64 / den as f64;
            if ev >= 0.0 {
                format!("+{} EV", format_decimal(ev))
            } else {
                format!("{} EV", format_decimal(ev))
            }
        }
        _ => {
            if den == 1 {
                format!("{}", num)
            } else {
                format!("{}/{}", num, den)
            }
        }
    }
}

fn format_decimal(val: f64) -> String {
    if (val - val.round()).abs() < 0.01 {
        format!("{:.0}", val)
    } else if (val * 10.0 - (val * 10.0).round()).abs() < 0.01 {
        format!("{:.1}", val)
    } else {
        format!("{:.2}", val)
    }
}

fn parse_gps_tags(d: &[u8], ifd_offset: usize, le: bool, tags: &mut Vec<(String, String)>) {
    let read_u16 = |off: usize| -> Option<u16> {
        if off + 2 > d.len() {
            return None;
        }
        Some(if le {
            u16::from_le_bytes([d[off], d[off + 1]])
        } else {
            u16::from_be_bytes([d[off], d[off + 1]])
        })
    };

    let read_u32 = |off: usize| -> Option<u32> {
        if off + 4 > d.len() {
            return None;
        }
        Some(if le {
            u32::from_le_bytes([d[off], d[off + 1], d[off + 2], d[off + 3]])
        } else {
            u32::from_be_bytes([d[off], d[off + 1], d[off + 2], d[off + 3]])
        })
    };

    if ifd_offset + 2 > d.len() {
        return;
    }
    let entry_count = match read_u16(ifd_offset) {
        Some(v) => v as usize,
        None => return,
    };
    let entries_start = ifd_offset + 2;

    let mut lat_ref: Option<u8> = None;
    let mut lon_ref: Option<u8> = None;
    let mut lat_vals: Option<(f64, f64, f64)> = None;
    let mut lon_vals: Option<(f64, f64, f64)> = None;
    let mut alt: Option<f64> = None;

    for i in 0..entry_count {
        let entry_off = entries_start + i * 12;
        if entry_off + 12 > d.len() {
            break;
        }
        let tag = match read_u16(entry_off) {
            Some(v) => v,
            None => continue,
        };
        let dtype = match read_u16(entry_off + 2) {
            Some(v) => v as usize,
            None => continue,
        };
        let count = match read_u32(entry_off + 4) {
            Some(v) => v as usize,
            None => continue,
        };

        let type_size = if dtype < TYPE_SIZES.len() {
            TYPE_SIZES[dtype]
        } else {
            continue;
        };
        let total_bytes = type_size * count;
        let data_off = if total_bytes <= 4 {
            entry_off + 8
        } else {
            match read_u32(entry_off + 8) {
                Some(v) => v as usize,
                None => continue,
            }
        };

        match tag {
            // GPSLatitudeRef
            0x0001 => {
                if data_off < d.len() {
                    lat_ref = Some(d[data_off]);
                }
            }
            // GPSLatitude (3 RATIONALs: degrees, minutes, seconds)
            0x0002 => {
                if dtype == 5 && count == 3 {
                    lat_vals = read_gps_coord(d, data_off, le);
                }
            }
            // GPSLongitudeRef
            0x0003 => {
                if data_off < d.len() {
                    lon_ref = Some(d[data_off]);
                }
            }
            // GPSLongitude
            0x0004 => {
                if dtype == 5 && count == 3 {
                    lon_vals = read_gps_coord(d, data_off, le);
                }
            }
            // GPSAltitude
            0x0006 => {
                if dtype == 5 {
                    let num = read_u32(data_off).unwrap_or(0) as f64;
                    let den = read_u32(data_off + 4).unwrap_or(1).max(1) as f64;
                    alt = Some(num / den);
                }
            }
            _ => {}
        }
    }

    // Format GPS coordinates
    if let (Some((deg, min, sec)), Some(r)) = (lat_vals, lat_ref) {
        let decimal = deg + min / 60.0 + sec / 3600.0;
        let sign = if r == b'S' { -1.0 } else { 1.0 };
        let lat = decimal * sign;

        if let (Some((ldeg, lmin, lsec)), Some(lr)) = (lon_vals, lon_ref) {
            let ldecimal = ldeg + lmin / 60.0 + lsec / 3600.0;
            let lsign = if lr == b'W' { -1.0 } else { 1.0 };
            let lon = ldecimal * lsign;
            tags.push(("GPS".to_string(), format!("{:.6}, {:.6}", lat, lon)));
        }
    }

    if let Some(altitude) = alt {
        tags.push(("Altitude".to_string(), format!("{:.1}m", altitude)));
    }
}

fn read_gps_coord(d: &[u8], off: usize, le: bool) -> Option<(f64, f64, f64)> {
    let read_u32 = |o: usize| -> Option<u32> {
        if o + 4 > d.len() {
            return None;
        }
        Some(if le {
            u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
        } else {
            u32::from_be_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
        })
    };

    let deg_n = read_u32(off)? as f64;
    let deg_d = read_u32(off + 4)?.max(1) as f64;
    let min_n = read_u32(off + 8)? as f64;
    let min_d = read_u32(off + 12)?.max(1) as f64;
    let sec_n = read_u32(off + 16)? as f64;
    let sec_d = read_u32(off + 20)?.max(1) as f64;

    Some((deg_n / deg_d, min_n / min_d, sec_n / sec_d))
}

fn flip_v(img: RgbaImage) -> RgbaImage {
    let (w, h) = (img.width, img.height);
    let mut out = RgbaImage::new(w, h);
    for y in 0..h {
        let src_row = (y * w * 4) as usize;
        let dst_row = ((h - 1 - y) * w * 4) as usize;
        let row_bytes = (w * 4) as usize;
        out.data[dst_row..dst_row + row_bytes]
            .copy_from_slice(&img.data[src_row..src_row + row_bytes]);
    }
    out
}
