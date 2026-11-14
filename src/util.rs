//! Logica pura (senza dipendenze esterne): estensioni supportate, ordinamento
//! "naturale" dei nomi file e calcolo delle griglie di stampa N-in-1.
//! Questo modulo ha test automatici: `cargo test`.

use std::cmp::Ordering;
use std::path::{Path, PathBuf};

/// Estensioni riconosciute (minuscole).
pub const EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "jpe", "jfif", "png", "gif", "bmp", "dib", "ico", "tif", "tiff", "webp", "tga",
    "dds", "hdr", "exr", "pnm", "pbm", "pgm", "ppm", "pam", "qoi", "ff", "svg",
    // formati decodificati da Windows (WIC)
    "heic", "heif", "hif", "avif", "jxl",
    // foto RAW (si usa la JPEG di anteprima incorporata)
    "cr2", "cr3", "crw", "nef", "nrw", "arw", "srf", "sr2", "dng", "orf", "rw2", "raf", "pef", "srw",
    "3fr", "erf", "kdc", "mrw", "rwl", "iiq",
];

/// Formati che si aprono solo con i codec di Windows (WIC).
pub const WIC_EXTENSIONS: &[&str] = &["heic", "heif", "hif", "avif", "jxl"];

/// Foto RAW.
pub const RAW_EXTENSIONS: &[&str] = &[
    "cr2", "cr3", "crw", "nef", "nrw", "arw", "srf", "sr2", "dng", "orf", "rw2", "raf", "pef", "srw",
    "3fr", "erf", "kdc", "mrw", "rwl", "iiq",
];

pub fn extension_lower(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default()
}

pub fn is_supported(path: &Path) -> bool {
    let ext = extension_lower(path);
    EXTENSIONS.contains(&ext.as_str())
}

/// Cerca in un file RAW la JPEG incorporata più grande (l'anteprima a piena risoluzione).
/// Restituisce (inizio, fine) dei byte. Non decodifica nulla: segue solo la struttura dei marcatori JPEG.
pub fn find_largest_jpeg(d: &[u8]) -> Option<(usize, usize)> {
    let mut best: Option<(usize, usize)> = None;
    let mut i = 0usize;
    while i + 3 < d.len() {
        if d[i] == 0xFF && d[i + 1] == 0xD8 && d[i + 2] == 0xFF {
            if let Some(end) = jpeg_end(d, i) {
                if best.map_or(true, |(s, e)| end - i > e - s) {
                    best = Some((i, end));
                }
                i = end;
                continue;
            }
        }
        i += 1;
    }
    best
}

/// Fine (esclusa) di una JPEG che inizia in `start`, oppure None se la struttura non è valida.
fn jpeg_end(d: &[u8], start: usize) -> Option<usize> {
    let mut p = start + 2;
    loop {
        if p + 2 > d.len() || d[p] != 0xFF {
            return None;
        }
        let m = d[p + 1];
        if m == 0xFF {
            p += 1; // byte di riempimento
            continue;
        }
        if m == 0xD9 {
            return Some(p + 2);
        }
        if m == 0x01 || m == 0xD8 || (0xD0..=0xD7).contains(&m) {
            p += 2; // marcatori senza lunghezza
            continue;
        }
        if p + 4 > d.len() {
            return None;
        }
        let len = ((d[p + 2] as usize) << 8) | d[p + 3] as usize;
        if len < 2 {
            return None;
        }
        p += 2 + len;
        if m == 0xDA {
            // dati compressi: si procede fino al prossimo marcatore vero
            loop {
                if p + 1 >= d.len() {
                    return None;
                }
                if d[p] == 0xFF {
                    let n = d[p + 1];
                    if n == 0x00 || (0xD0..=0xD7).contains(&n) {
                        p += 2;
                        continue;
                    }
                    if n == 0xFF {
                        p += 1;
                        continue;
                    }
                    break; // EOI o altro segmento: lo gestisce il ciclo esterno
                }
                p += 1;
            }
        }
    }
}

