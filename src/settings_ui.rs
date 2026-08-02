//! Fenêtre de réglages en GTK3 pur (gtk-rs). Créée une seule fois au
//! démarrage, cachée par défaut, réutilisée à chaque ouverture depuis le
//! menu de la barre système.

use crate::config::{Config, Preset};
use crate::sound::SoundChoice;
use crate::tray;
use crate::AppState;
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Toutes les références de widgets dont on a besoin après la construction
/// initiale (pour les rafraîchir à l'ouverture ou lire leurs valeurs dans
/// les gestionnaires de clic).
#[derive(Clone)]
pub struct SettingsWidgets {
    pub window: gtk::Window,
    presets_combo: gtk::ComboBoxText,
    name_entry: gtk::Entry,
    work_spin: gtk::SpinButton,
    short_spin: gtk::SpinButton,
    long_spin: gtk::SpinButton,
    cycles_spin: gtk::SpinButton,
    autostart_check: gtk::CheckButton,
    sound_combo: gtk::ComboBoxText,
    custom_sound_label: gtk::Label,
    /// Vrai pendant une mise à jour programmatique de `sound_combo` (ex. à
    /// l'ouverture de la fenêtre) : le gestionnaire "changed" l'ignore, pour
    /// ne pas rouvrir le sélecteur de fichier alors que l'utilisateur n'a
    /// rien cliqué.
    sound_syncing: Rc<Cell<bool>>,
}

