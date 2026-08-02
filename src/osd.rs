//! Petite fenêtre "toast" affichée lors des transitions de phase — plus
//! visible qu'une notification système standard.
//!
//! Trois modes, matérialisés par [`OsdMode`] :
//! - **Éphémère** : bref message auto-disparaissant (ex. reprise du
//!   travail), pas de bouton.
//! - **Pause en cours** : affiché en continu pendant toute la durée d'une
//!   pause, avec le chrono, un bouton pour cacher et un bouton pour passer
//!   la pause.
//! - **Pause terminée** : affiché quand la pause arrive à zéro, en attente
//!   d'une confirmation explicite (bouton "Reprendre le travail") avant de
//!   redémarrer une session de travail — voir `Timer::AwaitingBreakEnd`.
//!
//! Note : le positionnement centré et le "toujours au-dessus" ne sont
//! garantis que sous une session X11 (les compositeurs Wayland, session par
//! défaut sur Ubuntu récent, n'accordent pas ce contrôle aux applications
//! clientes ordinaires). Sous Wayland la fenêtre s'affiche quand même, sans
//! garantie de position ni de premier plan.

use crate::timer::Phase;
use gtk::prelude::*;
use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

const EPHEMERAL_DURATION: Duration = Duration::from_secs(4);

const CSS: &str = "
.pomodorust-osd {
    background-color: #202225;
    border-radius: 14px;
}
.pomodorust-osd-title {
    color: #ffffff;
    font-size: 20px;
    font-weight: bold;
}
.pomodorust-osd-subtitle {
    color: #c7c9cc;
    font-size: 13px;
}
.pomodorust-osd-countdown {
    color: #ffd166;
    font-size: 34px;
    font-weight: bold;
    font-family: monospace;
}
";

#[derive(Clone, Copy, PartialEq)]
enum OsdMode {
    Hidden,
    Ephemeral,
    BreakRunning(Phase),
    BreakEnded(Phase),
}

/// L'état courant du minuteur, tel que nécessaire pour piloter l'affichage
/// du toast — [`osd::sync`] est appelée après chaque changement d'état pour
/// réconcilier l'affichage, sans que ce module ait besoin de connaître
/// `AppState`/`Timer` directement.
pub enum BreakStatus {
    None,
    Running(Phase, String),
    AwaitingConfirmation(Phase),
}

/// Références aux widgets du toast, plus l'état interne (mode courant,
/// masquage manuel, génération pour l'auto-fermeture du mode éphémère).
#[derive(Clone)]
pub struct OsdWindow {
    window: gtk::Window,
    title_label: gtk::Label,
    detail_label: gtk::Label,
    button_row: gtk::Box,
    action_button: gtk::Button,
    mode: Rc<Cell<OsdMode>>,
    /// `true` si l'utilisateur a cliqué "Cacher" pour le mode courant : on
    /// ne réaffiche pas tant que le mode ne change pas réellement (sinon le
    /// tick suivant ferait immédiatement réapparaître la fenêtre).
    dismissed: Rc<Cell<bool>>,
    generation: Rc<Cell<u64>>,
}

