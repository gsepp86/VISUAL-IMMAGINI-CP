//! Stampa su Windows tramite GDI, senza dipendenze esterne (solo chiamate dirette alle DLL di sistema).
//! Usa la stampante scelta con le sue impostazioni predefinite (formato carta, qualità).

use std::ffi::c_void;
use std::ptr::null;

const HORZRES: i32 = 8;
const VERTRES: i32 = 10;
const LOGPIXELSX: i32 = 88;
const LOGPIXELSY: i32 = 90;
const PRINTER_ENUM_LOCAL: u32 = 2;
const PRINTER_ENUM_CONNECTIONS: u32 = 4;
const DIB_RGB_COLORS: u32 = 0;
const SRCCOPY: u32 = 0x00CC_0020;

#[repr(C)]
struct DocInfoW {
    cb_size: i32,
    doc_name: *const u16,
    output: *const u16,
    datatype: *const u16,
    fw_type: u32,
}

#[repr(C)]
struct BitmapInfoHeader {
    size: u32,
    width: i32,
    height: i32,
    planes: u16,
    bit_count: u16,
    compression: u32,
    size_image: u32,
    x_pels_per_meter: i32,
    y_pels_per_meter: i32,
    clr_used: u32,
    clr_important: u32,
}

#[repr(C)]
struct BitmapInfo {
    header: BitmapInfoHeader,
    colors: [u32; 1],
}

#[repr(C)]
struct PrinterInfo4W {
    name: *mut u16,
    server: *mut u16,
    attributes: u32,
}

#[link(name = "winspool")]
extern "system" {
    fn GetDefaultPrinterW(buffer: *mut u16, size: *mut u32) -> i32;
    fn EnumPrintersW(
        flags: u32,
        name: *const u16,
        level: u32,
        buf: *mut u8,
        cb: u32,
        needed: *mut u32,
        returned: *mut u32,
    ) -> i32;
}