/// Construit la fenêtre de réglages (cachée) et branche tous les
/// gestionnaires d'évènements. `app_state` est partagé avec le reste de
/// l'application (menu de la barre système, minuteur).
pub fn build(icon: &gtk::gdk_pixbuf::Pixbuf, app_state: Rc<RefCell<AppState>>) -> SettingsWidgets {
    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.set_title("Pomodorust — Réglages");
    window.set_default_size(380, 480);
    window.set_icon(Some(icon));
    window.set_border_width(12);

    let root = gtk::Box::new(gtk::Orientation::Vertical, 10);

    root.pack_start(&gtk::Label::new(Some("Préréglages")), false, false, 0);

    let presets_combo = gtk::ComboBoxText::new();
    root.pack_start(&presets_combo, false, false, 0);

    let grid = gtk::Grid::new();
    grid.set_row_spacing(6);
    grid.set_column_spacing(10);

    let name_entry = gtk::Entry::new();
    let work_spin = gtk::SpinButton::with_range(1.0, 180.0, 1.0);
    let short_spin = gtk::SpinButton::with_range(1.0, 60.0, 1.0);
    let long_spin = gtk::SpinButton::with_range(1.0, 120.0, 1.0);
    let cycles_spin = gtk::SpinButton::with_range(1.0, 12.0, 1.0);

    grid.attach(&gtk::Label::new(Some("Nom")), 0, 0, 1, 1);
    grid.attach(&name_entry, 1, 0, 1, 1);
    grid.attach(&gtk::Label::new(Some("Travail (min)")), 0, 1, 1, 1);
    grid.attach(&work_spin, 1, 1, 1, 1);
    grid.attach(&gtk::Label::new(Some("Pause courte (min)")), 0, 2, 1, 1);
    grid.attach(&short_spin, 1, 2, 1, 1);
    grid.attach(&gtk::Label::new(Some("Pause longue (min)")), 0, 3, 1, 1);
    grid.attach(&long_spin, 1, 3, 1, 1);
    grid.attach(&gtk::Label::new(Some("Cycles avant pause longue")), 0, 4, 1, 1);
    grid.attach(&cycles_spin, 1, 4, 1, 1);

    root.pack_start(&grid, false, false, 0);

    let buttons_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let save_btn = gtk::Button::with_label("Enregistrer");
    let set_active_btn = gtk::Button::with_label("Définir comme actif");
    let delete_btn = gtk::Button::with_label("Supprimer");
    buttons_box.pack_start(&save_btn, true, true, 0);
    buttons_box.pack_start(&set_active_btn, true, true, 0);
    buttons_box.pack_start(&delete_btn, true, true, 0);
    root.pack_start(&buttons_box, false, false, 0);

    root.pack_start(&gtk::Separator::new(gtk::Orientation::Horizontal), false, false, 6);

    root.pack_start(&gtk::Label::new(Some("Son de notification")), false, false, 0);
    let sound_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let sound_combo = gtk::ComboBoxText::new();
    for choice in SoundChoice::BUILTIN {
        sound_combo.append(Some(choice.combo_id()), choice.label());
    }
    sound_combo.append(Some("custom"), "Personnalisé…");
    let test_sound_btn = gtk::Button::with_label("🔊 Tester");
    sound_row.pack_start(&sound_combo, true, true, 0);
    sound_row.pack_start(&test_sound_btn, false, false, 0);
    root.pack_start(&sound_row, false, false, 0);

    let custom_sound_label = gtk::Label::new(None);
    custom_sound_label.set_halign(gtk::Align::Start);
    root.pack_start(&custom_sound_label, false, false, 0);

    root.pack_start(&gtk::Separator::new(gtk::Orientation::Horizontal), false, false, 6);

    let autostart_check = gtk::CheckButton::with_label("Lancer au démarrage");
    root.pack_start(&autostart_check, false, false, 0);

    let status_label = gtk::Label::new(None);
    root.pack_start(&status_label, false, false, 0);

    window.add(&root);

    let widgets = SettingsWidgets {
        window: window.clone(),
        presets_combo: presets_combo.clone(),
        name_entry: name_entry.clone(),
        work_spin: work_spin.clone(),
        short_spin: short_spin.clone(),
        long_spin: long_spin.clone(),
        cycles_spin: cycles_spin.clone(),
        autostart_check: autostart_check.clone(),
        sound_combo: sound_combo.clone(),
        custom_sound_label: custom_sound_label.clone(),
        sound_syncing: Rc::new(Cell::new(false)),
    };

    // Cacher plutôt que détruire la fenêtre à la fermeture, pour pouvoir la
    // rouvrir depuis le menu de la barre système.
    window.connect_delete_event(|win, _| {
        win.hide();
        glib::Propagation::Stop
    });

    // Sélectionner un préréglage dans la liste recharge ses valeurs dans
    // les champs d'édition.
    {
        let widgets = widgets.clone();
        let app_state = Rc::clone(&app_state);
        presets_combo.connect_changed(move |combo| {
            if let Some(name) = combo.active_text() {
                let name = name.to_string();
                let state = app_state.borrow();
                if let Some(preset) = state.config.presets.iter().find(|p| p.name == name) {
                    fill_fields(&widgets, preset);
                }
            }
        });
    }

    // Enregistrer : crée ou met à jour le préréglage portant le nom
    // actuellement saisi, avec les valeurs des champs.
    {
        let widgets = widgets.clone();
        let app_state = Rc::clone(&app_state);
        let status_label = status_label.clone();
        save_btn.connect_clicked(move |_| {
            let name = widgets.name_entry.text().trim().to_string();
            if name.is_empty() {
                status_label.set_text("Le nom du préréglage ne peut pas être vide.");
                return;
            }
            status_label.set_text("");
            let preset = Preset {
                name: name.clone(),
                work_min: widgets.work_spin.value() as u32,
                short_break_min: widgets.short_spin.value() as u32,
                long_break_min: widgets.long_spin.value() as u32,
                cycles_before_long_break: widgets.cycles_spin.value() as u32,
            };

            // Le RefCell doit être relâché avant de toucher aux widgets
            // ci-dessous : `repopulate_combo`/`fill_fields` peuvent déclencher
            // des signaux GTK synchrones (ex. "changed" sur le combo) dont les
            // gestionnaires empruntent `app_state` à leur tour.
            let config_snapshot = {
                let mut state = app_state.borrow_mut();
                if let Some(existing) = state.config.presets.iter_mut().find(|p| p.name == name) {
                    *existing = preset;
                } else {
                    state.config.presets.push(preset);
                }
                mutate_save_and_snapshot(&mut state)
            };
            refresh_widgets(&widgets, &config_snapshot, Some(&name));
        });
    }

    // Définir comme actif : le préréglage nommé dans le champ "Nom" devient
    // le préréglage actif, et le minuteur est réinitialisé.
    {
        let widgets = widgets.clone();
        let app_state = Rc::clone(&app_state);
        set_active_btn.connect_clicked(move |_| {
            let name = widgets.name_entry.text().trim().to_string();
            let config_snapshot = {
                let mut state = app_state.borrow_mut();
                if !state.config.presets.iter().any(|p| p.name == name) {
                    return;
                }
                state.config.active_preset = name.clone();
                state.timer.reset();
                mutate_save_and_snapshot(&mut state)
            };
            refresh_widgets(&widgets, &config_snapshot, Some(&name));
        });
    }

    // Supprimer : refuse de supprimer le dernier préréglage restant.
    {
        let widgets = widgets.clone();
        let app_state = Rc::clone(&app_state);
        delete_btn.connect_clicked(move |_| {
            let name = widgets.name_entry.text().trim().to_string();
            let config_snapshot = {
                let mut state = app_state.borrow_mut();
                if state.config.presets.len() <= 1 {
                    return;
                }
                state.config.presets.retain(|p| p.name != name);
                if state.config.active_preset == name {
                    state.config.active_preset = state.config.presets[0].name.clone();
                    state.timer.reset();
                }
                mutate_save_and_snapshot(&mut state)
            };
            refresh_widgets(&widgets, &config_snapshot, None);
        });
    }

    // Lancer au démarrage : synchronise immédiatement le fichier XDG
    // autostart et persiste la configuration.
    {
        let app_state = Rc::clone(&app_state);
        autostart_check.connect_toggled(move |check| {
            let enabled = check.is_active();
            let mut state = app_state.borrow_mut();
            state.config.autostart = enabled;
            let _ = state.config.save();
            crate::autostart::sync(enabled);
        });
    }

    // Son de notification : "Personnalisé…" ouvre un sélecteur de fichier ;
    // les autres options sont persistées immédiatement. Le drapeau
    // `sound_syncing` évite de rouvrir le sélecteur de fichier quand ce
    // combo est mis à jour programmatiquement (ex. `sync_sound_widgets`),
    // puisque `set_active_id` déclenche "changed" comme un vrai clic.
    {
        let widgets = widgets.clone();
        let app_state = Rc::clone(&app_state);
        sound_combo.connect_changed(move |combo| {
            if widgets.sound_syncing.get() {
                return;
            }
            let Some(id) = combo.active_id().map(|s| s.to_string()) else {
                return;
            };
            if id == "custom" {
                let existing = app_state
                    .borrow()
                    .config
                    .notification_sound
                    .custom_path()
                    .map(str::to_string);
                match pick_audio_file(&widgets.window, existing.as_deref()) {
                    Some(path) => {
                        let mut state = app_state.borrow_mut();
                        state.config.notification_sound = SoundChoice::Custom(path.clone());
                        let _ = state.config.save();
                        drop(state);
                        widgets.custom_sound_label.set_text(&display_filename(&path));
                    }
                    None => {
                        // Annulé : aucun emprunt actif ici, donc pas de risque à
                        // resynchroniser le combo avec la config inchangée (même
                        // si cela redéclenche "changed" en interne).
                        let current = app_state.borrow().config.notification_sound.clone();
                        sync_sound_widgets(&widgets, &current);
                    }
                }
            } else if let Some(choice) = SoundChoice::from_builtin_id(&id) {
                let mut state = app_state.borrow_mut();
                state.config.notification_sound = choice;
                let _ = state.config.save();
                drop(state);
                widgets.custom_sound_label.set_text("");
            }
        });
    }

    // Tester : joue immédiatement le son actuellement configuré (et non pas
    // simplement sélectionné dans le combo, pour éviter de rouvrir le
    // sélecteur de fichier si l'option affichée est "Personnalisé…").
    {
        let app_state = Rc::clone(&app_state);
        test_sound_btn.connect_clicked(move |_| {
            let choice = app_state.borrow().config.notification_sound.clone();
            crate::sound::preview(&choice);
        });
    }

    widgets
}

