use std::ffi::CString;
use std::fs;
use std::os::raw::{c_char, c_int, c_uchar, c_uint, c_void};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Supported image extensions (lowercase).
const SUPPORTED_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "webp", "bmp", "tiff", "tif", "svg", "avif", "heic", "heif", "jxl",
];

/// Maximum pixel count to prevent excessive memory allocation (256 megapixels).
const MAX_PIXEL_COUNT: u64 = 256 * 1024 * 1024;

/// Maximum file size to read into memory (512 MiB).
const MAX_FILE_SIZE: u64 = 512 * 1024 * 1024;

/// Maximum directory recursion depth to prevent stack overflow from symlink loops
/// or deeply nested directories.
const MAX_DIR_DEPTH: u32 = 64;

/// Simple RGBA image buffer.
#[derive(Clone, Debug)]
pub struct RgbaImage {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

impl RgbaImage {
    pub fn new(width: u32, height: u32) -> Self {
        let size = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(4))
            .expect("Image dimensions too large");
        Self {
            data: vec![0u8; size],
            width,
            height,
        }
    }

    pub fn from_raw(width: u32, height: u32, data: Vec<u8>) -> Option<Self> {
        let expected = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(4))?;
        if data.len() == expected {
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
#[derive(Debug)]
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

/// Read a file into memory with a size limit to prevent excessive allocation.
fn read_file_limited(path: &Path) -> Result<Vec<u8>, String> {
    let meta =
        fs::metadata(path).map_err(|e| format!("Failed to stat {}: {}", path.display(), e))?;
    if meta.len() > MAX_FILE_SIZE {
        return Err(format!(
            "File too large ({} bytes, max {}): {}",
            meta.len(),
            MAX_FILE_SIZE,
            path.display()
        ));
    }
    fs::read(path).map_err(|e| format!("Failed to read {}: {}", path.display(), e))
}

/// Validate image dimensions against maximum pixel count.
fn validate_dimensions(width: u32, height: u32, format: &str) -> Result<(), String> {
    let pixels = width as u64 * height as u64;
    if pixels > MAX_PIXEL_COUNT {
        return Err(format!(
            "{} image too large: {}x{} ({} pixels, max {})",
            format, width, height, pixels, MAX_PIXEL_COUNT
        ));
    }
    if width == 0 || height == 0 {
        return Err(format!(
            "{} image has zero dimension: {}x{}",
            format, width, height
        ));
    }
    Ok(())
}

/// Collect image paths from CLI arguments.
pub fn collect_paths(args: &[String]) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for arg in args {
        let p = PathBuf::from(arg);
        if p.is_dir() {
            scan_directory(&p, &mut paths, 0);
        } else if is_supported_image(&p) {
            paths.push(p);
        }
    }
    paths.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    paths
}

fn scan_directory(dir: &Path, out: &mut Vec<PathBuf>, depth: u32) {
    if depth >= MAX_DIR_DEPTH {
        return;
    }
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        // Skip symlinks to prevent symlink loops and traversal outside target
        if path.is_symlink() {
            continue;
        }
        if path.is_dir() {
            scan_directory(&path, out, depth + 1);
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
        "bmp" => load_bmp(path),
        "tiff" | "tif" => load_tiff(path),
        "svg" => load_svg(path),
        "avif" => load_avif(path),
        "heic" | "heif" => load_heic(path),
        "jxl" => load_jxl(path),
        _ => Err(format!("Unsupported format: {}", ext)),
    }
}

// ============================================================
// JPEG via system libturbojpeg
// ============================================================

fn load_jpeg(path: &Path) -> Result<LoadedImage, String> {
    let data = read_file_limited(path)?;

    let image = turbojpeg::decompress(&data, turbojpeg::PixelFormat::RGBA)
        .map_err(|e| format!("Failed to decode JPEG {}: {}", path.display(), e))?;

    validate_dimensions(image.width as u32, image.height as u32, "JPEG")?;

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
    let data = read_file_limited(path)?;

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

        // Set up error handling via setjmp.
        // SAFETY: longjmp will jump back here on libpng errors. We must ensure
        // no Rust objects with Drop impls are live across the longjmp boundary.
        // The Vec allocations (rgba_data, row_ptrs) below will be leaked if
        // longjmp fires during png_read_image, but this is preferable to UB.
        // We use raw pointers to track them for cleanup on the error path.
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

        // Validate dimensions before allocating buffers
        if width == 0 || height == 0 || (width as u64) * (height as u64) > MAX_PIXEL_COUNT {
            let mut pp = png_ptr;
            let mut ip = info_ptr;
            libpng::png_destroy_read_struct(&mut pp, &mut ip, std::ptr::null_mut());
            return Err(format!(
                "PNG dimensions too large or zero: {}x{} in {}",
                width,
                height,
                path.display()
            ));
        }

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

        let mut img = RgbaImage::from_raw(width, height, rgba_data)
            .ok_or_else(|| "PNG pixel buffer size mismatch".to_string())?;

        // Apply EXIF orientation from PNG eXIf chunk
        if let Some(orientation) = read_exif_orientation_png(&data) {
            img = apply_orientation(img, orientation);
        }

        Ok(LoadedImage::Static(img))
    }
}

// ============================================================
// WebP via system libwebp
// ============================================================

fn load_webp(path: &Path) -> Result<LoadedImage, String> {
    let data = read_file_limited(path)?;

    // Check if the WebP is animated using WebPGetFeatures
    let mut features: libwebp_sys::WebPBitstreamFeatures = unsafe { std::mem::zeroed() };
    let status = unsafe { libwebp_sys::WebPGetFeatures(data.as_ptr(), data.len(), &mut features) };
    if status != libwebp_sys::VP8_STATUS_OK {
        return Err(format!("Failed to read WebP features {}", path.display()));
    }

    if features.has_animation != 0 {
        return load_webp_animated(&data, path);
    }

    // Static WebP: decode with WebPDecodeRGBA
    let mut width: std::ffi::c_int = 0;
    let mut height: std::ffi::c_int = 0;

    let ptr =
        unsafe { libwebp_sys::WebPDecodeRGBA(data.as_ptr(), data.len(), &mut width, &mut height) };

    if ptr.is_null() {
        return Err(format!("Failed to decode WebP {}", path.display()));
    }

    if width <= 0 || height <= 0 {
        unsafe {
            libwebp_sys::WebPFree(ptr as *mut std::ffi::c_void);
        }
        return Err(format!(
            "Invalid WebP dimensions: {}x{} in {}",
            width,
            height,
            path.display()
        ));
    }

    let w = width as u32;
    let h = height as u32;
    validate_dimensions(w, h, "WebP").map_err(|e| {
        unsafe {
            libwebp_sys::WebPFree(ptr as *mut std::ffi::c_void);
        }
        e
    })?;

    let len = w as u64 * h as u64 * 4;
    let rgba_data = unsafe { std::slice::from_raw_parts(ptr, len as usize).to_vec() };
    unsafe {
        libwebp_sys::WebPFree(ptr as *mut std::ffi::c_void);
    }

    let mut img = RgbaImage::from_raw(w, h, rgba_data)
        .ok_or_else(|| "WebP pixel buffer size mismatch".to_string())?;

    // Apply EXIF orientation from WebP EXIF chunk
    if let Some(orientation) = read_exif_orientation_webp(&data) {
        img = apply_orientation(img, orientation);
    }

    Ok(LoadedImage::Static(img))
}