/// Construit le toast (caché par défaut) et installe le CSS de
/// l'application. Le bouton d'action n'est pas encore câblé : voir
/// [`set_action_handler`].
pub fn build(icon: &gtk::gdk_pixbuf::Pixbuf) -> OsdWindow {
    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.set_decorated(false);
    window.set_resizable(false);
    window.set_default_size(380, 170);
    window.set_keep_above(true);
    window.set_skip_taskbar_hint(true);
    window.set_skip_pager_hint(true);
    window.set_accept_focus(false);
    window.set_type_hint(gtk::gdk::WindowTypeHint::Notification);
    window.set_position(gtk::WindowPosition::CenterAlways);
    window.style_context().add_class("pomodorust-osd");

    let provider = gtk::CssProvider::new();
    if let Err(err) = provider.load_from_data(CSS.as_bytes()) {
        eprintln!("Pomodorust: CSS du toast invalide : {err}");
    }
    if let Some(screen) = gtk::gdk::Screen::default() {
        gtk::StyleContext::add_provider_for_screen(
            &screen,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    let root = gtk::Box::new(gtk::Orientation::Vertical, 10);
    root.set_border_width(18);

    let header_row = gtk::Box::new(gtk::Orientation::Horizontal, 14);

    let scaled_icon = icon
        .scale_simple(56, 56, gtk::gdk_pixbuf::InterpType::Bilinear)
        .unwrap_or_else(|| icon.clone());
    let icon_image = gtk::Image::from_pixbuf(Some(&scaled_icon));
    header_row.pack_start(&icon_image, false, false, 0);

    let text_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
    text_box.set_valign(gtk::Align::Center);

    let title_label = gtk::Label::new(None);
    title_label.set_halign(gtk::Align::Start);
    title_label.style_context().add_class("pomodorust-osd-title");

    let detail_label = gtk::Label::new(None);
    detail_label.set_halign(gtk::Align::Start);
    detail_label.style_context().add_class("pomodorust-osd-subtitle");

    text_box.pack_start(&title_label, false, false, 0);
    text_box.pack_start(&detail_label, false, false, 0);
    header_row.pack_start(&text_box, true, true, 0);
    root.pack_start(&header_row, false, false, 0);

    let button_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let hide_button = gtk::Button::with_label("✕ Cacher");
    let action_button = gtk::Button::new();
    button_row.pack_start(&hide_button, true, true, 0);
    button_row.pack_start(&action_button, true, true, 0);
    root.pack_start(&button_row, false, false, 0);

    window.add(&root);

    let mode = Rc::new(Cell::new(OsdMode::Hidden));
    let dismissed = Rc::new(Cell::new(false));

    {
        let window = window.clone();
        let dismissed = Rc::clone(&dismissed);
        hide_button.connect_clicked(move |_| {
            dismissed.set(true);
            window.hide();
        });
    }

    {
        let dismissed = Rc::clone(&dismissed);
        window.connect_delete_event(move |win, _| {
            dismissed.set(true);
            win.hide();
            glib::Propagation::Stop
        });
    }

    OsdWindow {
        window,
        title_label,
        detail_label,
        button_row,
        action_button,
        mode,
        dismissed,
        generation: Rc::new(Cell::new(0)),
    }
}

/// Câble le bouton d'action (contextuel : "Passer la pause" pendant une
/// pause en cours, "Reprendre le travail" une fois la pause terminée). Une
/// seule fonction suffit pour les deux rôles : `Timer::resolve_break` fait
/// exactement la bonne chose selon l'état courant du minuteur.
pub fn set_action_handler(osd: &OsdWindow, handler: impl Fn() + 'static) {
    osd.action_button.connect_clicked(move |_| handler());
}

fn style_as_countdown(label: &gtk::Label) {
    let ctx = label.style_context();
    ctx.remove_class("pomodorust-osd-subtitle");
    ctx.add_class("pomodorust-osd-countdown");
}

fn style_as_subtitle(label: &gtk::Label) {
    let ctx = label.style_context();
    ctx.remove_class("pomodorust-osd-countdown");
    ctx.add_class("pomodorust-osd-subtitle");
}

/// `show_all()` réaffiche récursivement tous les enfants, y compris ceux
/// cachés individuellement pour un mode précédent (ex. `button_row` en mode
/// éphémère) : on doit donc réappliquer la visibilité voulue juste après.
fn present(osd: &OsdWindow, show_buttons: bool) {
    osd.window.show_all();
    osd.button_row.set_visible(show_buttons);
    osd.window.present();
}

/// Affiche un message bref qui se referme tout seul, sans bouton. Utilisé
/// pour les transitions qui ne nécessitent aucune action (ex. retour au
/// travail après une pause skippée ou confirmée).
pub fn show_ephemeral(osd: &OsdWindow, title: &str, subtitle: &str) {
    osd.mode.set(OsdMode::Ephemeral);
    osd.dismissed.set(false);
    osd.title_label.set_text(title);
    osd.detail_label.set_text(subtitle);
    style_as_subtitle(&osd.detail_label);
    present(osd, false);

    let my_generation = osd.generation.get() + 1;
    osd.generation.set(my_generation);

    let window = osd.window.clone();
    let mode = Rc::clone(&osd.mode);
    let generation = Rc::clone(&osd.generation);
    glib::timeout_add_local(EPHEMERAL_DURATION, move || {
        if generation.get() == my_generation {
            mode.set(OsdMode::Hidden);
            window.hide();
        }
        glib::ControlFlow::Break
    });
}

/// Réconcilie l'affichage du toast avec l'état courant du minuteur. À
/// appeler après chaque tick et chaque action qui modifie le minuteur. Ne
/// touche jamais à un toast éphémère en cours : il suit son cours et se
/// referme tout seul.
pub fn sync(osd: &OsdWindow, status: &BreakStatus) {
    if osd.mode.get() == OsdMode::Ephemeral {
        return;
    }

    match status {
        BreakStatus::Running(phase, remaining_text) => {
            let target = OsdMode::BreakRunning(*phase);
            if osd.mode.get() != target {
                osd.mode.set(target);
                osd.dismissed.set(false);
                osd.title_label.set_text(phase.label());
                osd.action_button.set_label("⏭ Passer la pause");
                style_as_countdown(&osd.detail_label);
            }
            osd.detail_label.set_text(remaining_text);
            if !osd.dismissed.get() {
                present(osd, true);
            }
        }
        BreakStatus::AwaitingConfirmation(phase) => {
            let target = OsdMode::BreakEnded(*phase);
            if osd.mode.get() != target {
                osd.mode.set(target);
                osd.dismissed.set(false);
                osd.title_label.set_text("Pause terminée !");
                osd.detail_label.set_text("Prêt à reprendre le travail ?");
                style_as_subtitle(&osd.detail_label);
                osd.action_button.set_label("▶ Reprendre le travail");
                if !osd.dismissed.get() {
                    present(osd, true);
                }
            }
        }
        BreakStatus::None => {
            if osd.mode.get() != OsdMode::Hidden {
                osd.mode.set(OsdMode::Hidden);
                osd.dismissed.set(false);
                osd.window.hide();
            }
        }
    }
}
