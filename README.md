# Pomodorust

Un minuteur Pomodoro pour Ubuntu, qui vit dans la barre système (top bar /
menu bar) et se pilote entièrement depuis son menu : démarrage direct d'une
session, réglages, changement de préréglage, etc. Écrit en Rust avec
[`tray-icon`](https://crates.io/crates/tray-icon) et [`gtk-rs`](https://gtk-rs.org/)
(GTK3), spécifiquement pour Linux/Ubuntu (pas de support multiplateforme).

> Ce projet a été "vibe codé" avec [Claude](https://claude.com/claude-code)
> (Anthropic) : l'essentiel du code, de l'architecture et de ce README a été
> écrit par l'assistant, en échange avec l'auteur au fil des itérations.

## ⚠️ Prérequis indispensable sur Ubuntu + GNOME Shell

**Ubuntu de base avec GNOME Shell n'affiche pas les icônes de la barre
système (tray/appindicator) sans extension.** Pomodorust utilise ce
mécanisme pour son icône : sans l'extension ci-dessous, l'application
tournera (le minuteur fonctionne, les notifications s'affichent) mais
**vous ne verrez aucune icône ni menu dans la barre du haut.**

Installez impérativement l'extension GNOME Shell **"AppIndicator and
KStatusNotifierItem Support"** depuis
[extensions.gnome.org](https://extensions.gnome.org/extension/615/appindicator-support/),
puis activez-la (via l'application "Extensions" ou GNOME Tweaks). Ce n'est
pas optionnel : c'est une condition matérielle pour que l'icône soit
visible sur une installation GNOME standard.

## Fonctionnalités

- Icône dans la barre système, avec **le temps restant affiché à côté**
  (ex. `🍅 24:59` en travail, `☕ 04:12` en pause) via l'API AppIndicator —
  pas besoin de survoler l'icône pour voir où on en est.
- Menu déroulant avec une **ligne de statut non cliquable en haut**
  (ex. "Travail — 24:59"), qui sert de repli garanti si l'affichage à côté
  de l'icône n'est pas supporté par votre environnement de bureau, plus :
  - **▶ Démarrer / ⏸ Pause** — démarre ou met en pause la session en cours.
  - **⏭ Passer la phase** — passe immédiatement à la phase suivante.
  - **⏹ Réinitialiser** — arrête et revient à l'état "Prêt".
  - **Preset actif** — sous-menu listant les préréglages enregistrés
    (coche celui actif ; cliquer sur un autre l'active et réinitialise le
    minuteur).
  - **⚙ Réglages…** — ouvre la fenêtre de réglages.
  - **Quitter**.
- Enchaînement automatique **des sessions de travail vers les pauses**
  (Travail → Pause courte/longue), mais **pas l'inverse** : voir
  "Fin de pause" ci-dessous.
- **Toast de pause** : dès qu'une pause (courte ou longue) commence, une
  fenêtre reste affichée pendant toute sa durée, avec le chrono en gros,
  un bouton **✕ Cacher** (la masque pour cette pause ; elle réapparaîtra
  d'elle-même à la fin de la pause) et un bouton **⏭ Passer la pause**.
- **Fin de pause avec confirmation** : quand le chrono de la pause arrive à
  zéro, l'application **n'enchaîne pas automatiquement** sur le travail.
  Le toast passe en mode "Pause terminée !" avec un bouton
  **▶ Reprendre le travail** — le minuteur attend ce clic (ou le même geste
  depuis le menu de la barre système) avant de démarrer la session
  suivante.
- À chaque changement de phase : notification de bureau et son configurable
  (voir ci-dessous). Un bref toast supplémentaire (sans bouton, ~4s) marque
  le retour au travail ; il n'y en a pas en plus à l'entrée en pause, le
  toast persistant de pause faisant déjà foi.
- **Son de notification configurable** (réglages) : Carillon, Cloche,
  Marimba ou Silencieux (synthétisés en mémoire, aucun fichier audio
  externe, donc aucune question de licence), ou **"Personnalisé…"** pour
  choisir un fichier audio (wav/mp3/ogg/flac) sur votre disque via un
  sélecteur de fichier. Bouton "🔊 Tester" pour prévisualiser le son
  actuellement configuré.
- Fenêtre de réglages (GTK3) pour créer, modifier, activer et supprimer des
  préréglages nommés (durée de travail, pause courte, pause longue, nombre
  de cycles avant la pause longue), choisir le son de notification, et
  activer/désactiver le lancement automatique au démarrage.
- Configuration persistée en TOML dans
  `~/.config/pomodorust/config.toml` (chemin XDG standard).

### Limites connues sous Wayland

Ubuntu récent utilise Wayland par défaut. Le protocole Wayland interdit
volontairement aux applications ordinaires de s'imposer "toujours au-dessus"
ou de choisir leur propre position à l'écran (contrairement à X11). Le toast
de fin de phase s'affiche donc toujours, mais :

- sous une session **Xorg** ("Ubuntu sur Xorg", sélectionnable à l'écran de
  connexion) : centré et au premier plan, comme prévu ;
- sous **Wayland** : une petite fenêtre apparaît, mais sans garantie de
  position ni de premier plan (c'est une limitation du protocole, pas un
  bug de l'application).

Le texte à côté de l'icône (AppIndicator) et la ligne de statut du menu ne
sont pas concernés par cette limite : ils fonctionnent identiquement sous
X11 et Wayland.

### Préréglage par défaut

Au tout premier lancement, un préréglage **"Classique"** est créé
automatiquement :

| Travail | Pause courte | Pause longue | Cycles avant pause longue |
|---------|---------------|--------------|----------------------------|
| 25 min  | 5 min         | 15 min       | 4                          |

Le lancement automatique au démarrage (`autostart`) est activé par défaut ;
décochez la case correspondante dans les réglages pour le désactiver.

## Compiler via le devcontainer

Toute la chaîne de compilation (Rust + dépendances système GTK3 /
appindicator / dbus) est fournie par `.devcontainer/`, pour ne rien
installer sur la machine hôte :

```bash
docker build -t pomodorust-dev .devcontainer
docker run --rm -v "$PWD":/workspace -w /workspace pomodorust-dev \
    cargo run --bin gen-icon        # génère assets/icon.png (une seule fois, déjà committée)
docker run --rm -v "$PWD":/workspace -w /workspace pomodorust-dev \
    cargo build --release
```

Le binaire final se trouve alors dans `target/release/pomodorust`.

Le dossier `.devcontainer/` est aussi utilisable directement comme
environnement de développement VS Code ("Reopen in Container").

## Installer

Une fois le binaire compilé (nativement, ou via le devcontainer ci-dessus) :

```bash
./scripts/install.sh
```

Ce script installe, sans droits root :

- le binaire dans `~/.local/bin/pomodorust`
- l'icône dans `~/.local/share/icons/pomodorust.png`
- une entrée de menu dans `~/.local/share/applications/pomodorust.desktop`

Il ne force **pas** le lancement automatique au démarrage : c'est
l'application elle-même qui synchronise
`~/.config/autostart/pomodorust.desktop` à chaque lancement, selon la
valeur `autostart` de la configuration (activée par défaut au premier
lancement, modifiable ensuite dans les réglages).

## Structure du projet

```
.devcontainer/       Image Docker + config VS Code pour la chaîne de compilation
src/main.rs           Point d'entrée, boucle GTK, câblage des évènements
src/config.rs          Modèle de configuration (préréglages) + persistance TOML
src/timer.rs            Machine à états du minuteur
src/tray.rs               Construction/reconstruction du menu de la barre système
src/settings_ui.rs         Fenêtre de réglages GTK3
src/osd.rs                  Toast affiché lors des changements de phase
src/sound.rs                  Carillon synthétisé en mémoire (rodio)
src/autostart.rs            Synchronisation du fichier XDG autostart
src/bin/gen_icon.rs   Génère assets/icon.png (tomate dessinée en pur Rust)
packaging/             Entrée .desktop pour le lanceur d'applications
scripts/install.sh       Installation utilisateur (sans root)
```

## Dépendances système sur la machine hôte (pas seulement le devcontainer)

Normalement déjà présentes sur une installation Ubuntu de bureau standard :
`libgtk-3-0`, `libayatana-appindicator3-1`, `libxdo3`, `libdbus-1-3`,
`libasound2` (ALSA, pour le carillon). Si l'une manque, `apt install
<paquet>` suffit.