/// Decode an animated WebP using the WebPAnimDecoder API.
fn load_webp_animated(data: &[u8], path: &Path) -> Result<LoadedImage, String> {
    unsafe {
        // Initialize decoder options
        let mut options: libwebp_sys::WebPAnimDecoderOptions = std::mem::zeroed();
        if libwebp_sys::WebPAnimDecoderOptionsInit(&mut options) == 0 {
            return Err("WebPAnimDecoderOptionsInit failed".to_string());
        }
        options.color_mode = libwebp_sys::MODE_RGBA;
        options.use_threads = 0;

        // Create WebPData
        let webp_data = libwebp_sys::WebPData {
            bytes: data.as_ptr(),
            size: data.len(),
        };

        // Create the animation decoder
        let dec = libwebp_sys::WebPAnimDecoderNew(&webp_data, &options);
        if dec.is_null() {
            return Err(format!(
                "Failed to create WebP animation decoder for {}",
                path.display()
            ));
        }

        // Get animation info (canvas size, frame count)
        let mut info: libwebp_sys::WebPAnimInfo = std::mem::zeroed();
        if libwebp_sys::WebPAnimDecoderGetInfo(dec, &mut info) == 0 {
            libwebp_sys::WebPAnimDecoderDelete(dec);
            return Err(format!(
                "Failed to get WebP animation info for {}",
                path.display()
            ));
        }

        let canvas_w = info.canvas_width;
        let canvas_h = info.canvas_height;
        validate_dimensions(canvas_w, canvas_h, "WebP animated").map_err(|e| {
            libwebp_sys::WebPAnimDecoderDelete(dec);
            e
        })?;

        let frame_size = (canvas_w as u64 * canvas_h as u64 * 4) as usize;
        let mut frames: Vec<(RgbaImage, Duration)> = Vec::new();
        let mut prev_timestamp: i32 = 0;

        // Iterate through all frames
        while libwebp_sys::WebPAnimDecoderHasMoreFrames(dec) != 0 {
            let mut buf: *mut u8 = std::ptr::null_mut();
            let mut timestamp: std::ffi::c_int = 0;

            if libwebp_sys::WebPAnimDecoderGetNext(dec, &mut buf, &mut timestamp) == 0 {
                break; // Decode error on this frame, stop
            }

            // Frame duration = delta between consecutive cumulative timestamps
            let delay_ms = ((timestamp - prev_timestamp) as u64).max(10);
            prev_timestamp = timestamp;

            // Copy the RGBA buffer (it's owned by the decoder, valid until next GetNext or Delete)
            let rgba_data = std::slice::from_raw_parts(buf, frame_size).to_vec();
            if let Some(img) = RgbaImage::from_raw(canvas_w, canvas_h, rgba_data) {
                frames.push((img, Duration::from_millis(delay_ms)));
            }
        }

        libwebp_sys::WebPAnimDecoderDelete(dec);

        if frames.is_empty() {
            return Err(format!(
                "No frames decoded from animated WebP: {}",
                path.display()
            ));
        }

        if frames.len() == 1 {
            let (img, _) = frames.into_iter().next().unwrap();
            return Ok(LoadedImage::Static(img));
        }

        Ok(LoadedImage::Animated { frames })
    }
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

        // Validate canvas dimensions to prevent overflow in allocation
        if (canvas_w as u64) * (canvas_h as u64) > MAX_PIXEL_COUNT {
            libgif::DGifCloseFile(gif, std::ptr::null_mut());
            return Err(format!(
                "GIF canvas too large: {}x{} in {}",
                canvas_w,
                canvas_h,
                path.display()
            ));
        }

        let canvas_size = (canvas_w as usize)
            .checked_mul(canvas_h as usize)
            .and_then(|n| n.checked_mul(4))
            .ok_or_else(|| {
                libgif::DGifCloseFile(gif, std::ptr::null_mut());
                format!("GIF canvas overflow: {}x{}", canvas_w, canvas_h)
            })?;

        let mut frames: Vec<(RgbaImage, Duration)> = Vec::with_capacity(image_count);
        let mut canvas = vec![0u8; canvas_size];

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
// BMP (manual parsing - simple format)
// ============================================================

fn load_bmp(path: &Path) -> Result<LoadedImage, String> {
    let data = read_file_limited(path)?;
    decode_bmp(&data, &path.display().to_string())
}

/// Decode a BMP image from raw bytes. Separated from load_bmp for testability.
fn decode_bmp(data: &[u8], path_display: &str) -> Result<LoadedImage, String> {
    if data.len() < 54 {
        return Err("File too small to be BMP".to_string());
    }

    if &data[0..2] != b"BM" {
        return Err("Not a BMP file".to_string());
    }

    let data_offset = u32::from_le_bytes([data[10], data[11], data[12], data[13]]) as usize;
    let dib_header_size = u32::from_le_bytes([data[14], data[15], data[16], data[17]]) as usize;
    let width = i32::from_le_bytes([data[18], data[19], data[20], data[21]]);
    let height = i32::from_le_bytes([data[22], data[23], data[24], data[25]]);
    let bits_per_pixel = u16::from_le_bytes([data[28], data[29]]);
    let compression = if data.len() >= 34 {
        u32::from_le_bytes([data[30], data[31], data[32], data[33]])
    } else {
        0
    };

    if width <= 0 || height == 0 {
        return Err("Invalid BMP dimensions".to_string());
    }

    let (w, h) = (width as u32, height.unsigned_abs() as u32);
    validate_dimensions(w, h, "BMP")?;

    // Use u64 arithmetic to prevent overflow in row_size and expected_size calculations
    let row_size_u64 = ((w as u64 * bits_per_pixel as u64 + 31) / 32) * 4;
    let expected_size_u64 = data_offset as u64 + row_size_u64 * h as u64;

    if (data.len() as u64) < expected_size_u64 {
        return Err("BMP file truncated".to_string());
    }

    let row_size = row_size_u64 as usize;
    let pixel_count = (w as usize)
        .checked_mul(h as usize)
        .and_then(|n| n.checked_mul(4))
        .ok_or_else(|| "BMP dimensions overflow".to_string())?;
    let mut rgba_data = vec![0u8; pixel_count];

    match bits_per_pixel {
        24 => {
            for y in 0..h {
                for x in 0..w {
                    let src_row = if height > 0 {
                        (h - 1 - y) as usize
                    } else {
                        y as usize
                    };
                    let src_idx = data_offset + (src_row * row_size) + (x as usize * 3);
                    let dst = ((y * w + x) * 4) as usize;
                    rgba_data[dst] = data[src_idx + 2];
                    rgba_data[dst + 1] = data[src_idx + 1];
                    rgba_data[dst + 2] = data[src_idx];
                    rgba_data[dst + 3] = 255;
                }
            }
        }
        32 => {
            for y in 0..h {
                for x in 0..w {
                    let src_row = if height > 0 {
                        (h - 1 - y) as usize
                    } else {
                        y as usize
                    };
                    let src_idx = data_offset + (src_row * row_size) + (x as usize * 4);
                    let dst = ((y * w + x) * 4) as usize;
                    rgba_data[dst] = data[src_idx + 2];
                    rgba_data[dst + 1] = data[src_idx + 1];
                    rgba_data[dst + 2] = data[src_idx];
                    rgba_data[dst + 3] = data[src_idx + 3];
                }
            }
        }
        1 | 4 | 8 => {
            // Reject RLE compression
            if compression == 1 {
                return Err(format!(
                    "Unsupported BMP compression: BI_RLE8 in {}",
                    path_display
                ));
            }
            if compression == 2 {
                return Err(format!(
                    "Unsupported BMP compression: BI_RLE4 in {}",
                    path_display
                ));
            }

            // Parse color table
            let max_colors: u32 = 1 << bits_per_pixel;
            let clr_used = if data.len() >= 50 {
                let v = u32::from_le_bytes([data[46], data[47], data[48], data[49]]);
                if v == 0 {
                    max_colors
                } else {
                    v
                }
            } else {
                max_colors
            };
            if clr_used > max_colors {
                return Err(format!(
                    "Invalid BMP color table: biClrUsed {} exceeds max {} for {}-bit",
                    clr_used, max_colors, bits_per_pixel
                ));
            }

            let color_table_offset = 14 + dib_header_size;
            let color_table_end = color_table_offset + clr_used as usize * 4;
            if color_table_end > data.len() {
                return Err("BMP color table truncated".to_string());
            }

            // Read BGRA color table entries
            let mut palette = Vec::with_capacity(clr_used as usize);
            for i in 0..clr_used as usize {
                let off = color_table_offset + i * 4;
                palette.push([data[off + 2], data[off + 1], data[off], 255]); // BGR_ -> RGBA
            }

            // Decode indexed pixels
            for y in 0..h {
                let src_row = if height > 0 {
                    (h - 1 - y) as usize
                } else {
                    y as usize
                };
                let row_start = data_offset + src_row * row_size;

                match bits_per_pixel {
                    8 => {
                        for x in 0..w {
                            let idx = data[row_start + x as usize] as usize;
                            let dst = ((y * w + x) * 4) as usize;
                            if idx < palette.len() {
                                rgba_data[dst..dst + 4].copy_from_slice(&palette[idx]);
                            }
                        }
                    }
                    4 => {
                        for x in 0..w {
                            let byte = data[row_start + (x as usize / 2)];
                            let idx = if x % 2 == 0 {
                                (byte >> 4) as usize // high nibble = left pixel
                            } else {
                                (byte & 0x0F) as usize // low nibble = right pixel
                            };
                            let dst = ((y * w + x) * 4) as usize;
                            if idx < palette.len() {
                                rgba_data[dst..dst + 4].copy_from_slice(&palette[idx]);
                            }
                        }
                    }
                    1 => {
                        for x in 0..w {
                            let byte = data[row_start + (x as usize / 8)];
                            let bit = 7 - (x % 8); // MSB = leftmost pixel
                            let idx = ((byte >> bit) & 1) as usize;
                            let dst = ((y * w + x) * 4) as usize;
                            if idx < palette.len() {
                                rgba_data[dst..dst + 4].copy_from_slice(&palette[idx]);
                            }
                        }
                    }
                    _ => unreachable!(),
                }
            }
        }
        _ => {
            return Err(format!("Unknown BMP bit depth: {}", bits_per_pixel));
        }
    }

    let img = RgbaImage::from_raw(w, h, rgba_data)
        .ok_or_else(|| "BMP pixel buffer size mismatch".to_string())?;

    Ok(LoadedImage::Static(img))
}

// ============================================================
// TIFF via system libtiff
// ============================================================

#[allow(non_camel_case_types)]
mod libtiff {
    use std::os::raw::{c_char, c_int, c_uint, c_void};

    pub type TIFF = c_void;

    pub const TIFFTAG_IMAGEWIDTH: c_uint = 256;
    pub const TIFFTAG_IMAGELENGTH: c_uint = 257;
    pub const ORIENTATION_TOPLEFT: c_int = 1;

    #[link(name = "tiff")]
    extern "C" {
        pub fn TIFFOpen(filename: *const c_char, mode: *const c_char) -> *mut TIFF;
        pub fn TIFFClose(tif: *mut TIFF);
        pub fn TIFFGetField(tif: *mut TIFF, tag: c_uint, ...) -> c_int;
        pub fn TIFFReadRGBAImageOriented(
            tif: *mut TIFF,
            width: c_uint,
            height: c_uint,
            raster: *mut u32,
            orientation: c_int,
            stop: c_int,
        ) -> c_int;
    }
}

fn load_tiff(path: &Path) -> Result<LoadedImage, String> {
    let c_path = CString::new(path.to_str().ok_or_else(|| "Invalid path".to_string())?)
        .map_err(|_| "Path contains null byte".to_string())?;
    let mode = b"r\0".as_ptr() as *const c_char;

    unsafe {
        let tif = libtiff::TIFFOpen(c_path.as_ptr(), mode);
        if tif.is_null() {
            return Err(format!("Failed to open TIFF {}", path.display()));
        }

        let mut w: c_uint = 0;
        let mut h: c_uint = 0;
        if libtiff::TIFFGetField(tif, libtiff::TIFFTAG_IMAGEWIDTH, &mut w as *mut c_uint) == 0
            || libtiff::TIFFGetField(tif, libtiff::TIFFTAG_IMAGELENGTH, &mut h as *mut c_uint) == 0
        {
            libtiff::TIFFClose(tif);
            return Err(format!("Failed to get TIFF dimensions {}", path.display()));
        }

        // Validate dimensions before allocation
        if w == 0 || h == 0 || (w as u64) * (h as u64) > MAX_PIXEL_COUNT {
            libtiff::TIFFClose(tif);
            return Err(format!(
                "TIFF dimensions invalid or too large: {}x{} in {}",
                w,
                h,
                path.display()
            ));
        }

        let npixels = (w as usize).checked_mul(h as usize).ok_or_else(|| {
            libtiff::TIFFClose(tif);
            format!("TIFF dimensions overflow: {}x{}", w, h)
        })?;
        let mut raster: Vec<u32> = vec![0u32; npixels];

        let ok = libtiff::TIFFReadRGBAImageOriented(
            tif,
            w,
            h,
            raster.as_mut_ptr(),
            libtiff::ORIENTATION_TOPLEFT,
            0,
        );
        libtiff::TIFFClose(tif);

        if ok == 0 {
            return Err(format!("Failed to decode TIFF {}", path.display()));
        }

        // libtiff returns ABGR packed u32 (R in lowest byte). Convert to RGBA bytes.
        let mut rgba = Vec::with_capacity(npixels * 4);
        for &pixel in &raster {
            rgba.push((pixel & 0xFF) as u8);
            rgba.push(((pixel >> 8) & 0xFF) as u8);
            rgba.push(((pixel >> 16) & 0xFF) as u8);
            rgba.push(((pixel >> 24) & 0xFF) as u8);
        }

        let img = RgbaImage::from_raw(w as u32, h as u32, rgba)
            .ok_or_else(|| "TIFF pixel buffer size mismatch".to_string())?;

        Ok(LoadedImage::Static(img))
    }
}

// ============================================================
// SVG via system librsvg + cairo
// ============================================================

#[allow(non_camel_case_types)]
mod librsvg {
    use std::os::raw::{c_char, c_int, c_uchar, c_void};

    pub type RsvgHandle = c_void;
    pub type cairo_surface_t = c_void;
    pub type cairo_t = c_void;
    pub type GError = c_void;

    pub const CAIRO_FORMAT_ARGB32: c_int = 0;

    #[repr(C)]
    pub struct RsvgRectangle {
        pub x: f64,
        pub y: f64,
        pub width: f64,
        pub height: f64,
    }

    #[link(name = "rsvg-2")]
    extern "C" {
        pub fn rsvg_handle_new_from_file(
            file_name: *const c_char,
            error: *mut *mut GError,
        ) -> *mut RsvgHandle;
        pub fn rsvg_handle_get_intrinsic_size_in_pixels(
            handle: *mut RsvgHandle,
            out_width: *mut f64,
            out_height: *mut f64,
        ) -> c_int;
        pub fn rsvg_handle_render_document(
            handle: *mut RsvgHandle,
            cr: *mut cairo_t,
            viewport: *const RsvgRectangle,
            error: *mut *mut GError,
        ) -> c_int;
        pub fn rsvg_handle_set_dpi(handle: *mut RsvgHandle, dpi: f64);
    }

    #[link(name = "gobject-2.0")]
    extern "C" {
        pub fn g_object_unref(object: *mut c_void);
    }

    #[link(name = "glib-2.0")]
    extern "C" {
        pub fn g_error_free(error: *mut GError);
    }

    #[link(name = "cairo")]
    extern "C" {
        pub fn cairo_image_surface_create(
            format: c_int,
            width: c_int,
            height: c_int,
        ) -> *mut cairo_surface_t;
        pub fn cairo_create(target: *mut cairo_surface_t) -> *mut cairo_t;
        pub fn cairo_destroy(cr: *mut cairo_t);
        pub fn cairo_surface_destroy(surface: *mut cairo_surface_t);
        pub fn cairo_surface_flush(surface: *mut cairo_surface_t);
        pub fn cairo_image_surface_get_data(surface: *mut cairo_surface_t) -> *mut c_uchar;
        pub fn cairo_image_surface_get_stride(surface: *mut cairo_surface_t) -> c_int;
    }
}

fn load_svg(path: &Path) -> Result<LoadedImage, String> {
    let c_path = CString::new(path.to_str().ok_or_else(|| "Invalid path".to_string())?)
        .map_err(|_| "Path contains null byte".to_string())?;

    unsafe {
        // Load SVG
        let mut error: *mut librsvg::GError = std::ptr::null_mut();
        let handle = librsvg::rsvg_handle_new_from_file(c_path.as_ptr(), &mut error);
        if handle.is_null() {
            if !error.is_null() {
                librsvg::g_error_free(error);
            }
            return Err(format!("Failed to load SVG {}", path.display()));
        }

        librsvg::rsvg_handle_set_dpi(handle, 96.0);

        // Get intrinsic dimensions; fall back to 1024x1024
        let mut w: f64 = 0.0;
        let mut h: f64 = 0.0;
        if librsvg::rsvg_handle_get_intrinsic_size_in_pixels(handle, &mut w, &mut h) == 0
            || w <= 0.0
            || h <= 0.0
        {
            w = 1024.0;
            h = 1024.0;
        }

        // Clamp SVG dimensions to prevent excessive memory allocation
        let max_svg_dim = 16384.0; // 16K pixels per side
        if w > max_svg_dim || h > max_svg_dim {
            let scale = (max_svg_dim / w).min(max_svg_dim / h);
            w *= scale;
            h *= scale;
        }

        let pw = w.ceil() as c_int;
        let ph = h.ceil() as c_int;

        // Validate pixel count
        if (pw as u64) * (ph as u64) > MAX_PIXEL_COUNT {
            librsvg::g_object_unref(handle);
            return Err(format!(
                "SVG dimensions too large: {}x{} in {}",
                pw,
                ph,
                path.display()
            ));
        }

        // Create cairo image surface
        let surface = librsvg::cairo_image_surface_create(librsvg::CAIRO_FORMAT_ARGB32, pw, ph);
        if surface.is_null() {
            librsvg::g_object_unref(handle);
            return Err(format!(
                "Failed to create cairo surface for {}",
                path.display()
            ));
        }

        let cr = librsvg::cairo_create(surface);
        if cr.is_null() {
            librsvg::cairo_surface_destroy(surface);
            librsvg::g_object_unref(handle);
            return Err(format!(
                "Failed to create cairo context for {}",
                path.display()
            ));
        }

        // Render SVG to surface
        let viewport = librsvg::RsvgRectangle {
            x: 0.0,
            y: 0.0,
            width: w,
            height: h,
        };
        let mut render_error: *mut librsvg::GError = std::ptr::null_mut();
        let ok = librsvg::rsvg_handle_render_document(handle, cr, &viewport, &mut render_error);

        if ok == 0 {
            if !render_error.is_null() {
                librsvg::g_error_free(render_error);
            }
            librsvg::cairo_destroy(cr);
            librsvg::cairo_surface_destroy(surface);
            librsvg::g_object_unref(handle);
            return Err(format!("Failed to render SVG {}", path.display()));
        }

        librsvg::cairo_surface_flush(surface);

        // Read pixel data from surface
        let data_ptr = librsvg::cairo_image_surface_get_data(surface);
        let stride = librsvg::cairo_image_surface_get_stride(surface) as usize;
        let width = pw as u32;
        let height = ph as u32;

        // Convert from cairo premultiplied ARGB32 (native endian) to straight RGBA.
        // On little-endian x86_64, bytes in memory are: B, G, R, A.
        let mut rgba = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height {
            let row = data_ptr.add(y as usize * stride);
            for x in 0..width {
                let px = row.add(x as usize * 4);
                let b = *px;
                let g = *px.add(1);
                let r = *px.add(2);
                let a = *px.add(3);

                // Un-premultiply alpha
                if a == 0 {
                    rgba.extend_from_slice(&[0, 0, 0, 0]);
                } else if a == 255 {
                    rgba.extend_from_slice(&[r, g, b, a]);
                } else {
                    let aa = a as u16;
                    rgba.push(((r as u16 * 255 + aa / 2) / aa).min(255) as u8);
                    rgba.push(((g as u16 * 255 + aa / 2) / aa).min(255) as u8);
                    rgba.push(((b as u16 * 255 + aa / 2) / aa).min(255) as u8);
                    rgba.push(a);
                }
            }
        }

        librsvg::cairo_destroy(cr);
        librsvg::cairo_surface_destroy(surface);
        librsvg::g_object_unref(handle);

        let img = RgbaImage::from_raw(width, height, rgba)
            .ok_or_else(|| "SVG pixel buffer size mismatch".to_string())?;

        Ok(LoadedImage::Static(img))
    }
}

// ============================================================
// AVIF via system libavif
// ============================================================

#[allow(non_camel_case_types)]
mod libavif {
    use std::os::raw::{c_int, c_uint, c_void};

    pub const AVIF_RESULT_OK: c_int = 0;
    pub const AVIF_RGB_FORMAT_RGBA: c_int = 0;

    #[repr(C)]
    pub struct avifImageTiming {
        pub timescale: u64,
        pub pts: f64,
        pub pts_in_timescales: u64,
        pub duration: f64,
        pub duration_in_timescales: u64,
    }

    // Opaque types
    pub type avifImage = c_void;
    pub type avifDecoder = c_void;

    // We access avifDecoder fields via offsets, but since the struct layout
    // may change, we use getter-style FFI. Actually, avifDecoder fields are
    // public in the C header, but we'll access them through helper functions
    // to keep things safe. Since libavif doesn't have getter functions for
    // all fields, we define a partial struct for the decoder.

    #[repr(C)]
    pub struct avifRGBImage {
        pub width: c_uint,
        pub height: c_uint,
        pub depth: c_uint,
        pub format: c_int,
        pub chroma_upsampling: c_int,
        pub chroma_downsampling: c_int,
        pub avoid_libyuv: c_int,
        pub ignore_alpha: c_int,
        pub alpha_premultiplied: c_int,
        pub is_float: c_int,
        pub max_threads: c_int,
        pub pixels: *mut u8,
        pub row_bytes: c_uint,
    }

    #[link(name = "avif")]
    extern "C" {
        pub fn avifDecoderCreate() -> *mut avifDecoder;
        pub fn avifDecoderDestroy(decoder: *mut avifDecoder);
        pub fn avifDecoderSetIOMemory(
            decoder: *mut avifDecoder,
            data: *const u8,
            size: usize,
        ) -> c_int;
        pub fn avifDecoderParse(decoder: *mut avifDecoder) -> c_int;
        pub fn avifDecoderNextImage(decoder: *mut avifDecoder) -> c_int;
        pub fn avifDecoderNthImageTiming(
            decoder: *const avifDecoder,
            frame_index: c_uint,
            out_timing: *mut avifImageTiming,
        ) -> c_int;
        pub fn avifRGBImageSetDefaults(rgb: *mut avifRGBImage, image: *const avifImage);
        pub fn avifRGBImageAllocatePixels(rgb: *mut avifRGBImage) -> c_int;
        pub fn avifRGBImageFreePixels(rgb: *mut avifRGBImage);
        pub fn avifImageYUVToRGB(image: *const avifImage, rgb: *mut avifRGBImage) -> c_int;
    }
}

/// Read avifDecoder->image (offset depends on the struct layout).
/// avifDecoder struct: first field is codecChoice (enum = c_int), then maxThreads (c_int),
/// requestedSource (enum = c_int), allowProgressive (c_int), allowIncremental (c_int),
/// ignoreExif (c_int), ignoreXMP (c_int), imageSizeLimit (u32), imageDimensionLimit (u32),
/// imageCountLimit (u32), strictFlags (u32), then image (*mut avifImage).
/// That's 10 * 4 + 4 = 44 bytes on most platforms, but we need to account for pointer alignment.
/// Actually we should read from the struct pointer directly. Let's use a more reliable approach.
///
/// Helper to read decoder fields by casting to known offsets.
/// We define a repr(C) partial mirror of the decoder struct for safe field access.
#[repr(C)]
struct AvifDecoderPartial {
    codec_choice: c_int,
    max_threads: c_int,
    requested_source: c_int,
    allow_progressive: c_int,
    allow_incremental: c_int,
    ignore_exif: c_int,
    ignore_xmp: c_int,
    image_size_limit: u32,
    image_dimension_limit: u32,
    image_count_limit: u32,
    strict_flags: u32,
    // Pointer-aligned field follows
    image: *mut c_void,
    image_index: c_int,
    image_count: c_int,
}

fn load_avif(path: &Path) -> Result<LoadedImage, String> {
    let data = read_file_limited(path)?;

    unsafe {
        let decoder = libavif::avifDecoderCreate();
        if decoder.is_null() {
            return Err("Failed to create AVIF decoder".to_string());
        }

        let result = libavif::avifDecoderSetIOMemory(decoder, data.as_ptr(), data.len());
        if result != libavif::AVIF_RESULT_OK {
            libavif::avifDecoderDestroy(decoder);
            return Err(format!("Failed to set AVIF IO for {}", path.display()));
        }

        let result = libavif::avifDecoderParse(decoder);
        if result != libavif::AVIF_RESULT_OK {
            libavif::avifDecoderDestroy(decoder);
            return Err(format!("Failed to parse AVIF {}", path.display()));
        }

        let dec = &*(decoder as *const AvifDecoderPartial);
        let image_count = dec.image_count;
        let is_animated = image_count > 1;

        if is_animated {
            let mut frames = Vec::new();
            for i in 0..image_count {
                let result = libavif::avifDecoderNextImage(decoder);
                if result != libavif::AVIF_RESULT_OK {
                    libavif::avifDecoderDestroy(decoder);
                    return Err(format!(
                        "Failed to decode AVIF frame {} of {}",
                        i,
                        path.display()
                    ));
                }

                let dec = &*(decoder as *const AvifDecoderPartial);
                let image = dec.image;

                let mut rgb: libavif::avifRGBImage = std::mem::zeroed();
                libavif::avifRGBImageSetDefaults(&mut rgb, image);
                rgb.format = libavif::AVIF_RGB_FORMAT_RGBA;
                rgb.depth = 8;

                let res = libavif::avifRGBImageAllocatePixels(&mut rgb);
                if res != libavif::AVIF_RESULT_OK {
                    libavif::avifDecoderDestroy(decoder);
                    return Err(format!(
                        "Failed to allocate AVIF RGB pixels for {}",
                        path.display()
                    ));
                }

                let res = libavif::avifImageYUVToRGB(image, &mut rgb);
                if res != libavif::AVIF_RESULT_OK {
                    libavif::avifRGBImageFreePixels(&mut rgb);
                    libavif::avifDecoderDestroy(decoder);
                    return Err(format!(
                        "Failed to convert AVIF to RGB for {}",
                        path.display()
                    ));
                }

                let w = rgb.width;
                let h = rgb.height;
                validate_dimensions(w, h, "AVIF").map_err(|e| {
                    libavif::avifRGBImageFreePixels(&mut rgb);
                    libavif::avifDecoderDestroy(decoder);
                    e
                })?;

                let pixel_count = (w as usize) * (h as usize) * 4;
                let mut pixels = vec![0u8; pixel_count];
                let src_ptr = rgb.pixels;
                let row_bytes = rgb.row_bytes as usize;
                for y in 0..h as usize {
                    let src_offset = y * row_bytes;
                    let dst_offset = y * (w as usize) * 4;
                    std::ptr::copy_nonoverlapping(
                        src_ptr.add(src_offset),
                        pixels.as_mut_ptr().add(dst_offset),
                        (w as usize) * 4,
                    );
                }
                libavif::avifRGBImageFreePixels(&mut rgb);

                let img = RgbaImage::from_raw(w, h, pixels)
                    .ok_or_else(|| "AVIF pixel buffer size mismatch".to_string())?;

                // Get frame timing
                let mut timing: libavif::avifImageTiming = std::mem::zeroed();
                libavif::avifDecoderNthImageTiming(decoder, i as c_uint, &mut timing);
                let duration_ms = (timing.duration * 1000.0) as u64;
                let duration = Duration::from_millis(duration_ms.max(10));

                frames.push((img, duration));
            }

            libavif::avifDecoderDestroy(decoder);

            if frames.is_empty() {
                return Err(format!("AVIF contains no frames: {}", path.display()));
            }

            Ok(LoadedImage::Animated { frames })
        } else {
            // Static AVIF
            let result = libavif::avifDecoderNextImage(decoder);
            if result != libavif::AVIF_RESULT_OK {
                libavif::avifDecoderDestroy(decoder);
                return Err(format!("Failed to decode AVIF {}", path.display()));
            }

            let dec = &*(decoder as *const AvifDecoderPartial);
            let image = dec.image;

            let mut rgb: libavif::avifRGBImage = std::mem::zeroed();
            libavif::avifRGBImageSetDefaults(&mut rgb, image);
            rgb.format = libavif::AVIF_RGB_FORMAT_RGBA;
            rgb.depth = 8;

            let res = libavif::avifRGBImageAllocatePixels(&mut rgb);
            if res != libavif::AVIF_RESULT_OK {
                libavif::avifDecoderDestroy(decoder);
                return Err(format!(
                    "Failed to allocate AVIF RGB pixels for {}",
                    path.display()
                ));
            }

            let res = libavif::avifImageYUVToRGB(image, &mut rgb);
            if res != libavif::AVIF_RESULT_OK {
                libavif::avifRGBImageFreePixels(&mut rgb);
                libavif::avifDecoderDestroy(decoder);
                return Err(format!(
                    "Failed to convert AVIF to RGB for {}",
                    path.display()
                ));
            }

            let w = rgb.width;
            let h = rgb.height;
            validate_dimensions(w, h, "AVIF").map_err(|e| {
                libavif::avifRGBImageFreePixels(&mut rgb);
                libavif::avifDecoderDestroy(decoder);
                e
            })?;

            let pixel_count = (w as usize) * (h as usize) * 4;
            let mut pixels = vec![0u8; pixel_count];
            let src_ptr = rgb.pixels;
            let row_bytes = rgb.row_bytes as usize;
            for y in 0..h as usize {
                let src_offset = y * row_bytes;
                let dst_offset = y * (w as usize) * 4;
                std::ptr::copy_nonoverlapping(
                    src_ptr.add(src_offset),
                    pixels.as_mut_ptr().add(dst_offset),
                    (w as usize) * 4,
                );
            }
            libavif::avifRGBImageFreePixels(&mut rgb);

            // Extract EXIF orientation before destroying decoder
            // avifImage.exif is at a known offset — we extract it from raw data instead
            // since the struct layout is complex. We'll use our own EXIF parser on the
            // raw AVIF container.
            libavif::avifDecoderDestroy(decoder);

            let mut img = RgbaImage::from_raw(w, h, pixels)
                .ok_or_else(|| "AVIF pixel buffer size mismatch".to_string())?;

            // Apply EXIF orientation from raw AVIF data
            if let Some(orientation) = read_exif_orientation_avif(&data) {
                img = apply_orientation(img, orientation);
            }

            Ok(LoadedImage::Static(img))
        }
    }
}

/// Extract EXIF data from an AVIF/HEIF container (ISOBMFF format).
/// Searches for an "Exif" box within the meta box's iloc-referenced items,
/// but the simplest approach for AVIF is to scan for the Exif header pattern.
fn extract_avif_exif(data: &[u8]) -> Option<Vec<u8>> {
    // AVIF uses ISOBMFF (ISO Base Media File Format).
    // The Exif data is stored as an item referenced by iloc.
    // A simpler approach: scan for "Exif" followed by TIFF header.
    // The AVIF Exif item starts with a 4-byte big-endian offset to the TIFF header,
    // followed by the TIFF data ("II" or "MM").

    // Walk ISOBMFF boxes looking for meta box containing Exif
    let mut pos = 0;
    while pos + 8 <= data.len() {
        let box_size =
            u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        if pos + 4 + 4 > data.len() {
            break;
        }
        let box_type = &data[pos + 4..pos + 8];

        let actual_size = if box_size == 0 {
            data.len() - pos // box extends to EOF
        } else if box_size == 1 {
            // 64-bit extended size
            if pos + 16 > data.len() {
                break;
            }
            let ext = u64::from_be_bytes([
                data[pos + 8],
                data[pos + 9],
                data[pos + 10],
                data[pos + 11],
                data[pos + 12],
                data[pos + 13],
                data[pos + 14],
                data[pos + 15],
            ]) as usize;
            ext
        } else {
            box_size
        };

        if actual_size < 8 || pos + actual_size > data.len() {
            break;
        }

        if box_type == b"meta" {
            // meta box has a 4-byte version/flags field after the header
            let inner_start = pos + 12; // 8 (header) + 4 (version/flags)
            if let Some(exif) = find_exif_in_meta(&data[inner_start..pos + actual_size]) {
                return Some(exif);
            }
        }

        pos += actual_size;
    }
    None
}

/// Search within a meta box's children for Exif item data.
/// This is a simplified parser that looks for "Exif" boxes or
/// scans the iloc data for Exif items.
fn find_exif_in_meta(data: &[u8]) -> Option<Vec<u8>> {
    // Walk sub-boxes of meta looking for "iinf" to find Exif item ID,
    // then "iloc" to find offset/length, then extract from file data.
    // This is complex, so we use a simpler heuristic: scan for the
    // Exif TIFF header pattern preceded by a 4-byte offset.

    // Look for pattern: 4-byte offset (usually 0x00000000 or small) + "II" or "MM" + 0x002A
    for i in 0..data.len().saturating_sub(10) {
        let b = &data[i..];
        if b.len() < 10 {
            break;
        }
        // Check for TIFF header at offset i+4
        let has_tiff = (b[4] == b'I' && b[5] == b'I' && b[6] == 0x2A && b[7] == 0x00)
            || (b[4] == b'M' && b[5] == b'M' && b[6] == 0x00 && b[7] == 0x2A);
        if has_tiff {
            // The 4 bytes before TIFF header are the exif_tiff_header_offset
            // (should be 0 for standard Exif)
            let _offset = u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize;
            // Return the TIFF data (starting from b[4])
            return Some(data[i + 4..].to_vec());
        }
    }
    None
}

/// Read EXIF orientation from raw AVIF data.
fn read_exif_orientation_avif(data: &[u8]) -> Option<u32> {
    let exif_data = extract_avif_exif(data)?;
    parse_tiff_orientation(&exif_data, 0)
}

/// Read all EXIF tags from raw AVIF data.
pub fn read_exif_tags_avif(data: &[u8]) -> Vec<(String, String)> {
    if let Some(exif_data) = extract_avif_exif(data) {
        return parse_all_exif_tags(&exif_data, 0);
    }
    Vec::new()
}

// ============================================================
// HEIC/HEIF via system libheif
// ============================================================

#[allow(non_camel_case_types)]
mod libheif {
    use std::os::raw::{c_char, c_int, c_void};

    pub const HEIF_ERROR_OK: c_int = 0;
    pub const HEIF_COLORSPACE_RGB: c_int = 1;
    pub const HEIF_CHROMA_INTERLEAVED_RGBA: c_int = 11;
    pub const HEIF_CHANNEL_INTERLEAVED: c_int = 10;

    #[repr(C)]
    pub struct heif_error {
        pub code: c_int,
        pub subcode: c_int,
        pub message: *const c_char,
    }

    pub type heif_context = c_void;
    pub type heif_image_handle = c_void;
    pub type heif_image = c_void;
    pub type heif_decoding_options = c_void;

    pub type heif_item_id = u32;

    #[link(name = "heif")]
    extern "C" {
        pub fn heif_context_alloc() -> *mut heif_context;
        pub fn heif_context_free(ctx: *mut heif_context);
        pub fn heif_context_read_from_memory_without_copy(
            ctx: *mut heif_context,
            mem: *const u8,
            size: usize,
            options: *const c_void,
        ) -> heif_error;
        pub fn heif_context_get_primary_image_handle(
            ctx: *mut heif_context,
            handle: *mut *mut heif_image_handle,
        ) -> heif_error;
        pub fn heif_image_handle_release(handle: *mut heif_image_handle);
        pub fn heif_image_handle_get_width(handle: *const heif_image_handle) -> c_int;
        pub fn heif_image_handle_get_height(handle: *const heif_image_handle) -> c_int;
        pub fn heif_decode_image(
            handle: *const heif_image_handle,
            out_img: *mut *mut heif_image,
            colorspace: c_int,
            chroma: c_int,
            options: *const heif_decoding_options,
        ) -> heif_error;
        pub fn heif_image_get_plane_readonly(
            image: *const heif_image,
            channel: c_int,
            out_stride: *mut c_int,
        ) -> *const u8;
        pub fn heif_image_release(image: *mut heif_image);

        // EXIF metadata
        pub fn heif_image_handle_get_number_of_metadata_blocks(
            handle: *const heif_image_handle,
            type_filter: *const c_char,
        ) -> c_int;
        pub fn heif_image_handle_get_list_of_metadata_block_IDs(
            handle: *const heif_image_handle,
            type_filter: *const c_char,
            ids: *mut heif_item_id,
            count: c_int,
        ) -> c_int;
        pub fn heif_image_handle_get_metadata_size(
            handle: *const heif_image_handle,
            metadata_id: heif_item_id,
        ) -> usize;
        pub fn heif_image_handle_get_metadata(
            handle: *const heif_image_handle,
            metadata_id: heif_item_id,
            out_data: *mut u8,
        ) -> heif_error;
    }
}

fn load_heic(path: &Path) -> Result<LoadedImage, String> {
    let data = read_file_limited(path)?;

    unsafe {
        let ctx = libheif::heif_context_alloc();
        if ctx.is_null() {
            return Err("Failed to allocate HEIF context".to_string());
        }

        let err = libheif::heif_context_read_from_memory_without_copy(
            ctx,
            data.as_ptr(),
            data.len(),
            std::ptr::null(),
        );
        if err.code != libheif::HEIF_ERROR_OK {
            libheif::heif_context_free(ctx);
            return Err(format!("Failed to read HEIC {}", path.display()));
        }

        let mut handle: *mut libheif::heif_image_handle = std::ptr::null_mut();
        let err = libheif::heif_context_get_primary_image_handle(ctx, &mut handle);
        if err.code != libheif::HEIF_ERROR_OK {
            libheif::heif_context_free(ctx);
            return Err(format!(
                "Failed to get HEIC primary image handle for {}",
                path.display()
            ));
        }

        let w = libheif::heif_image_handle_get_width(handle) as u32;
        let h = libheif::heif_image_handle_get_height(handle) as u32;
        validate_dimensions(w, h, "HEIC").map_err(|e| {
            libheif::heif_image_handle_release(handle);
            libheif::heif_context_free(ctx);
            e
        })?;

        let mut img_ptr: *mut libheif::heif_image = std::ptr::null_mut();
        let err = libheif::heif_decode_image(
            handle,
            &mut img_ptr,
            libheif::HEIF_COLORSPACE_RGB,
            libheif::HEIF_CHROMA_INTERLEAVED_RGBA,
            std::ptr::null(),
        );
        if err.code != libheif::HEIF_ERROR_OK {
            libheif::heif_image_handle_release(handle);
            libheif::heif_context_free(ctx);
            return Err(format!("Failed to decode HEIC {}", path.display()));
        }

        let mut stride: c_int = 0;
        let plane = libheif::heif_image_get_plane_readonly(
            img_ptr,
            libheif::HEIF_CHANNEL_INTERLEAVED,
            &mut stride,
        );
        if plane.is_null() {
            libheif::heif_image_release(img_ptr);
            libheif::heif_image_handle_release(handle);
            libheif::heif_context_free(ctx);
            return Err(format!(
                "Failed to get HEIC pixel data for {}",
                path.display()
            ));
        }

        let stride = stride as usize;
        let pixel_count = (w as usize) * (h as usize) * 4;
        let mut pixels = vec![0u8; pixel_count];
        for y in 0..h as usize {
            let src_offset = y * stride;
            let dst_offset = y * (w as usize) * 4;
            std::ptr::copy_nonoverlapping(
                plane.add(src_offset),
                pixels.as_mut_ptr().add(dst_offset),
                (w as usize) * 4,
            );
        }

        // Extract EXIF metadata before releasing handle
        let exif_data = extract_heif_exif(handle);

        libheif::heif_image_release(img_ptr);
        libheif::heif_image_handle_release(handle);
        libheif::heif_context_free(ctx);

        let img = RgbaImage::from_raw(w, h, pixels)
            .ok_or_else(|| "HEIC pixel buffer size mismatch".to_string())?;

        // libheif applies geometric transforms (rotation/mirror) by default
        // (ignore_transformations=false in decoding options), so we do NOT apply
        // EXIF orientation ourselves. The EXIF data is kept for tag display only.
        let _ = exif_data;

        Ok(LoadedImage::Static(img))
    }
}

/// Extract raw EXIF data from a HEIF image handle via libheif metadata API.
unsafe fn extract_heif_exif(handle: *const libheif::heif_image_handle) -> Option<Vec<u8>> {
    let exif_filter = b"Exif\0".as_ptr() as *const c_char;
    let count = libheif::heif_image_handle_get_number_of_metadata_blocks(handle, exif_filter);
    if count <= 0 {
        return None;
    }

    let mut ids = vec![0u32; count as usize];
    libheif::heif_image_handle_get_list_of_metadata_block_IDs(
        handle,
        exif_filter,
        ids.as_mut_ptr(),
        count,
    );

    let size = libheif::heif_image_handle_get_metadata_size(handle, ids[0]);
    if size == 0 || size > 64 * 1024 * 1024 {
        return None;
    }

    let mut buf = vec![0u8; size];
    let err = libheif::heif_image_handle_get_metadata(handle, ids[0], buf.as_mut_ptr());
    if err.code != libheif::HEIF_ERROR_OK {
        return None;
    }

    // libheif returns: 4-byte Exif TIFF header offset (big-endian) + TIFF data
    if buf.len() < 8 {
        return None;
    }
    let tiff_offset = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    let tiff_start = 4 + tiff_offset;
    if tiff_start >= buf.len() {
        return None;
    }

    Some(buf[tiff_start..].to_vec())
}

/// Read EXIF tags from HEIC/HEIF by parsing the ISOBMFF container.
/// We reuse the AVIF EXIF extraction since HEIC uses the same container format.
pub fn read_exif_tags_heic(data: &[u8]) -> Vec<(String, String)> {
    // For HEIC, we can use the same ISOBMFF scanning approach as AVIF
    if let Some(exif_data) = extract_avif_exif(data) {
        return parse_all_exif_tags(&exif_data, 0);
    }
    Vec::new()
}

// ============================================================
// JPEG XL via system libjxl
// ============================================================

#[allow(non_camel_case_types)]
mod libjxl {
    use std::os::raw::c_void;

    // JxlDecoderStatus values
    pub const JXL_DEC_SUCCESS: u32 = 0;
    pub const JXL_DEC_ERROR: u32 = 1;
    #[allow(dead_code)]
    pub const JXL_DEC_NEED_MORE_INPUT: u32 = 2;
    pub const JXL_DEC_NEED_IMAGE_OUT_BUFFER: u32 = 5;
    pub const JXL_DEC_BASIC_INFO: u32 = 0x40;
    pub const JXL_DEC_FRAME: u32 = 0x400;
    pub const JXL_DEC_FULL_IMAGE: u32 = 0x1000;

    // JxlDataType values
    pub const JXL_TYPE_UINT8: u32 = 2;

    // JxlEndianness
    pub const JXL_NATIVE_ENDIAN: u32 = 0;

    pub type JxlDecoder = c_void;

    #[repr(C)]
    pub struct JxlPixelFormat {
        pub num_channels: u32,
        pub data_type: u32,
        pub endianness: u32,
        pub align: usize,
    }

    #[repr(C)]
    pub struct JxlAnimationHeader {
        pub tps_numerator: u32,
        pub tps_denominator: u32,
        pub num_loops: u32,
        pub have_timecodes: i32,
    }

    #[repr(C)]
    pub struct JxlPreviewHeader {
        pub xsize: u32,
        pub ysize: u32,
    }

    #[repr(C)]
    pub struct JxlBasicInfo {
        pub have_container: i32,
        pub xsize: u32,
        pub ysize: u32,
        pub bits_per_sample: u32,
        pub exponent_bits_per_sample: u32,
        pub intensity_target: f32,
        pub min_nits: f32,
        pub relative_to_max_display: i32,
        pub linear_below: f32,
        pub uses_original_profile: i32,
        pub have_preview: i32,
        pub have_animation: i32,
        pub orientation: u32,
        pub num_color_channels: u32,
        pub num_extra_channels: u32,
        pub alpha_bits: u32,
        pub alpha_exponent_bits: u32,
        pub alpha_premultiplied: i32,
        pub preview: JxlPreviewHeader,
        pub animation: JxlAnimationHeader,
        pub intrinsic_xsize: u32,
        pub intrinsic_ysize: u32,
        pub padding: [u8; 100],
    }

    #[repr(C)]
    pub struct JxlLayerInfo {
        pub have_crop: i32,
        pub crop_x0: i32,
        pub crop_y0: i32,
        pub xsize: u32,
        pub ysize: u32,
        pub blend_info: JxlBlendInfo,
        pub save_as_reference: u32,
    }

    #[repr(C)]
    pub struct JxlBlendInfo {
        pub blendmode: u32,
        pub source: u32,
        pub alpha: u32,
        pub clamp: i32,
    }

    #[repr(C)]
    pub struct JxlFrameHeader {
        pub duration: u32,
        pub timecode: u32,
        pub name_length: u32,
        pub is_last: i32,
        pub layer_info: JxlLayerInfo,
    }

    #[link(name = "jxl")]
    extern "C" {
        pub fn JxlDecoderCreate(memory_manager: *const c_void) -> *mut JxlDecoder;
        pub fn JxlDecoderDestroy(dec: *mut JxlDecoder);
        pub fn JxlDecoderSubscribeEvents(dec: *mut JxlDecoder, events_wanted: i32) -> u32;
        pub fn JxlDecoderSetInput(dec: *mut JxlDecoder, data: *const u8, size: usize) -> u32;
        pub fn JxlDecoderCloseInput(dec: *mut JxlDecoder);
        pub fn JxlDecoderProcessInput(dec: *mut JxlDecoder) -> u32;
        pub fn JxlDecoderGetBasicInfo(dec: *const JxlDecoder, info: *mut JxlBasicInfo) -> u32;
        pub fn JxlDecoderGetFrameHeader(dec: *const JxlDecoder, header: *mut JxlFrameHeader)
            -> u32;
        pub fn JxlDecoderImageOutBufferSize(
            dec: *const JxlDecoder,
            format: *const JxlPixelFormat,
            size: *mut usize,
        ) -> u32;
        pub fn JxlDecoderSetImageOutBuffer(
            dec: *mut JxlDecoder,
            format: *const JxlPixelFormat,
            buffer: *mut u8,
            size: usize,
        ) -> u32;
        pub fn JxlDecoderSetParallelRunner(
            dec: *mut JxlDecoder,
            parallel_runner: *const c_void,
            parallel_runner_opaque: *mut c_void,
        ) -> u32;
    }

    #[link(name = "jxl_threads")]
    extern "C" {
        pub fn JxlThreadParallelRunnerCreate(
            memory_manager: *const c_void,
            num_worker_threads: usize,
        ) -> *mut c_void;
        pub fn JxlThreadParallelRunnerDestroy(runner_opaque: *mut c_void);
        pub fn JxlThreadParallelRunnerDefaultNumWorkerThreads() -> usize;

        // The actual runner function — used as a function pointer
        pub fn JxlThreadParallelRunner(
            runner_opaque: *mut c_void,
            jpegxl_opaque: *mut c_void,
            init: *mut c_void,
            func: *mut c_void,
            start_range: u32,
            end_range: u32,
        ) -> i32;
    }
}

fn load_jxl(path: &Path) -> Result<LoadedImage, String> {
    let data = read_file_limited(path)?;

    unsafe {
        let dec = libjxl::JxlDecoderCreate(std::ptr::null());
        if dec.is_null() {
            return Err("Failed to create JPEG XL decoder".to_string());
        }

        // Set up thread parallel runner
        let num_threads = libjxl::JxlThreadParallelRunnerDefaultNumWorkerThreads();
        let runner = libjxl::JxlThreadParallelRunnerCreate(std::ptr::null(), num_threads);
        if !runner.is_null() {
            libjxl::JxlDecoderSetParallelRunner(
                dec,
                libjxl::JxlThreadParallelRunner as *const c_void,
                runner,
            );
        }

        // Subscribe to events
        let events = (libjxl::JXL_DEC_BASIC_INFO
            | libjxl::JXL_DEC_FRAME
            | libjxl::JXL_DEC_FULL_IMAGE) as i32;
        if libjxl::JxlDecoderSubscribeEvents(dec, events) != libjxl::JXL_DEC_SUCCESS {
            cleanup_jxl(dec, runner);
            return Err(format!(
                "Failed to subscribe JXL events for {}",
                path.display()
            ));
        }

        // Set input
        if libjxl::JxlDecoderSetInput(dec, data.as_ptr(), data.len()) != libjxl::JXL_DEC_SUCCESS {
            cleanup_jxl(dec, runner);
            return Err(format!("Failed to set JXL input for {}", path.display()));
        }
        libjxl::JxlDecoderCloseInput(dec);

        let pixel_format = libjxl::JxlPixelFormat {
            num_channels: 4,
            data_type: libjxl::JXL_TYPE_UINT8,
            endianness: libjxl::JXL_NATIVE_ENDIAN,
            align: 0,
        };

        let mut info: libjxl::JxlBasicInfo = std::mem::zeroed();
        let mut frames: Vec<(RgbaImage, Duration)> = Vec::new();
        let mut current_buffer: Vec<u8> = Vec::new();
        let mut is_animated = false;

        loop {
            let status = libjxl::JxlDecoderProcessInput(dec);

            match status {
                s if s == libjxl::JXL_DEC_BASIC_INFO => {
                    if libjxl::JxlDecoderGetBasicInfo(dec, &mut info) != libjxl::JXL_DEC_SUCCESS {
                        cleanup_jxl(dec, runner);
                        return Err(format!(
                            "Failed to get JXL basic info for {}",
                            path.display()
                        ));
                    }
                    validate_dimensions(info.xsize, info.ysize, "JXL").map_err(|e| {
                        cleanup_jxl(dec, runner);
                        e
                    })?;
                    is_animated = info.have_animation != 0;
                }
                s if s == libjxl::JXL_DEC_FRAME => {
                    // Get frame header for duration
                    let mut frame_header: libjxl::JxlFrameHeader = std::mem::zeroed();
                    libjxl::JxlDecoderGetFrameHeader(dec, &mut frame_header);

                    // Allocate output buffer
                    let mut buf_size: usize = 0;
                    if libjxl::JxlDecoderImageOutBufferSize(dec, &pixel_format, &mut buf_size)
                        != libjxl::JXL_DEC_SUCCESS
                    {
                        cleanup_jxl(dec, runner);
                        return Err(format!(
                            "Failed to get JXL output buffer size for {}",
                            path.display()
                        ));
                    }

                    current_buffer = vec![0u8; buf_size];
                    if libjxl::JxlDecoderSetImageOutBuffer(
                        dec,
                        &pixel_format,
                        current_buffer.as_mut_ptr(),
                        buf_size,
                    ) != libjxl::JXL_DEC_SUCCESS
                    {
                        cleanup_jxl(dec, runner);
                        return Err(format!(
                            "Failed to set JXL output buffer for {}",
                            path.display()
                        ));
                    }

                    if is_animated {
                        // Calculate frame duration
                        let tps_num = info.animation.tps_numerator as f64;
                        let tps_den = info.animation.tps_denominator as f64;
                        let duration_secs = if tps_num > 0.0 {
                            (frame_header.duration as f64) * tps_den / tps_num
                        } else {
                            0.1 // fallback 100ms
                        };
                        let duration_ms = (duration_secs * 1000.0) as u64;
                        // Store duration temporarily; we'll pair it with the image at FULL_IMAGE
                        // We push a placeholder that we'll update
                        frames.push((
                            RgbaImage::new(1, 1), // placeholder
                            Duration::from_millis(duration_ms.max(10)),
                        ));
                    }
                }
                s if s == libjxl::JXL_DEC_NEED_IMAGE_OUT_BUFFER => {
                    // Buffer already set at FRAME event
                    // If we somehow get here without having set the buffer, set it now
                    let mut buf_size: usize = 0;
                    if libjxl::JxlDecoderImageOutBufferSize(dec, &pixel_format, &mut buf_size)
                        != libjxl::JXL_DEC_SUCCESS
                    {
                        cleanup_jxl(dec, runner);
                        return Err(format!(
                            "Failed to get JXL output buffer size for {}",
                            path.display()
                        ));
                    }
                    if current_buffer.is_empty() {
                        current_buffer = vec![0u8; buf_size];
                    }
                    if libjxl::JxlDecoderSetImageOutBuffer(
                        dec,
                        &pixel_format,
                        current_buffer.as_mut_ptr(),
                        current_buffer.len(),
                    ) != libjxl::JXL_DEC_SUCCESS
                    {
                        cleanup_jxl(dec, runner);
                        return Err(format!(
                            "Failed to set JXL output buffer for {}",
                            path.display()
                        ));
                    }
                }
                s if s == libjxl::JXL_DEC_FULL_IMAGE => {
                    let img = RgbaImage::from_raw(
                        info.xsize,
                        info.ysize,
                        std::mem::take(&mut current_buffer),
                    )
                    .ok_or_else(|| "JXL pixel buffer size mismatch".to_string())?;

                    if is_animated {
                        // Replace placeholder with actual image
                        if let Some(last) = frames.last_mut() {
                            last.0 = img;
                        }
                    } else {
                        // Static image — apply orientation and return
                        let orientation = info.orientation;
                        let img = if orientation >= 2 && orientation <= 8 {
                            apply_orientation(img, orientation)
                        } else {
                            img
                        };
                        cleanup_jxl(dec, runner);
                        return Ok(LoadedImage::Static(img));
                    }
                }
                s if s == libjxl::JXL_DEC_SUCCESS => {
                    break;
                }
                s if s == libjxl::JXL_DEC_ERROR => {
                    cleanup_jxl(dec, runner);
                    return Err(format!("JXL decode error for {}", path.display()));
                }
                _ => {
                    // Unknown status, try to continue
                    break;
                }
            }
        }

        cleanup_jxl(dec, runner);

        if is_animated && !frames.is_empty() {
            // Apply orientation to all frames
            let orientation = info.orientation;
            if orientation >= 2 && orientation <= 8 {
                for frame in &mut frames {
                    let rotated = apply_orientation(
                        std::mem::replace(&mut frame.0, RgbaImage::new(1, 1)),
                        orientation,
                    );
                    frame.0 = rotated;
                }
            }
            Ok(LoadedImage::Animated { frames })
        } else {
            Err(format!("JXL contains no frames: {}", path.display()))
        }
    }
}

unsafe fn cleanup_jxl(dec: *mut libjxl::JxlDecoder, runner: *mut c_void) {
    libjxl::JxlDecoderDestroy(dec);
    if !runner.is_null() {
        libjxl::JxlThreadParallelRunnerDestroy(runner);
    }
}

/// Read EXIF tags from JXL data.
/// JXL stores EXIF in a box of type "Exif" in the container.
/// The Exif box starts with a 4-byte big-endian TIFF header offset, then TIFF data.
pub fn read_exif_tags_jxl(data: &[u8]) -> Vec<(String, String)> {
    if let Some(exif_data) = extract_jxl_exif(data) {
        return parse_all_exif_tags(&exif_data, 0);
    }
    Vec::new()
}

/// Extract EXIF data from a JXL container.
/// JXL container format uses ISOBMFF-like boxes: 4-byte size + 4-byte type.
fn extract_jxl_exif(data: &[u8]) -> Option<Vec<u8>> {
    // JXL container starts with JXL signature box:
    // 0x0000000C 'JXL ' 0x0D0A870A (12 bytes)
    // Then followed by jxlc/jxlp boxes and metadata boxes.
    // Check for JXL container signature
    if data.len() < 12 {
        return None;
    }
    // JXL container signature: 00 00 00 0C 4A 58 4C 20 0D 0A 87 0A
    let jxl_sig = [
        0x00, 0x00, 0x00, 0x0C, 0x4A, 0x58, 0x4C, 0x20, 0x0D, 0x0A, 0x87, 0x0A,
    ];
    if data[..12] != jxl_sig {
        // Not a JXL container (might be bare codestream) — no EXIF boxes
        return None;
    }

    let mut pos = 0;
    while pos + 8 <= data.len() {
        let box_size =
            u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        let box_type = &data[pos + 4..pos + 8];

        let (header_size, actual_size) = if box_size == 0 {
            (8, data.len() - pos)
        } else if box_size == 1 {
            if pos + 16 > data.len() {
                break;
            }
            let ext = u64::from_be_bytes([
                data[pos + 8],
                data[pos + 9],
                data[pos + 10],
                data[pos + 11],
                data[pos + 12],
                data[pos + 13],
                data[pos + 14],
                data[pos + 15],
            ]) as usize;
            (16, ext)
        } else {
            (8, box_size)
        };

        if actual_size < header_size || pos + actual_size > data.len() {
            break;
        }

        if box_type == b"Exif" {
            let payload = &data[pos + header_size..pos + actual_size];
            // Exif box: 4-byte big-endian TIFF header offset + TIFF data
            if payload.len() < 8 {
                return None;
            }
            let tiff_offset =
                u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]) as usize;
            let tiff_start = 4 + tiff_offset;
            if tiff_start >= payload.len() {
                return None;
            }
            return Some(payload[tiff_start..].to_vec());
        }

        pos += actual_size;
    }
    None
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
    let data = read_file_limited(path)?;

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

