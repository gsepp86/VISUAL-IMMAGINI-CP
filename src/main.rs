//! Visualizzatore Immagini CP
//! Lettore immagini opensource creato da Peppe CP in collaborazione con IA.

#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

mod loader;
mod print;
mod print_queue;
#[cfg(all(windows, feature = "wic"))]
mod wic;
mod util;
#[cfg(any(windows, force_win_ffi))]
mod winprint;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;

use egui::{pos2, vec2, Align2, Color32, FontId, Pos2, Rect, Shape, Stroke, Vec2};

const APP_NAME: &str = "Visualizzatore Immagini CP";
const ABOUT_TEXT: &str = "Lettore immagini opensource creato da Peppe CP in collaborazione con IA";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const TITLE_H: f32 = 40.0;
const STRIP_H: f32 = 100.0;
/// Larghezza riservata ai pulsanti a destra nella barra del titolo (strumenti + finestra).
const RIGHT_W: f32 = 7.0 * 32.0 + 16.0 + 3.0 * 46.0 + 24.0;

mod col {
    use egui::Color32;
    use std::sync::atomic::{AtomicBool, Ordering};

    /// tema chiaro attivo (false = scuro, predefinito)
    static LIGHT: AtomicBool = AtomicBool::new(false);

    pub fn set_light(v: bool) {
        LIGHT.store(v, Ordering::Relaxed);
    }
    pub fn is_light() -> bool {
        LIGHT.load(Ordering::Relaxed)
    }
    fn pick(dark: (u8, u8, u8), light: (u8, u8, u8)) -> Color32 {
        let (r, g, b) = if is_light() { light } else { dark };
        Color32::from_rgb(r, g, b)
    }
    pub fn bg() -> Color32 {
        pick((22, 22, 24), (246, 246, 248))
    }
    pub fn bar() -> Color32 {
        pick((34, 35, 38), (226, 228, 233))
    }
    pub fn panel() -> Color32 {
        pick((42, 43, 47), (255, 255, 255))
    }
    pub fn tab_active() -> Color32 {
        pick((64, 66, 73), (255, 255, 255))
    }
    pub fn tab_hover() -> Color32 {
        pick((48, 50, 56), (232, 234, 239))
    }
    pub fn text() -> Color32 {
        pick((235, 236, 240), (28, 29, 33))
    }
    pub fn text_dim() -> Color32 {
        pick((160, 164, 172), (96, 100, 110))
    }
    pub fn accent() -> Color32 {
        pick((56, 160, 255), (0, 103, 192))
    }
    pub fn close_red() -> Color32 {
        Color32::from_rgb(232, 17, 35)
    }
}

// ---------------------------------------------------------------------------
// Dati
// ---------------------------------------------------------------------------

/// Una scheda = una cartella di immagini con un'immagine corrente e il suo stato di zoom.
struct Tab {
    files: Vec<PathBuf>,
    index: usize,
    /// true = adatta alla finestra
    fit: bool,
    /// pixel dell'immagine per pixel dello schermo (1.0 = 100%), valido se `fit` è false
    scale: f32,
    /// spostamento del centro dell'immagine rispetto al centro della vista (in punti)
    pan: Vec2,
}

impl Tab {
    fn current(&self) -> &Path {
        &self.files[self.index]
    }
    fn title(&self) -> String {
        self.current().file_name().and_then(|n| n.to_str()).unwrap_or("Immagine").to_string()
    }
}

/// Texture di un'immagine: anteprima veloce e/o versione completa.
struct ImgTex {
    preview: Option<egui::TextureHandle>,
    full: Option<egui::TextureHandle>,
    orig: (u32, u32),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Scope {
    Current,
    Folder,
    Tabs,
}

struct Running {
    rx: Receiver<print::Msg>,
    cancel: Arc<AtomicBool>,
    done: usize,
    total: usize,
}

struct PrintUi {
    open: bool,
    scope: Scope,
    per_page: usize,
    orientation: print::Orientation,
    margin_mm: f32,
    gap_mm: f32,
    printers: Vec<String>,
    selected: usize,
    running: Option<Running>,
    message: String,
    photo: PhotoFmt,
    /// area stampabile della stampante selezionata (mm), per l'anteprima reale
    page_mm: (f32, f32),
    page_for: Option<String>,
    /// margine minimo non stampabile della stampante selezionata (mm)
    printer_margin_mm: f32,
    brightness: f32,
    contrast: f32,
    /// numero di copie da stampare
    copies: u32,
    /// valori con cui sono state preparate le anteprime
    tone_applied: Option<(f32, f32)>,
}

impl PrintUi {
    fn new() -> Self {
        PrintUi {
            open: false,
            scope: Scope::Current,
            per_page: 1,
            copies: 1,
            orientation: print::Orientation::Auto,
            margin_mm: 10.0,
            gap_mm: 5.0,
            printers: Vec::new(),
            selected: 0,
            running: None,
            message: String::new(),
            photo: PhotoFmt::Page,
            page_mm: (210.0, 297.0),
            page_for: None,
            printer_margin_mm: 5.0,
            brightness: 0.0,
            contrast: 0.0,
            tone_applied: None,
        }
    }
}

/// Azioni richieste dall'interfaccia, eseguite a fine frame.
enum Act {
    Next,
    Prev,
    First,
    Last,
    Goto(usize),
    ActivateTab(usize),
    CloseTab(usize),
    OpenDialog,
    OpenFolder,
    RotateLeft,
    RotateRight,
    Fit,
    Actual,
    ShowPrint(Scope),
    About,
    DefaultApps,
    Fullscreen,
    ToggleCrop,
    SaveCropCopy,
    ShowSettings,
    OpenWith,
    Quit,
}

struct App {
    tabs: Vec<Tab>,
    active: usize,
    loader: loader::Loader,
    full: HashMap<PathBuf, ImgTex>,
    thumbs: HashMap<PathBuf, egui::TextureHandle>,
    bad_thumbs: HashSet<PathBuf>,
    failed: HashMap<PathBuf, String>,
    rotations: HashMap<PathBuf, u8>,
    acts: Vec<Act>,
    show_about: bool,
    show_settings: bool,
    light_theme: bool,
    show_strip: bool,
    print: PrintUi,
    icon_tex: Option<egui::TextureHandle>,
    last_full_req: Vec<PathBuf>,
    last_preview_req: Vec<PathBuf>,
    last_thumb_req: Vec<PathBuf>,
    /// true = l'immagine corrente è ingrandita e serve la versione intera
    want_full: bool,
    /// ritagli per immagine (in orientamento di visualizzazione)
    crops: HashMap<PathBuf, [f32; 4]>,
    crop_mode: bool,
    crop_edit: [f32; 4],
    /// piccole copie delle anteprime (per la stampa con chiaro/scuro in tempo reale)
    small_src: HashMap<PathBuf, egui::ColorImage>,
    /// dialogo "mantenere i ritagli?" alla chiusura
    close_prompt: bool,
    allow_close: bool,
    /// anteprime della stampa già regolate
    print_tex: HashMap<PathBuf, egui::TextureHandle>,
    zoom_text: String,
    meta_cache: (PathBuf, u64),
    last_title: String,
}

// ---------------------------------------------------------------------------
// Avvio
// ---------------------------------------------------------------------------

fn load_window_icon() -> egui::IconData {
    match image::load_from_memory(include_bytes!("../assets/icon.png")) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            egui::IconData { rgba: rgba.into_raw(), width: w, height: h }
        }
        Err(_) => egui::IconData::default(),
    }
}

fn main() -> eframe::Result<()> {
    let mut args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    // "--print": arriva dal menu destro di Esplora risorse con più file selezionati
    let print_mode = args.first().map(|a| a.as_os_str() == "--print").unwrap_or(false);
    if print_mode {
        args.remove(0);
        // ogni foto selezionata arriva in un processo a parte: le raccoglie un solo processo
        let file = match args.first() {
            Some(f) => f.clone(),
            None => return Ok(()),
        };
        args = print_queue::submit_and_collect(&file);
        if args.is_empty() {
            return Ok(());
        }
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(APP_NAME)
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([780.0, 460.0])
            .with_decorations(false)
            .with_icon(load_window_icon())
            .with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native(APP_NAME, options, Box::new(move |cc| Ok(Box::new(App::new(cc, args, print_mode)))))
}

fn setup_style(ctx: &egui::Context) {
    let light = col::is_light();
    let mut v = if light { egui::Visuals::light() } else { egui::Visuals::dark() };
    v.panel_fill = col::bg();
    v.window_fill = col::panel();
    v.extreme_bg_color = if light { Color32::WHITE } else { Color32::from_rgb(30, 30, 33) };
    v.selection.bg_fill = col::accent().gamma_multiply(0.55);
    ctx.set_visuals(v);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = vec2(8.0, 6.0);
        s.spacing.button_padding = vec2(10.0, 5.0);
    });
}

