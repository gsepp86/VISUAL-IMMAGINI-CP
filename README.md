# Visualizzatore Immagini CP

## Se vuoi contribuire al mio lavoro ti lascio in basso il pulsante per contribuire anche con una piccola donazione simbolica visto che una app così utile gratis per sempre non la trovi ovunque apprezzo il gesto ;-)

[![Donate via PayPal](https://img.shields.io/badge/Donate-PayPal-blue.svg?logo=paypal)](https://paypal.me/PeppeCP)

Lettore immagini opensource creato da Peppe CP in collaborazione con IA.

Semplice come il Visualizzatore foto di Windows 7, ma moderno e veloce. Scritto in Rust.

## Funzioni

- **Interfaccia a schede** nella barra del titolo (Ctrl+O apre in una nuova scheda, Ctrl+W la chiude)
- **Pulsanti nella barra del titolo**: precedente, successiva, ruota, stampa, stampa tutto
- **Anteprima di stampa reale**: mostra la prima pagina come verrà stampata (foto vere, ritaglio, rotazione, margini, formato foto e area stampabile della stampante)
- **Anteprima dinamica** dei file successivi nella striscia in basso (si può nascondere da Vista)
- **Stampa N-in-1**: 1, 2, 4, 6, 8, 9, 12, 16 immagini per pagina, orientamento, margini, spazio tra le foto
- **Ritaglio libero** (tasto C o icona della barra): trascina angoli, lati o l'interno; Invio applica, Esc annulla. Il ritaglio entra nella stampa.
- **Orientamento automatico**: per ogni foto sceglie il verso migliore, e il foglio si orienta da solo
- **Regolazione foto**: chiaro/scuro e contrasto nella finestra di stampa (anche nell'anteprima)
- **Preferenze e driver della stampante**: apre le impostazioni del driver per modifiche avanzate
- **Formato foto**: adatta alla pagina, oppure 10×15 cm o 13×18 cm stampate in proporzione (se il foglio è più piccolo la foto si riduce senza deformarsi)
- **Menu destro in Esplora risorse**: seleziona una o più foto → "Stampa con Visualizzatore Immagini CP" apre le foto nelle schede e la finestra di stampa
- **Stampa tutto**: tutta la cartella o le immagini di tutte le schede
- Zoom con rotella, trascinamento, doppio clic (100% / adatta), rotazione con R
- Formati: JPG, PNG, GIF, BMP, ICO, TIFF, WebP, TGA, DDS, HDR, EXR, PNM, QOI, SVG
- **Foto RAW** (CR2, CR3, NEF, ARW, DNG, ORF, RW2, RAF, PEF, SRW…): si mostra la JPEG di anteprima incorporata, quindi l'apertura è istantanea
- JPEG XL: supportato (decoder Rust integrato, nessuna DLL)
- HEIC / AVIF: decodificati con i codec di Windows. Serve l'estensione gratuita "Estensioni immagine HEIF" e/o "Estensioni immagine AVIF" da Microsoft Store
- Menu **File > Info**: "Lettore immagini opensource creato da Peppe CP in collaborazione con IA"

## Velocità

- Decodifica su più thread: l'interfaccia non si blocca mai
- Per le foto grandi si mostra subito un'anteprima, poi arriva la versione completa
- Si caricano in anticipo l'immagine precedente e le successive
- Se scorri velocemente, le immagini saltate vengono scartate dalla coda
- Disegno accelerato dalla GPU (OpenGL), nessuna DLL da installare

## Scorciatoie

| Tasto | Azione |
|---|---|
| ← → / Spazio / Backspace | precedente / successiva |
| Home / End | prima / ultima |
| 0 / 1 | adatta alla finestra / 100% |
| R / Maiusc+R | ruota a destra / sinistra |
| Ctrl+O / Ctrl+W | apri / chiudi scheda |
| Ctrl+P / Ctrl+Maiusc+P | stampa / stampa tutto |
| F11 | schermo intero |

## Compilare

Su Windows, con [Rust](https://rustup.rs) e [NSIS](https://nsis.sourceforge.io):

```
build-windows.bat
```

Crea nella cartella `dist`: l'installer e l'exe portatile.
Senza Windows: carica il progetto su GitHub, il workflow `.github/workflows/build.yml` compila tutto da solo.

Per rigenerare l'icona: `python tools/make_icon.py`.

## Licenza

MIT
