//! Construction et reconstruction du menu de l'icône de la barre système.
//!
//! Le menu est entièrement reconstruit (via [`rebuild_menu`]) à chaque fois
//! que la liste des préréglages change, et remplacé dans le `TrayIcon` via
//! `set_menu`. Les nouvelles références aux éléments (nécessaires pour
//! changer leur texte/état plus tard) sont retournées dans [`TrayHandles`].

use crate::config::Config;
use tray_icon::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

pub const ID_START_PAUSE: &str = "start_pause";
pub const ID_SKIP: &str = "skip";
pub const ID_RESET: &str = "reset";
pub const ID_SETTINGS: &str = "settings";
pub const ID_QUIT: &str = "quit";
pub const PRESET_ID_PREFIX: &str = "preset:";

pub const LABEL_START: &str = "▶ Démarrer";
pub const LABEL_PAUSE: &str = "⏸ Pause";
pub const LABEL_RESUME_WORK: &str = "▶ Reprendre le travail";

/// Références stables vers les éléments de menu qu'on doit pouvoir mettre à
/// jour après leur création (texte du bouton démarrer/pause, coche du
/// préréglage actif, etc).
pub struct TrayHandles {
    /// Ligne non cliquable en haut du menu, tenue à jour avec la phase et le
    /// temps restant : c'est le repli garanti pour voir l'état du minuteur,
    /// puisque tous les environnements de bureau n'affichent pas l'infobulle
    /// des icônes AppIndicator au survol.
    pub status_item: MenuItem,
    pub start_pause_item: MenuItem,
    #[allow(dead_code)]
    pub skip_item: MenuItem,
    #[allow(dead_code)]
    pub reset_item: MenuItem,
    #[allow(dead_code)]
    pub settings_item: MenuItem,
    #[allow(dead_code)]
    pub quit_item: MenuItem,
    #[allow(dead_code)]
    pub preset_items: Vec<(String, CheckMenuItem)>,
}

fn build_menu(config: &Config, running: bool) -> (Menu, TrayHandles) {
    let menu = Menu::new();

    let status_item = MenuItem::with_id("status", "Pomodorust — Prêt", false, None);

    let start_pause_item = MenuItem::with_id(
        ID_START_PAUSE,
        if running { LABEL_PAUSE } else { LABEL_START },
        true,
        None,
    );
    let skip_item = MenuItem::with_id(ID_SKIP, "⏭ Passer la phase", true, None);
    let reset_item = MenuItem::with_id(ID_RESET, "⏹ Réinitialiser", true, None);

    let preset_submenu = Submenu::new("Preset actif", true);
    let mut preset_items = Vec::with_capacity(config.presets.len());
    for preset in &config.presets {
        let id = format!("{PRESET_ID_PREFIX}{}", preset.name);
        let checked = preset.name == config.active_preset;
        let item = CheckMenuItem::with_id(id, &preset.name, true, checked, None);
        preset_submenu
            .append(&item)
            .expect("échec de l'ajout d'un préréglage au sous-menu");
        preset_items.push((preset.name.clone(), item));
    }

    let settings_item = MenuItem::with_id(ID_SETTINGS, "⚙ Réglages…", true, None);
    let quit_item = MenuItem::with_id(ID_QUIT, "Quitter", true, None);

    menu.append_items(&[
        &status_item,
        &PredefinedMenuItem::separator(),
        &start_pause_item,
        &skip_item,
        &reset_item,
        &PredefinedMenuItem::separator(),
        &preset_submenu,
        &PredefinedMenuItem::separator(),
        &settings_item,
        &PredefinedMenuItem::separator(),
        &quit_item,
    ])
    .expect("échec de la construction du menu de la barre système");

    (
        menu,
        TrayHandles {
            status_item,
            start_pause_item,
            skip_item,
            reset_item,
            settings_item,
            quit_item,
            preset_items,
        },
    )
}

/// Construit l'icône de la barre système avec son menu initial.
pub fn build_tray(icon: Icon, config: &Config) -> (TrayIcon, TrayHandles) {
    let (menu, handles) = build_menu(config, false);
    let tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Pomodorust — Prêt")
        .with_icon(icon)
        .build()
        .expect("échec de la création de l'icône de la barre système");
    (tray_icon, handles)
}

/// Reconstruit entièrement le menu (par ex. après modification des
/// préréglages) et le remplace dans le `TrayIcon` existant.
pub fn rebuild_menu(tray_icon: &TrayIcon, config: &Config, running: bool) -> TrayHandles {
    let (menu, handles) = build_menu(config, running);
    tray_icon.set_menu(Some(Box::new(menu)));
    handles
}