fn uv_full() -> Rect {
    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0))
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>, args: Vec<PathBuf>, print_mode: bool) -> Self {
        col::set_light(load_light_theme());
        setup_style(&cc.egui_ctx);

        let icon_tex = image::load_from_memory(include_bytes!("../assets/icon.png")).ok().map(|img| {
            let rgba = img.to_rgba8();
            let size = [rgba.width() as usize, rgba.height() as usize];
            let ci = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
            cc.egui_ctx.load_texture("app-icon", ci, egui::TextureOptions::LINEAR)
        });

        let mut app = App {
            tabs: Vec::new(),
            active: 0,
            loader: loader::Loader::new(cc.egui_ctx.clone()),
            full: HashMap::new(),
            thumbs: HashMap::new(),
            bad_thumbs: HashSet::new(),
            failed: HashMap::new(),
            rotations: HashMap::new(),
            acts: Vec::new(),
            show_about: false,
            show_settings: false,
            light_theme: col::is_light(),
            show_strip: true,
            print: PrintUi::new(),
            icon_tex,
            last_full_req: Vec::new(),
            last_preview_req: Vec::new(),
            last_thumb_req: Vec::new(),
            want_full: false,
            crops: HashMap::new(),
            crop_mode: false,
            crop_edit: FULL_CROP,
            small_src: HashMap::new(),
            close_prompt: false,
            allow_close: false,
            print_tex: HashMap::new(),
            zoom_text: String::new(),
            meta_cache: (PathBuf::new(), 0),
            last_title: String::new(),
        };
        // ritagli mantenuti dalla sessione precedente
        let (saved_crops, saved_rot) = load_saved_crops();
        app.crops = saved_crops;
        for (p, r) in saved_rot {
            app.rotations.entry(p).or_insert(r);
        }
        for p in args {
            app.open_any(p);
        }
        // stampa dal menu destro: le foto scelte sono già schede, si apre subito la finestra di stampa
        if print_mode && !app.tabs.is_empty() {
            app.open_print(Scope::Tabs);
        }
        app
    }

    // ----- apertura file -------------------------------------------------

    /// Apre un file (in una nuova scheda) o la prima immagine di una cartella.
    fn open_any(&mut self, path: PathBuf) {
        if path.is_dir() {
            if let Ok(rd) = std::fs::read_dir(&path) {
                let mut files: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.is_file() && util::is_supported(p)).collect();
                files.sort_by(|a, b| {
                    let na = a.file_name().and_then(|s| s.to_str()).unwrap_or("");
                    let nb = b.file_name().and_then(|s| s.to_str()).unwrap_or("");
                    util::natural_cmp(na, nb)
                });
                if let Some(first) = files.into_iter().next() {
                    self.open_file(first);
                }
            }
        } else if path.is_file() {
            self.open_file(path);
        }
    }

    fn open_file(&mut self, path: PathBuf) {
        let (files, index) = util::list_siblings(&path);
        self.tabs.push(Tab { files, index, fit: true, scale: 1.0, pan: Vec2::ZERO });
        self.active = self.tabs.len() - 1;
    }

    fn open_dialog(&mut self) {
        let picked = rfd::FileDialog::new()
            .set_title("Apri immagine")
            .add_filter("Immagini", util::EXTENSIONS)
            .add_filter("Tutti i file", &["*"])
            .pick_files();
        if let Some(files) = picked {
            for f in files.into_iter().take(30) {
                self.open_file(f);
            }
        }
    }

    fn open_folder_dialog(&mut self) {
        if let Some(dir) = rfd::FileDialog::new().set_title("Apri cartella").pick_folder() {
            self.open_any(dir);
        }
    }

    // ----- navigazione ---------------------------------------------------

    fn goto(&mut self, idx: usize) {
        self.crop_mode = false;
        if let Some(tab) = self.tabs.get_mut(self.active) {
            if idx < tab.files.len() && idx != tab.index {
                tab.index = idx;
                tab.fit = true;
                tab.pan = Vec2::ZERO;
            }
        }
    }

    fn step(&mut self, delta: isize) {
        let (n, i) = match self.tabs.get(self.active) {
            Some(t) => (t.files.len() as isize, t.index as isize),
            None => return,
        };
        if n > 1 {
            let next = (i + delta).rem_euclid(n);
            self.goto(next as usize);
        }
    }

    fn close_tab(&mut self, i: usize) {
        if i < self.tabs.len() {
            self.tabs.remove(i);
            if self.tabs.is_empty() {
                self.active = 0;
            } else if self.active >= self.tabs.len() {
                self.active = self.tabs.len() - 1;
            } else if i < self.active {
                self.active -= 1;
            }
        }
    }

    fn rotate(&mut self, quarter_turns: u8) {
        self.crop_mode = false;
        if let Some(tab) = self.tabs.get_mut(self.active) {
            let path = tab.current().to_path_buf();
            tab.fit = true;
            tab.pan = Vec2::ZERO;
            // il ritaglio ruota insieme alla foto
            if let Some(c) = self.crops.get_mut(&path) {
                for _ in 0..quarter_turns {
                    *c = rot_cw(*c);
                }
            }
            let r = self.rotations.entry(path).or_insert(0);
            *r = (*r + quarter_turns) % 4;
        }
    }

    fn open_print(&mut self, scope: Scope) {
        let (printers, default) = print::list_printers();
        self.print.selected = default.as_ref().and_then(|d| printers.iter().position(|p| p == d)).unwrap_or(0);
        self.print.printers = printers;
        self.print.scope = scope;
        self.print.message.clear();
        self.print.open = true;
    }

    fn apply_actions(&mut self, ctx: &egui::Context) {
        let acts = std::mem::take(&mut self.acts);
        for a in acts {
            match a {
                Act::Next => self.step(1),
                Act::Prev => self.step(-1),
                Act::First => self.goto(0),
                Act::Last => {
                    let n = self.tabs.get(self.active).map(|t| t.files.len()).unwrap_or(0);
                    if n > 0 {
                        self.goto(n - 1);
                    }
                }
                Act::Goto(i) => self.goto(i),
                Act::ActivateTab(i) => {
                    if i < self.tabs.len() {
                        self.active = i;
                    }
                }
                Act::CloseTab(i) => self.close_tab(i),
                Act::OpenDialog => self.open_dialog(),
                Act::OpenFolder => self.open_folder_dialog(),
                Act::RotateLeft => self.rotate(3),
                Act::RotateRight => self.rotate(1),
                Act::Fit => {
                    if let Some(t) = self.tabs.get_mut(self.active) {
                        t.fit = true;
                        t.pan = Vec2::ZERO;
                    }
                }
                Act::Actual => {
                    if let Some(t) = self.tabs.get_mut(self.active) {
                        t.fit = false;
                        t.scale = 1.0;
                        t.pan = Vec2::ZERO;
                    }
                }
                Act::ShowPrint(scope) => self.open_print(scope),
                Act::About => self.show_about = true,
                Act::DefaultApps => {
                    #[cfg(windows)]
                    {
                        let _ = std::process::Command::new("explorer.exe").arg("ms-settings:defaultapps").spawn();
                    }
                }
                Act::Fullscreen => {
                    let fs = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fs));
                }
                Act::ToggleCrop => self.toggle_crop(),
                Act::SaveCropCopy => self.save_crop_copy(),
                Act::ShowSettings => self.show_settings = true,
                Act::OpenWith => self.open_with(),
                Act::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            }
        }
    }

    // ----- ricezione risultati dal loader ---------------------------------

    fn poll_loader(&mut self, ctx: &egui::Context) {
        while let Ok(d) = self.loader.rx.try_recv() {
            match d.image {
                Ok(ci) => {
                    if d.kind != loader::Kind::Thumb {
                        if self.small_src.len() > 64 {
                            self.small_src.clear();
                        }
                        self.small_src.insert(d.path.clone(), shrink(&ci, 320));
                    }
                    let name = format!("{}#{:?}", d.path.display(), d.kind);
                    let tex = ctx.load_texture(name, ci, egui::TextureOptions::LINEAR);
                    match d.kind {
                        loader::Kind::Thumb => {
                            self.thumbs.insert(d.path, tex);
                        }
                        loader::Kind::Preview | loader::Kind::Full => {
                            let entry = self.full.entry(d.path).or_insert(ImgTex { preview: None, full: None, orig: d.orig });
                            entry.orig = d.orig;
                            if d.kind == loader::Kind::Preview {
                                entry.preview = Some(tex);
                            } else {
                                entry.full = Some(tex);
                            }
                        }
                    }
                }
                Err(msg) => match d.kind {
                    loader::Kind::Thumb => {
                        self.bad_thumbs.insert(d.path);
                    }
                    _ => {
                        self.failed.insert(d.path, msg);
                    }
                },
            }
        }
    }

    /// Libera le texture delle immagini non più vicine a quella corrente.
    fn evict(&mut self) {
        let mut keep: HashSet<PathBuf> = HashSet::new();
        for (ti, tab) in self.tabs.iter().enumerate() {
            keep.insert(tab.current().to_path_buf());
            if ti == self.active && tab.files.len() > 1 {
                let n = tab.files.len();
                keep.insert(tab.files[(tab.index + 1) % n].clone());
                keep.insert(tab.files[(tab.index + n - 1) % n].clone());
            }
        }
        if self.full.len() > keep.len() + 1 {
            self.full.retain(|p, _| keep.contains(p));
        }
        if self.thumbs.len() > 700 {
            self.thumbs.clear();
            self.last_thumb_req.clear();
        }
    }

    /// Chiede al loader le immagini e le miniature che servono adesso.
    /// Le immagini vicine si caricano solo come anteprima leggera; la versione intera
    /// solo per l'immagine corrente e solo se è davvero ingrandita.
    fn request_loads(&mut self) {
        let (mut preview_paths, full_paths, thumb_paths) = match self.tabs.get(self.active) {
            Some(tab) => {
                let n = tab.files.len();
                let i = tab.index;
                let mut idxs: Vec<usize> = vec![i];
                for k in [(i + 1) % n, (i + n - 1) % n, (i + 2) % n] {
                    if !idxs.contains(&k) {
                        idxs.push(k);
                    }
                }
                let preview_paths: Vec<PathBuf> = idxs.iter().map(|&k| tab.files[k].clone()).collect();
                let full_paths: Vec<PathBuf> = if self.want_full { vec![tab.files[i].clone()] } else { Vec::new() };

                let mut order: Vec<usize> = Vec::new();
                for k in 0..40 {
                    if i + k < n {
                        order.push(i + k);
                    }
                }
                for k in 1..4 {
                    if i >= k {
                        order.push(i - k);
                    }
                }
                let thumb_paths: Vec<PathBuf> = order.iter().map(|&k| tab.files[k].clone()).collect();
                (preview_paths, full_paths, thumb_paths)
            }
            None => return,
        };

        // le foto che vanno in stampa si preparano anche se sono in altre schede (per l'anteprima reale)
        if self.print.open {
            for (p, _, _) in self.print_items().into_iter().take(self.print.per_page) {
                if !preview_paths.contains(&p) {
                    preview_paths.push(p);
                }
            }
        }

        let preview_req: Vec<PathBuf> = preview_paths
            .into_iter()
            .filter(|p| {
                let have = self.full.get(p).map(|e| e.preview.is_some() || e.full.is_some()).unwrap_or(false);
                !have && !self.failed.contains_key(p)
            })
            .collect();
        if preview_req != self.last_preview_req {
            self.loader.set_previews(preview_req.clone());
            self.last_preview_req = preview_req;
        }

        let full_req: Vec<PathBuf> = full_paths
            .into_iter()
            .filter(|p| !self.full.get(p).map(|e| e.full.is_some()).unwrap_or(false) && !self.failed.contains_key(p))
            .collect();
        if full_req != self.last_full_req {
            self.loader.set_full(full_req.clone());
            self.last_full_req = full_req;
        }

        let thumb_req: Vec<PathBuf> =
            thumb_paths.into_iter().filter(|p| !self.thumbs.contains_key(p) && !self.bad_thumbs.contains(p)).collect();
        if thumb_req != self.last_thumb_req {
            self.loader.set_thumbs(thumb_req.clone());
            self.last_thumb_req = thumb_req;
        }
    }

    /// Prepara le anteprime di stampa con chiaro/scuro e contrasto (rifatte solo quando cambiano).
    fn refresh_print_tex(&mut self, ctx: &egui::Context) {
        let want = (self.print.brightness, self.print.contrast);
        if self.print.tone_applied != Some(want) {
            self.print.tone_applied = Some(want);
            self.print_tex.clear();
        }
        let lut = util::tone_lut(want.0, want.1);
        for (path, _, _) in self.print_items().into_iter().take(self.print.per_page) {
            if self.print_tex.contains_key(&path) {
                continue;
            }
            let Some(src) = self.small_src.get(&path) else { continue };
            let mut img = src.clone();
            for px in img.pixels.iter_mut() {
                let [r, g, b, a] = px.to_array();
                *px = egui::Color32::from_rgba_premultiplied(lut[r as usize], lut[g as usize], lut[b as usize], a);
            }
            let tex = ctx.load_texture(format!("print-{}", path.display()), img, egui::TextureOptions::LINEAR);
            self.print_tex.insert(path, tex);
        }
    }

    /// Salva la foto ritagliata (con rotazione) come nuovo file: l'originale resta intatto.
    fn save_crop_copy(&mut self) {
        let Some(path) = self.current_path() else { return };
        let Some(crop) = self.crops.get(&path).copied() else { return };
        let rot = self.rotations.get(&path).copied().unwrap_or(0);
        let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "immagine".to_string());
        let mut dialog = rfd::FileDialog::new()
            .set_file_name(format!("{}_ritaglio.jpg", stem))
            .add_filter("JPEG", &["jpg"])
            .add_filter("PNG", &["png"]);
        if let Some(dir) = path.parent() {
            dialog = dialog.set_directory(dir);
        }
        let Some(dest) = dialog.save_file() else { return };
        let jpg = dest
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("jpg") || e.eq_ignore_ascii_case("jpeg"));
        let result = print::load_for_print(&path, rot, Some(crop), u32::MAX).and_then(|img| {
            if jpg {
                img.to_rgb8().save(&dest)
            } else {
                img.save(&dest)
            }
            .map_err(|e| e.to_string())
        });
        if let Err(e) = result {
            rfd::MessageDialog::new()
                .set_level(rfd::MessageLevel::Error)
                .set_title("Salvataggio non riuscito")
                .set_description(e.as_str())
                .show();
        }
    }

    fn close_dialog(&mut self, ctx: &egui::Context) {
        let mut choice: Option<u8> = None; // 0 mantieni, 1 scarta, 2 annulla
        let n = self.crops.len();
        egui::Window::new("Ritagli")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label(format!(
                    "Ci sono {} foto ritagliate, ma il ritaglio non modifica i file originali:\nle immagini NON sono state salvate definitivamente.\n\nVuoi mantenere i ritagli alla prossima apertura?",
                    n
                ));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Mantieni").clicked() {
                        choice = Some(0);
                    }
                    if ui.button("Scarta").clicked() {
                        choice = Some(1);
                    }
                    if ui.button("Annulla").clicked() {
                        choice = Some(2);
                    }
                });
            });
        match choice {
            Some(0) => {
                save_crops(&self.crops, &self.rotations);
                self.allow_close = true;
                self.close_prompt = false;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Some(1) => {
                if let Some(f) = crops_file() {
                    let _ = fs::remove_file(f);
                }
                self.allow_close = true;
                self.close_prompt = false;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Some(_) => self.close_prompt = false,
            None => {}
        }
    }

    /// Foto della prima pagina di stampa, con rotazione e ritaglio, per l'anteprima reale.
    fn preview_items(&self) -> Vec<PreviewItem> {
        self.print_items()
            .into_iter()
            .take(self.print.per_page)
            .map(|(path, rot, crop)| {
                let (w, h) = match self.full.get(&path) {
                    Some(e) if e.orig.0 > 0 => e.orig,
                    _ => (2, 3),
                };
                let disp = if rot % 2 == 1 { (h, w) } else { (w, h) };
                let tex = self
                    .print_tex
                    .get(&path)
                    .map(|t| t.id())
                    .or_else(|| self.full.get(&path).and_then(|e| e.preview.as_ref().or(e.full.as_ref())).map(|t| t.id()))
                    .or_else(|| self.thumbs.get(&path).map(|t| t.id()));
                PreviewItem { tex, disp, rot, crop }
            })
            .collect()
    }

    // ----- ritaglio ------------------------------------------------------------

    fn current_path(&self) -> Option<PathBuf> {
        self.tabs.get(self.active).map(|t| t.current().to_path_buf())
    }

    fn toggle_crop(&mut self) {
        if self.crop_mode {
            self.crop_mode = false;
            return;
        }
        if let Some(path) = self.current_path() {
            if let Some(tab) = self.tabs.get_mut(self.active) {
                tab.fit = true;
                tab.pan = Vec2::ZERO;
            }
            self.crop_edit = self.crops.get(&path).copied().unwrap_or(FULL_CROP);
            self.crop_mode = true;
        }
    }

    fn apply_crop(&mut self) {
        if let Some(path) = self.current_path() {
            if is_full(self.crop_edit) {
                self.crops.remove(&path);
            } else {
                self.crops.insert(path, self.crop_edit);
            }
        }
        self.crop_mode = false;
    }

    /// Dimensioni della foto corrente (con rotazione e ritaglio) a 300 dpi, per la finestra di stampa.
    fn current_size_text(&self) -> String {
        let path = match self.current_path() {
            Some(p) => p,
            None => return String::new(),
        };
        let (w, h) = match self.full.get(&path) {
            Some(e) if e.orig.0 > 0 => e.orig,
            _ => return "Dimensioni: in caricamento…".to_string(),
        };
        let rot = self.rotations.get(&path).copied().unwrap_or(0);
        let (mut w, mut h) = if rot % 2 == 1 { (h, w) } else { (w, h) };
        if let Some(c) = self.crops.get(&path) {
            w = (((c[2] - c[0]).max(0.0)) * w as f32).round() as u32;
            h = (((c[3] - c[1]).max(0.0)) * h as f32).round() as u32;
        }
        let cm = |px: u32| px as f32 / 300.0 * 2.54;
        format!("Foto corrente: {} × {} px → a 300 dpi: {:.1} × {:.1} cm", w, h, cm(w), cm(h))
    }

    /// Riquadro di ritaglio sopra l'immagine. In modifica si trascina (angoli, lati o interno);
    /// fuori dalla modifica mostra solo il risultato in ombra.
    fn crop_overlay(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, painter: &egui::Painter, disp: Rect, path: &Path) {
        let editing = self.crop_mode;
        let mut c = if editing { self.crop_edit } else { self.crops.get(path).copied().unwrap_or(FULL_CROP) };
        let pt = |x: f32, y: f32| pos2(disp.left() + x * disp.width(), disp.top() + y * disp.height());
        let r = Rect::from_min_max(pt(c[0], c[1]), pt(c[2], c[3]));

        // parte fuori dal ritaglio in ombra
        let dim = Color32::from_black_alpha(if editing { 150 } else { 110 });
        painter.rect_filled(Rect::from_min_max(disp.min, pos2(disp.right(), r.top())), 0.0, dim);
        painter.rect_filled(Rect::from_min_max(pos2(disp.left(), r.bottom()), disp.max), 0.0, dim);
        painter.rect_filled(Rect::from_min_max(pos2(disp.left(), r.top()), pos2(r.left(), r.bottom())), 0.0, dim);
        painter.rect_filled(Rect::from_min_max(pos2(r.right(), r.top()), pos2(disp.right(), r.bottom())), 0.0, dim);
        painter.rect_stroke(r, 0.0, Stroke::new(1.5_f32, Color32::WHITE), egui::StrokeKind::Outside);
        if !editing {
            return;
        }

        // trascinando l'interno si sposta il riquadro (registrato prima: le maniglie hanno la precedenza)
        let inner = ui.interact(r, ui.id().with("crop_move"), egui::Sense::drag());
        if inner.dragged() {
            let d = inner.drag_delta();
            let (dx, dy) = (d.x / disp.width(), d.y / disp.height());
            let (w, h) = (c[2] - c[0], c[3] - c[1]);
            c[0] = (c[0] + dx).min(1.0 - w).max(0.0);
            c[2] = c[0] + w;
            c[1] = (c[1] + dy).min(1.0 - h).max(0.0);
            c[3] = c[1] + h;
        }

        // maniglie: angoli e lati (posizione, lato sinistro, destro, alto, basso)
        let (mx, my) = ((r.left() + r.right()) / 2.0, (r.top() + r.bottom()) / 2.0);
        let handles = [
            (pos2(r.left(), r.top()), true, false, true, false),
            (pos2(mx, r.top()), false, false, true, false),
            (pos2(r.right(), r.top()), false, true, true, false),
            (pos2(r.right(), my), false, true, false, false),
            (pos2(r.right(), r.bottom()), false, true, false, true),
            (pos2(mx, r.bottom()), false, false, false, true),
            (pos2(r.left(), r.bottom()), true, false, false, true),
            (pos2(r.left(), my), true, false, false, false),
        ];
        for (i, (pos, left, right, top, bottom)) in handles.into_iter().enumerate() {
            let hr = Rect::from_center_size(pos, vec2(20.0, 20.0));
            let resp = ui.interact(hr, ui.id().with(("crop_h", i)), egui::Sense::drag());
            if resp.dragged() {
                let d = resp.drag_delta();
                let (dx, dy) = (d.x / disp.width(), d.y / disp.height());
                if left {
                    c[0] = (c[0] + dx).min(c[2] - MIN_CROP).max(0.0);
                }
                if right {
                    c[2] = (c[2] + dx).max(c[0] + MIN_CROP).min(1.0);
                }
                if top {
                    c[1] = (c[1] + dy).min(c[3] - MIN_CROP).max(0.0);
                }
                if bottom {
                    c[3] = (c[3] + dy).max(c[1] + MIN_CROP).min(1.0);
                }
            }
            painter.rect_filled(Rect::from_center_size(pos, vec2(10.0, 10.0)), 2.0, Color32::WHITE);
        }
        self.crop_edit = c;

        // barra con i comandi
        let (mut apply, mut cancel, mut reset) = (false, false, false);
        egui::Area::new(egui::Id::new("crop_bar"))
            .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -44.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(format!("Ritaglio libero: {:.0}% × {:.0}%", (c[2] - c[0]) * 100.0, (c[3] - c[1]) * 100.0));
                        apply = ui.button("Applica (Invio)").clicked();
                        reset = ui.button("Ripristina").clicked();
                        cancel = ui.button("Annulla (Esc)").clicked();
                    });
                });
            });
        if apply {
            self.apply_crop();
        } else if cancel {
            self.crop_mode = false;
        } else if reset {
            self.crops.remove(path);
            self.crop_edit = FULL_CROP;
        }
    }

    // ----- tastiera e trascinamento ----------------------------------------

    fn handle_input(&mut self, ctx: &egui::Context) {
        // file trascinati sulla finestra
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        for f in dropped {
            if let Some(p) = f.path {
                self.open_any(p);
            }
        }

        let (open, close_tab, print_all, print, fullscreen_key) = ctx.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::CTRL, egui::Key::O),
                i.consume_key(egui::Modifiers::CTRL, egui::Key::W),
                i.consume_key(egui::Modifiers { ctrl: true, shift: true, ..Default::default() }, egui::Key::P),
                i.consume_key(egui::Modifiers::CTRL, egui::Key::P),
                i.consume_key(egui::Modifiers::NONE, egui::Key::F11),
            )
        });
        if open {
            self.acts.push(Act::OpenDialog);
        }
        if close_tab && !self.tabs.is_empty() {
            self.acts.push(Act::CloseTab(self.active));
        }
        if print_all && !self.tabs.is_empty() {
            self.acts.push(Act::ShowPrint(Scope::Folder));
        } else if print && !self.tabs.is_empty() {
            self.acts.push(Act::ShowPrint(Scope::Current));
        }
        if fullscreen_key {
            self.acts.push(Act::Fullscreen);
        }

        if self.tabs.is_empty() {
            return;
        }
        // ritaglio: C attiva, Invio applica, Esc annulla
        let (c_key, enter_key, esc_key) =
            ctx.input(|i| (i.key_pressed(egui::Key::C), i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Escape)));
        let mut esc_used = false;
        if self.crop_mode {
            if enter_key {
                self.apply_crop();
            }
            if esc_key {
                self.crop_mode = false;
                esc_used = true;
            }
        } else if c_key {
            self.acts.push(Act::ToggleCrop);
        }
        let k = |key: egui::Key| ctx.input(|i| i.key_pressed(key));
        if k(egui::Key::ArrowRight) || k(egui::Key::ArrowDown) || k(egui::Key::PageDown) || k(egui::Key::Space) {
            self.acts.push(Act::Next);
        }
        if k(egui::Key::ArrowLeft) || k(egui::Key::ArrowUp) || k(egui::Key::PageUp) || k(egui::Key::Backspace) {
            self.acts.push(Act::Prev);
        }
        if k(egui::Key::Home) {
            self.acts.push(Act::First);
        }
        if k(egui::Key::End) {
            self.acts.push(Act::Last);
        }
        if k(egui::Key::Num0) {
            self.acts.push(Act::Fit);
        }
        if k(egui::Key::Num1) {
            self.acts.push(Act::Actual);
        }
        if k(egui::Key::R) {
            let shift = ctx.input(|i| i.modifiers.shift);
            self.acts.push(if shift { Act::RotateLeft } else { Act::RotateRight });
        }
        if !esc_used && k(egui::Key::Escape) && ctx.input(|i| i.viewport().fullscreen.unwrap_or(false)) {
            self.acts.push(Act::Fullscreen);
        }
    }

    /// Ridimensionamento dai bordi (la finestra non ha la cornice di sistema).
    fn edge_resize(&self, ctx: &egui::Context) {
        let (fullscreen, maximized) = ctx.input(|i| (i.viewport().fullscreen.unwrap_or(false), i.viewport().maximized.unwrap_or(false)));
        if fullscreen || maximized {
            return;
        }
        let pos = match ctx.input(|i| i.pointer.hover_pos()) {
            Some(p) => p,
            None => return,
        };
        let screen = ctx.screen_rect();
        let m = 5.0;
        let left = pos.x - screen.left() < m;
        let right = screen.right() - pos.x < m;
        let top = pos.y - screen.top() < m;
        let bottom = screen.bottom() - pos.y < m;
        use egui::viewport::ResizeDirection as D;
        let (dir, cursor) = match (left, right, top, bottom) {
            (true, _, true, _) => (D::NorthWest, egui::CursorIcon::ResizeNwSe),
            (_, true, true, _) => (D::NorthEast, egui::CursorIcon::ResizeNeSw),
            (true, _, _, true) => (D::SouthWest, egui::CursorIcon::ResizeNeSw),
            (_, true, _, true) => (D::SouthEast, egui::CursorIcon::ResizeNwSe),
            (true, _, _, _) => (D::West, egui::CursorIcon::ResizeHorizontal),
            (_, true, _, _) => (D::East, egui::CursorIcon::ResizeHorizontal),
            (_, _, true, _) => (D::North, egui::CursorIcon::ResizeVertical),
            (_, _, _, true) => (D::South, egui::CursorIcon::ResizeVertical),
            _ => return,
        };
        ctx.set_cursor_icon(cursor);
        if ctx.input(|i| i.pointer.primary_pressed()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(dir));
        }
    }

    // ----- barra del titolo --------------------------------------------------

    fn title_bar(&mut self, ctx: &egui::Context) {
        let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        let has_tabs = !self.tabs.is_empty();
        let multi = self.tabs.get(self.active).map(|t| t.files.len() > 1).unwrap_or(false);

        egui::TopBottomPanel::top("titlebar")
            .exact_height(TITLE_H)
            .frame(egui::Frame::default().fill(col::bar()))
            .show(ctx, |ui| {
                // zona di trascinamento: tutta la barra (i pulsanti sopra hanno la precedenza)
                let bar = ui.max_rect();
                let drag = ui.interact(bar, ui.id().with("title_drag"), egui::Sense::click_and_drag());
                if drag.drag_started() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                if drag.double_clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                }

                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.add_space(10.0);
                    let (r, _) = ui.allocate_exact_size(vec2(20.0, 20.0), egui::Sense::hover());
                    if let Some(t) = &self.icon_tex {
                        ui.painter().image(t.id(), r, uv_full(), Color32::WHITE);
                    }
                    ui.add_space(2.0);
                    self.menus(ui);
                    ui.add_space(8.0);

                    let budget = (ui.available_width() - RIGHT_W - 44.0).max(80.0);
                    self.tabs_ui(ui, budget);

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        if caption_button(ui, Ico::Close).clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        let max_ico = if maximized { Ico::Restore } else { Ico::Max };
                        if caption_button(ui, max_ico).clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                        }
                        if caption_button(ui, Ico::Min).clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                        }
                        ui.add_space(12.0);
                        if tool_button(ui, Ico::PrintAll, "Stampa tutto (Ctrl+Maiusc+P)", has_tabs).clicked() && has_tabs {
                            self.acts.push(Act::ShowPrint(Scope::Folder));
                        }
                        if tool_button(ui, Ico::Print, "Stampa (Ctrl+P)", has_tabs).clicked() && has_tabs {
                            self.acts.push(Act::ShowPrint(Scope::Current));
                        }
                        if tool_button(ui, Ico::RotR, "Ruota a destra (R)", has_tabs).clicked() && has_tabs {
                            self.acts.push(Act::RotateRight);
                        }
                        if tool_button(ui, Ico::RotL, "Ruota a sinistra (Maiusc+R)", has_tabs).clicked() && has_tabs {
                            self.acts.push(Act::RotateLeft);
                        }
                        if tool_button(ui, Ico::Crop, "Ritaglia (C)", has_tabs).clicked() && has_tabs {
                            self.acts.push(Act::ToggleCrop);
                        }
                        if tool_button(ui, Ico::Next, "Successiva (→)", multi).clicked() && multi {
                            self.acts.push(Act::Next);
                        }
                        if tool_button(ui, Ico::Prev, "Precedente (←)", multi).clicked() && multi {
                            self.acts.push(Act::Prev);
                        }
                    });
                });
            });
    }

    fn menus(&mut self, ui: &mut egui::Ui) {
        let has_tabs = !self.tabs.is_empty();
        ui.menu_button("File", |ui| {
            if menu_item(ui, "Apri immagine…", "Ctrl+O", true) {
                self.acts.push(Act::OpenDialog);
            }
            if menu_item(ui, "Apri cartella…", "", true) {
                self.acts.push(Act::OpenFolder);
            }
            if menu_item(ui, "Apri con… (per modificare)", "", has_tabs && cfg!(windows)) {
                self.acts.push(Act::OpenWith);
            }
            if menu_item(ui, "Chiudi scheda", "Ctrl+W", has_tabs) {
                self.acts.push(Act::CloseTab(self.active));
            }
            let crop_here = self.current_path().is_some_and(|p| self.crops.contains_key(&p));
            if menu_item(ui, "Salva ritaglio come copia…", "", crop_here) {
                self.acts.push(Act::SaveCropCopy);
            }
            ui.separator();
            if menu_item(ui, "Stampa…", "Ctrl+P", has_tabs) {
                self.acts.push(Act::ShowPrint(Scope::Current));
            }
            if menu_item(ui, "Stampa tutto…", "Ctrl+Maiusc+P", has_tabs) {
                self.acts.push(Act::ShowPrint(Scope::Folder));
            }
            ui.separator();
            if menu_item(ui, "Imposta come programma predefinito…", "", cfg!(windows)) {
                self.acts.push(Act::DefaultApps);
            }
            if menu_item(ui, "Impostazioni…", "", true) {
                self.acts.push(Act::ShowSettings);
            }
            if menu_item(ui, "Info", "", true) {
                self.acts.push(Act::About);
            }
            ui.separator();
            if menu_item(ui, "Esci", "", true) {
                self.acts.push(Act::Quit);
            }
        });
        ui.menu_button("Vista", |ui| {
            ui.checkbox(&mut self.show_strip, "Anteprime dei file successivi");
            ui.separator();
            if menu_item(ui, "Adatta alla finestra", "0", has_tabs) {
                self.acts.push(Act::Fit);
            }
            if menu_item(ui, "Dimensione reale (100%)", "1", has_tabs) {
                self.acts.push(Act::Actual);
            }
            if menu_item(ui, "Ruota a destra", "R", has_tabs) {
                self.acts.push(Act::RotateRight);
            }
            if menu_item(ui, "Ruota a sinistra", "Maiusc+R", has_tabs) {
                self.acts.push(Act::RotateLeft);
            }
            if menu_item(ui, "Ritaglia…", "C", has_tabs) {
                self.acts.push(Act::ToggleCrop);
            }
            ui.separator();
            if menu_item(ui, "Schermo intero", "F11", true) {
                self.acts.push(Act::Fullscreen);
            }
        });
    }

    fn tabs_ui(&mut self, ui: &mut egui::Ui, budget: f32) {
        let n = self.tabs.len();
        if n > 0 {
            let gap = 4.0;
            let w = ((budget - gap * (n as f32 - 1.0)) / n as f32).clamp(70.0, 200.0);
            for i in 0..n {
                let active = i == self.active;
                let title = self.tabs[i].title();
                let full_path = self.tabs[i].current().display().to_string();
                let (rect, resp) = ui.allocate_exact_size(vec2(w, 30.0), egui::Sense::click());
                let hovered = resp.hovered();

                let bg = if active {
                    col::tab_active()
                } else if hovered {
                    col::tab_hover()
                } else {
                    Color32::TRANSPARENT
                };
                ui.painter().rect_filled(rect, 7.0, bg);

                // pulsante di chiusura della scheda
                let close_rect = Rect::from_center_size(pos2(rect.right() - 15.0, rect.center().y), vec2(20.0, 20.0));
                let close_resp = ui.interact(close_rect, ui.id().with(("tab_close", i)), egui::Sense::click());
                if close_resp.hovered() {
                    ui.painter().rect_filled(close_rect, 5.0, Color32::from_white_alpha(30));
                }
                draw_icon(ui.painter(), Ico::XSmall, close_rect.center(), if active || hovered { col::text() } else { col::text_dim() });

                // titolo troncato con "…"
                let text_color = if active { col::text() } else { col::text_dim() };
                let mut job = egui::text::LayoutJob::simple_singleline(title, FontId::proportional(13.0), text_color);
                job.wrap = egui::text::TextWrapping::truncate_at_width((w - 44.0).max(10.0));
                let galley = ui.painter().layout_job(job);
                let pos = pos2(rect.left() + 11.0, rect.center().y - galley.size().y / 2.0);
                ui.painter().galley(pos, galley, text_color);

                if close_resp.clicked() || resp.middle_clicked() {
                    self.acts.push(Act::CloseTab(i));
                } else if resp.clicked() {
                    self.acts.push(Act::ActivateTab(i));
                }
                let _ = resp.on_hover_text(full_path);
            }
        }
        if tool_button(ui, Ico::Plus, "Apri in una nuova scheda (Ctrl+O)", true).clicked() {
            self.acts.push(Act::OpenDialog);
        }
    }

    // ----- area centrale -----------------------------------------------------

    fn central(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().frame(egui::Frame::default().fill(col::bg())).show(ctx, |ui| {
            if self.tabs.is_empty() {
                self.welcome(ui);
            } else {
                self.viewer(ui, ctx);
            }
        });
    }

    fn welcome(&mut self, ui: &mut egui::Ui) {
        let rect = ui.available_rect_before_wrap();
        ui.allocate_rect(rect, egui::Sense::hover());
        let c = rect.center();
        if let Some(t) = &self.icon_tex {
            let r = Rect::from_center_size(pos2(c.x, c.y - 90.0), vec2(110.0, 110.0));
            ui.painter().image(t.id(), r, uv_full(), Color32::WHITE);
        }
        ui.painter().text(pos2(c.x, c.y - 10.0), Align2::CENTER_CENTER, APP_NAME, FontId::proportional(26.0), col::text());
        ui.painter().text(
            pos2(c.x, c.y + 24.0),
            Align2::CENTER_CENTER,
            "Trascina qui un'immagine oppure premi Ctrl+O",
            FontId::proportional(15.0),
            col::text_dim(),
        );
        let btn = Rect::from_center_size(pos2(c.x, c.y + 70.0), vec2(190.0, 36.0));
        let resp = ui.interact(btn, ui.id().with("welcome_open"), egui::Sense::click());
        let fill = if resp.hovered() { col::accent() } else { col::accent().gamma_multiply(0.8) };
        ui.painter().rect_filled(btn, 8.0, fill);
        ui.painter().text(btn.center(), Align2::CENTER_CENTER, "Apri immagine…", FontId::proportional(15.0), Color32::WHITE);
        if resp.clicked() {
            self.acts.push(Act::OpenDialog);
        }
    }

    fn viewer(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let rect = ui.available_rect_before_wrap();
        let resp = ui.allocate_rect(rect, egui::Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        let ppp = ctx.pixels_per_point();
        let active = self.active;
        self.want_full = false;
        let path = self.tabs[active].current().to_path_buf();
        let rot = self.rotations.get(&path).copied().unwrap_or(0);
        let n_files = self.tabs[active].files.len();
        let hover = ctx.input(|i| i.pointer.hover_pos()).filter(|p| rect.contains(*p));
        let (scroll, zoom_d) = ctx.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));

        let mut zoom_text = String::new();
        let mut disp_rect: Option<Rect> = None;
        {
            let tab = &mut self.tabs[active];
            match self.full.get(&path) {
                Some(entry) if entry.preview.is_some() || entry.full.is_some() => {
                    let (mut iw, mut ih) = (entry.orig.0.max(1) as f32, entry.orig.1.max(1) as f32);
                    if rot % 2 == 1 {
                        std::mem::swap(&mut iw, &mut ih);
                    }
                    // dimensioni intere (per lo zoom); con un ritaglio applicato la vista mostra solo la parte ritagliata
                    let (full_w, full_h) = (iw, ih);
                    let crop_show = if self.crop_mode { None } else { self.crops.get(&path).copied() };
                    if let Some(c) = crop_show {
                        iw *= (c[2] - c[0]).max(0.001);
                        ih *= (c[3] - c[1]).max(0.001);
                    }
                    let avail_px = rect.size() * ppp;
                    let fit_scale = (avail_px.x / iw).min(avail_px.y / ih).min(1.0).max(0.001);

                    // zoom con la rotella (o pizzico) attorno al puntatore
                    if let Some(hp) = hover {
                        if scroll != 0.0 || zoom_d != 1.0 {
                            let cur = if tab.fit { fit_scale } else { tab.scale };
                            let cur_center = rect.center() + if tab.fit { Vec2::ZERO } else { tab.pan };
                            let new_scale = (cur * zoom_d * (scroll * 0.002).exp()).clamp(0.02, 50.0);
                            zoom_about(tab, rect.center(), hp, cur, cur_center, new_scale);
                        }
                    }
                    // doppio clic: 100% sul punto indicato, oppure di nuovo adatta
                    if resp.double_clicked() {
                        if let Some(hp) = hover {
                            if tab.fit && fit_scale < 0.999 {
                                zoom_about(tab, rect.center(), hp, fit_scale, rect.center(), 1.0);
                            } else {
                                tab.fit = true;
                                tab.pan = Vec2::ZERO;
                            }
                        }
                    }
                    // trascinamento per spostare l'immagine (solo se è più grande della vista)
                    let scale_now = if tab.fit { fit_scale } else { tab.scale };
                    let big = iw * scale_now > avail_px.x + 0.5 || ih * scale_now > avail_px.y + 0.5;
                    if resp.dragged() && big && !self.crop_mode {
                        if tab.fit {
                            tab.scale = fit_scale;
                            tab.fit = false;
                            tab.pan = Vec2::ZERO;
                        }
                        tab.pan += resp.drag_delta();
                    }
                    if resp.hovered() && big {
                        ctx.set_cursor_icon(if resp.dragged() { egui::CursorIcon::Grabbing } else { egui::CursorIcon::Grab });
                    }

                    let scale = if tab.fit { fit_scale } else { tab.scale };
                    let size_pts = vec2(iw, ih) * scale / ppp;
                    if !tab.fit {
                        // non lasciare uscire del tutto l'immagine dalla vista
                        let max_pan = ((size_pts + rect.size()) / 2.0 - vec2(60.0, 60.0)).max(Vec2::ZERO);
                        tab.pan.x = tab.pan.x.clamp(-max_pan.x, max_pan.x);
                        tab.pan.y = tab.pan.y.clamp(-max_pan.y, max_pan.y);
                    }
                    let center = rect.center() + if tab.fit { Vec2::ZERO } else { tab.pan };

                    // anteprima leggera se si vede piccola, versione completa se si ingrandisce
                    let shown_long = full_w.max(full_h) * scale;
                    self.want_full = shown_long > loader::PREVIEW_SIDE as f32 * 0.9;
                    let tex = match (&entry.full, &entry.preview) {
                        (Some(f), Some(p)) => {
                            if shown_long > loader::PREVIEW_SIDE as f32 * 0.9 {
                                Some(f)
                            } else {
                                Some(p)
                            }
                        }
                        (Some(f), None) => Some(f),
                        (None, Some(p)) => Some(p),
                        (None, None) => None,
                    };
                    if let Some(t) = tex {
                        match crop_show {
                            None => paint_image(&painter, t.id(), center, size_pts, rot),
                            Some(c) => {
                                // la foto intera sta dietro e si vede solo la parte ritagliata
                                let full_pts = vec2(full_w, full_h) * scale / ppp;
                                let origin = center - size_pts / 2.0 - vec2(c[0] * full_pts.x, c[1] * full_pts.y);
                                let clipped = painter.with_clip_rect(Rect::from_center_size(center, size_pts));
                                paint_image(&clipped, t.id(), origin + full_pts / 2.0, full_pts, rot);
                            }
                        }
                        disp_rect = Some(Rect::from_center_size(center, size_pts));
                    }
                    zoom_text = format!("{:.0}%", scale * 100.0);
                }
                _ => {
                    let msg = match self.failed.get(&path) {
                        Some(e) => format!("Impossibile aprire questo file\n{}", e),
                        None => "Caricamento…".to_string(),
                    };
                    painter.text(rect.center(), Align2::CENTER_CENTER, msg, FontId::proportional(16.0), col::text_dim());
                }
            }
        }
        self.zoom_text = zoom_text;
        if let Some(d) = disp_rect {
            if self.crop_mode {
                self.crop_overlay(ui, ctx, &painter, d, &path);
            }
        }

        // frecce laterali visibili quando il mouse è vicino ai bordi (come nel vecchio visualizzatore)
        if n_files > 1 {
            for dir in [-1i32, 1] {
                let x = if dir < 0 { rect.left() + 36.0 } else { rect.right() - 36.0 };
                let r = Rect::from_center_size(pos2(x, rect.center().y), vec2(52.0, 52.0));
                let near = hover.map(|h| (h.x - x).abs() < 110.0).unwrap_or(false);
                if !near {
                    continue;
                }
                let rsp = ui.interact(r, ui.id().with(("nav", dir)), egui::Sense::click());
                let bg = if rsp.hovered() { Color32::from_black_alpha(190) } else { Color32::from_black_alpha(120) };
                painter.circle_filled(r.center(), 24.0, bg);
                draw_icon(&painter, if dir < 0 { Ico::Prev } else { Ico::Next }, r.center(), Color32::WHITE);
                if rsp.clicked() {
                    self.acts.push(if dir < 0 { Act::Prev } else { Act::Next });
                }
            }
        }
    }

    // ----- barra di stato e striscia di anteprime ------------------------------

    fn status_bar(&mut self, ctx: &egui::Context) {
        let (name, idx, n, path) = match self.tabs.get(self.active) {
            Some(t) => (t.title(), t.index, t.files.len(), t.current().to_path_buf()),
            None => return,
        };
        if self.meta_cache.0 != path {
            let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            self.meta_cache = (path.clone(), size);
        }
        let dims = match self.full.get(&path) {
            Some(e) if e.orig.0 > 0 => {
                let (w, h) = if self.rotations.get(&path).copied().unwrap_or(0) % 2 == 1 { (e.orig.1, e.orig.0) } else { e.orig };
                format!("{} × {} px   •   ", w, h)
            }
            _ => String::new(),
        };
        let text = format!(
            "{}   •   {}{}   •   {} di {}   •   {}",
            name,
            dims,
            util::human_size(self.meta_cache.1),
            idx + 1,
            n,
            self.zoom_text
        );
        let text = if self.crops.contains_key(&path) { format!("{}   •   ritagliata", text) } else { text };
        egui::TopBottomPanel::bottom("status").exact_height(26.0).frame(egui::Frame::default().fill(col::bar())).show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.add_space(12.0);
                ui.label(egui::RichText::new(text).size(12.5).color(col::text_dim()));
            });
        });
    }

    fn filmstrip(&mut self, ctx: &egui::Context) {
        if !self.show_strip || self.tabs.is_empty() {
            return;
        }
        let (n, idx) = {
            let t = &self.tabs[self.active];
            (t.files.len(), t.index)
        };
        egui::TopBottomPanel::bottom("strip").exact_height(STRIP_H).frame(egui::Frame::default().fill(col::bar())).show(ctx, |ui| {
            let rect = ui.max_rect();
            let painter = ui.painter_at(rect);
            let item_w = 104.0;
            let item_h = STRIP_H - 16.0;
            let gap = 8.0;
            let per = (((rect.width() - 16.0) / (item_w + gap)).floor() as usize).max(1);
            // parte dal file precedente, così si vede sempre quello corrente e i successivi
            let mut start = idx.saturating_sub(1);
            if start + per > n {
                start = n.saturating_sub(per);
            }

            // rotella sopra la striscia = file precedente/successivo
            if ui.rect_contains_pointer(rect) {
                let raw = ctx.input(|i| i.raw_scroll_delta.y);
                if raw > 0.0 {
                    self.acts.push(Act::Prev);
                } else if raw < 0.0 {
                    self.acts.push(Act::Next);
                }
            }

            for k in 0..per {
                let i = start + k;
                if i >= n {
                    break;
                }
                let path = self.tabs[self.active].files[i].clone();
                let r = Rect::from_min_size(pos2(rect.left() + 8.0 + k as f32 * (item_w + gap), rect.top() + 8.0), vec2(item_w, item_h));
                let rsp = ui.interact(r, ui.id().with(("thumb", i)), egui::Sense::click());
                let current = i == idx;

                let bg = if current {
                    col::tab_active()
                } else if rsp.hovered() {
                    col::tab_hover()
                } else {
                    col::panel()
                };
                painter.rect_filled(r, 6.0, bg);
                if current {
                    painter.add(Shape::closed_line(
                        vec![r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()],
                        Stroke::new(2.0_f32, col::accent()),
                    ));
                }
                let inner = r.shrink(6.0);
                match self.thumbs.get(&path) {
                    Some(t) => {
                        let ts = t.size_vec2();
                        let s = (inner.width() / ts.x).min(inner.height() / ts.y);
                        let tr = Rect::from_center_size(inner.center(), ts * s);
                        painter.image(t.id(), tr, uv_full(), Color32::WHITE);
                    }
                    None => {
                        painter.text(inner.center(), Align2::CENTER_CENTER, "…", FontId::proportional(18.0), col::text_dim());
                    }
                }
                painter.text(
                    r.left_bottom() + vec2(6.0, -4.0),
                    Align2::LEFT_BOTTOM,
                    format!("{}", i + 1),
                    FontId::proportional(10.5),
                    Color32::from_white_alpha(200),
                );
                if rsp.clicked() {
                    self.acts.push(Act::Goto(i));
                }
                let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
                let _ = rsp.on_hover_text(name);
            }
        });
    }

    // ----- finestre: Info e Stampa ---------------------------------------------

    /// Apre la foto con il programma scelto da Windows (finestra "Apri con"), per modificarla.
    fn open_with(&mut self) {
        let Some(path) = self.current_path() else { return };
        #[cfg(windows)]
        {
            let _ = std::process::Command::new("rundll32")
                .arg("shell32.dll,OpenAs_RunDLL")
                .arg(&path)
                .spawn();
        }
        #[cfg(not(windows))]
        {
            let _ = path;
        }
    }

    fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.show_settings {
            return;
        }
        let mut open = true;
        let before = self.light_theme;
        egui::Window::new("Impostazioni")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label("Tema");
                ui.radio_value(&mut self.light_theme, false, "Scuro (predefinito)");
                ui.radio_value(&mut self.light_theme, true, "Chiaro");
            });
        if self.light_theme != before {
            col::set_light(self.light_theme);
            setup_style(ctx);
            save_light_theme(self.light_theme);
        }
        self.show_settings = open;
    }

    fn about_window(&mut self, ctx: &egui::Context) {
        if !self.show_about {
            return;
        }
        let mut open = true;
        let icon = self.icon_tex.as_ref().map(|t| t.id());
        egui::Window::new("Info")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_min_width(360.0);
                ui.vertical_centered(|ui| {
                    ui.add_space(6.0);
                    if let Some(id) = icon {
                        let (r, _) = ui.allocate_exact_size(vec2(84.0, 84.0), egui::Sense::hover());
                        ui.painter().image(id, r, uv_full(), Color32::WHITE);
                    }
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new(APP_NAME).size(20.0).strong());
                    ui.label(egui::RichText::new(format!("Versione {}", VERSION)).color(col::text_dim()));
                    ui.add_space(10.0);
                    ui.label(ABOUT_TEXT);
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("Licenza MIT  •  Scritto in Rust con egui").color(col::text_dim()).size(12.5));
                    ui.add_space(10.0);
                    ui.label("Se vuoi contribuire al mio lavoro, puoi farlo con una piccola donazione simbolica: un'app così utile e gratuita per sempre non la trovi ovunque. Apprezzo il gesto ;-)");
                    ui.add_space(4.0);
                    ui.hyperlink_to(egui::RichText::new("Donate via PayPal").strong(), "https://paypal.me/PeppeCP");
                    ui.add_space(6.0);
                    ui.hyperlink_to("Codice sorgente su GitHub", "https://github.com/gsepp86/VISUAL-IMMAGINI-CP");
                    ui.add_space(6.0);
                });
            });
        self.show_about = open;
    }

    fn poll_print(&mut self) {
        let mut finished = false;
        if let Some(run) = &mut self.print.running {
            while let Ok(msg) = run.rx.try_recv() {
                match msg {
                    print::Msg::Progress { done, total } => {
                        run.done = done;
                        run.total = total;
                    }
                    print::Msg::Done => {
                        self.print.message = "Stampa completata.".to_string();
                        finished = true;
                    }
                    print::Msg::Cancelled => {
                        self.print.message = "Stampa annullata.".to_string();
                        finished = true;
                    }
                    print::Msg::Error(e) => {
                        self.print.message = format!("Errore: {}", e);
                        finished = true;
                    }
                }
            }
        }
        if finished {
            self.print.running = None;
        }
    }

    fn print_items(&self) -> Vec<print::Item> {
        let item = |p: &PathBuf| -> print::Item {
            (p.clone(), self.rotations.get(p).copied().unwrap_or(0), self.crops.get(p).copied())
        };
        let mut items: Vec<print::Item> = Vec::new();
        match self.print.scope {
            Scope::Current => {
                if let Some(t) = self.tabs.get(self.active) {
                    items.push(item(&t.current().to_path_buf()));
                }
            }
            Scope::Folder => {
                if let Some(t) = self.tabs.get(self.active) {
                    for p in &t.files {
                        items.push(item(p));
                    }
                }
            }
            Scope::Tabs => {
                for t in &self.tabs {
                    items.push(item(&t.current().to_path_buf()));
                }
            }
        }
        items
    }

    fn start_print(&mut self, ctx: &egui::Context) {
        let items = self.print_items();
        let printer = match self.print.printers.get(self.print.selected) {
            Some(p) => p.clone(),
            None => {
                self.print.message = "Nessuna stampante disponibile.".to_string();
                return;
            }
        };
        if items.is_empty() {
            self.print.message = "Nessuna immagine da stampare.".to_string();
            return;
        }
        let copies = self.print.copies.max(1);
        let total = items.len() * copies as usize;
        let job = print::Job {
            printer,
            items,
            per_page: self.print.per_page,
            orientation: self.print.orientation,
            margin_mm: self.print.margin_mm,
            gap_mm: self.print.gap_mm,
            photo_mm: self.print.photo.mm(),
            tone: (self.print.brightness, self.print.contrast),
            copies,
        };
        let (tx, rx) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel2 = Arc::clone(&cancel);
        let ctx2 = ctx.clone();
        std::thread::spawn(move || print::run(job, cancel2, tx, move || ctx2.request_repaint()));
        self.print.message.clear();
        self.print.running = Some(Running { rx, cancel, done: 0, total });
    }

    fn print_window(&mut self, ctx: &egui::Context) {
        if !self.print.open {
            return;
        }
        let n_folder = self.tabs.get(self.active).map(|t| t.files.len()).unwrap_or(0);
        let n_tabs = self.tabs.len();
        let count = match self.print.scope {
            Scope::Current => n_tabs.min(1),
            Scope::Folder => n_folder,
            Scope::Tabs => n_tabs,
        };
        let mut open = true;
        let cur_info = self.current_size_text();
        let sel_name = self.print.printers.get(self.print.selected).cloned();
        if self.print.page_for != sel_name {
            self.print.page_mm = print::printer_page_mm(sel_name.as_deref());
            self.print.printer_margin_mm = print::printer_min_margin_mm(sel_name.as_deref());
            if self.print.photo == PhotoFmt::Page {
                self.print.margin_mm = self.print.printer_margin_mm.min(30.0);
            }
            self.print.page_for = sel_name.clone();
        }
        self.refresh_print_tex(ctx);
        let preview_items = self.preview_items();
        let mut start = false;
        let mut cancel = false;
        let mut close = false;
        let mut light = self.light_theme;
        {
            let p = &mut self.print;
            egui::Window::new("Stampa")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.set_min_width(520.0);
                    if !cfg!(windows) {
                        ui.colored_label(Color32::from_rgb(255, 190, 80), "La stampa è disponibile solo su Windows.");
                    }
                    ui.horizontal(|ui| {
                        ui.label("Stampante:");
                        let sel = p.printers.get(p.selected).cloned().unwrap_or_else(|| "Nessuna stampante trovata".to_string());
                        egui::ComboBox::from_id_salt("printer_combo").width(340.0).selected_text(sel).show_ui(ui, |ui| {
                            for (i, name) in p.printers.iter().enumerate() {
                                ui.selectable_value(&mut p.selected, i, name.as_str());
                            }
                        });
                    });
                    if ui.button("Preferenze e driver della stampante…").clicked() {
                        if let Some(n) = &sel_name {
                            open_printer_driver(n);
                        }
                    }
                    ui.horizontal(|ui| {
                        ui.label("Tema:");
                        ui.radio_value(&mut light, false, "Scuro");
                        ui.radio_value(&mut light, true, "Chiaro");
                    });
                    ui.separator();

                    ui.horizontal_top(|ui| {
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new("Cosa stampare").strong());
                            ui.radio_value(&mut p.scope, Scope::Current, "Immagine corrente");
                            ui.radio_value(&mut p.scope, Scope::Folder, format!("Tutte le immagini della cartella ({})", n_folder));
                            ui.radio_value(&mut p.scope, Scope::Tabs, format!("Immagini di tutte le schede ({})", n_tabs));
                            ui.add_space(6.0);

                            ui.label(egui::RichText::new("Immagini per pagina").strong());
                            ui.horizontal_wrapped(|ui| {
                                for &n in util::PER_PAGE_CHOICES {
                                    let label = if n == 1 { "1".to_string() } else { format!("{} in 1", n) };
                                    ui.selectable_value(&mut p.per_page, n, label);
                                }
                            });
                            ui.add_space(6.0);

                            ui.label(egui::RichText::new("Orientamento").strong());
                            ui.horizontal(|ui| {
                                ui.selectable_value(&mut p.orientation, print::Orientation::Auto, "Automatico (miglior verso)");
                                ui.selectable_value(&mut p.orientation, print::Orientation::Portrait, "Verticale");
                                ui.selectable_value(&mut p.orientation, print::Orientation::Landscape, "Orizzontale");
                            });
                            ui.add_space(6.0);
                            ui.add(egui::Slider::new(&mut p.margin_mm, 0.0..=30.0).text("Margine (mm)"));
                            if p.photo == PhotoFmt::Page {
                                ui.label(
                                    egui::RichText::new(format!("Margine minimo della stampante: {:.1} mm", p.printer_margin_mm))
                                        .color(col::text_dim())
                                        .size(12.0),
                                );
                            }
                            ui.add(egui::Slider::new(&mut p.gap_mm, 0.0..=20.0).text("Spazio tra le foto (mm)"));
                            ui.add_space(6.0);
                            ui.label(egui::RichText::new("Regolazione foto").strong());
                            ui.add(egui::Slider::new(&mut p.brightness, -1.0..=1.0).text("Chiaro / scuro"));
                            ui.add(egui::Slider::new(&mut p.contrast, -1.0..=1.0).text("Contrasto"));
                            ui.add_space(6.0);
                            ui.label(egui::RichText::new("Dimensione foto").strong());
                            let photo_before = p.photo;
                            for f in [PhotoFmt::Page, PhotoFmt::Cm10x15, PhotoFmt::Cm13x18] {
                                ui.radio_value(&mut p.photo, f, f.label());
                            }
                            if p.photo != photo_before && p.photo == PhotoFmt::Page {
                                p.margin_mm = p.printer_margin_mm.min(30.0);
                            }
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                ui.label("Numero di copie");
                                ui.add(egui::DragValue::new(&mut p.copies).range(1..=99));
                            });
                            ui.label(egui::RichText::new(cur_info.clone()).color(col::text_dim()).size(12.0));
                            if let Some((w, h)) = p.photo.mm() {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "Ogni foto a {:.0}×{:.0} cm, ridotta in proporzione se il foglio è più piccolo",
                                        w / 10.0,
                                        h / 10.0
                                    ))
                                    .color(col::text_dim())
                                    .size(12.0),
                                );
                            }
                        });
                        ui.add_space(16.0);
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new("Anteprima").strong());
                            ui.label(egui::RichText::new("Anteprima reale: prima pagina, come verrà stampata").color(col::text_dim()).size(12.0));
                            draw_page_preview(ui, &preview_items, p.page_mm, p.per_page, p.orientation, p.margin_mm, p.gap_mm, p.photo.mm());
                            let pages = util::page_count(count, p.per_page);
                            ui.label(egui::RichText::new(format!("{} immagini → {} pagine", count, pages)).color(col::text_dim()));
                        });
                    });
                    ui.separator();

                    if let Some(run) = &p.running {
                        let frac = if run.total > 0 { run.done as f32 / run.total as f32 } else { 0.0 };
                        ui.add(egui::ProgressBar::new(frac).text(format!("Stampa in corso… {} di {}", run.done, run.total)));
                        if ui.button("Annulla stampa").clicked() {
                            cancel = true;
                        }
                    } else {
                        if !p.message.is_empty() {
                            ui.label(p.message.clone());
                        }
                        ui.horizontal(|ui| {
                            let can = cfg!(windows) && !p.printers.is_empty() && count > 0;
                            if ui.add_enabled(can, egui::Button::new("Stampa")).clicked() {
                                start = true;
                            }
                            if ui.button("Chiudi").clicked() {
                                close = true;
                            }
                        });
                    }
                });
        }
        if light != self.light_theme {
            self.light_theme = light;
            col::set_light(light);
            setup_style(ctx);
            save_light_theme(light);
        }
        if cancel {
            if let Some(run) = &self.print.running {
                run.cancel.store(true, Ordering::Relaxed);
            }
        }
        if start {
            self.start_print(ctx);
        }
        self.print.open = open && !close;
    }
}