fn fill_fields(widgets: &SettingsWidgets, preset: &Preset) {
    widgets.name_entry.set_text(&preset.name);
    widgets.work_spin.set_value(preset.work_min as f64);
    widgets.short_spin.set_value(preset.short_break_min as f64);
    widgets.long_spin.set_value(preset.long_break_min as f64);
    widgets.cycles_spin.set_value(preset.cycles_before_long_break as f64);
}

/// Ouvre un sélecteur de fichier modal pour choisir un son de notification.
/// `initial_path` présélectionne le fichier actuellement configuré, le cas
/// échéant. Retourne `None` si l'utilisateur annule.
fn pick_audio_file(parent: &gtk::Window, initial_path: Option<&str>) -> Option<String> {
    let dialog = gtk::FileChooserDialog::new(
        Some("Choisir un son de notification"),
        Some(parent),
        gtk::FileChooserAction::Open,
    );
    dialog.add_button("Annuler", gtk::ResponseType::Cancel);
    dialog.add_button("Choisir", gtk::ResponseType::Accept);
    dialog.set_default_response(gtk::ResponseType::Accept);

    let filter = gtk::FileFilter::new();
    filter.set_name(Some("Fichiers audio (wav, mp3, ogg, flac)"));
    for pattern in ["*.wav", "*.mp3", "*.ogg", "*.oga", "*.flac"] {
        filter.add_pattern(pattern);
    }
    dialog.add_filter(filter);

    if let Some(path) = initial_path {
        let _ = dialog.set_filename(path);
    }

    let response = dialog.run();
    let result = if response == gtk::ResponseType::Accept {
        dialog.filename().map(|p| p.to_string_lossy().into_owned())
    } else {
        None
    };
    dialog.close();
    result
}

