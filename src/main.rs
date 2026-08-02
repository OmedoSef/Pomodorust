//! Pomodorust — un minuteur Pomodoro pour la barre système d'Ubuntu.
//!
//! Toute l'application tourne sur la boucle d'évènements de GTK3 (un seul
//! thread) : l'icône de la barre système (crate `tray-icon`, qui utilise
//! GTK/libappindicator en interne sous Linux) et la fenêtre de réglages
//! (widgets gtk-rs) partagent le même `gtk::main()`.

mod autostart;
mod config;
mod osd;
mod settings_ui;
mod sound;
mod timer;
mod tray;

use config::Config;
use sound::SoundChoice;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;
use timer::{Phase, PhaseCompleted, TickEvent, Timer};
use tray_icon::menu::MenuEvent;
use tray_icon::TrayIconEvent;

/// Icône de l'application, générée par `cargo run --bin gen-icon` et
/// embarquée directement dans le binaire : aucune recherche de fichier au
/// démarrage n'est nécessaire.
const ICON_PNG: &[u8] = include_bytes!("../assets/icon.png");

/// État partagé de l'application, accédé depuis le tick périodique et les
/// gestionnaires de la fenêtre de réglages via `Rc<RefCell<_>>` (tout tourne
/// sur le thread principal de GTK, pas besoin de synchronisation).
pub struct AppState {
    pub config: Config,
    pub timer: Timer,
    pub tray_icon: tray_icon::TrayIcon,
    pub handles: tray::TrayHandles,
}

fn decode_icon() -> (Vec<u8>, u32, u32) {
    let image = image::load_from_memory(ICON_PNG)
        .expect("icône intégrée invalide")
        .into_rgba8();
    let (width, height) = image.dimensions();
    (image.into_raw(), width, height)
}

fn main() {
    gtk::init().expect("échec de l'initialisation de GTK (affichage X11/Wayland introuvable ?)");

    let config = Config::load_or_init();
    autostart::sync(config.autostart);

    let (rgba, width, height) = decode_icon();

    let tray_icon_image =
        tray_icon::Icon::from_rgba(rgba.clone(), width, height).expect("icône de la barre système invalide");
    let (tray_icon, handles) = tray::build_tray(tray_icon_image, &config);

    let app_state = Rc::new(RefCell::new(AppState {
        config,
        timer: Timer::new(),
        tray_icon,
        handles,
    }));

    let row_stride = (width * 4) as i32;
    let window_icon = gtk::gdk_pixbuf::Pixbuf::from_mut_slice(
        rgba,
        gtk::gdk_pixbuf::Colorspace::Rgb,
        true,
        8,
        width as i32,
        height as i32,
        row_stride,
    );

    let settings_widgets = settings_ui::build(&window_icon, Rc::clone(&app_state));
    let osd = osd::build(&window_icon);

    // Bouton d'action du toast de pause ("Passer la pause" / "Reprendre le
    // travail" selon le mode) : une seule fonction gère les deux rôles, car
    // `Timer::resolve_break` fait la bonne chose selon l'état courant.
    {
        let app_state = Rc::clone(&app_state);
        let osd_for_action = osd.clone();
        osd::set_action_handler(&osd, move || {
            resolve_break_action(&app_state, &osd_for_action);
        });
    }

    {
        let app_state = Rc::clone(&app_state);
        let osd = osd.clone();
        glib::timeout_add_local(Duration::from_millis(250), move || {
            // Draine les évènements de clic sur l'icône elle-même (on ne
            // fait rien de spécial : le clic gauche/droit ouvre déjà le
            // menu nativement via libappindicator).
            while TrayIconEvent::receiver().try_recv().is_ok() {}

            while let Ok(event) = MenuEvent::receiver().try_recv() {
                handle_menu_event(&event, &app_state, &settings_widgets, &osd);
            }

            tick_timer(&app_state, &osd);

            glib::ControlFlow::Continue
        });
    }

    gtk::main();
}