// ---------------------------------------------------------------------------
// Ciclo principale
// ---------------------------------------------------------------------------

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // alla chiusura, se ci sono ritagli, si chiede cosa farne
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close && !self.crops.is_empty() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.close_prompt = true;
        }
        if self.close_prompt {
            self.close_dialog(ctx);
        }
        self.poll_loader(ctx);
        self.poll_print();
        self.handle_input(ctx);

        self.title_bar(ctx);
        self.status_bar(ctx);
        self.filmstrip(ctx);
        self.central(ctx);
        self.about_window(ctx);
        self.settings_window(ctx);
        self.print_window(ctx);
        self.edge_resize(ctx);

        self.apply_actions(ctx);
        self.evict();
        self.request_loads();

        // titolo della finestra = nome del file (utile nella barra delle applicazioni)
        let title = match self.tabs.get(self.active) {
            Some(t) => format!("{} - {}", t.title(), APP_NAME),
            None => APP_NAME.to_string(),
        };
        if title != self.last_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }
    }
}

// ---------------------------------------------------------------------------
// Disegno
// ---------------------------------------------------------------------------

/// Zoom mantenendo fermo il punto sotto il puntatore.
fn zoom_about(tab: &mut Tab, rect_center: Pos2, anchor: Pos2, cur_scale: f32, cur_center: Pos2, new_scale: f32) {
    let v = anchor - cur_center;
    let ratio = new_scale / cur_scale.max(0.0001);
    let new_center = anchor - v * ratio;
    tab.pan = new_center - rect_center;
    tab.scale = new_scale;
    tab.fit = false;
}

