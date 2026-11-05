//! Caricamento delle immagini in background.
//!
//! La decodifica avviene su più thread, così l'interfaccia non si blocca mai:
//! - per ogni immagine grande viene prima prodotta una *anteprima* leggera (max 2048 px),
//!   mostrata subito, e poi la versione completa (max 8192 px) per lo zoom;
//! - le miniature della striscia in basso hanno una coda a priorità più bassa;
//! - ogni nuova richiesta sostituisce la coda precedente, così scorrendo velocemente
//!   non si perde tempo a decodificare immagini che non servono più.

use std::collections::{HashSet, VecDeque};
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};

use image::imageops::FilterType;
use image::{DynamicImage, ImageDecoder};

use crate::util;

/// Lato massimo dell'anteprima veloce.
pub const PREVIEW_SIDE: u32 = 2048;
/// Lato massimo della texture completa (limite sicuro per quasi tutte le GPU).
pub const MAX_SIDE: u32 = 8192;
/// Lato massimo delle miniature.
pub const THUMB_SIDE: u32 = 128;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Preview,
    Full,
    Thumb,
}

pub struct Decoded {
    pub path: PathBuf,
    pub kind: Kind,
    /// Dimensioni originali (dopo la rotazione EXIF).
    pub orig: (u32, u32),
    pub image: Result<egui::ColorImage, String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Job {
    /// anteprima leggera (max 2048 px): veloce, per l'immagine corrente e le vicine
    Preview,
    /// versione intera: solo quando serve per lo zoom
    Full,
    /// miniatura della striscia
    Thumb,
}

#[derive(Default)]
struct Queues {
    preview: VecDeque<PathBuf>,
    full: VecDeque<PathBuf>,
    thumb: VecDeque<PathBuf>,
    running: HashSet<(PathBuf, Job)>,
}

type Shared = Arc<(Mutex<Queues>, Condvar)>;

pub struct Loader {
    shared: Shared,
    pub rx: Receiver<Decoded>,
}

impl Loader {
    pub fn new(ctx: egui::Context) -> Loader {
        let shared: Shared = Arc::new((Mutex::new(Queues::default()), Condvar::new()));
        let (tx, rx) = channel::<Decoded>();
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).saturating_sub(1).clamp(2, 6);
        for _ in 0..threads {
            let shared = Arc::clone(&shared);
            let tx = tx.clone();
            let ctx = ctx.clone();
            std::thread::spawn(move || worker_loop(shared, tx, ctx));
        }
        Loader { shared, rx }
    }

    /// Imposta le immagini da caricare per intero (in ordine di priorità). Sostituisce la coda precedente.
    pub fn set_full(&self, wanted: Vec<PathBuf>) {
        let (lock, cv) = &*self.shared;
        let mut q = lock.lock().unwrap();
        let list: VecDeque<PathBuf> = wanted.into_iter().filter(|p| !q.running.contains(&(p.clone(), Job::Full))).collect();
        q.full = list;
        drop(q);
        cv.notify_all();
    }

    /// Imposta le anteprime leggere da preparare (in ordine di priorità). Sostituisce la coda precedente.
    pub fn set_previews(&self, wanted: Vec<PathBuf>) {
        let (lock, cv) = &*self.shared;
        let mut q = lock.lock().unwrap();
        let list: VecDeque<PathBuf> = wanted.into_iter().filter(|p| !q.running.contains(&(p.clone(), Job::Preview))).collect();
        q.preview = list;
        drop(q);
        cv.notify_all();
    }

    /// Imposta le miniature da caricare (in ordine di priorità). Sostituisce la coda precedente.
    pub fn set_thumbs(&self, wanted: Vec<PathBuf>) {
        let (lock, cv) = &*self.shared;
        let mut q = lock.lock().unwrap();
        let list: VecDeque<PathBuf> = wanted.into_iter().filter(|p| !q.running.contains(&(p.clone(), Job::Thumb))).collect();
        q.thumb = list;
        drop(q);
        cv.notify_all();
    }
}

