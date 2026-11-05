//! Stampa: composizione delle pagine N-in-1 e invio alla stampante (solo Windows).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;

use image::imageops::{self, FilterType};
use image::{DynamicImage, RgbImage, RgbaImage};

use crate::loader;
use crate::util;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Orientation {
    Auto,
    Portrait,
    Landscape,
}

/// (file, rotazione in quarti di giro orari, ritaglio normalizzato [x0, y0, x1, y1] se presente)
pub type Item = (PathBuf, u8, Option<[f32; 4]>);

pub struct Job {
    pub printer: String,
    pub items: Vec<Item>,
    pub per_page: usize,
    pub orientation: Orientation,
    pub margin_mm: f32,
    pub gap_mm: f32,
    /// formato reale della foto in mm (lato corto, lato lungo); None = adatta alla pagina
    pub photo_mm: Option<(f32, f32)>,
    /// (chiaro/scuro, contrasto), entrambi -1..1
    pub tone: (f32, f32),
}

pub enum Msg {
    Progress { done: usize, total: usize },
    Done,
    Cancelled,
    Error(String),
}

#[cfg(windows)]
pub fn list_printers() -> (Vec<String>, Option<String>) {
    crate::winprint::list_printers()
}

#[cfg(not(windows))]
pub fn list_printers() -> (Vec<String>, Option<String>) {
    (Vec::new(), None)
}

/// Foglio orizzontale o verticale. Con "Automatico" sceglie il verso che fa stare più foto a dimensione migliore.
#[allow(clippy::too_many_arguments)]
pub fn sheet_landscape(
    orientation: Orientation,
    cw: i32,
    ch: i32,
    per_page: usize,
    margin: i32,
    gap: i32,
    dims: &[(u32, u32)],
    photo: Option<(f32, f32)>,
) -> bool {
    match orientation {
        Orientation::Portrait => false,
        Orientation::Landscape => true,
        Orientation::Auto => util::auto_landscape(cw, ch, per_page, margin, gap, dims, photo),
    }
}

/// Area stampabile della stampante in mm (A4 se non disponibile).
#[cfg(windows)]
pub fn printer_page_mm(name: Option<&str>) -> (f32, f32) {
    name.and_then(|n| crate::winprint::PrinterDc::open(n).ok())
        .map(|dc| dc.printable_mm())
        .unwrap_or((210.0, 297.0))
}

#[cfg(not(windows))]
pub fn printer_page_mm(_name: Option<&str>) -> (f32, f32) {
    (210.0, 297.0)
}

/// Esegue la stampa (bloccante: va chiamata in un thread). `notify` serve a svegliare l'interfaccia.
pub fn run(job: Job, cancel: Arc<AtomicBool>, tx: Sender<Msg>, notify: impl Fn()) {
    let msg = match run_inner(&job, &cancel, &tx, &notify) {
        Ok(true) => Msg::Done,
        Ok(false) => Msg::Cancelled,
        Err(e) => Msg::Error(e),
    };
    let _ = tx.send(msg);
    notify();
}

#[cfg(not(windows))]
fn run_inner(_job: &Job, _cancel: &AtomicBool, _tx: &Sender<Msg>, _notify: &dyn Fn()) -> Result<bool, String> {
    Err("La stampa è disponibile solo su Windows.".to_string())
}

#[cfg(windows)]
fn run_inner(job: &Job, cancel: &AtomicBool, tx: &Sender<Msg>, notify: &dyn Fn()) -> Result<bool, String> {
    use crate::winprint::PrinterDc;

    if job.items.is_empty() {
        return Err("Nessuna immagine da stampare.".to_string());
    }
    let dc = PrinterDc::open(&job.printer)?;
    let (pw, ph) = dc.page_px();
    let (dpi_x, _dpi_y) = dc.dpi();
    if pw <= 0 || ph <= 0 || dpi_x <= 0 {
        return Err("La stampante non ha restituito un'area di stampa valida.".to_string());
    }

    // Si compone a max 300 dpi: oltre non serve e si risparmia memoria; la stampante scala il resto.
    let scale = if dpi_x > 300 { 300.0 / dpi_x as f32 } else { 1.0 };
    let cw = ((pw as f32 * scale).round() as i32).max(1);
    let ch = ((ph as f32 * scale).round() as i32).max(1);
    let px_per_mm = dpi_x as f32 * scale / 25.4;
    let margin = (job.margin_mm * px_per_mm).round() as i32;
    let gap = (job.gap_mm * px_per_mm).round() as i32;
    let photo_px = job.photo_mm.map(|(w, h)| (w * px_per_mm, h * px_per_mm));

    let total = job.items.len();
    let per_page = job.per_page.max(1);
    dc.start_doc("Visualizzatore Immagini CP")?;

    let mut done = 0usize;
    for chunk in job.items.chunks(per_page) {
        if cancel.load(Ordering::Relaxed) {
            dc.abort_doc();
            return Ok(false);
        }
        let sheet = match compose_sheet(chunk, cw, ch, per_page, job.orientation, margin, gap, photo_px, job.tone) {
            Ok(s) => s,
            Err(e) => {
                dc.abort_doc();
                return Err(e);
            }
        };
        let page_result = (|| -> Result<(), String> {
            dc.start_page()?;
            dc.draw_rgb(sheet.width() as i32, sheet.height() as i32, sheet.as_raw(), pw, ph)?;
            dc.end_page()
        })();
        if let Err(e) = page_result {
            dc.abort_doc();
            return Err(e);
        }
        done += chunk.len();
        let _ = tx.send(Msg::Progress { done, total });
        notify();
    }
    dc.end_doc();
    Ok(true)
}