/// Disegna una texture centrata, ruotata di `rot` quarti di giro in senso orario.
/// `size` è la dimensione visibile (già ruotata).
fn paint_image(painter: &egui::Painter, tex: egui::TextureId, center: Pos2, size: Vec2, rot: u8) {
    let (w, h) = if rot % 2 == 1 { (size.y, size.x) } else { (size.x, size.y) };
    let (hw, hh) = (w / 2.0, h / 2.0);
    let corners = [(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)];
    let uvs = [pos2(0.0, 0.0), pos2(1.0, 0.0), pos2(1.0, 1.0), pos2(0.0, 1.0)];
    let (s, c) = match rot % 4 {
        0 => (0.0f32, 1.0f32),
        1 => (1.0, 0.0),
        2 => (0.0, -1.0),
        _ => (-1.0, 0.0),
    };
    let mut mesh = egui::Mesh::with_texture(tex);
    for k in 0..4 {
        let (x, y) = corners[k];
        let px = x * c - y * s;
        let py = x * s + y * c;
        mesh.vertices.push(egui::epaint::Vertex { pos: center + vec2(px, py), uv: uvs[k], color: Color32::WHITE });
    }
    mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
    painter.add(Shape::mesh(mesh));
}

/// Copia ridotta (lato lungo max `max` px) per l'anteprima di stampa.
fn shrink(ci: &egui::ColorImage, max: usize) -> egui::ColorImage {
    let [w, h] = ci.size;
    if w == 0 || h == 0 {
        return ci.clone();
    }
    let step = (w.max(h) + max - 1) / max.max(1);
    let step = step.max(1);
    let (nw, nh) = ((w + step - 1) / step, (h + step - 1) / step);
    let mut px = Vec::with_capacity(nw * nh);
    for y in 0..nh {
        for x in 0..nw {
            px.push(ci.pixels[(y * step).min(h - 1) * w + (x * step).min(w - 1)]);
        }
    }
    let mut img = egui::ColorImage::new([nw, nh], egui::Color32::TRANSPARENT);
    img.pixels = px;
    img
}