/// Confronto "naturale" come in Esplora risorse: img2 < img10, senza badare alle maiuscole.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (mut i, mut j) = (0usize, 0usize);
    while i < a.len() && j < b.len() {
        if a[i].is_ascii_digit() && b[j].is_ascii_digit() {
            let si = i;
            while i < a.len() && a[i].is_ascii_digit() {
                i += 1;
            }
            let sj = j;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            let na: String = a[si..i].iter().collect();
            let nb: String = b[sj..j].iter().collect();
            let na = na.trim_start_matches('0');
            let nb = nb.trim_start_matches('0');
            let ord = na.len().cmp(&nb.len()).then_with(|| na.cmp(nb));
            if ord != Ordering::Equal {
                return ord;
            }
        } else {
            let ca = a[i].to_lowercase().next().unwrap_or(a[i]);
            let cb = b[j].to_lowercase().next().unwrap_or(b[j]);
            if ca != cb {
                return ca.cmp(&cb);
            }
            i += 1;
            j += 1;
        }
    }
    (a.len() - i).cmp(&(b.len() - j))
}

/// Elenca le immagini supportate nella cartella di `path` (ordine naturale) e
/// restituisce anche la posizione di `path` nell'elenco.
pub fn list_siblings(path: &Path) -> (Vec<PathBuf>, usize) {
    let mut files: Vec<PathBuf> = Vec::new();
    if let Some(dir) = path.parent() {
        let dir = if dir.as_os_str().is_empty() { Path::new(".") } else { dir };
        if let Ok(rd) = std::fs::read_dir(dir) {
            for entry in rd.flatten() {
                let p = entry.path();
                if p.is_file() && is_supported(&p) {
                    files.push(p);
                }
            }
        }
    }
    files.sort_by(|x, y| {
        let nx = x.file_name().and_then(|s| s.to_str()).unwrap_or("");
        let ny = y.file_name().and_then(|s| s.to_str()).unwrap_or("");
        natural_cmp(nx, ny)
    });
    let idx = files.iter().position(|p| p == path).unwrap_or_else(|| {
        files.push(path.to_path_buf());
        files.len() - 1
    });
    (files, idx)
}

// ---------------------------------------------------------------------------
// Stampa N-in-1
// ---------------------------------------------------------------------------

/// Scelte disponibili per "immagini per pagina".
pub const PER_PAGE_CHOICES: &[usize] = &[1, 2, 4, 6, 8, 9, 12, 16];

