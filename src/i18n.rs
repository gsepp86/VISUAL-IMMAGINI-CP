//! Traduzioni dell'interfaccia: italiano (predefinito) e inglese, scelto dalle Impostazioni.

use std::sync::atomic::{AtomicBool, Ordering};

static ENGLISH: AtomicBool = AtomicBool::new(false);

pub fn set_english(on: bool) {
    ENGLISH.store(on, Ordering::Relaxed);
}

pub fn is_english() -> bool {
    ENGLISH.load(Ordering::Relaxed)
}

/// Sostituisce in ordine i segnaposto `{}` del testo tradotto (i valori vanno già formattati).
pub fn fill(template: &str, args: &[String]) -> String {
    let mut out = String::with_capacity(template.len() + 24);
    let mut rest = template;
    for a in args {
        match rest.find("{}") {
            Some(i) => {
                out.push_str(&rest[..i]);
                out.push_str(a);
                rest = &rest[i + 2..];
            }
            None => break,
        }
    }
    out.push_str(rest);
    out
}

/// Testo dell'interfaccia nella lingua scelta. La chiave è il testo italiano.
pub fn t(it: &'static str) -> &'static str {
    if !is_english() {
        return it;
    }
    match it {
        // ----- menu e pulsanti -----
        "Apri immagine…" => "Open image…",
        "Apri cartella…" => "Open folder…",
        "Apri con… (per modificare)" => "Open with… (to edit)",
        "Chiudi scheda" => "Close tab",
        "Salva ritaglio come copia…" => "Save crop as copy…",
        "Stampa…" => "Print…",
        "Stampa tutto…" => "Print all…",
        "Imposta come programma predefinito…" => "Set as default program…",
        "Impostazioni…" => "Settings…",
        "Info" => "About",
        "Esci" => "Exit",
        "Vista" => "View",
        "Anteprime dei file successivi" => "Previews of the next files",
        "Adatta alla finestra" => "Fit to window",
        "Dimensione reale (100%)" => "Actual size (100%)",
        "Ruota a destra" => "Rotate right",
        "Ruota a sinistra" => "Rotate left",
        "Ritaglia…" => "Crop…",
        "Schermo intero" => "Full screen",
        "Ctrl+Maiusc+P" => "Ctrl+Shift+P",
        "Maiusc+R" => "Shift+R",
        "Stampa tutto (Ctrl+Maiusc+P)" => "Print all (Ctrl+Shift+P)",
        "Stampa (Ctrl+P)" => "Print (Ctrl+P)",
        "Ruota a destra (R)" => "Rotate right (R)",
        "Ruota a sinistra (Maiusc+R)" => "Rotate left (Shift+R)",
        "Ritaglia (C)" => "Crop (C)",
        "Successiva (→)" => "Next (→)",
        "Precedente (←)" => "Previous (←)",
        "Apri in una nuova scheda (Ctrl+O)" => "Open in a new tab (Ctrl+O)",

        // ----- vista e stato -----
        "Immagine" => "Image",
        "Trascina qui un'immagine oppure premi Ctrl+O" => "Drag an image here or press Ctrl+O",
        "Impossibile aprire questo file\n{}" => "Cannot open this file\n{}",
        "Caricamento…" => "Loading…",
        "ritagliata" => "cropped",
        "{}   •   {}{}   •   {} di {}   •   {}" => "{}   •   {}{}   •   {} of {}   •   {}",

        // ----- dialogo dei ritagli -----
        "Ritagli" => "Crops",
        "Ci sono {} foto ritagliate, ma il ritaglio non modifica i file originali:\nle immagini NON sono state salvate definitivamente.\n\nVuoi mantenere i ritagli alla prossima apertura?" => {
            "There are {} cropped photos, but cropping does not modify the original files:\nthe images were NOT saved permanently.\n\nDo you want to keep the crops for next time?"
        }
        "Mantieni" => "Keep",
        "Scarta" => "Discard",
        "Annulla" => "Cancel",
        "Ritaglio libero: {}% × {}%" => "Free crop: {}% × {}",
        "Applica (Invio)" => "Apply (Enter)",
        "Ripristina" => "Reset",
        "Annulla (Esc)" => "Cancel (Esc)",
        "Apri immagine" => "Open image",
        "Apri cartella" => "Open folder",
        "Immagini" => "Images",
        "Tutti i file" => "All files",
        "Salvataggio non riuscito" => "Save failed",
        "immagine" => "image",
        "{}_ritaglio.jpg" => "{}_crop.jpg",

        // ----- impostazioni -----
        "Impostazioni" => "Settings",
        "Lingua" => "Language",
        "Tema" => "Theme",
        "Scuro (predefinito)" => "Dark (default)",
        "Chiaro" => "Light",
        "Stampante all'avvio" => "Startup printer",
        "Predefinita di Windows" => "Windows default",
        "Ultima utilizzata" => "Last used",
        "È la stampante proposta quando apri la finestra di stampa." => {
            "This is the printer proposed when you open the print window."
        }

        // ----- info -----
        "Versione {}" => "Version {}",
        "Lettore immagini opensource creato da Peppe CP in collaborazione con IA" => {
            "Open source image viewer made by Peppe CP with the help of AI"
        }
        "Licenza MIT  •  Scritto in Rust con egui" => "MIT License  •  Written in Rust with egui",
        "Se vuoi contribuire al mio lavoro, puoi farlo con una piccola donazione simbolica: un'app così utile e gratuita per sempre non la trovi ovunque. Apprezzo il gesto ;-)" => {
            "If you'd like to support my work, you can do so with a small symbolic donation: an app this useful and free forever is not easy to find. I appreciate it ;-)"
        }
        "Cerca aggiornamenti" => "Check for updates",
        "Controllo in corso…" => "Checking…",
        "Hai già l'ultima versione ({})." => "You already have the latest version ({}).",
        "È disponibile la versione {}." => "Version {} is available.",
        "Controllo non riuscito: {}" => "Check failed: {}",
        "Scarica la nuova versione" => "Download the new version",
        "Codice sorgente su GitHub" => "Source code on GitHub",
        "nessuna versione pubblicata" => "no published release",
        "risposta non valida" => "invalid response",

        // ----- stampa -----
        "Stampa" => "Print",
        "La stampa è disponibile solo su Windows." => "Printing is only available on Windows.",
        "Stampante:" => "Printer:",
        "Nessuna stampante trovata" => "No printer found",
        "Cosa stampare" => "What to print",
        "Immagine corrente" => "Current image",
        "Tutte le immagini della cartella ({})" => "All images in the folder ({})",
        "Immagini di tutte le schede ({})" => "Images from all tabs ({})",
        "Immagini per pagina" => "Images per page",
        "{} in 1" => "{} per sheet",
        "Stessa foto più volte nel foglio" => "Same photo multiple times on the sheet",
        "Ogni foto occupa {} posizioni nel foglio" => "Each photo takes {} positions on the sheet",
        "Orientamento" => "Orientation",
        "Automatico (miglior verso)" => "Automatic (best direction)",
        "Verticale" => "Portrait",
        "Orizzontale" => "Landscape",
        "Colori di stampa" => "Print colors",
        "Automatico" => "Automatic",
        "Colori" => "Color",
        "Bianco e nero" => "Black and white",
        "Bianco e nero stampa in scala di grigi; Automatico lo usa solo per le foto già in grigio" => {
            "Black and white prints in grayscale; Automatic uses it only for photos already in gray"
        }
        "Margine (mm)" => "Margin (mm)",
        "Margine minimo della stampante: {} mm (misurato dal bordo del foglio)" => {
            "Printer minimum margin: {} mm (measured from the paper edge)"
        }
        "Spazio tra le foto (mm)" => "Space between photos (mm)",
        "Dimensione foto" => "Photo size",
        "Adatta al foglio (massimo ingrandimento)" => "Fit the sheet (maximum enlargement)",
        "10×15 cm (in proporzione)" => "10×15 cm (proportional)",
        "13×18 cm (in proporzione)" => "13×18 cm (proportional)",
        "Personalizzato (in proporzione)" => "Custom (proportional)",
        "Larghezza" => "Width",
        "Altezza" => "Height",
        "Poster su più fogli" => "Poster across multiple sheets",
        "In orizzontale" => "Across",
        "In verticale" => "Down",
        "Una sola immagine distribuita su {} fogli. Per unirli senza strisce, sovrapponi i bordi di circa {} mm." => {
            "A single image spread over {} sheets. To join them without seams, overlap the edges by about {} mm."
        }
        "Numero di copie" => "Number of copies",
        "Usa queste impostazioni come predefinite" => "Use these settings as defaults",
        "Ogni foto a {}×{} cm, ridotta in proporzione se il foglio è più piccolo" => {
            "Each photo at {}×{} cm, scaled proportionally if the sheet is smaller"
        }
        "Anteprima" => "Preview",
        "Anteprima reale: prima pagina, come verrà stampata" => "Real preview: first page as it will be printed",
        "Poster: {} fogli" => "Poster: {} sheets",
        "{} immagini → {} pagine" => "{} images → {} pages",
        "Regolazione foto" => "Photo adjustment",
        "Chiaro / scuro" => "Brightness",
        "Contrasto" => "Contrast",
        "Preferenze e driver della stampante…" => "Printer preferences and driver…",
        "Stampa in corso… {} di {}" => "Printing… {} of {}",
        "Annulla stampa" => "Cancel printing",
        "Chiudi" => "Close",
        "Stampa completata." => "Printing completed.",
        "Stampa annullata." => "Printing cancelled.",
        "Errore: {}" => "Error: {}",
        "Nessuna stampante disponibile." => "No printer available.",
        "Nessuna immagine da stampare." => "No images to print.",
        "Dimensioni: in caricamento…" => "Size: loading…",
        "Foto corrente: {} × {} px → a 300 dpi: {} × {} cm" => {
            "Current photo: {} × {} px → at 300 dpi: {} × {} cm"
        }

        // ----- errori della stampa, del caricamento e del driver -----
        "La stampante non ha restituito un'area di stampa valida." => "The printer did not return a valid printable area.",
        "errore interno" => "internal error",
        "Impossibile aprire la stampante \"{}\"." => "Cannot open printer \"{}\".",
        "La stampante ha rifiutato il documento (StartDoc)." => "The printer rejected the document (StartDoc).",
        "Impossibile iniziare la pagina (StartPage)." => "Cannot start the page (StartPage).",
        "Impossibile chiudere la pagina (EndPage)." => "Cannot end the page (EndPage).",
        "Immagine di stampa non valida." => "Invalid print image.",
        "La stampante non ha accettato l'immagine (StretchDIBits)." => {
            "The printer did not accept the image (StretchDIBits)."
        }
        "Errore interno durante la decodifica" => "Internal error while decoding",
        "Questo RAW non contiene un'anteprima JPEG leggibile." => {
            "This RAW file does not contain a readable JPEG preview."
        }
        "SVG senza dimensioni" => "SVG without dimensions",
        "SVG troppo grande" => "SVG too large",
        "SVG non valido" => "Invalid SVG",
        "Supporto SVG non incluso in questa versione" => "SVG support not included in this version",
        "Immagine JPEG XL non valida" => "Invalid JPEG XL image",
        "Supporto JPEG XL non incluso in questa versione" => "JPEG XL support not included in this version",
        "Per HEIC/AVIF installa da Microsoft Store \"Estensioni immagine HEIF\" o \"Estensioni immagine AVIF\"." => {
            "For HEIC/AVIF, install \"HEIF Image Extensions\" or \"AVIF Image Extensions\" from the Microsoft Store."
        }
        "Immagine HEIC/AVIF non valida" => "Invalid HEIC/AVIF image",
        "HEIC/AVIF: disponibili solo su Windows con la funzione wic" => {
            "HEIC/AVIF: available only on Windows with the wic feature"
        }

        _ => it,
    }
}