fn handle_menu_event(
    event: &MenuEvent,
    app_state: &Rc<RefCell<AppState>>,
    settings_widgets: &settings_ui::SettingsWidgets,
    osd: &osd::OsdWindow,
) {
    let id = event.id.0.as_str();

    if id == tray::ID_START_PAUSE {
        let (completed, sound_choice) = {
            let mut state = app_state.borrow_mut();
            let preset = state.config.active_preset().clone();
            let sound_choice = state.config.notification_sound.clone();
            (state.timer.toggle_start_pause(&preset), sound_choice)
        };
        refresh_tray_display(app_state);
        sync_osd(app_state, osd);
        if let Some(completed) = completed {
            notify_phase_change(&completed, osd, sound_choice);
        }
    } else if id == tray::ID_SKIP {
        let (completed, sound_choice) = {
            let mut state = app_state.borrow_mut();
            let preset = state.config.active_preset().clone();
            let sound_choice = state.config.notification_sound.clone();
            (state.timer.skip(&preset), sound_choice)
        };
        refresh_tray_display(app_state);
        sync_osd(app_state, osd);
        if let Some(completed) = completed {
            notify_phase_change(&completed, osd, sound_choice);
        }
    } else if id == tray::ID_RESET {
        {
            let mut state = app_state.borrow_mut();
            state.timer.reset();
        }
        refresh_tray_display(app_state);
        sync_osd(app_state, osd);
    } else if id == tray::ID_SETTINGS {
        settings_ui::open(settings_widgets, app_state);
    } else if id == tray::ID_QUIT {
        gtk::main_quit();
    } else if let Some(name) = id.strip_prefix(tray::PRESET_ID_PREFIX) {
        {
            let mut state = app_state.borrow_mut();
            state.config.active_preset = name.to_string();
            let _ = state.config.save();
            state.timer.reset();
            let new_handles = tray::rebuild_menu(&state.tray_icon, &state.config, false);
            state.handles = new_handles;
        }
        refresh_tray_display(app_state);
        sync_osd(app_state, osd);
    }
}

/// Gestionnaire du bouton d'action du toast de pause ("Passer la pause" ou
/// "Reprendre le travail" selon le mode) : `Timer::resolve_break` fait la
/// bonne chose dans les deux cas.
fn resolve_break_action(app_state: &Rc<RefCell<AppState>>, osd: &osd::OsdWindow) {
    let (completed, sound_choice) = {
        let mut state = app_state.borrow_mut();
        let preset = state.config.active_preset().clone();
        let sound_choice = state.config.notification_sound.clone();
        (state.timer.resolve_break(&preset), sound_choice)
    };
    refresh_tray_display(app_state);
    sync_osd(app_state, osd);
    if let Some(completed) = completed {
        notify_phase_change(&completed, osd, sound_choice);
    }
}

/// Appelé toutes les 250ms : recalcule le temps restant, avance
/// automatiquement à la phase suivante si nécessaire, ou passe en attente de
/// confirmation si une pause vient de se terminer (voir
/// `Timer::AwaitingBreakEnd`) ; tient l'affichage de la barre système et du
/// toast à jour dans tous les cas.
fn tick_timer(app_state: &Rc<RefCell<AppState>>, osd: &osd::OsdWindow) {
    let (event, sound_choice) = {
        let mut state = app_state.borrow_mut();
        if state.timer.is_idle() {
            return;
        }
        let preset = state.config.active_preset().clone();
        let sound_choice = state.config.notification_sound.clone();
        (state.timer.tick(&preset), sound_choice)
    };
    refresh_tray_display(app_state);
    sync_osd(app_state, osd);

    match event {
        Some(TickEvent::Advanced(completed)) => notify_phase_change(&completed, osd, sound_choice),
        Some(TickEvent::BreakEnded { phase }) => notify_break_ended(phase, sound_choice),
        None => {}
    }
}

/// Réconcilie l'affichage du toast avec l'état courant du minuteur — à
/// appeler après chaque action qui le modifie.
fn sync_osd(app_state: &Rc<RefCell<AppState>>, osd: &osd::OsdWindow) {
    let status = {
        let state = app_state.borrow();
        break_status(&state)
    };
    osd::sync(osd, &status);
}

fn break_status(state: &AppState) -> osd::BreakStatus {
    if state.timer.is_awaiting_break_end() {
        if let Some(phase) = state.timer.current_phase() {
            return osd::BreakStatus::AwaitingConfirmation(phase);
        }
    }
    if state.timer.is_running() {
        if let Some(phase) = state.timer.current_phase() {
            if matches!(phase, Phase::ShortBreak | Phase::LongBreak) {
                let remaining = state.timer.remaining().unwrap_or_default();
                return osd::BreakStatus::Running(phase, format_mmss(remaining));
            }
        }
    }
    osd::BreakStatus::None
}

/// Met à jour les trois surfaces qui reflètent l'état du minuteur :
/// l'infobulle de l'icône, le texte accolé à l'icône dans la barre système
/// (via l'API AppIndicator, Linux uniquement), et la ligne de statut en haut
/// du menu déroulant — le repli garanti si l'infobulle ou le texte ne
/// s'affichent pas selon l'environnement de bureau. Met aussi à jour le
/// libellé du bouton démarrer/pause du menu.
fn refresh_tray_display(app_state: &Rc<RefCell<AppState>>) {
    let state = app_state.borrow();
    let tooltip = tooltip_text(&state);
    let _ = state.tray_icon.set_tooltip(Some(&tooltip));
    state.handles.status_item.set_text(&tooltip);
    state.handles.start_pause_item.set_text(start_pause_label(&state.timer));
    set_tray_label(&state.tray_icon, &tray_label_text(&state));
}

