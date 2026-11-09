//! HEIC e AVIF con i codec di Windows (WIC). Su Windows 10/11 servono le estensioni gratuite
//! "Estensioni immagine HEIF" e "Estensioni immagine AVIF" di Microsoft Store.

use std::cell::Cell;
use std::path::Path;

use windows::core::HSTRING;
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppRGBA, IWICImagingFactory, WICBitmapDitherTypeNone,
    WICBitmapPaletteTypeCustom, WICDecodeMetadataCacheOnDemand,
};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};

/// GENERIC_READ
const GENERIC_READ: u32 = 0x8000_0000;

thread_local! {
    static COM_READY: Cell<bool> = const { Cell::new(false) };
}

/// Ogni thread di decodifica inizializza COM una sola volta.
fn ensure_com() {
    COM_READY.with(|ready| {
        if !ready.get() {
            unsafe {
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            }
            ready.set(true);
        }
    });
}

fn err<E: std::fmt::Display>(e: E) -> String {
    format!("{e}")
}

/// Decodifica il file in RGBA (8 bit per canale). Restituisce (larghezza, altezza, pixel).
pub fn decode(path: &Path) -> Result<(u32, u32, Vec<u8>), String> {
    ensure_com();
    unsafe {
        let factory: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).map_err(err)?;
        let name = HSTRING::from(path.to_string_lossy().as_ref());
        let decoder = factory
            .CreateDecoderFromFilename(&name, None, GENERIC_READ, WICDecodeMetadataCacheOnDemand)
            .map_err(err)?;
        let frame = decoder.GetFrame(0).map_err(err)?;
        let (mut w, mut h) = (0u32, 0u32);
        frame.GetSize(&mut w, &mut h).map_err(err)?;
        let converter = factory.CreateFormatConverter().map_err(err)?;
        converter
            .Initialize(
                &frame,
                &GUID_WICPixelFormat32bppRGBA,
                WICBitmapDitherTypeNone,
                None,
                0.0,
                WICBitmapPaletteTypeCustom,
            )
            .map_err(err)?;
        let stride = w * 4;
        let mut buf = vec![0u8; (stride as usize) * (h as usize)];
        converter.CopyPixels(std::ptr::null(), stride, &mut buf).map_err(err)?;
        Ok((w, h, buf))
    }
}