pub(crate) fn rotate_180(img: RgbaImage) -> RgbaImage {
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

pub(crate) fn flip_h(img: RgbaImage) -> RgbaImage {
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

/// Read EXIF tags from raw TIFF data.
/// TIFF files ARE TIFF structures, so the header is at byte 0.
pub fn read_exif_tags_tiff(data: &[u8]) -> Vec<(String, String)> {
    parse_all_exif_tags(data, 0)
}

/// Read EXIF orientation from raw TIFF data.
/// (TIFF orientation is handled by libtiff during loading, but this is kept
/// for API symmetry with the WebP/PNG orientation functions.)
#[allow(dead_code)]
pub fn read_exif_orientation_tiff(data: &[u8]) -> Option<u32> {
    parse_tiff_orientation(data, 0)
}

/// Read EXIF tags from raw WebP data by walking the RIFF container for the EXIF chunk.
pub fn read_exif_tags_webp(data: &[u8]) -> Vec<(String, String)> {
    if let Some(exif_data) = extract_webp_exif(data) {
        return parse_all_exif_tags(&exif_data, 0);
    }
    Vec::new()
}

/// Read EXIF orientation from raw WebP data.
pub fn read_exif_orientation_webp(data: &[u8]) -> Option<u32> {
    let exif_data = extract_webp_exif(data)?;
    parse_tiff_orientation(&exif_data, 0)
}

/// Extract the EXIF payload from a WebP RIFF container.
/// Returns the raw TIFF data (with Exif\0\0 prefix stripped if present).
fn extract_webp_exif(data: &[u8]) -> Option<Vec<u8>> {
    // Verify RIFF header: "RIFF" + 4-byte size + "WEBP"
    if data.len() < 12 {
        return None;
    }
    if &data[0..4] != b"RIFF" || &data[8..12] != b"WEBP" {
        return None;
    }

    // Walk RIFF chunks starting at offset 12
    let mut pos = 12;
    while pos + 8 <= data.len() {
        let fourcc = &data[pos..pos + 4];
        let chunk_size =
            u32::from_le_bytes([data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]])
                as usize;
        let payload_start = pos + 8;
        let payload_end = payload_start + chunk_size;

        if fourcc == b"EXIF" {
            if payload_end > data.len() {
                return None;
            }
            let payload = &data[payload_start..payload_end];
            // Some encoders prepend "Exif\0\0" (6 bytes) before the TIFF header
            if payload.len() >= 6 && &payload[0..6] == b"Exif\0\0" {
                return Some(payload[6..].to_vec());
            }
            return Some(payload.to_vec());
        }

        // Chunks are padded to even size
        let padded_size = (chunk_size + 1) & !1;
        pos = payload_start + padded_size;
    }
    None
}

/// Read EXIF tags from raw PNG data by scanning for the eXIf chunk.
pub fn read_exif_tags_png(data: &[u8]) -> Vec<(String, String)> {
    if let Some(exif_data) = extract_png_exif(data) {
        return parse_all_exif_tags(&exif_data, 0);
    }
    Vec::new()
}

/// Read EXIF orientation from raw PNG data.
pub fn read_exif_orientation_png(data: &[u8]) -> Option<u32> {
    let exif_data = extract_png_exif(data)?;
    parse_tiff_orientation(&exif_data, 0)
}

/// Extract EXIF payload from a PNG file by walking chunks for "eXIf".
/// PNG chunks: 4-byte length + 4-byte type + payload + 4-byte CRC.
fn extract_png_exif(data: &[u8]) -> Option<Vec<u8>> {
    // PNG signature is 8 bytes
    if data.len() < 8 || &data[0..4] != b"\x89PNG" {
        return None;
    }

    let mut pos = 8; // skip PNG signature
    while pos + 12 <= data.len() {
        let chunk_len =
            u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        let chunk_type = &data[pos + 4..pos + 8];
        let payload_start = pos + 8;
        let payload_end = payload_start + chunk_len;

        if chunk_type == b"eXIf" {
            if payload_end > data.len() {
                return None;
            }
            // eXIf payload is raw TIFF data (no Exif\0\0 prefix)
            return Some(data[payload_start..payload_end].to_vec());
        }

        // Move to next chunk: length + type(4) + payload + CRC(4)
        pos = payload_end + 4;
    }
    None
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

pub(crate) fn flip_v(img: RgbaImage) -> RgbaImage {
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

#[cfg(test)]
mod tests {
    use super::*;

    // ========== Helper: create a small test image ==========

    /// Create a 2x3 RGBA image with distinct pixel values.
    /// Layout (row-major):
    ///   (0,0)=R  (1,0)=G
    ///   (0,1)=B  (1,1)=W
    ///   (0,2)=Y  (1,2)=C
    fn make_2x3_image() -> RgbaImage {
        let mut img = RgbaImage::new(2, 3);
        let pixels: &[[u8; 4]] = &[
            [255, 0, 0, 255],     // R
            [0, 255, 0, 255],     // G
            [0, 0, 255, 255],     // B
            [255, 255, 255, 255], // W
            [255, 255, 0, 255],   // Y
            [0, 255, 255, 255],   // C
        ];
        for (i, px) in pixels.iter().enumerate() {
            let off = i * 4;
            img.data[off..off + 4].copy_from_slice(px);
        }
        img
    }

    fn pixel_at(img: &RgbaImage, x: u32, y: u32) -> [u8; 4] {
        let off = ((y * img.width + x) * 4) as usize;
        [
            img.data[off],
            img.data[off + 1],
            img.data[off + 2],
            img.data[off + 3],
        ]
    }

    // ========== Transform tests ==========

    #[test]
    fn test_rotate_90() {
        let img = make_2x3_image(); // 2x3
        let out = rotate_90(img);
        assert_eq!(out.dimensions(), (3, 2)); // width=old_h, height=old_w
                                              // Original layout:
                                              //   R G     Rotated 90 CW:
                                              //   B W     Y B R
                                              //   Y C     C W G
        assert_eq!(pixel_at(&out, 0, 0), [255, 255, 0, 255]); // Y
        assert_eq!(pixel_at(&out, 1, 0), [0, 0, 255, 255]); // B
        assert_eq!(pixel_at(&out, 2, 0), [255, 0, 0, 255]); // R
        assert_eq!(pixel_at(&out, 0, 1), [0, 255, 255, 255]); // C
        assert_eq!(pixel_at(&out, 1, 1), [255, 255, 255, 255]); // W
        assert_eq!(pixel_at(&out, 2, 1), [0, 255, 0, 255]); // G
    }

    #[test]
    fn test_rotate_180() {
        let img = make_2x3_image();
        let out = rotate_180(img);
        assert_eq!(out.dimensions(), (2, 3));
        // 180: reverse all pixels
        //   C Y
        //   W B
        //   G R
        assert_eq!(pixel_at(&out, 0, 0), [0, 255, 255, 255]); // C
        assert_eq!(pixel_at(&out, 1, 0), [255, 255, 0, 255]); // Y
        assert_eq!(pixel_at(&out, 0, 1), [255, 255, 255, 255]); // W
        assert_eq!(pixel_at(&out, 1, 1), [0, 0, 255, 255]); // B
        assert_eq!(pixel_at(&out, 0, 2), [0, 255, 0, 255]); // G
        assert_eq!(pixel_at(&out, 1, 2), [255, 0, 0, 255]); // R
    }

    #[test]
    fn test_rotate_270() {
        let img = make_2x3_image();
        let out = rotate_270(img);
        assert_eq!(out.dimensions(), (3, 2));
        // 270 CW (= 90 CCW):
        //   G W C
        //   R B Y
        assert_eq!(pixel_at(&out, 0, 0), [0, 255, 0, 255]); // G
        assert_eq!(pixel_at(&out, 1, 0), [255, 255, 255, 255]); // W
        assert_eq!(pixel_at(&out, 2, 0), [0, 255, 255, 255]); // C
        assert_eq!(pixel_at(&out, 0, 1), [255, 0, 0, 255]); // R
        assert_eq!(pixel_at(&out, 1, 1), [0, 0, 255, 255]); // B
        assert_eq!(pixel_at(&out, 2, 1), [255, 255, 0, 255]); // Y
    }

    #[test]
    fn test_flip_h() {
        // Use a 2x2 image
        let mut img = RgbaImage::new(2, 2);
        img.data[0..4].copy_from_slice(&[255, 0, 0, 255]); // (0,0)=R
        img.data[4..8].copy_from_slice(&[0, 255, 0, 255]); // (1,0)=G
        img.data[8..12].copy_from_slice(&[0, 0, 255, 255]); // (0,1)=B
        img.data[12..16].copy_from_slice(&[255, 255, 0, 255]); // (1,1)=Y

        let out = flip_h(img);
        assert_eq!(out.dimensions(), (2, 2));
        assert_eq!(pixel_at(&out, 0, 0), [0, 255, 0, 255]); // G (was right)
        assert_eq!(pixel_at(&out, 1, 0), [255, 0, 0, 255]); // R (was left)
        assert_eq!(pixel_at(&out, 0, 1), [255, 255, 0, 255]); // Y
        assert_eq!(pixel_at(&out, 1, 1), [0, 0, 255, 255]); // B
    }

    #[test]
    fn test_flip_v() {
        let mut img = RgbaImage::new(2, 2);
        img.data[0..4].copy_from_slice(&[255, 0, 0, 255]); // (0,0)=R
        img.data[4..8].copy_from_slice(&[0, 255, 0, 255]); // (1,0)=G
        img.data[8..12].copy_from_slice(&[0, 0, 255, 255]); // (0,1)=B
        img.data[12..16].copy_from_slice(&[255, 255, 0, 255]); // (1,1)=Y

        let out = flip_v(img);
        assert_eq!(out.dimensions(), (2, 2));
        assert_eq!(pixel_at(&out, 0, 0), [0, 0, 255, 255]); // B (was bottom-left)
        assert_eq!(pixel_at(&out, 1, 0), [255, 255, 0, 255]); // Y (was bottom-right)
        assert_eq!(pixel_at(&out, 0, 1), [255, 0, 0, 255]); // R (was top-left)
        assert_eq!(pixel_at(&out, 1, 1), [0, 255, 0, 255]); // G (was top-right)
    }

    // ========== BMP parser tests ==========

    /// Build a minimal BMP byte array with the given parameters.
    fn build_bmp(
        width: u32,
        height: i32,
        bpp: u16,
        compression: u32,
        color_table: &[[u8; 4]],
        pixel_data: &[u8],
    ) -> Vec<u8> {
        let dib_header_size: u32 = 40;
        let color_table_bytes = color_table.len() as u32 * 4;
        let data_offset = 14 + dib_header_size + color_table_bytes;
        let file_size = data_offset + pixel_data.len() as u32;

        let mut buf = Vec::with_capacity(file_size as usize);
        // File header (14 bytes)
        buf.extend_from_slice(b"BM");
        buf.extend_from_slice(&file_size.to_le_bytes());
        buf.extend_from_slice(&[0u8; 4]); // reserved
        buf.extend_from_slice(&data_offset.to_le_bytes());
        // DIB header (40 bytes - BITMAPINFOHEADER)
        buf.extend_from_slice(&dib_header_size.to_le_bytes());
        buf.extend_from_slice(&(width as i32).to_le_bytes());
        buf.extend_from_slice(&height.to_le_bytes());
        buf.extend_from_slice(&1u16.to_le_bytes()); // planes
        buf.extend_from_slice(&bpp.to_le_bytes());
        buf.extend_from_slice(&compression.to_le_bytes());
        let image_size = pixel_data.len() as u32;
        buf.extend_from_slice(&image_size.to_le_bytes());
        buf.extend_from_slice(&[0u8; 8]); // x/y pixels per meter
        let clr_used = color_table.len() as u32;
        buf.extend_from_slice(&clr_used.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes()); // clrImportant
                                                    // Color table
        for entry in color_table {
            buf.extend_from_slice(entry); // BGRA
        }
        // Pixel data
        buf.extend_from_slice(pixel_data);
        buf
    }

    #[test]
    fn test_bmp_24bit() {
        // 2x2 24-bit BMP, bottom-up
        // Row size: (2*24+31)/32 * 4 = 8 bytes (2*3=6, padded to 8)
        // Bottom-up: row0 in file = bottom row of image
        let mut pixels = Vec::new();
        // File row 0 -> image row 1 (bottom): B0=(0,1), B1=(1,1)
        pixels.extend_from_slice(&[255, 0, 0]); // BGR -> pixel (0,1) = Red
        pixels.extend_from_slice(&[0, 255, 0]); // BGR -> pixel (1,1) = Green
        pixels.extend_from_slice(&[0, 0]); // padding to 8 bytes
                                           // File row 1 -> image row 0 (top): B0=(0,0), B1=(1,0)
        pixels.extend_from_slice(&[0, 0, 255]); // BGR -> pixel (0,0) = Blue
        pixels.extend_from_slice(&[255, 255, 0]); // BGR -> pixel (1,0) = Yellow (B=0 G=255 R=255->Cyan??)
        pixels.extend_from_slice(&[0, 0]); // padding

        // Wait — BMP BGR means: byte0=Blue, byte1=Green, byte2=Red
        // So [255,0,0] = B=255,G=0,R=0 -> Blue pixel
        // Let me redo:
        // pixel (0,1) -> in BMP file: [B, G, R] -> RGBA output = [R, G, B, 255]
        // Let's use: pixel(0,0)=Red(R=255,G=0,B=0), pixel(1,0)=Green
        let mut pixels = Vec::new();
        // File row 0 = image row 1 (bottom-up)
        // pixel(0,1) = want Blue: BGR=[255,0,0]
        pixels.extend_from_slice(&[255, 0, 0]);
        // pixel(1,1) = want White: BGR=[255,255,255]
        pixels.extend_from_slice(&[255, 255, 255]);
        pixels.extend_from_slice(&[0, 0]); // pad to 8
                                           // File row 1 = image row 0
                                           // pixel(0,0) = want Red: BGR=[0,0,255]
        pixels.extend_from_slice(&[0, 0, 255]);
        // pixel(1,0) = want Green: BGR=[0,255,0]
        pixels.extend_from_slice(&[0, 255, 0]);
        pixels.extend_from_slice(&[0, 0]); // pad to 8

        let bmp = build_bmp(2, 2, 24, 0, &[], &pixels);
        let result = decode_bmp(&bmp, "test").unwrap();
        let img = match result {
            LoadedImage::Static(img) => img,
            _ => panic!("Expected static image"),
        };
        assert_eq!(img.dimensions(), (2, 2));
        assert_eq!(pixel_at(&img, 0, 0), [255, 0, 0, 255]); // Red
        assert_eq!(pixel_at(&img, 1, 0), [0, 255, 0, 255]); // Green
        assert_eq!(pixel_at(&img, 0, 1), [0, 0, 255, 255]); // Blue
        assert_eq!(pixel_at(&img, 1, 1), [255, 255, 255, 255]); // White
    }

    #[test]
    fn test_bmp_8bit() {
        // 2x1 8-bit BMP with 4-entry palette
        // Row size: (2*8+31)/32 * 4 = 4 bytes
        let palette: Vec<[u8; 4]> = vec![
            [255, 0, 0, 0],     // index 0: B=255 -> Blue
            [0, 255, 0, 0],     // index 1: G=255 -> Green
            [0, 0, 255, 0],     // index 2: R=255 -> Red
            [255, 255, 255, 0], // index 3: White
        ];
        // pixel(0,0) = index 2 (Red), pixel(1,0) = index 0 (Blue)
        // Bottom-up 1-row, so file row 0 = image row 0
        let pixels = vec![2, 0, 0, 0]; // indices + padding to 4 bytes

        let bmp = build_bmp(2, 1, 8, 0, &palette, &pixels);
        let result = decode_bmp(&bmp, "test").unwrap();
        let img = match result {
            LoadedImage::Static(img) => img,
            _ => panic!("Expected static image"),
        };
        assert_eq!(img.dimensions(), (2, 1));
        assert_eq!(pixel_at(&img, 0, 0), [255, 0, 0, 255]); // index 2 -> R=255 (palette entry [0,0,255,0] -> RGBA=[255,0,0,255])
        assert_eq!(pixel_at(&img, 1, 0), [0, 0, 255, 255]); // index 0 -> B=255 (palette entry [255,0,0,0] -> RGBA=[0,0,255,255])
    }

    #[test]
    fn test_bmp_4bit() {
        // 3x1 4-bit BMP with 2-entry palette
        // Row size: (3*4+31)/32 * 4 = 4 bytes
        let palette: Vec<[u8; 4]> = vec![
            [0, 0, 0, 0],       // index 0: Black -> RGBA=[0,0,0,255]
            [255, 255, 255, 0], // index 1: White -> RGBA=[255,255,255,255]
        ];
        // 3 pixels: indices 1, 0, 1
        // byte0: high nibble=1, low nibble=0 -> pixels 0,1
        // byte1: high nibble=1, low nibble=0 (unused) -> pixel 2
        let pixels = vec![0x10, 0x10, 0, 0]; // padded to 4 bytes

        let bmp = build_bmp(3, 1, 4, 0, &palette, &pixels);
        let result = decode_bmp(&bmp, "test").unwrap();
        let img = match result {
            LoadedImage::Static(img) => img,
            _ => panic!("Expected static image"),
        };
        assert_eq!(img.dimensions(), (3, 1));
        assert_eq!(pixel_at(&img, 0, 0), [255, 255, 255, 255]); // index 1 = white
        assert_eq!(pixel_at(&img, 1, 0), [0, 0, 0, 255]); // index 0 = black
        assert_eq!(pixel_at(&img, 2, 0), [255, 255, 255, 255]); // index 1 = white
    }

    #[test]
    fn test_bmp_1bit() {
        // 8x1 1-bit BMP with 2-entry palette
        // Row size: (8*1+31)/32 * 4 = 4 bytes
        let palette: Vec<[u8; 4]> = vec![
            [0, 0, 0, 0],       // index 0: Black
            [255, 255, 255, 0], // index 1: White
        ];
        // 8 pixels: 1,0,1,0,1,0,1,0 = 0b10101010 = 0xAA
        let pixels = vec![0xAA, 0, 0, 0]; // padded to 4 bytes

        let bmp = build_bmp(8, 1, 1, 0, &palette, &pixels);
        let result = decode_bmp(&bmp, "test").unwrap();
        let img = match result {
            LoadedImage::Static(img) => img,
            _ => panic!("Expected static image"),
        };
        assert_eq!(img.dimensions(), (8, 1));
        // 0xAA = 10101010: MSB first, so pixels 0,2,4,6 = white; 1,3,5,7 = black
        assert_eq!(pixel_at(&img, 0, 0), [255, 255, 255, 255]); // 1=white
        assert_eq!(pixel_at(&img, 1, 0), [0, 0, 0, 255]); // 0=black
        assert_eq!(pixel_at(&img, 2, 0), [255, 255, 255, 255]); // 1=white
        assert_eq!(pixel_at(&img, 3, 0), [0, 0, 0, 255]); // 0=black
        assert_eq!(pixel_at(&img, 7, 0), [0, 0, 0, 255]); // 0=black
    }

    #[test]
    fn test_bmp_rle8_rejected() {
        let palette: Vec<[u8; 4]> = vec![[0, 0, 0, 0]; 2];
        let pixels = vec![0; 4];
        let bmp = build_bmp(2, 1, 8, 1, &palette, &pixels); // compression=1 (BI_RLE8)
        let result = decode_bmp(&bmp, "test.bmp");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("BI_RLE8"));
    }

    #[test]
    fn test_bmp_rle4_rejected() {
        let palette: Vec<[u8; 4]> = vec![[0, 0, 0, 0]; 2];
        let pixels = vec![0; 4];
        let bmp = build_bmp(2, 1, 4, 2, &palette, &pixels); // compression=2 (BI_RLE4)
        let result = decode_bmp(&bmp, "test.bmp");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("BI_RLE4"));
    }

    // ========== EXIF parser tests ==========

    /// Build a minimal TIFF structure with one IFD entry.
    fn build_tiff_with_orientation(le: bool, orientation: u16) -> Vec<u8> {
        let mut buf = Vec::new();
        // Byte order
        if le {
            buf.extend_from_slice(b"II");
        } else {
            buf.extend_from_slice(b"MM");
        }
        // Magic number 42
        let magic: u16 = 42;
        if le {
            buf.extend_from_slice(&magic.to_le_bytes());
        } else {
            buf.extend_from_slice(&magic.to_be_bytes());
        }
        // IFD0 offset (= 8, right after header)
        let ifd_offset: u32 = 8;
        if le {
            buf.extend_from_slice(&ifd_offset.to_le_bytes());
        } else {
            buf.extend_from_slice(&ifd_offset.to_be_bytes());
        }

        // IFD0: 1 entry
        let entry_count: u16 = 1;
        if le {
            buf.extend_from_slice(&entry_count.to_le_bytes());
        } else {
            buf.extend_from_slice(&entry_count.to_be_bytes());
        }

        // IFD entry: tag=0x0112 (Orientation), type=SHORT(3), count=1, value=orientation
        let tag: u16 = 0x0112;
        let typ: u16 = 3; // SHORT
        let count: u32 = 1;
        if le {
            buf.extend_from_slice(&tag.to_le_bytes());
            buf.extend_from_slice(&typ.to_le_bytes());
            buf.extend_from_slice(&count.to_le_bytes());
            buf.extend_from_slice(&orientation.to_le_bytes());
            buf.extend_from_slice(&[0, 0]); // pad value field to 4 bytes
        } else {
            buf.extend_from_slice(&tag.to_be_bytes());
            buf.extend_from_slice(&typ.to_be_bytes());
            buf.extend_from_slice(&count.to_be_bytes());
            buf.extend_from_slice(&orientation.to_be_bytes());
            buf.extend_from_slice(&[0, 0]); // pad
        }

        // Next IFD offset = 0 (no more IFDs)
        buf.extend_from_slice(&[0, 0, 0, 0]);

        buf
    }

    #[test]
    fn test_exif_orientation_le() {
        let data = build_tiff_with_orientation(true, 6);
        let result = parse_tiff_orientation(&data, 0);
        assert_eq!(result, Some(6));
    }

    #[test]
    fn test_exif_orientation_be() {
        let data = build_tiff_with_orientation(false, 3);
        let result = parse_tiff_orientation(&data, 0);
        assert_eq!(result, Some(3));
    }

    #[test]
    fn test_exif_tags_le() {
        let data = build_tiff_with_orientation(true, 6);
        let tags = parse_all_exif_tags(&data, 0);
        // Should find at least the Orientation tag
        let orient = tags.iter().find(|(label, _)| label == "Orientation");
        assert!(orient.is_some(), "Orientation tag not found in {:?}", tags);
    }

    #[test]
    fn test_exif_tags_be() {
        let data = build_tiff_with_orientation(false, 1);
        let tags = parse_all_exif_tags(&data, 0);
        let orient = tags.iter().find(|(label, _)| label == "Orientation");
        assert!(orient.is_some(), "Orientation tag not found in {:?}", tags);
    }

    #[test]
    fn test_exif_webp_extraction() {
        // Build a minimal RIFF/WEBP with an EXIF chunk containing a TIFF header
        let tiff = build_tiff_with_orientation(true, 8);
        let exif_chunk_size = tiff.len() as u32;

        let mut webp = Vec::new();
        webp.extend_from_slice(b"RIFF");
        let riff_size = 4 + 8 + tiff.len(); // "WEBP" + chunk header + chunk data
        webp.extend_from_slice(&(riff_size as u32).to_le_bytes());
        webp.extend_from_slice(b"WEBP");
        webp.extend_from_slice(b"EXIF");
        webp.extend_from_slice(&exif_chunk_size.to_le_bytes());
        webp.extend_from_slice(&tiff);
        // Pad to even if needed
        if tiff.len() % 2 != 0 {
            webp.push(0);
        }

        let result = read_exif_orientation_webp(&webp);
        assert_eq!(result, Some(8));
    }

    #[test]
    fn test_exif_png_extraction() {
        // Build a minimal PNG with an eXIf chunk containing a TIFF header
        let tiff = build_tiff_with_orientation(false, 5);

        let mut png = Vec::new();
        // PNG signature
        png.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        // eXIf chunk: length(4 BE) + "eXIf" + payload + CRC(4)
        png.extend_from_slice(&(tiff.len() as u32).to_be_bytes());
        png.extend_from_slice(b"eXIf");
        png.extend_from_slice(&tiff);
        png.extend_from_slice(&[0, 0, 0, 0]); // dummy CRC

        let result = read_exif_orientation_png(&png);
        assert_eq!(result, Some(5));
    }

    #[test]
    fn test_exif_tiff_direct() {
        let data = build_tiff_with_orientation(true, 2);
        let result = read_exif_orientation_tiff(&data);
        assert_eq!(result, Some(2));
    }

    #[test]
    fn test_exif_invalid_data() {
        let result = parse_tiff_orientation(&[0, 0, 0, 0], 0);
        assert_eq!(result, None);
    }

    #[test]
    fn test_exif_empty() {
        let result = parse_tiff_orientation(&[], 0);
        assert_eq!(result, None);
    }

    #[test]
    fn test_load_avif() {
        let path = std::path::Path::new("test_images/test.avif");
        if !path.exists() {
            return; // skip if test image not available
        }
        let result = load_image(path);
        assert!(result.is_ok(), "Failed to load AVIF: {:?}", result.err());
        match result.unwrap() {
            LoadedImage::Static(img) => {
                assert_eq!(img.width, 64);
                assert_eq!(img.height, 64);
            }
            _ => panic!("Expected static image"),
        }
    }

    #[test]
    fn test_load_heic() {
        let path = std::path::Path::new("test_images/test.heic");
        if !path.exists() {
            return; // skip if test image not available
        }
        let result = load_image(path);
        assert!(result.is_ok(), "Failed to load HEIC: {:?}", result.err());
        match result.unwrap() {
            LoadedImage::Static(img) => {
                assert_eq!(img.width, 64);
                assert_eq!(img.height, 64);
            }
            _ => panic!("Expected static image"),
        }
    }

    #[test]
    fn test_load_jxl() {
        let path = std::path::Path::new("test_images/test.jxl");
        if !path.exists() {
            return; // skip if test image not available
        }
        let result = load_image(path);
        assert!(result.is_ok(), "Failed to load JXL: {:?}", result.err());
        match result.unwrap() {
            LoadedImage::Static(img) => {
                assert_eq!(img.width, 64);
                assert_eq!(img.height, 64);
            }
            _ => panic!("Expected static image"),
        }
    }

    #[test]
    fn test_supported_extensions_include_new_formats() {
        assert!(is_supported_image(std::path::Path::new("test.avif")));
        assert!(is_supported_image(std::path::Path::new("test.heic")));
        assert!(is_supported_image(std::path::Path::new("test.heif")));
        assert!(is_supported_image(std::path::Path::new("test.jxl")));
        assert!(is_supported_image(std::path::Path::new("test.AVIF")));
        assert!(is_supported_image(std::path::Path::new("test.HEIC")));
        assert!(is_supported_image(std::path::Path::new("test.JXL")));
    }
}