/// File delle impostazioni (%APPDATA%\VisualizzatoreImmaginiCP\impostazioni.txt).
fn settings_file() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("VisualizzatoreImmaginiCP").join("impostazioni.txt"))
}

fn load_light_theme() -> bool {
    settings_file()
        .and_then(|f| fs::read_to_string(f).ok())
        .is_some_and(|t| t.lines().any(|l| l.trim() == "tema=chiaro"))
}

fn save_light_theme(light: bool) {
    let Some(file) = settings_file() else { return };
    if let Some(dir) = file.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let _ = fs::write(file, if light { "tema=chiaro\n" } else { "tema=scuro\n" });
}

/// File dei ritagli mantenuti (%APPDATA%\VisualizzatoreImmaginiCP\ritagli.txt).
fn crops_file() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("VisualizzatoreImmaginiCP").join("ritagli.txt"))
}

/// Salva ritagli e rotazione di ogni foto ritagliata: "rot x0 y0 x1 y1<TAB>percorso".
fn save_crops(crops: &HashMap<PathBuf, [f32; 4]>, rotations: &HashMap<PathBuf, u8>) {
    let Some(file) = crops_file() else { return };
    if let Some(dir) = file.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let text: String = crops
        .iter()
        .map(|(p, c)| {
            let rot = rotations.get(p).copied().unwrap_or(0);
            format!("{} {} {} {} {}\t{}\n", rot, c[0], c[1], c[2], c[3], p.to_string_lossy())
        })
        .collect();
    let _ = fs::write(file, text);
}