fn display_filename(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string())
}

/// Met à jour `sound_combo` et le libellé de fichier à partir d'un
/// `SoundChoice`, sans déclencher le sélecteur de fichier même si le combo
/// passe par "custom" (voir `sound_syncing`).
fn sync_sound_widgets(widgets: &SettingsWidgets, choice: &SoundChoice) {
    widgets.sound_syncing.set(true);
    widgets.sound_combo.set_active_id(Some(choice.combo_id()));
    widgets.sound_syncing.set(false);
    match choice.custom_path() {
        Some(path) => widgets.custom_sound_label.set_text(&display_filename(path)),
        None => widgets.custom_sound_label.set_text(""),
    }
}

fn repopulate_combo(widgets: &SettingsWidgets, config: &Config) {
    widgets.presets_combo.remove_all();
    for preset in &config.presets {
        widgets.presets_combo.append(Some(&preset.name), &preset.name);
    }
    widgets.presets_combo.set_active_id(Some(&config.active_preset));
}

/// Sauvegarde la config sur disque et reconstruit le menu de la barre
/// système, tout en gardant le `RefCell` emprunté le temps le plus court
/// possible. Retourne une copie de la config pour rafraîchir les widgets
/// une fois l'emprunt relâché (voir `refresh_widgets`).
fn mutate_save_and_snapshot(state: &mut AppState) -> Config {
    let _ = state.config.save();
    let running = state.timer.is_running();
    state.handles = tray::rebuild_menu(&state.tray_icon, &state.config, running);
    state.config.clone()
}

/// Rafraîchit les widgets de la fenêtre de réglages à partir d'une copie de
/// la config. Ne touche jamais à `app_state` : peut être appelée sans risque
/// de réentrance, même si elle déclenche des signaux GTK synchrones (ex.
/// "changed" sur le combo des préréglages).
fn refresh_widgets(widgets: &SettingsWidgets, config: &Config, select_preset: Option<&str>) {
    repopulate_combo(widgets, config);
    let name_to_show = select_preset.unwrap_or(&config.active_preset);
    if let Some(preset) = config.presets.iter().find(|p| p.name == name_to_show) {
        fill_fields(widgets, preset);
    } else {
        fill_fields(widgets, config.active_preset());
    }
    sync_sound_widgets(widgets, &config.notification_sound);
}

/// Affiche la fenêtre de réglages, en rafraîchissant d'abord son contenu
/// avec l'état courant de la configuration.
pub fn open(widgets: &SettingsWidgets, app_state: &Rc<RefCell<AppState>>) {
    // On clone la config puis on relâche l'emprunt avant de toucher aux
    // widgets : `set_active` sur la case à cocher déclenche son signal
    // "toggled" de façon synchrone, dont le gestionnaire emprunte
    // `app_state` à son tour (voir plus haut).
    let config_snapshot = app_state.borrow().config.clone();
    refresh_widgets(widgets, &config_snapshot, None);
    widgets.autostart_check.set_active(config_snapshot.autostart);
    widgets.window.show_all();
    widgets.window.present();
}