fn worker_loop(shared: Shared, tx: Sender<Decoded>, ctx: egui::Context) {
    loop {
        let (path, job) = {
            let (lock, cv) = &*shared;
            let mut q = lock.lock().unwrap();
            loop {
                if let Some(p) = q.preview.pop_front() {
                    q.running.insert((p.clone(), Job::Preview));
                    break (p, Job::Preview);
                }
                if let Some(p) = q.full.pop_front() {
                    q.running.insert((p.clone(), Job::Full));
                    break (p, Job::Full);
                }
                if let Some(p) = q.thumb.pop_front() {
                    q.running.insert((p.clone(), Job::Thumb));
                    break (p, Job::Thumb);
                }
                q = cv.wait(q).unwrap();
            }
        };

        let alive = process(&path, job, &tx, &ctx);

        shared.0.lock().unwrap().running.remove(&(path, job));
        ctx.request_repaint();
        if !alive {
            return; // l'app è stata chiusa
        }
    }
}

/// Decodifica un file e invia i risultati. Restituisce false se il canale è chiuso.
fn process(path: &Path, job: Job, tx: &Sender<Decoded>, ctx: &egui::Context) -> bool {
    let decoded = std::panic::catch_unwind(AssertUnwindSafe(|| decode(path)))
        .unwrap_or_else(|_| Err("Errore interno durante la decodifica".to_string()));

    let img = match decoded {
        Ok(img) => img,
        Err(e) => {
            let kind = if job == Job::Thumb { Kind::Thumb } else { Kind::Full };
            return tx.send(Decoded { path: path.to_path_buf(), kind, orig: (0, 0), image: Err(e) }).is_ok();
        }
    };
    let orig = (img.width(), img.height());
    let send = |kind: Kind, image: egui::ColorImage| {
        tx.send(Decoded { path: path.to_path_buf(), kind, orig, image: Ok(image) }).is_ok()
    };

    if job == Job::Preview {
        // solo anteprima: niente versione intera, così le immagini vicine costano poco
        let longest = orig.0.max(orig.1);
        let small = if longest > PREVIEW_SIDE { img.resize(PREVIEW_SIDE, PREVIEW_SIDE, FilterType::Triangle) } else { img };
        return send(Kind::Preview, to_color_image(&small));
    }

    if job == Job::Thumb {
        let t = img.thumbnail(THUMB_SIDE, THUMB_SIDE);
        return send(Kind::Thumb, to_color_image(&t));
    }

    let longest = orig.0.max(orig.1);
    if longest > PREVIEW_SIDE {
        let preview = img.resize(PREVIEW_SIDE, PREVIEW_SIDE, FilterType::Triangle);
        if !send(Kind::Preview, to_color_image(&preview)) {
            return false;
        }
        ctx.request_repaint();
        let full = if longest > MAX_SIDE { img.resize(MAX_SIDE, MAX_SIDE, FilterType::Triangle) } else { img };
        send(Kind::Full, to_color_image(&full))
    } else {
        send(Kind::Full, to_color_image(&img))
    }
}

fn to_color_image(img: &DynamicImage) -> egui::ColorImage {
    let rgba = img.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw())
}

/// Decodifica un'immagine applicando la rotazione EXIF. Usata anche dalla stampa.
pub fn decode(path: &Path) -> Result<DynamicImage, String> {
    let ext = util::extension_lower(path);
    if ext == "svg" {
        return decode_svg(path);
    }
    if util::RAW_EXTENSIONS.contains(&ext.as_str()) {
        return decode_raw(path);
    }
    if ext == "jxl" {
        return decode_jxl(path);
    }
    if util::WIC_EXTENSIONS.contains(&ext.as_str()) {
        // HEIC/AVIF: si useranno i codec di Windows (WIC), ancora da collegare
        return decode_wic(path);
    }
    let mut reader = image::ImageReader::open(path)
        .map_err(|e| format!("{e}"))?
        .with_guessed_format()
        .map_err(|e| format!("{e}"))?;
    reader.no_limits();
    decode_reader(reader)
}

/// Foto RAW: si estrae la JPEG di anteprima incorporata (la più grande) e si decodifica quella.
/// Funziona con quasi tutte le fotocamere; non si fa demosaicing, quindi nessun calcolo pesante.
fn decode_raw(path: &Path) -> Result<DynamicImage, String> {
    let data = std::fs::read(path).map_err(|e| format!("{e}"))?;
    let (s, e) = util::find_largest_jpeg(&data)
        .ok_or_else(|| "Questo RAW non contiene un'anteprima JPEG leggibile.".to_string())?;
    let mut reader = image::ImageReader::new(std::io::Cursor::new(&data[s..e]))
        .with_guessed_format()
        .map_err(|e| format!("{e}"))?;
    reader.no_limits();
    decode_reader(reader)
}

