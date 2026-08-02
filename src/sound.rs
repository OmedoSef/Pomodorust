//! Petits sons de notification synthétisés en mémoire — pas de fichier audio
//! externe embarqué, donc aucune question de licence, et un binaire plus
//! léger. L'utilisateur choisit parmi quelques options dans les réglages.

use rodio::buffer::SamplesBuffer;
use rodio::{Decoder, OutputStream, Sink};
use serde::{Deserialize, Serialize};
use std::f32::consts::TAU;
use std::fs::File;
use std::io::BufReader;
use std::time::Duration;

const SAMPLE_RATE: u32 = 44_100;

/// Le son de notification choisi par l'utilisateur : soit un des trois
/// timbres synthétisés, le silence, soit un fichier audio de son choix
/// (`Custom` porte le chemin absolu, joué via le décodeur intégré de
/// `rodio` — wav/mp3/ogg/flac sont supportés).
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SoundChoice {
    #[default]
    Chime,
    Bell,
    Marimba,
    Silence,
    Custom(String),
}

impl SoundChoice {
    /// Les options intégrées (sans `Custom`, qui n'a pas de valeur par
    /// défaut sensée — voir le sélecteur de fichier des réglages).
    pub const BUILTIN: [SoundChoice; 4] = [
        SoundChoice::Chime,
        SoundChoice::Bell,
        SoundChoice::Marimba,
        SoundChoice::Silence,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            SoundChoice::Chime => "Carillon",
            SoundChoice::Bell => "Cloche",
            SoundChoice::Marimba => "Marimba",
            SoundChoice::Silence => "Silencieux",
            SoundChoice::Custom(_) => "Personnalisé…",
        }
    }

    /// Identifiant stable utilisé comme id de `gtk::ComboBoxText`
    /// (indépendant du texte affiché, qui peut changer).
    pub fn combo_id(&self) -> &'static str {
        match self {
            SoundChoice::Chime => "chime",
            SoundChoice::Bell => "bell",
            SoundChoice::Marimba => "marimba",
            SoundChoice::Silence => "silence",
            SoundChoice::Custom(_) => "custom",
        }
    }

    /// Ne résout que les options intégrées : `Custom` a besoin d'un chemin,
    /// fourni séparément par le sélecteur de fichier.
    pub fn from_builtin_id(id: &str) -> Option<Self> {
        Self::BUILTIN.into_iter().find(|choice| choice.combo_id() == id)
    }

    pub fn custom_path(&self) -> Option<&str> {
        match self {
            SoundChoice::Custom(path) => Some(path),
            _ => None,
        }
    }
}

/// Note pure avec enveloppe attaque/relâchement linéaire douce (évite les
/// clics audibles en début/fin de note).
fn tone(freq: f32, duration: Duration) -> Vec<f32> {
    let n = ((SAMPLE_RATE as f32) * duration.as_secs_f32()) as usize;
    let attack = (n / 10).max(1);
    let release = (n / 4).max(1);
    (0..n)
        .map(|i| {
            let t = i as f32 / SAMPLE_RATE as f32;
            let envelope = if i < attack {
                i as f32 / attack as f32
            } else if i + release > n {
                (n - i) as f32 / release as f32
            } else {
                1.0
            };
            (t * freq * TAU).sin() * envelope * 0.35
        })
        .collect()
}

/// Note avec attaque quasi instantanée et décroissance exponentielle,
/// utilisée pour les timbres plus percussifs (cloche, marimba).
fn exp_decay_tone(freq: f32, duration: Duration, decay: f32) -> Vec<f32> {
    let n = ((SAMPLE_RATE as f32) * duration.as_secs_f32()) as usize;
    let attack = (n / 50).max(1);
    (0..n)
        .map(|i| {
            let t = i as f32 / SAMPLE_RATE as f32;
            let attack_env = if i < attack { i as f32 / attack as f32 } else { 1.0 };
            let decay_env = (-decay * t).exp();
            (t * freq * TAU).sin() * attack_env * decay_env
        })
        .collect()
}

fn mix_into(base: &mut [f32], overlay: &[f32], gain: f32) {
    for (b, o) in base.iter_mut().zip(overlay) {
        *b += o * gain;
    }
}

fn normalize(samples: &mut [f32], peak: f32) {
    let max = samples.iter().fold(0.0f32, |m, &v| m.max(v.abs()));
    if max > 0.0 {
        let scale = peak / max;
        for s in samples.iter_mut() {
            *s *= scale;
        }
    }
}

