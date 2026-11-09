// Incorpora l'icona e le informazioni di versione nell'eseguibile Windows.
fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("ProductName", "Visualizzatore Immagini CP");
        res.set("FileDescription", "Visualizzatore Immagini CP");
        res.set("CompanyName", "Peppe CP");
        res.set("LegalCopyright", "Peppe CP - licenza MIT");
        if let Err(e) = res.compile() {
            // non è bloccante: il programma funziona anche senza icona incorporata
            println!("cargo:warning=Impossibile incorporare l'icona nell'exe: {e}");
        }
    }
}
