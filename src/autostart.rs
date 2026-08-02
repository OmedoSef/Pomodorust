//! Gestion du lancement automatique au démarrage de la session, via un
//! fichier `.desktop` XDG Autostart dans `~/.config/autostart/`.

use std::path::PathBuf;

const AUTOSTART_FILE_NAME: &str = "pomodorust.desktop";

fn autostart_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".config").join("autostart"))
}

fn autostart_file() -> Option<PathBuf> {
    autostart_dir().map(|dir| dir.join(AUTOSTART_FILE_NAME))
}

/// Chemin de l'exécutable actuellement en cours d'exécution, utilisé comme
/// cible `Exec=` du fichier autostart (fonctionne aussi bien en
/// développement qu'une fois installé via `scripts/install.sh`).
fn current_exe_path() -> String {
    std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "pomodorust".to_string())
}

/// Synchronise le fichier autostart XDG avec la valeur voulue. Idempotent :
/// peut être appelé à chaque démarrage de l'application sans effet de bord
/// si l'état est déjà correct.
pub fn sync(enabled: bool) {
    let Some(path) = autostart_file() else {
        eprintln!("Impossible de déterminer $HOME, autostart non synchronisé.");
        return;
    };

    if enabled {
        if let Some(dir) = autostart_dir() {
            if let Err(err) = std::fs::create_dir_all(&dir) {
                eprintln!("Impossible de créer {} : {err}", dir.display());
                return;
            }
        }
        let exe = current_exe_path();
        let contents = format!(
            "[Desktop Entry]\n\
             Type=Application\n\
             Name=Pomodorust\n\
             Comment=Minuteur Pomodoro dans la barre système\n\
             Exec={exe}\n\
             Terminal=false\n\
             X-GNOME-Autostart-enabled=true\n"
        );
        if let Err(err) = std::fs::write(&path, contents) {
            eprintln!("Impossible d'écrire {} : {err}", path.display());
        }
    } else if path.exists() {
        if let Err(err) = std::fs::remove_file(&path) {
            eprintln!("Impossible de supprimer {} : {err}", path.display());
        }
    }
}