fn decode_reader<R: std::io::BufRead + std::io::Seek>(reader: image::ImageReader<R>) -> Result<DynamicImage, String> {
    let mut decoder = reader.into_decoder().map_err(|e| format!("{e}"))?;
    let orientation = decoder.orientation().ok();
    let mut img = DynamicImage::from_decoder(decoder).map_err(|e| format!("{e}"))?;
    if let Some(o) = orientation {
        img.apply_orientation(o);
    }
    Ok(img)
}

#[cfg(feature = "svg")]
fn decode_svg(path: &Path) -> Result<DynamicImage, String> {
    let data = std::fs::read(path).map_err(|e| format!("{e}"))?;
    let opt = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_data(&data, &opt).map_err(|e| format!("{e}"))?;
    let size = tree.size();
    let (w, h) = (size.width(), size.height());
    if !(w > 0.0 && h > 0.0) {
        return Err("SVG senza dimensioni".to_string());
    }
    // rasterizza con il lato lungo a 2048 px
    let scale = 2048.0 / w.max(h);
    let pw = ((w * scale).round() as u32).clamp(1, 4096);
    let ph = ((h * scale).round() as u32).clamp(1, 4096);
    let mut pixmap = resvg::tiny_skia::Pixmap::new(pw, ph).ok_or_else(|| "SVG troppo grande".to_string())?;
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    // tiny-skia usa alfa premoltiplicato: convertiamo in alfa normale
    let mut buf: Vec<u8> = Vec::with_capacity((pw * ph * 4) as usize);
    for p in pixmap.pixels() {
        let c = p.demultiply();
        buf.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    let img = image::RgbaImage::from_raw(pw, ph, buf).ok_or_else(|| "SVG non valido".to_string())?;
    Ok(DynamicImage::ImageRgba8(img))
}

#[cfg(not(feature = "svg"))]
fn decode_svg(_path: &Path) -> Result<DynamicImage, String> {
    Err("Supporto SVG non incluso in questa versione".to_string())
}

/// JPEG XL con il decoder Rust puro `jxl-oxide`.
#[cfg(feature = "jxl")]
fn decode_jxl(path: &Path) -> Result<DynamicImage, String> {
    let data = std::fs::read(path).map_err(|e| format!("{e}"))?;
    let image = jxl_oxide::JxlImage::builder()
        .read(std::io::Cursor::new(data))
        .map_err(|e| format!("{e}"))?;
    let render = image.render_frame(0).map_err(|e| format!("{e}"))?;
    let fb = render.image_all_channels();
    let (w, h, ch) = (fb.width() as u32, fb.height() as u32, fb.channels());
    let buf = fb.buf();
    let to8 = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    let mut out: Vec<u8> = Vec::with_capacity((w as usize) * (h as usize) * 4);
    for px in buf.chunks_exact(ch) {
        let (r, g, b, a) = match ch {
            1 => (px[0], px[0], px[0], 1.0),
            2 => (px[0], px[0], px[0], px[1]),
            3 => (px[0], px[1], px[2], 1.0),
            _ => (px[0], px[1], px[2], px[3]),
        };
        out.extend_from_slice(&[to8(r), to8(g), to8(b), to8(a)]);
    }
    image::RgbaImage::from_raw(w, h, out)
        .map(DynamicImage::ImageRgba8)
        .ok_or_else(|| "Immagine JPEG XL non valida".to_string())
}

#[cfg(not(feature = "jxl"))]
fn decode_jxl(_path: &Path) -> Result<DynamicImage, String> {
    Err("Supporto JPEG XL non incluso in questa versione".to_string())
}

/// HEIC / AVIF con i codec di Windows.
#[cfg(all(windows, feature = "wic"))]
fn decode_wic(path: &Path) -> Result<DynamicImage, String> {
    let (w, h, buf) = crate::wic::decode(path).map_err(|e| {
        format!("{e}\nPer HEIC/AVIF installa da Microsoft Store \"Estensioni immagine HEIF\" o \"Estensioni immagine AVIF\".")
    })?;
    image::RgbaImage::from_raw(w, h, buf)
        .map(DynamicImage::ImageRgba8)
        .ok_or_else(|| "Immagine HEIC/AVIF non valida".to_string())
}

#[cfg(not(all(windows, feature = "wic")))]
fn decode_wic(_path: &Path) -> Result<DynamicImage, String> {
    Err("HEIC/AVIF: disponibili solo su Windows con la funzione wic".to_string())
}
