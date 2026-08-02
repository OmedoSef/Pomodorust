//! Modèle de configuration persistée en TOML sous
//! `<XDG_CONFIG_HOME>/pomodorust/config.toml`.

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Un préréglage nommé de durées pour le minuteur.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Preset {
    pub name: String,
    pub work_min: u32,
    pub short_break_min: u32,
    pub long_break_min: u32,
    pub cycles_before_long_break: u32,
}

impl Preset {
    pub fn default_preset() -> Self {
        Preset {
            name: "Classique".to_string(),
            work_min: 25,
            short_break_min: 5,
            long_break_min: 15,
            cycles_before_long_break: 4,
        }
    }
}

/// La configuration complète de l'application.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Config {
    pub presets: Vec<Preset>,
    pub active_preset: String,
    pub autostart: bool,
    /// `#[serde(default)]` : les fichiers de configuration écrits avant
    /// l'ajout de ce champ continuent de se charger sans erreur.
    #[serde(default)]
    pub notification_sound: crate::sound::SoundChoice,
}

impl Default for Config {
    fn default() -> Self {
        let preset = Preset::default_preset();
        Config {
            active_preset: preset.name.clone(),
            presets: vec![preset],
            autostart: true,
            notification_sound: crate::sound::SoundChoice::default(),
        }
    }
}

impl Config {
    /// Répertoire de configuration XDG dédié à l'application.
    pub fn config_dir() -> PathBuf {
        ProjectDirs::from("dev", "pomodorust", "Pomodorust")
            .expect("impossible de déterminer le répertoire de configuration utilisateur")
            .config_dir()
            .to_path_buf()
    }

    pub fn config_file() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    /// Charge la configuration depuis le disque, ou crée et sauvegarde la
    /// configuration par défaut si le fichier n'existe pas encore.
    pub fn load_or_init() -> Self {
        let path = Self::config_file();
        if let Ok(contents) = std::fs::read_to_string(&path) {
            match toml::from_str::<Config>(&contents) {
                Ok(mut cfg) => {
                    cfg.ensure_valid();
                    return cfg;
                }
                Err(err) => {
                    eprintln!(
                        "Impossible d'analyser {} ({err}), utilisation des valeurs par défaut.",
                        path.display()
                    );
                }
            }
        }
        let cfg = Config::default();
        if let Err(err) = cfg.save() {
            eprintln!("Impossible d'écrire la configuration initiale : {err}");
        }
        cfg
    }

    /// Garantit qu'il existe toujours au moins un préréglage et que
    /// `active_preset` pointe vers un préréglage existant.
    fn ensure_valid(&mut self) {
        if self.presets.is_empty() {
            self.presets.push(Preset::default_preset());
        }
        if !self.presets.iter().any(|p| p.name == self.active_preset) {
            self.active_preset = self.presets[0].name.clone();
        }
    }

    pub fn active_preset(&self) -> &Preset {
        self.presets
            .iter()
            .find(|p| p.name == self.active_preset)
            .unwrap_or(&self.presets[0])
    }

    pub fn save(&self) -> std::io::Result<()> {
        let dir = Self::config_dir();
        std::fs::create_dir_all(&dir)?;
        let contents = toml::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(Self::config_file(), contents)
    }
}