/// Carillon deux notes. `rising` détermine si la mélodie monte (fin de
/// pause : au travail) ou descend (fin de session : pause bien méritée).
fn chime_samples(rising: bool) -> Vec<f32> {
    const DO5: f32 = 523.25;
    const MI5: f32 = 659.25;
    let (first, second) = if rising { (DO5, MI5) } else { (MI5, DO5) };
    let mut samples = tone(first, Duration::from_millis(160));
    samples.extend(tone(second, Duration::from_millis(260)));
    samples
}

/// Timbre de cloche : fondamentale + deux partiels non harmoniques, chacun
/// avec sa propre décroissance exponentielle (synthèse additive simple).
fn bell_samples() -> Vec<f32> {
    let fundamental = 440.0;
    let duration = Duration::from_millis(900);
    let mut out = exp_decay_tone(fundamental, duration, 3.0);
    mix_into(&mut out, &exp_decay_tone(fundamental * 2.0, duration, 4.0), 0.5);
    mix_into(&mut out, &exp_decay_tone(fundamental * 2.4, duration, 5.0), 0.3);
    normalize(&mut out, 0.4);
    out
}

/// Deux notes percussives courtes, façon marimba.
fn marimba_samples(rising: bool) -> Vec<f32> {
    const SOL4: f32 = 392.0;
    const DO5: f32 = 523.25;
    let (first, second) = if rising { (SOL4, DO5) } else { (DO5, SOL4) };
    let mut out = exp_decay_tone(first, Duration::from_millis(220), 9.0);
    out.extend(exp_decay_tone(second, Duration::from_millis(300), 9.0));
    normalize(&mut out, 0.5);
    out
}

fn open_sink() -> Option<(OutputStream, Sink)> {
    let (stream, handle) = match OutputStream::try_default() {
        Ok(pair) => pair,
        Err(err) => {
            eprintln!("Pomodorust: impossible d'ouvrir la sortie audio : {err}");
            return None;
        }
    };
    match Sink::try_new(&handle) {
        Ok(sink) => Some((stream, sink)),
        Err(err) => {
            eprintln!("Pomodorust: impossible de créer la piste audio : {err}");
            None
        }
    }
}

fn play_samples(samples: Vec<f32>) {
    if let Some((_stream, sink)) = open_sink() {
        sink.append(SamplesBuffer::new(1, SAMPLE_RATE, samples));
        sink.sleep_until_end();
    }
}

/// Décode et joue un fichier audio choisi par l'utilisateur (wav, mp3, ogg,
/// flac — tout ce que le décodeur intégré de `rodio` sait lire).
fn play_file(path: &str) {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(err) => {
            eprintln!("Pomodorust: impossible d'ouvrir le fichier audio {path} : {err}");
            return;
        }
    };
    let source = match Decoder::new(BufReader::new(file)) {
        Ok(source) => source,
        Err(err) => {
            eprintln!("Pomodorust: impossible de décoder le fichier audio {path} : {err}");
            return;
        }
    };
    if let Some((_stream, sink)) = open_sink() {
        sink.append(source);
        sink.sleep_until_end();
    }
}

/// Joue le son choisi sur un thread dédié (jamais sur le thread GTK).
/// `rising` détermine le sens de la mélodie pour les timbres synthétisés qui
/// en ont un (ignoré pour `Silence` et `Custom`).
pub fn play(choice: &SoundChoice, rising: bool) {
    match choice {
        SoundChoice::Silence => {}
        SoundChoice::Custom(path) => {
            let path = path.clone();
            std::thread::spawn(move || play_file(&path));
        }
        SoundChoice::Chime | SoundChoice::Bell | SoundChoice::Marimba => {
            let choice = choice.clone();
            std::thread::spawn(move || {
                let samples = match choice {
                    SoundChoice::Chime => chime_samples(rising),
                    SoundChoice::Bell => bell_samples(),
                    SoundChoice::Marimba => marimba_samples(rising),
                    SoundChoice::Silence | SoundChoice::Custom(_) => unreachable!(),
                };
                play_samples(samples);
            });
        }
    }
}

/// Joue un aperçu du son choisi (bouton "Tester" des réglages), toujours
/// dans sa variante "montante".
pub fn preview(choice: &SoundChoice) {
    play(choice, true);
}
