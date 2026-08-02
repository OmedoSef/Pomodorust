//! Machine à états du minuteur Pomodoro.

use crate::config::Preset;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Work,
    ShortBreak,
    LongBreak,
}

impl Phase {
    pub fn label(&self) -> &'static str {
        match self {
            Phase::Work => "Travail",
            Phase::ShortBreak => "Pause courte",
            Phase::LongBreak => "Pause longue",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TimerState {
    Idle,
    Running {
        session: SessionHandle,
        started_at: Instant,
    },
    Paused {
        session: SessionHandle,
        remaining: Duration,
    },
    /// Une pause vient d'arriver à zéro, mais on n'avance PAS
    /// automatiquement au travail : on attend une confirmation explicite de
    /// l'utilisateur (bouton du toast, ou menu de la barre système).
    AwaitingBreakEnd {
        session: SessionHandle,
    },
}

/// Copie légère et `Copy` des infos de session (pas de champs non-Copy).
#[derive(Debug, Clone, Copy)]
pub struct SessionHandle {
    phase: Phase,
    duration: Duration,
    cycle_count: u32,
}

/// Résultat d'un tick : indique si une phase vient de se terminer et
/// laquelle a suivi, pour permettre l'envoi d'une notification appropriée.
pub struct PhaseCompleted {
    pub finished_phase: Phase,
    pub next_phase: Phase,
}

/// Évènement retourné par [`Timer::tick`].
pub enum TickEvent {
    /// La phase a avancé automatiquement (fin d'une session de travail :
    /// on entre en pause sans confirmation nécessaire).
    Advanced(PhaseCompleted),
    /// Une pause vient d'arriver à zéro ; le minuteur attend une
    /// confirmation explicite avant de reprendre le travail.
    BreakEnded { phase: Phase },
}

pub struct Timer {
    state: TimerState,
}

impl Timer {
    pub fn new() -> Self {
        Timer {
            state: TimerState::Idle,
        }
    }

    pub fn state(&self) -> &TimerState {
        &self.state
    }

    pub fn is_running(&self) -> bool {
        matches!(self.state, TimerState::Running { .. })
    }

    pub fn is_idle(&self) -> bool {
        matches!(self.state, TimerState::Idle)
    }

    /// Démarre (ou reprend) le minuteur. Si le minuteur était à l'arrêt
    /// (Idle), commence une nouvelle session de travail.
    pub fn start(&mut self, preset: &Preset) {
        match self.state {
            TimerState::Idle => {
                let session = SessionHandle {
                    phase: Phase::Work,
                    duration: Duration::from_secs(preset.work_min as u64 * 60),
                    cycle_count: 0,
                };
                self.state = TimerState::Running {
                    session,
                    started_at: Instant::now(),
                };
            }
            TimerState::Paused { session, remaining } => {
                // On "recale" started_at pour que duration - elapsed == remaining.
                let started_at = Instant::now()
                    .checked_sub(session.duration.saturating_sub(remaining))
                    .unwrap_or_else(Instant::now);
                self.state = TimerState::Running { session, started_at };
            }
            TimerState::Running { .. } | TimerState::AwaitingBreakEnd { .. } => {}
        }
    }

    pub fn pause(&mut self) {
        if let TimerState::Running { session, started_at } = self.state {
            let elapsed = started_at.elapsed();
            let remaining = session.duration.saturating_sub(elapsed);
            self.state = TimerState::Paused { session, remaining };
        }
    }

    /// Bascule démarrer/pause selon l'état courant. Si une pause vient de se
    /// terminer et attend confirmation, ce même bouton confirme et démarre
    /// la session de travail suivante.
    pub fn toggle_start_pause(&mut self, preset: &Preset) -> Option<PhaseCompleted> {
        match self.state {
            TimerState::Running { .. } => {
                self.pause();
                None
            }
            TimerState::Idle | TimerState::Paused { .. } => {
                self.start(preset);
                None
            }
            TimerState::AwaitingBreakEnd { session } => Some(self.advance(session, preset)),
        }
    }

    pub fn reset(&mut self) {
        self.state = TimerState::Idle;
    }

    pub fn is_awaiting_break_end(&self) -> bool {
        matches!(self.state, TimerState::AwaitingBreakEnd { .. })
    }

    /// Passe immédiatement à la phase suivante (comme si le temps était
    /// écoulé), qu'une pause soit en cours, en pause, ou déjà terminée en
    /// attente de confirmation.
    pub fn skip(&mut self, preset: &Preset) -> Option<PhaseCompleted> {
        match self.state {
            TimerState::Running { session, .. }
            | TimerState::Paused { session, .. }
            | TimerState::AwaitingBreakEnd { session } => Some(self.advance(session, preset)),
            TimerState::Idle => None,
        }
    }

    /// Utilisé par le bouton d'action du toast de pause : passe la pause en
    /// cours, ou confirme une fin de pause en attente, et démarre le
    /// travail. Ne fait rien si aucune pause n'est en cours ni en attente.
    pub fn resolve_break(&mut self, preset: &Preset) -> Option<PhaseCompleted> {
        match self.state {
            TimerState::Running { session, .. } | TimerState::Paused { session, .. }
                if matches!(session.phase, Phase::ShortBreak | Phase::LongBreak) =>
            {
                Some(self.advance(session, preset))
            }
            TimerState::AwaitingBreakEnd { session } => Some(self.advance(session, preset)),
            _ => None,
        }
    }

    /// À appeler périodiquement (ex. toutes les 250ms). Si une session de
    /// travail vient d'expirer, avance automatiquement à la pause suivante.
    /// Si c'est une pause qui vient d'expirer, le minuteur passe en attente
    /// de confirmation (voir [`TimerState::AwaitingBreakEnd`]) au lieu de
    /// reprendre le travail tout seul.
    pub fn tick(&mut self, preset: &Preset) -> Option<TickEvent> {
        if let TimerState::Running { session, started_at } = self.state {
            if started_at.elapsed() >= session.duration {
                if matches!(session.phase, Phase::ShortBreak | Phase::LongBreak) {
                    self.state = TimerState::AwaitingBreakEnd { session };
                    return Some(TickEvent::BreakEnded { phase: session.phase });
                }
                return Some(TickEvent::Advanced(self.advance(session, preset)));
            }
        }
        None
    }

    /// Temps restant dans la session en cours. `None` si Idle ; zéro si une
    /// pause vient de se terminer et attend confirmation.
    pub fn remaining(&self) -> Option<Duration> {
        match self.state {
            TimerState::Running { session, started_at } => {
                Some(session.duration.saturating_sub(started_at.elapsed()))
            }
            TimerState::Paused { remaining, .. } => Some(remaining),
            TimerState::AwaitingBreakEnd { .. } => Some(Duration::ZERO),
            TimerState::Idle => None,
        }
    }

    pub fn current_phase(&self) -> Option<Phase> {
        match self.state {
            TimerState::Running { session, .. } => Some(session.phase),
            TimerState::Paused { session, .. } => Some(session.phase),
            TimerState::AwaitingBreakEnd { session } => Some(session.phase),
            TimerState::Idle => None,
        }
    }

    fn advance(&mut self, session: SessionHandle, preset: &Preset) -> PhaseCompleted {
        let finished_phase = session.phase;
        let (next_phase, cycle_count) = match session.phase {
            Phase::Work => {
                let new_cycle_count = session.cycle_count + 1;
                if new_cycle_count >= preset.cycles_before_long_break {
                    (Phase::LongBreak, 0)
                } else {
                    (Phase::ShortBreak, new_cycle_count)
                }
            }
            Phase::ShortBreak | Phase::LongBreak => (Phase::Work, session.cycle_count),
        };

        let duration_min = match next_phase {
            Phase::Work => preset.work_min,
            Phase::ShortBreak => preset.short_break_min,
            Phase::LongBreak => preset.long_break_min,
        };

        let next_session = SessionHandle {
            phase: next_phase,
            duration: Duration::from_secs(duration_min as u64 * 60),
            cycle_count,
        };

        self.state = TimerState::Running {
            session: next_session,
            started_at: Instant::now(),
        };

        PhaseCompleted {
            finished_phase,
            next_phase,
        }
    }
}