/// Compone una pagina intera (RGB) di dimensioni esatte `cw` x `ch`.
/// Le celle senza immagine (ultima pagina o file illeggibili) restano bianche.
#[cfg_attr(not(windows), allow(dead_code))]
fn compose_sheet(
    chunk: &[Item],
    cw: i32,
    ch: i32,
    per_page: usize,
    orientation: Orientation,
    margin: i32,
    gap: i32,
    photo_px: Option<(f32, f32)>,
    tone: (f32, f32),
) -> Result<RgbImage, String> {
    // lato massimo utile: non serve decodificare più della pagina intera
    let max_side = cw.max(ch).max(1) as u32;

    // decodifica a gruppi di 4 in parallelo (limita la memoria con 12 o 16 foto per pagina)
    let mut decoded: Vec<Result<DynamicImage, String>> = Vec::with_capacity(chunk.len());
    for group in chunk.chunks(4) {
        let part: Vec<Result<DynamicImage, String>> = std::thread::scope(|s| {
            let handles: Vec<_> = group
                .iter()
                .map(|(path, rot, crop)| {
                    let path = path.clone();
                    let rot = *rot;
                    let crop = *crop;
                    s.spawn(move || load_for_print(&path, rot, crop, max_side))
                })
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().unwrap_or_else(|_| Err("errore interno".to_string())))
                .collect()
        });
        decoded.extend(part);
    }

    let dims: Vec<(u32, u32)> = decoded.iter().filter_map(|r| r.as_ref().ok()).map(|i| (i.width(), i.height())).collect();
    let landscape = sheet_landscape(orientation, cw, ch, per_page, margin, gap, &dims, photo_px);

    // celle già nell'orientamento della stampante: stessa geometria dell'anteprima
    let cells = util::page_cells(cw, ch, per_page, landscape, margin, gap);
    let lut = util::tone_lut(tone.0, tone.1);
    let mut sheet = RgbaImage::from_pixel(cw as u32, ch as u32, image::Rgba([255, 255, 255, 255]));
    for (img, cell) in decoded.into_iter().zip(cells.iter()) {
        if let Ok(img) = img {
            // ogni foto sceglie il verso che la fa stare più grande nella cella
            let (turn, r) = util::place_photo(*cell, img.width(), img.height(), photo_px);
            let img = if turn { img.rotate90() } else { img };
            let mut small = img.resize_exact(r.w as u32, r.h as u32, FilterType::Lanczos3).to_rgba8();
            for px in small.pixels_mut() {
                px.0[0] = lut[px.0[0] as usize];
                px.0[1] = lut[px.0[1] as usize];
                px.0[2] = lut[px.0[2] as usize];
            }
            imageops::overlay(&mut sheet, &small, r.x as i64, r.y as i64);
        }
    }
    Ok(DynamicImage::ImageRgba8(sheet).to_rgb8())
}

#[cfg_attr(not(windows), allow(dead_code))]
fn load_for_print(path: &std::path::Path, rot: u8, crop: Option<[f32; 4]>, max_side: u32) -> Result<DynamicImage, String> {
    let mut img = loader::decode(path)?;
    img = match rot % 4 {
        1 => img.rotate90(),
        2 => img.rotate180(),
        3 => img.rotate270(),
        _ => img,
    };
    // ritaglio sull'immagine a piena risoluzione, poi riduzione: così i dettagli restano nitidi
    if let Some(c) = crop {
        let (w, h) = (img.width() as f32, img.height() as f32);
        let x0 = (c[0].clamp(0.0, 1.0) * w).floor() as u32;
        let y0 = (c[1].clamp(0.0, 1.0) * h).floor() as u32;
        let x1 = ((c[2].clamp(0.0, 1.0) * w).ceil() as u32).min(img.width());
        let y1 = ((c[3].clamp(0.0, 1.0) * h).ceil() as u32).min(img.height());
        if x1 > x0 && y1 > y0 {
            img = img.crop_imm(x0, y0, x1 - x0, y1 - y0);
        }
    }
    if img.width().max(img.height()) > max_side {
        img = img.resize(max_side, max_side, FilterType::Triangle);
    }
    Ok(img)
}