/// Griglia (colonne, righe) per `n` immagini per pagina.
/// Pagina verticale: 2 in 1 = 1 colonna x 2 righe, 6 in 1 = 2 x 3, ...
/// Pagina orizzontale: colonne e righe scambiate.
pub fn grid_for(n: usize, landscape: bool) -> (usize, usize) {
    let (c, r) = match n {
        0 | 1 => (1, 1),
        2 => (1, 2),
        4 => (2, 2),
        6 => (2, 3),
        8 => (2, 4),
        9 => (3, 3),
        12 => (3, 4),
        16 => (4, 4),
        _ => {
            let cols = (n as f64).sqrt().ceil() as usize;
            let rows = (n + cols - 1) / cols;
            (cols, rows)
        }
    };
    if landscape {
        (r, c)
    } else {
        (c, r)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RectI {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// Celle della griglia, in ordine di lettura (riga per riga).
/// `margin` = margine della pagina, `gap` = spazio tra le celle (stessa unità di `page_w/h`).
pub fn cell_rects(page_w: i32, page_h: i32, cols: usize, rows: usize, margin: i32, gap: i32) -> Vec<RectI> {
    let cols = cols.max(1) as i32;
    let rows = rows.max(1) as i32;
    let inner_w = (page_w - 2 * margin - gap * (cols - 1)).max(cols);
    let inner_h = (page_h - 2 * margin - gap * (rows - 1)).max(rows);
    let cw = inner_w / cols;
    let ch = inner_h / rows;
    let mut v = Vec::with_capacity((cols * rows) as usize);
    for r in 0..rows {
        for c in 0..cols {
            v.push(RectI { x: margin + c * (cw + gap), y: margin + r * (ch + gap), w: cw, h: ch });
        }
    }
    v
}

/// Rettangolo con le proporzioni dell'immagine, centrato nella cella (adatta senza ritagliare).
pub fn fit_rect(img_w: u32, img_h: u32, cell: RectI) -> RectI {
    if img_w == 0 || img_h == 0 || cell.w <= 0 || cell.h <= 0 {
        return RectI { x: cell.x, y: cell.y, w: 1, h: 1 };
    }
    let sx = cell.w as f64 / img_w as f64;
    let sy = cell.h as f64 / img_h as f64;
    let s = sx.min(sy);
    let w = ((img_w as f64 * s).round() as i32).clamp(1, cell.w);
    let h = ((img_h as f64 * s).round() as i32).clamp(1, cell.h);
    RectI { x: cell.x + (cell.w - w) / 2, y: cell.y + (cell.h - h) / 2, w, h }
}

/// Celle della pagina nell'orientamento della stampante: stessa geometria per stampa e anteprima.
/// `landscape` = foglio composto in orizzontale; se la stampante è girata, le celle vengono ruotate.
pub fn page_cells(cw: i32, ch: i32, per_page: usize, landscape: bool, margin: i32, gap: i32) -> Vec<RectI> {
    let (long, short) = (cw.max(ch), cw.min(ch));
    let (sw, sh) = if landscape { (long, short) } else { (short, long) };
    let (cols, rows) = grid_for(per_page, landscape);
    let cells = cell_rects(sw, sh, cols, rows, margin, gap);
    if (sw > sh) != (cw > ch) {
        // ruotazione oraria del foglio: (x, y, w, h) -> (sh - (y + h), x, h, w)
        cells.into_iter().map(|c| RectI { x: sh - (c.y + c.h), y: c.x, w: c.h, h: c.w }).collect()
    } else {
        cells
    }
}

/// Dove sta una foto nella cella, scegliendo il verso (ruotata o no) che la fa stare più grande.
/// Restituisce (ruotare la foto di un quarto di giro, rettangolo occupato nella cella).
pub fn place_photo(cell: RectI, w: u32, h: u32, photo: Option<(f32, f32)>) -> (bool, RectI) {
    let place = |w: u32, h: u32| -> RectI {
        let slot = match photo {
            Some((tw, th)) => photo_slot(cell, w, h, tw, th),
            None => cell,
        };
        fit_rect(w, h, slot)
    };
    let normal = place(w, h);
    let turned = place(h, w);
    let area = |r: &RectI| r.w as i64 * r.h as i64;
    if area(&turned) > area(&normal) {
        (true, turned)
    } else {
        (false, normal)
    }
}

/// Orientamento automatico del foglio: sceglie orizzontale o verticale in base alla superficie
/// totale che le foto occupano a dimensione reale (o massima) nelle celle.
pub fn auto_landscape(cw: i32, ch: i32, per_page: usize, margin: i32, gap: i32, dims: &[(u32, u32)], photo: Option<(f32, f32)>) -> bool {
    let total = |landscape: bool| -> i64 {
        let cells = page_cells(cw, ch, per_page, landscape, margin, gap);
        dims.iter()
            .zip(cells.iter())
            .map(|(&(w, h), cell)| {
                let (_, r) = place_photo(*cell, w, h, photo);
                r.w as i64 * r.h as i64
            })
            .sum()
    };
    total(true) > total(false)
}

/// Tabella di regolazione chiaro/scuro (-1..1) e contrasto (-1..1). Con 0, 0 l'immagine non cambia.
/// Usata uguale da stampa e anteprima.
pub fn tone_lut(brightness: f32, contrast: f32) -> [u8; 256] {
    let f = 1.0 + contrast.clamp(-1.0, 1.0);
    let add = brightness.clamp(-1.0, 1.0) * 255.0;
    let mut lut = [0u8; 256];
    for (i, v) in lut.iter_mut().enumerate() {
        let x = (i as f32 - 128.0) * f + 128.0 + add;
        *v = x.round().clamp(0.0, 255.0) as u8;
    }
    lut
}

/// Riquadro della foto dentro una cella, alla dimensione reale `tw` x `th` (stessa unità della cella).
/// Il lato lungo della foto va sul lato lungo del formato; se la cella è più piccola la foto
/// si riduce in proporzione (mai deformata).
pub fn photo_slot(cell: RectI, img_w: u32, img_h: u32, tw: f32, th: f32) -> RectI {
    if img_w == 0 || img_h == 0 || tw <= 0.0 || th <= 0.0 || cell.w <= 0 || cell.h <= 0 {
        return cell;
    }
    let (tw, th) = if (img_w > img_h) != (tw > th) { (th, tw) } else { (tw, th) };
    let s = (cell.w as f32 / tw).min(cell.h as f32 / th).min(1.0);
    let w = ((tw * s).round() as i32).clamp(1, cell.w);
    let h = ((th * s).round() as i32).clamp(1, cell.h);
    RectI { x: cell.x + (cell.w - w) / 2, y: cell.y + (cell.h - h) / 2, w, h }
}

/// Quante pagine servono per `total` immagini con `per_page` per pagina.
pub fn page_count(total: usize, per_page: usize) -> usize {
    let p = per_page.max(1);
    (total + p - 1) / p
}

/// Posizioni occupate in ogni foglio. Con più copie della stessa foto il foglio ha una posizione per copia.
pub fn cells_per_page(per_page: usize, repeat: usize) -> usize {
    if repeat > 1 {
        repeat
    } else {
        per_page.max(1)
    }
}

/// Fogli di una stampa, con le immagini da mettere in ogni foglio.
/// Con la ripetizione ogni foto ha un foglio con `repeat` copie; altrimenti N immagini per foglio.
pub fn sheet_pages<T: Clone>(items: &[T], per_page: usize, repeat: usize) -> Vec<Vec<T>> {
    if repeat > 1 {
        items.iter().map(|it| vec![it.clone(); repeat]).collect()
    } else {
        items.chunks(per_page.max(1)).map(|c| c.to_vec()).collect()
    }
}

/// Numero di fogli per `total` immagini (vedi `sheet_pages`).
pub fn sheet_count(total: usize, per_page: usize, repeat: usize) -> usize {
    if repeat > 1 {
        total
    } else {
        page_count(total, per_page)
    }
}

pub fn human_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    let b = bytes as f64;
    if b < KB {
        format!("{} B", bytes)
    } else if b < KB * KB {
        format!("{:.0} KB", b / KB)
    } else if b < KB * KB * KB {
        format!("{:.1} MB", b / (KB * KB))
    } else {
        format!("{:.2} GB", b / (KB * KB * KB))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_order() {
        let mut v = vec!["img10.jpg", "IMG2.jpg", "img1.jpg", "img02.jpg", "a.jpg"];
        v.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(v, vec!["a.jpg", "img1.jpg", "IMG2.jpg", "img02.jpg", "img10.jpg"]);
        assert_eq!(natural_cmp("a", "a"), Ordering::Equal);
        assert_eq!(natural_cmp("a", "ab"), Ordering::Less);
        assert_eq!(natural_cmp("foto 9", "foto 10"), Ordering::Less);
    }

    #[test]
    fn supported_extensions() {
        assert!(is_supported(Path::new("C:\\foto\\a.JPG")));
        assert!(is_supported(Path::new("x.webp")));
        assert!(is_supported(Path::new("logo.svg")));
        assert!(!is_supported(Path::new("note.txt")));
        assert!(!is_supported(Path::new("senza_estensione")));
    }

    #[test]
    fn raw_and_wic_are_supported() {
        for e in ["cr2", "NEF", "dng", "heic", "avif"] {
            assert!(is_supported(Path::new(&format!("foto.{e}"))), "{e}");
        }
        for e in WIC_EXTENSIONS.iter().chain(RAW_EXTENSIONS.iter()) {
            assert!(EXTENSIONS.contains(e), "{e} manca da EXTENSIONS");
        }
    }

    #[test]
    fn finds_embedded_jpeg() {
        // JPEG finta: SOI, APP0 (len 4), SOS (len 3), dati con FF00, EOI
        let jpg = |n: usize| -> Vec<u8> {
            let mut v = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0xAA, 0xBB, 0xFF, 0xDA, 0x00, 0x03, 0x01];
            v.extend(std::iter::repeat(0x11).take(n));
            v.extend_from_slice(&[0xFF, 0x00, 0x22, 0xFF, 0xD9]);
            v
        };
        let small = jpg(10);
        let big = jpg(500);
        let mut file = vec![0x49, 0x49, 0x2A, 0x00, 1, 2, 3];
        let s0 = file.len();
        file.extend(&small);
        file.extend([9, 9, 9]);
        let b0 = file.len();
        file.extend(&big);
        file.extend([7, 7]);
        assert_eq!(find_largest_jpeg(&file), Some((b0, b0 + big.len())));
        assert_eq!(find_largest_jpeg(&file[..b0]), Some((s0, s0 + small.len())));
        assert_eq!(find_largest_jpeg(&[1, 2, 3, 4, 5]), None);
        // troncata: nessun risultato, niente panico
        assert_eq!(find_largest_jpeg(&big[..100]), None);
    }

    #[test]
    fn grids() {
        assert_eq!(grid_for(1, false), (1, 1));
        assert_eq!(grid_for(2, false), (1, 2));
        assert_eq!(grid_for(4, false), (2, 2));
        assert_eq!(grid_for(6, false), (2, 3));
        assert_eq!(grid_for(6, true), (3, 2));
        assert_eq!(grid_for(9, false), (3, 3));
        assert_eq!(grid_for(16, false), (4, 4));
        // ogni scelta offre almeno n celle
        for &n in PER_PAGE_CHOICES {
            let (c, r) = grid_for(n, false);
            assert!(c * r >= n, "griglia troppo piccola per {}", n);
            assert_eq!(c * r, n, "griglia non esatta per {}", n);
        }
    }

    #[test]
    fn cells_stay_inside_page() {
        for &n in PER_PAGE_CHOICES {
            for landscape in [false, true] {
                let (c, r) = grid_for(n, landscape);
                let cells = cell_rects(2480, 3508, c, r, 118, 59);
                assert_eq!(cells.len(), n);
                for (i, a) in cells.iter().enumerate() {
                    assert!(a.x >= 118 && a.y >= 118);
                    assert!(a.x + a.w <= 2480 - 118 + 1 && a.y + a.h <= 3508 - 118 + 1);
                    for b in cells.iter().skip(i + 1) {
                        let overlap = a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
                        assert!(!overlap, "celle sovrapposte con n={}", n);
                    }
                }
            }
        }
    }

    #[test]
    fn reading_order() {
        let cells = cell_rects(1000, 1000, 2, 2, 0, 0);
        assert_eq!((cells[0].x, cells[0].y), (0, 0));
        assert_eq!((cells[1].x, cells[1].y), (500, 0));
        assert_eq!((cells[2].x, cells[2].y), (0, 500));
    }

    #[test]
    fn fit_keeps_aspect_and_centers() {
        let cell = RectI { x: 100, y: 100, w: 400, h: 300 };
        let r = fit_rect(2000, 1000, cell); // 2:1 in cella 4:3 -> larga quanto la cella
        assert_eq!((r.w, r.h), (400, 200));
        assert_eq!((r.x, r.y), (100, 150));
        let r = fit_rect(1000, 2000, cell); // verticale -> alta quanto la cella
        assert_eq!((r.w, r.h), (150, 300));
        assert_eq!(r.x, 100 + (400 - 150) / 2);
        // immagini degeneri non mandano in panico
        let r = fit_rect(0, 0, cell);
        assert!(r.w >= 1 && r.h >= 1);
    }

    #[test]
    fn tone_is_identity_at_zero() {
        let lut = tone_lut(0.0, 0.0);
        for i in 0..256 {
            assert_eq!(lut[i] as usize, i);
        }
        assert!(tone_lut(1.0, 0.0)[0] == 255);
        assert!(tone_lut(-1.0, 0.0)[255] == 0);
    }

    #[test]
    fn place_photo_turns_landscape_in_tall_cell() {
        let cell = RectI { x: 0, y: 0, w: 100, h: 300 };
        let (turn, r) = place_photo(cell, 300, 200, None);
        assert!(turn);
        assert_eq!((r.w, r.h), (100, 150));
        // una foto verticale nella stessa cella resta com'è
        let (turn, _) = place_photo(cell, 200, 300, None);
        assert!(!turn);
    }

    #[test]
    fn page_cells_fit_inside_both_orientations() {
        for &n in PER_PAGE_CHOICES {
            for landscape in [false, true] {
                for (cw, ch) in [(2480, 3508), (3508, 2480)] {
                    let cells = page_cells(cw, ch, n, landscape, 118, 59);
                    assert_eq!(cells.len(), n);
                    for (i, a) in cells.iter().enumerate() {
                        assert!(a.x >= 0 && a.y >= 0 && a.x + a.w <= cw && a.y + a.h <= ch, "fuori pagina n={n}");
                        for b in cells.iter().skip(i + 1) {
                            let overlap = a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
                            assert!(!overlap, "celle sovrapposte n={n}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn photo_slot_keeps_real_size_or_scales() {
        // 15x10 cm (1500x1000 unità) in una cella grande: dimensione reale, nessuna riduzione
        let big = RectI { x: 0, y: 0, w: 2000, h: 2000 };
        let r = photo_slot(big, 3000, 2000, 1000.0, 1500.0);
        assert_eq!((r.w, r.h), (1500, 1000));
        // cella piccola: riduzione in proporzione, proporzioni 3:2 mantenute
        let small = RectI { x: 0, y: 0, w: 1000, h: 1000 };
        let r = photo_slot(small, 3000, 2000, 1000.0, 1500.0);
        assert!(r.w <= 1000 && r.h <= 1000);
        assert!((r.w as f32 / r.h as f32 - 1.5).abs() < 0.01);
        assert_eq!(r.x, 0);
    }

    #[test]
    fn pages_and_sizes() {
        assert_eq!(page_count(0, 4), 0);
        assert_eq!(page_count(1, 4), 1);
        assert_eq!(page_count(4, 4), 1);
        assert_eq!(page_count(5, 4), 2);
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(2048), "2 KB");
        assert_eq!(human_size(3 * 1024 * 1024), "3.0 MB");
    }
}

/// Valore di un campo stringa in un JSON semplice, es. "tag_name":"v2.7.0".
pub fn json_string_field(body: &str, key: &str) -> Option<String> {
    let pat = format!("\"{}\"", key);
    let rest = &body[body.find(&pat)? + pat.len()..];
    let rest = rest.trim_start().strip_prefix(':')?.trim_start().strip_prefix('"')?;
    Some(rest[..rest.find('"')?].to_string())
}

/// Numeri di versione di una stringa come "v2.6.0" o "2.6". None se non è una versione.
pub fn parse_version(v: &str) -> Option<Vec<u64>> {
    let v = v.trim().trim_start_matches(|c| c == 'v' || c == 'V');
    let parts: Option<Vec<u64>> = v.split('.').map(|p| p.trim().parse().ok()).collect();
    parts.filter(|p| !p.is_empty())
}

/// true se la versione remota è più recente di quella locale.
pub fn is_newer_version(remote: &str, local: &str) -> bool {
    let (Some(mut r), Some(mut l)) = (parse_version(remote), parse_version(local)) else {
        return false;
    };
    let n = r.len().max(l.len()).max(3);
    r.resize(n, 0);
    l.resize(n, 0);
    r > l
}

#[cfg(test)]
mod update_tests {
    use super::*;

    #[test]
    fn legge_il_tag_della_release() {
        let body = r#"{"url":"x","tag_name":"v2.7.0","name":"Versione 2.7.0"}"#;
        assert_eq!(json_string_field(body, "tag_name").as_deref(), Some("v2.7.0"));
        let spaced = "{\n  \"tag_name\": \"v2.6.1\"\n}";
        assert_eq!(json_string_field(spaced, "tag_name").as_deref(), Some("v2.6.1"));
        assert_eq!(json_string_field("{}", "tag_name"), None);
    }

    #[test]
    fn confronta_le_versioni() {
        assert!(is_newer_version("v2.7.0", "2.6.0"));
        assert!(is_newer_version("v3.0", "2.6.0"));
        assert!(!is_newer_version("v2.6.0", "2.6.0"));
        assert!(!is_newer_version("v2.5.9", "2.6.0"));
        assert!(is_newer_version("v2.6.0.1", "2.6.0"));
        assert!(!is_newer_version("beta", "2.6.0"));
    }
}

#[cfg(test)]
mod repeat_tests {
    use super::*;

    #[test]
    fn ripetizione_una_pagina_per_foto() {
        let items = vec![1, 2, 3];
        let pages = sheet_pages(&items, 4, 4);
        assert_eq!(pages.len(), 3);
        assert!(pages.iter().all(|p| p.len() == 4));
        assert_eq!(pages[1], vec![2, 2, 2, 2]);
        assert_eq!(sheet_count(3, 4, 4), 3);
        assert_eq!(cells_per_page(1, 4), 4);
    }

    #[test]
    fn senza_ripetizione_si_raggruppa_per_pagina() {
        let items: Vec<u32> = (0..10).collect();
        let pages = sheet_pages(&items, 4, 1);
        assert_eq!(pages.len(), 3);
        assert_eq!(pages[2], vec![8, 9]);
        assert_eq!(sheet_count(10, 4, 1), 3);
        assert_eq!(cells_per_page(4, 1), 4);
        assert_eq!(cells_per_page(0, 1), 1);
    }

    #[test]
    fn ripetizione_celle_dentro_il_foglio() {
        for rep in 2..=16usize {
            let cells = cell_rects(2100, 2970, grid_for(rep, false).0, grid_for(rep, false).1, 50, 30);
            assert!(cells.len() >= rep);
        }
    }
}