fn load_saved_crops() -> (HashMap<PathBuf, [f32; 4]>, Vec<(PathBuf, u8)>) {
    let mut crops = HashMap::new();
    let mut rots = Vec::new();
    let Some(file) = crops_file() else { return (crops, rots) };
    let Ok(text) = fs::read_to_string(file) else { return (crops, rots) };
    for line in text.lines() {
        let Some((nums, path)) = line.split_once('\t') else { continue };
        let v: Vec<f32> = nums.split_whitespace().filter_map(|x| x.parse().ok()).collect();
        if v.len() == 5 {
            let p = PathBuf::from(path);
            crops.insert(p.clone(), [v[1], v[2], v[3], v[4]]);
            rots.push((p, v[0] as u8));
        }
    }
    (crops, rots)
}

/// Apre le preferenze e il driver della stampante (formato carta, qualità, colore…).
fn open_printer_driver(name: &str) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("rundll32")
            .args(["printui.dll,PrintUIEntry", "/e", "/n", name])
            .spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = name;
    }
}

/// Foto di una cella dell'anteprima: texture, dimensione dopo rotazione, rotazione e ritaglio.
struct PreviewItem {
    tex: Option<egui::TextureId>,
    /// dimensione visualizzata (dopo rotazione, prima del ritaglio)
    disp: (u32, u32),
    rot: u8,
    crop: Option<[f32; 4]>,
}