fn start_pause_label(timer: &Timer) -> &'static str {
    if timer.is_awaiting_break_end() {
        tray::LABEL_RESUME_WORK
    } else if timer.is_running() {
        tray::LABEL_PAUSE
    } else {
        tray::LABEL_START
    }
}

/// Écrit le texte affiché à côté de l'icône dans la barre système, via
/// l'API `app_indicator_set_label` (accès bas niveau explicitement exposé
/// par `tray-icon` pour les besoins spécifiques à Linux — voir sa
/// documentation pour `TrayIcon::app_indicator`).
fn set_tray_label(tray_icon: &tray_icon::TrayIcon, label: &str) {
    unsafe {
        let ptr = tray_icon.app_indicator() as *mut libappindicator::AppIndicator;
        if let Some(indicator) = ptr.as_mut() {
            indicator.set_label(label, "");
        }
    }
}

fn tray_label_text(state: &AppState) -> String {
    if state.timer.is_awaiting_break_end() {
        return "✅ Prêt".to_string();
    }
    match state.timer.current_phase() {
        Some(phase) => {
            let remaining = state.timer.remaining().unwrap_or_default();
            let emoji = match phase {
                Phase::Work => "🍅",
                Phase::ShortBreak | Phase::LongBreak => "☕",
            };
            format!("{emoji} {}", format_mmss(remaining))
        }
        None => String::new(),
    }
}

fn tooltip_text(state: &AppState) -> String {
    if state.timer.is_awaiting_break_end() {
        return "Pause terminée — cliquez sur \"Reprendre le travail\"".to_string();
    }
    match state.timer.current_phase() {
        Some(phase) => {
            let remaining = state.timer.remaining().unwrap_or_default();
            format!("{} — {}", phase.label(), format_mmss(remaining))
        }
        None => "Pomodorust — Prêt".to_string(),
    }
}

fn format_mmss(duration: Duration) -> String {
    let total_secs = duration.as_secs();
    format!("{:02}:{:02}", total_secs / 60, total_secs % 60)
}

/// Notifie une transition de phase qui vient d'avoir lieu immédiatement
/// (fin de travail → entrée en pause automatique, ou pause skippée/confirmée
/// → retour au travail). Le toast éphémère n'est affiché que lorsqu'on entre
/// en travail : lorsqu'on entre en pause, le toast persistant de pause
/// (piloté par `sync_osd`) prend le relai sur le même tick, un toast
/// éphémère en plus ferait doublon.
fn notify_phase_change(completed: &PhaseCompleted, osd: &osd::OsdWindow, sound_choice: SoundChoice) {
    let body = match completed.finished_phase {
        Phase::Work => "Session terminée — pause bien méritée !",
        Phase::ShortBreak | Phase::LongBreak => "Pause terminée — au travail !",
    };
    let next_hint = match completed.next_phase {
        Phase::Work => "Prochaine étape : Travail",
        Phase::ShortBreak => "Prochaine étape : Pause courte",
        Phase::LongBreak => "Prochaine étape : Pause longue",
    };

    let result = notify_rust::Notification::new()
        .summary("Pomodorust")
        .body(&format!("{body}\n{next_hint}"))
        .appname("Pomodorust")
        .show();
    if let Err(err) = result {
        eprintln!("Échec de l'envoi de la notification : {err}");
    }

    if completed.next_phase == Phase::Work {
        osd::show_ephemeral(osd, body, next_hint);
    }
    sound::play(&sound_choice, matches!(completed.next_phase, Phase::Work));
}

/// Notifie qu'une pause vient d'arriver à zéro et attend confirmation. Le
/// toast persistant de confirmation est déjà affiché par `sync_osd` (appelé
/// juste avant, dans le même tick) : on ne fait ici que la notification
/// système et le son.
fn notify_break_ended(phase: Phase, sound_choice: SoundChoice) {
    let result = notify_rust::Notification::new()
        .summary("Pomodorust")
        .body(&format!(
            "{} terminée — cliquez sur \"Reprendre le travail\" quand vous êtes prêt.",
            phase.label()
        ))
        .appname("Pomodorust")
        .show();
    if let Err(err) = result {
        eprintln!("Échec de l'envoi de la notification : {err}");
    }
    sound::play(&sound_choice, true);
}
