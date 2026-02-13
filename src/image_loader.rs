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

fn rotate_90(img: RgbaImage) -> RgbaImage {
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

fn rotate_270(img: RgbaImage) -> RgbaImage {
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