impl PreviewItem {
    /// Dimensione della foto come viene stampata (dopo rotazione e ritaglio).
    fn cropped(&self) -> (u32, u32) {
        let c = self.crop.unwrap_or(FULL_CROP);
        let w = ((self.disp.0 as f32) * (c[2] - c[0])).round().max(1.0) as u32;
        let h = ((self.disp.1 as f32) * (c[3] - c[1])).round().max(1.0) as u32;
        (w, h)
    }

    /// Rotazione e ritaglio da usare per disegnare la foto; `turn` = ruotata di un quarto di giro in più
    /// (scelta dall'orientamento automatico), come fa la stampa.
    fn frame(&self, turn: bool) -> (u8, [f32; 4]) {
        let c = self.crop.unwrap_or(FULL_CROP);
        if turn {
            ((self.rot + 1) % 4, rot_cw(c))
        } else {
            (self.rot, c)
        }
    }
}

/// Anteprima reale della prima pagina: stessa geometria della stampa (celle, margini, spazi,
/// orientamento automatico, dimensione foto), con le foto vere ruotate, ritagliate e regolate.
#[allow(clippy::too_many_arguments)]
fn draw_page_preview(
    ui: &mut egui::Ui,
    items: &[PreviewItem],
    page_mm: (f32, f32),
    per_page: usize,
    orientation: print::Orientation,
    margin_mm: f32,
    gap_mm: f32,
    photo: Option<(f32, f32)>,
) {
    let (pw_mm, ph_mm) = page_mm;
    // la pagina entra in 260 x 340 punti
    let s = (260.0 / pw_mm).min(340.0 / ph_mm);
    let (rect, _) = ui.allocate_exact_size(vec2(pw_mm * s, ph_mm * s), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, Color32::WHITE);
    painter.rect_stroke(rect, 2.0, Stroke::new(1.0_f32, Color32::from_gray(160)), egui::StrokeKind::Outside);

    // geometria in decimi di millimetro, come nella stampa
    let (cw, ch) = ((pw_mm * 10.0).round() as i32, (ph_mm * 10.0).round() as i32);
    let (mg, gp) = ((margin_mm * 10.0).round() as i32, (gap_mm * 10.0).round() as i32);
    let photo_u = photo.map(|(w, h)| (w * 10.0, h * 10.0));
    let dims: Vec<(u32, u32)> = items.iter().map(|it| it.cropped()).collect();
    let landscape = print::sheet_landscape(orientation, cw, ch, per_page, mg, gp, &dims, photo_u);
    let cells = util::page_cells(cw, ch, per_page, landscape, mg, gp);
    let k = s / 10.0; // punti per decimo di millimetro
    for (i, cell) in cells.iter().enumerate() {
        let Some(it) = items.get(i) else { break };
        let (fw, fh) = it.cropped();
        let (turn, r) = util::place_photo(*cell, fw, fh, photo_u);
        let shown = Rect::from_min_size(
            rect.min + vec2(r.x as f32 * k, r.y as f32 * k),
            vec2(r.w as f32 * k, r.h as f32 * k),
        );
        match it.tex {
            Some(tex) => {
                // la foto intera sta dietro, e si vede solo la parte ritagliata
                let (rot, c) = it.frame(turn);
                let full_size = vec2(shown.width() / (c[2] - c[0]), shown.height() / (c[3] - c[1]));
                let origin = shown.min - vec2(c[0] * full_size.x, c[1] * full_size.y);
                let full_rect = Rect::from_min_size(origin, full_size);
                let clipped = painter.with_clip_rect(shown);
                paint_image(&clipped, tex, full_rect.center(), full_size, rot);
            }
            None => {
                painter.rect_filled(shown, 0.0, Color32::from_rgb(150, 178, 215));
            }
        }
    }
}

