//! Coda per "Stampa con…" dal menu destro di Esplora risorse.
//! Windows lancia un processo per ogni foto selezionata: ogni processo deposita il suo file
//! nella cartella di coda, poi uno solo (quello che ottiene il lock) raccoglie tutte le foto
//! arrivate in poco tempo e apre una sola finestra di stampa. Gli altri escono subito.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Pausa senza nuove foto dopo la quale la raccolta si chiude.
const QUIET: Duration = Duration::from_millis(500);
/// Un lock più vecchio di così appartiene a un processo interrotto.
const LOCK_STALE: Duration = Duration::from_secs(10);
/// Tempo massimo di attesa per ottenere il lock.
const LOCK_WAIT: Duration = Duration::from_secs(15);

fn queue_dir() -> PathBuf {
    std::env::temp_dir().join("VisualizzatoreImmaginiCP-print")
}

/// Deposita `file` nella coda. Restituisce le foto da stampare se questo processo le raccoglie;
/// lista vuota = un altro processo le ha già raccolte, questo esce.
pub fn submit_and_collect(file: &Path) -> Vec<PathBuf> {
    submit_in(&queue_dir(), file)
}

fn submit_in(dir: &Path, file: &Path) -> Vec<PathBuf> {
    if fs::create_dir_all(dir).is_err() {
        return vec![file.to_path_buf()];
    }
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let name = format!("q-{}-{}", std::process::id(), nanos);
    let tmp = dir.join(format!("{name}.tmp"));
    let entry = dir.join(format!("{name}.txt"));
    // scrittura in .tmp e poi rinomina: il raccoglitore non legge mai un file a metà
    if fs::write(&tmp, file.as_os_str().to_string_lossy().as_bytes()).is_err()
        || fs::rename(&tmp, &entry).is_err()
    {
        return vec![file.to_path_buf()];
    }

    let lock = dir.join("collect.lock");
    let start = Instant::now();
    loop {
        if fs::OpenOptions::new().write(true).create_new(true).open(&lock).is_ok() {
            break;
        }
        if start.elapsed() > LOCK_WAIT {
            return Vec::new();
        }
        if is_stale(&lock) {
            let _ = fs::remove_file(&lock);
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    // raccolta: finché arrivano foto nuove, si continua
    let mut paths = Vec::new();
    loop {
        std::thread::sleep(QUIET);
        let batch = take_all(dir);
        if batch.is_empty() {
            break;
        }
        paths.extend(batch);
    }
    let _ = fs::remove_file(&lock);
    paths
}

/// Legge e rimuove tutte le voci in coda.
fn take_all(dir: &Path) -> Vec<PathBuf> {
    let mut entries: Vec<PathBuf> = match fs::read_dir(dir) {
        Ok(rd) => rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("txt"))
            .collect(),
        Err(_) => return Vec::new(),
    };
    entries.sort();
    let mut out = Vec::new();
    for p in entries {
        if let Ok(text) = fs::read_to_string(&p) {
            let _ = fs::remove_file(&p);
            if !text.is_empty() {
                out.push(PathBuf::from(text));
            }
        }
    }
    out
}

fn is_stale(lock: &Path) -> bool {
    fs::metadata(lock)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .map_or(false, |age| age > LOCK_STALE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_all_queued_files() {
        let dir = std::env::temp_dir().join(format!("vic-queue-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        // una foto già in coda da un altro processo
        fs::write(dir.join("q-other-1.txt"), "/foto/a.jpg").unwrap();
        let got = submit_in(&dir, Path::new("/foto/b.jpg"));
        assert_eq!(got.len(), 2);
        assert!(got.contains(&PathBuf::from("/foto/a.jpg")));
        assert!(got.contains(&PathBuf::from("/foto/b.jpg")));
        // la coda è vuota e il lock liberato
        assert!(!dir.join("collect.lock").exists());
        assert!(take_all(&dir).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}
