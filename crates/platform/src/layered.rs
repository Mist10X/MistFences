//! Per-pixel-alpha layered windows (`UpdateLayeredWindow`), used for the fence shadow.

use crate::bindings::*;
use windows_core::{Error, Result};

/// A premultiplied BGRA image in a GDI DIB section (selected into its own memory DC), ready
/// to present to a `WS_EX_LAYERED` window. After presenting, the window keeps the content:
/// drop the image and use [`set_alpha`] for fades. The GDI objects are released on drop.
pub struct LayeredImage {
    mem: HDC,
    dib: HBITMAP,
    old: HGDIOBJ,
    /// The DIB section's pixels (owned by `dib`).
    bits: *mut u8,
    width: i32,
    height: i32,
}

impl LayeredImage {
    /// Uploads a `w` x `h` premultiplied BGRA image (`bgra.len()` must be `w * h * 4`).
    pub fn new(w: i32, h: i32, bgra: &[u8]) -> Result<Self> {
        if w <= 0 || h <= 0 || bgra.len() != (w * h * 4) as usize {
            return Err(Error::from_hresult(E_INVALIDARG));
        }
        // SAFETY: the DIB section's bits are written before use and the buffer size matches
        // the header; the DC and bitmap created here are owned by the returned value and
        // released in `Drop` (or right here on failure).
        unsafe {
            let screen = GetDC(None);
            let mem = CreateCompatibleDC(Some(screen));
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w,
                    biHeight: -h, // top-down
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB as u32,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits: *mut core::ffi::c_void = core::ptr::null_mut();
            let dib = CreateDIBSection(
                Some(screen),
                &info,
                DIB_RGB_COLORS as u32,
                &mut bits,
                None,
                0,
            );
            let _ = ReleaseDC(None, screen);
            if dib.0.is_null() || bits.is_null() {
                let err = Error::from_thread();
                let _ = DeleteDC(mem);
                return Err(err);
            }
            core::ptr::copy_nonoverlapping(bgra.as_ptr(), bits.cast::<u8>(), bgra.len());
            let old = SelectObject(mem, HGDIOBJ(dib.0));
            Ok(Self {
                mem,
                dib,
                old,
                bits: bits.cast::<u8>(),
                width: w,
                height: h,
            })
        }
    }

    /// Pixel size of the image.
    pub fn size(&self) -> (i32, i32) {
        (self.width, self.height)
    }

    /// Presents the image to `hwnd` at (`x`, `y`) (screen pixels), sizing the window to the
    /// image. `alpha` is the constant alpha multiplied into every pixel
    /// (`SourceConstantAlpha`): 255 shows the image as is, smaller values fade the whole window
    /// without re-rendering it.
    pub fn present(&self, hwnd: HWND, x: i32, y: i32, alpha: u8) -> Result<()> {
        // SAFETY: the memory DC still has the DIB selected (both live as long as `self`); the
        // screen DC is released before returning.
        unsafe {
            let screen = GetDC(None);
            let dst = POINT { x, y };
            let size = SIZE {
                cx: self.width,
                cy: self.height,
            };
            let src = POINT { x: 0, y: 0 };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: alpha,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            let ok = UpdateLayeredWindow(
                hwnd,
                Some(screen),
                Some(&dst),
                Some(&size),
                Some(self.mem),
                Some(&src),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA as u32,
            );
            let _ = ReleaseDC(None, screen);
            ok.ok()
        }
    }
}

/// Changes only the constant alpha of a layered window whose content was presented before.
/// The system keeps that content, so the source bitmap may already be gone
/// (`UpdateLayeredWindow` with a NULL source DC: "the shape and visual context of the window
/// will not change").
pub fn set_alpha(hwnd: HWND, alpha: u8) -> Result<()> {
    let blend = BLENDFUNCTION {
        BlendOp: AC_SRC_OVER as u8,
        BlendFlags: 0,
        SourceConstantAlpha: alpha,
        AlphaFormat: AC_SRC_ALPHA as u8,
    };
    // SAFETY: plain call; every optional pointer is NULL except the blend on the stack.
    unsafe {
        UpdateLayeredWindow(
            hwnd,
            None,
            None,
            None,
            None,
            None,
            COLORREF(0),
            Some(&blend),
            ULW_ALPHA as u32,
        )
        .ok()
    }
}

impl Drop for LayeredImage {
    fn drop(&mut self) {
        // SAFETY: the objects were created in `new` and are released exactly once here.
        unsafe {
            let _ = SelectObject(self.mem, self.old);
            let _ = DeleteObject(HGDIOBJ(self.dib.0));
            let _ = DeleteDC(self.mem);
        }
    }
}

#[repr(C)]
struct UpdateLayeredWindowInfo {
    cb_size: u32,
    hdc_dst: HDC,
    ppt_dst: *const POINT,
    psize: *const SIZE,
    hdc_src: HDC,
    ppt_src: *const POINT,
    cr_key: COLORREF,
    pblend: *const BLENDFUNCTION,
    dw_flags: u32,
    prc_dirty: *const RECT,
}

windows_core::link!("user32.dll" "system" fn UpdateLayeredWindowIndirect(hwnd: HWND, info: *const UpdateLayeredWindowInfo) -> windows_core::BOOL);

impl LayeredImage {
    /// A transparent `w` x `h` image to draw into ([`Self::pixels_mut`]) and present again and
    /// again ([`Self::present_dirty`]): the desktop icon layer keeps one per window and redraws
    /// only the cells that changed.
    pub fn blank(w: i32, h: i32) -> Result<Self> {
        Self::new(w, h, &vec![0u8; (w.max(0) * h.max(0) * 4) as usize])
    }

    /// The premultiplied BGRA pixels, top-down, `width * 4` bytes a row.
    pub fn pixels_mut(&mut self) -> &mut [u8] {
        // SAFETY: the DIB section of `width` x `height` 32-bit pixels lives as long as `self`;
        // `&mut self` keeps GDI from reading it meanwhile (presenting borrows `self`).
        unsafe {
            core::slice::from_raw_parts_mut(self.bits, (self.width * self.height * 4) as usize)
        }
    }

    /// Presents the image at (`x`, `y`) like [`Self::present`], telling the system that only
    /// `dirty` (image pixels) changed since the last present, so only that part is recomposed.
    /// `None` = everything.
    pub fn present_dirty(&self, hwnd: HWND, x: i32, y: i32, dirty: Option<RECT>) -> Result<()> {
        let dst = POINT { x, y };
        let size = SIZE {
            cx: self.width,
            cy: self.height,
        };
        let src = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        // SAFETY: GdiFlush makes our writes to the DIB visible to GDI; every pointer in the info
        // block points at a local that outlives the call; the DC has the DIB selected.
        unsafe {
            let _ = GdiFlush();
            let screen = GetDC(None);
            let info = UpdateLayeredWindowInfo {
                cb_size: size_of::<UpdateLayeredWindowInfo>() as u32,
                hdc_dst: screen,
                ppt_dst: &dst,
                psize: &size,
                hdc_src: self.mem,
                ppt_src: &src,
                cr_key: COLORREF(0),
                pblend: &blend,
                dw_flags: ULW_ALPHA as u32,
                prc_dirty: dirty
                    .as_ref()
                    .map_or(core::ptr::null(), |r| r as *const RECT),
            };
            let ok = UpdateLayeredWindowIndirect(hwnd, &info);
            let _ = ReleaseDC(None, screen);
            ok.ok()
        }
    }
}