#[link(name = "gdi32")]
extern "system" {
    fn CreateDCW(driver: *const u16, device: *const u16, port: *const u16, devmode: *const c_void) -> isize;
    fn DeleteDC(hdc: isize) -> i32;
    fn GetDeviceCaps(hdc: isize, index: i32) -> i32;
    fn StartDocW(hdc: isize, info: *const DocInfoW) -> i32;
    fn EndDoc(hdc: isize) -> i32;
    fn AbortDoc(hdc: isize) -> i32;
    fn StartPage(hdc: isize) -> i32;
    fn EndPage(hdc: isize) -> i32;
    fn StretchDIBits(
        hdc: isize,
        x_dest: i32,
        y_dest: i32,
        dest_w: i32,
        dest_h: i32,
        x_src: i32,
        y_src: i32,
        src_w: i32,
        src_h: i32,
        bits: *const c_void,
        info: *const BitmapInfo,
        usage: u32,
        rop: u32,
    ) -> i32;
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe fn wide_ptr_to_string(p: *const u16) -> String {
    if p.is_null() {
        return String::new();
    }
    let mut len = 0usize;
    while *p.add(len) != 0 {
        len += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(p, len))
}

/// Elenco delle stampanti installate e nome di quella predefinita.
pub fn list_printers() -> (Vec<String>, Option<String>) {
    let mut names: Vec<String> = Vec::new();
    unsafe {
        let flags = PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS;
        let mut needed: u32 = 0;
        let mut returned: u32 = 0;
        // prima chiamata: serve solo a sapere quanto spazio occorre
        EnumPrintersW(flags, null(), 4, std::ptr::null_mut(), 0, &mut needed, &mut returned);
        if needed > 0 {
            // buffer di u64 per garantire l'allineamento a 8 byte
            let mut buf = vec![0u64; (needed as usize + 7) / 8];
            let ok = EnumPrintersW(
                flags,
                null(),
                4,
                buf.as_mut_ptr() as *mut u8,
                needed,
                &mut needed,
                &mut returned,
            );
            if ok != 0 {
                let infos = std::slice::from_raw_parts(buf.as_ptr() as *const PrinterInfo4W, returned as usize);
                for info in infos {
                    let n = wide_ptr_to_string(info.name);
                    if !n.is_empty() {
                        names.push(n);
                    }
                }
            }
        }
    }
    names.sort_by_key(|n| n.to_lowercase());

    let mut default_name = None;
    unsafe {
        let mut buf = vec![0u16; 512];
        let mut size: u32 = 512;
        if GetDefaultPrinterW(buf.as_mut_ptr(), &mut size) != 0 {
            let len = buf.iter().position(|&c| c == 0).unwrap_or(0);
            let n = String::from_utf16_lossy(&buf[..len]);
            if !n.is_empty() {
                default_name = Some(n);
            }
        }
    }
    (names, default_name)
}

/// Contesto di stampa (HDC) di una stampante. Si chiude da solo quando esce di scope.
pub struct PrinterDc {
    hdc: isize,
}

impl PrinterDc {
    pub fn open(printer: &str) -> Result<Self, String> {
        let driver = wide("WINSPOOL");
        let device = wide(printer);
        let hdc = unsafe { CreateDCW(driver.as_ptr(), device.as_ptr(), null(), null()) };
        if hdc == 0 {
            Err(format!("Impossibile aprire la stampante \"{}\".", printer))
        } else {
            Ok(PrinterDc { hdc })
        }
    }

    /// Area stampabile in pixel del dispositivo.
    pub fn page_px(&self) -> (i32, i32) {
        unsafe { (GetDeviceCaps(self.hdc, HORZRES), GetDeviceCaps(self.hdc, VERTRES)) }
    }

    /// Area stampabile in millimetri (per l'anteprima reale).
    pub fn printable_mm(&self) -> (f32, f32) {
        let (w, h) = self.page_px();
        let (dx, dy) = self.dpi();
        if dx <= 0 || dy <= 0 {
            return (210.0, 297.0);
        }
        (w as f32 / dx as f32 * 25.4, h as f32 / dy as f32 * 25.4)
    }

    /// Risoluzione (dpi) in orizzontale e verticale.
    pub fn dpi(&self) -> (i32, i32) {
        unsafe { (GetDeviceCaps(self.hdc, LOGPIXELSX), GetDeviceCaps(self.hdc, LOGPIXELSY)) }
    }

    pub fn start_doc(&self, title: &str) -> Result<(), String> {
        let name = wide(title);
        let info = DocInfoW {
            cb_size: std::mem::size_of::<DocInfoW>() as i32,
            doc_name: name.as_ptr(),
            output: null(),
            datatype: null(),
            fw_type: 0,
        };
        let r = unsafe { StartDocW(self.hdc, &info) };
        if r > 0 {
            Ok(())
        } else {
            Err("La stampante ha rifiutato il documento (StartDoc).".to_string())
        }
    }

    pub fn start_page(&self) -> Result<(), String> {
        if unsafe { StartPage(self.hdc) } > 0 {
            Ok(())
        } else {
            Err("Impossibile iniziare la pagina (StartPage).".to_string())
        }
    }

    pub fn end_page(&self) -> Result<(), String> {
        if unsafe { EndPage(self.hdc) } > 0 {
            Ok(())
        } else {
            Err("Impossibile chiudere la pagina (EndPage).".to_string())
        }
    }

    pub fn end_doc(&self) {
        unsafe {
            EndDoc(self.hdc);
        }
    }

    pub fn abort_doc(&self) {
        unsafe {
            AbortDoc(self.hdc);
        }
    }

    /// Disegna un'immagine RGB (3 byte per pixel, righe consecutive senza padding)
    /// riempiendo `dest_w` x `dest_h` pixel della pagina a partire dall'angolo in alto a sinistra.
    pub fn draw_rgb(&self, w: i32, h: i32, rgb: &[u8], dest_w: i32, dest_h: i32) -> Result<(), String> {
        if w <= 0 || h <= 0 || rgb.len() < (w as usize) * (h as usize) * 3 {
            return Err("Immagine di stampa non valida.".to_string());
        }
        // GDI vuole BGR con righe allineate a 4 byte
        let stride = ((w as usize) * 3 + 3) & !3;
        let mut bgr = vec![0u8; stride * h as usize];
        for y in 0..h as usize {
            let src = &rgb[y * w as usize * 3..(y + 1) * w as usize * 3];
            let dst = &mut bgr[y * stride..y * stride + w as usize * 3];
            for (d, s) in dst.chunks_exact_mut(3).zip(src.chunks_exact(3)) {
                d[0] = s[2];
                d[1] = s[1];
                d[2] = s[0];
            }
        }
        let info = BitmapInfo {
            header: BitmapInfoHeader {
                size: std::mem::size_of::<BitmapInfoHeader>() as u32,
                width: w,
                height: -h, // negativo = righe dall'alto verso il basso
                planes: 1,
                bit_count: 24,
                compression: 0, // BI_RGB
                size_image: 0,
                x_pels_per_meter: 0,
                y_pels_per_meter: 0,
                clr_used: 0,
                clr_important: 0,
            },
            colors: [0],
        };
        let r = unsafe {
            StretchDIBits(
                self.hdc,
                0,
                0,
                dest_w,
                dest_h,
                0,
                0,
                w,
                h,
                bgr.as_ptr() as *const c_void,
                &info,
                DIB_RGB_COLORS,
                SRCCOPY,
            )
        };
        if r > 0 {
            Ok(())
        } else {
            Err("La stampante non ha accettato l'immagine (StretchDIBits).".to_string())
        }
    }
}

impl Drop for PrinterDc {
    fn drop(&mut self) {
        unsafe {
            DeleteDC(self.hdc);
        }
    }
}