/// Ritaglio a schermo intero (nessun ritaglio): [x0, y0, x1, y1] normalizzati 0..1.
const FULL_CROP: [f32; 4] = [0.0, 0.0, 1.0, 1.0];
/// Lato minimo di un ritaglio (2% dell'immagine).
const MIN_CROP: f32 = 0.02;

fn is_full(c: [f32; 4]) -> bool {
    c[0] <= 0.001 && c[1] <= 0.001 && c[2] >= 0.999 && c[3] >= 0.999
}

/// Ruota di un quarto di giro in senso orario un ritaglio normalizzato (come la foto ruotata).
fn rot_cw(c: [f32; 4]) -> [f32; 4] {
    [1.0 - c[3], c[0], 1.0 - c[1], c[2]]
}

/// Formato reale della foto alla stampa.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PhotoFmt {
    Page,
    Cm10x15,
    Cm13x18,
}

impl PhotoFmt {
    /// Dimensione in mm (lato corto, lato lungo); None = adatta alla pagina.
    fn mm(self) -> Option<(f32, f32)> {
        match self {
            PhotoFmt::Page => None,
            PhotoFmt::Cm10x15 => Some((100.0, 150.0)),
            PhotoFmt::Cm13x18 => Some((130.0, 180.0)),
        }
    }
    fn label(self) -> &'static str {
        match self {
            PhotoFmt::Page => "Adatta al foglio (massimo ingrandimento)",
            PhotoFmt::Cm10x15 => "10×15 cm (in proporzione)",
            PhotoFmt::Cm13x18 => "13×18 cm (in proporzione)",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Ico {
    Prev,
    Next,
    RotL,
    RotR,
    Print,
    PrintAll,
    Min,
    Max,
    Restore,
    Close,
    XSmall,
    Plus,
    Crop,
}

fn draw_icon(p: &egui::Painter, ico: Ico, c: Pos2, color: Color32) {
    let st = Stroke::new(1.7_f32, color);
    let v = |x: f32, y: f32| pos2(c.x + x, c.y + y);
    match ico {
        Ico::Prev => {
            p.add(Shape::line(vec![v(3.5, -7.0), v(-3.5, 0.0), v(3.5, 7.0)], st));
        }
        Ico::Next => {
            p.add(Shape::line(vec![v(-3.5, -7.0), v(3.5, 0.0), v(-3.5, 7.0)], st));
        }
        Ico::Min => {
            p.add(Shape::line_segment([v(-5.0, 0.0), v(5.0, 0.0)], st));
        }
        Ico::Max => {
            p.add(Shape::closed_line(vec![v(-5.0, -5.0), v(5.0, -5.0), v(5.0, 5.0), v(-5.0, 5.0)], st));
        }
        Ico::Restore => {
            p.add(Shape::line(vec![v(-2.0, -5.0), v(5.0, -5.0), v(5.0, 2.0)], st));
            p.add(Shape::closed_line(vec![v(-5.0, -2.0), v(2.0, -2.0), v(2.0, 5.0), v(-5.0, 5.0)], st));
        }
        Ico::Close | Ico::XSmall => {
            let s = if ico == Ico::Close { 5.0 } else { 3.5 };
            p.add(Shape::line_segment([v(-s, -s), v(s, s)], st));
            p.add(Shape::line_segment([v(-s, s), v(s, -s)], st));
        }
        Ico::Crop => {
            // quattro angoli di ritaglio
            for (sx, sy) in [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                p.add(Shape::line(vec![v(sx * 3.0, sy * 7.0), v(sx * 7.0, sy * 7.0), v(sx * 7.0, sy * 3.0)], st));
            }
        }
        Ico::Plus => {
            p.add(Shape::line_segment([v(-6.0, 0.0), v(6.0, 0.0)], st));
            p.add(Shape::line_segment([v(0.0, -6.0), v(0.0, 6.0)], st));
        }
        Ico::RotL | Ico::RotR => {
            let m = if ico == Ico::RotL { -1.0f32 } else { 1.0 };
            let (a0, a1) = ((-50.0f32).to_radians(), 220.0f32.to_radians());
            let r = 6.5f32;
            let pts: Vec<Pos2> = (0..=16)
                .map(|k| {
                    let a = a0 + (a1 - a0) * (k as f32) / 16.0;
                    pos2(c.x + m * r * a.cos(), c.y + r * a.sin())
                })
                .collect();
            p.add(Shape::line(pts, st));
            let end = pos2(c.x + m * r * a1.cos(), c.y + r * a1.sin());
            let t = vec2(m * (-a1.sin()), a1.cos());
            let n = vec2(m * a1.cos(), a1.sin());
            p.add(Shape::convex_polygon(vec![end + t * 5.0, end + n * 3.8, end - n * 3.8], color, Stroke::NONE));
        }
        Ico::Print | Ico::PrintAll => {
            p.add(Shape::closed_line(vec![v(-4.0, -7.0), v(4.0, -7.0), v(4.0, -2.0), v(-4.0, -2.0)], st));
            p.add(Shape::closed_line(vec![v(-7.5, -2.0), v(7.5, -2.0), v(7.5, 4.5), v(-7.5, 4.5)], st));
            p.add(Shape::closed_line(vec![v(-4.0, 1.5), v(4.0, 1.5), v(4.0, 7.5), v(-4.0, 7.5)], st));
            if ico == Ico::PrintAll {
                p.circle_filled(v(7.5, -6.5), 3.8, col::accent());
            }
        }
    }
}

/// Pulsante della barra del titolo (minimizza / ingrandisci / chiudi), a tutta altezza.
fn caption_button(ui: &mut egui::Ui, ico: Ico) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(46.0, TITLE_H), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let hover = resp.hovered();
        let (bg, fg) = if ico == Ico::Close && hover {
            (col::close_red(), Color32::WHITE)
        } else if hover {
            (Color32::from_white_alpha(22), col::text())
        } else {
            (Color32::TRANSPARENT, col::text())
        };
        ui.painter().rect_filled(rect, 0.0, bg);
        draw_icon(ui.painter(), ico, rect.center(), fg);
    }
    resp
}

/// Pulsante strumento con icona disegnata e suggerimento.
fn tool_button(ui: &mut egui::Ui, ico: Ico, tip: &str, enabled: bool) -> egui::Response {
    let sense = if enabled { egui::Sense::click() } else { egui::Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(vec2(32.0, 30.0), sense);
    if ui.is_rect_visible(rect) {
        if resp.hovered() && enabled {
            ui.painter().rect_filled(rect.shrink(1.0), 6.0, Color32::from_white_alpha(24));
        }
        let color = if enabled { col::text() } else { Color32::from_gray(90) };
        draw_icon(ui.painter(), ico, rect.center(), color);
    }
    resp.on_hover_text(tip)
}

fn menu_item(ui: &mut egui::Ui, text: &str, shortcut: &str, enabled: bool) -> bool {
    let mut b = egui::Button::new(text);
    if !shortcut.is_empty() {
        b = b.shortcut_text(shortcut);
    }
    let r = ui.add_enabled(enabled, b);
    if r.clicked() {
        ui.close_menu();
        true
    } else {
        false
    }
}
