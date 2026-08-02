#!/usr/bin/env bash
# Installe Pomodorust pour l'utilisateur courant (installation "user-local",
# aucun droit root requis).
#
# Prérequis : le binaire doit déjà être compilé en mode release, soit
# nativement (si vous avez rustup + les paquets système listés dans
# .devcontainer/Dockerfile), soit via le devcontainer :
#
#   docker build -t pomodorust-dev .devcontainer
#   docker run --rm -v "$PWD":/workspace -w /workspace pomodorust-dev \
#       cargo build --release
#
# Ce script ne force PAS le lancement au démarrage : c'est l'application
# elle-même qui synchronise ~/.config/autostart/pomodorust.desktop à chaque
# lancement, selon la valeur de `autostart` dans config.toml (true par
# défaut au premier lancement).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

BIN_SRC="$PROJECT_ROOT/target/release/pomodorust"
ICON_SRC="$PROJECT_ROOT/assets/icon.png"
DESKTOP_SRC="$PROJECT_ROOT/packaging/pomodorust.desktop"

BIN_DEST_DIR="$HOME/.local/bin"
ICON_DEST_DIR="$HOME/.local/share/icons"
APPS_DEST_DIR="$HOME/.local/share/applications"

BIN_DEST="$BIN_DEST_DIR/pomodorust"
ICON_DEST="$ICON_DEST_DIR/pomodorust.png"
DESKTOP_DEST="$APPS_DEST_DIR/pomodorust.desktop"

if [ ! -f "$BIN_SRC" ]; then
    echo "Erreur : binaire introuvable à $BIN_SRC" >&2
    echo "Compilez d'abord le projet (voir l'en-tête de ce script ou le README)." >&2
    exit 1
fi

if [ ! -f "$ICON_SRC" ]; then
    echo "Erreur : icône introuvable à $ICON_SRC" >&2
    echo "Générez-la d'abord avec : cargo run --bin gen-icon" >&2
    exit 1
fi

mkdir -p "$BIN_DEST_DIR" "$ICON_DEST_DIR" "$APPS_DEST_DIR"

install -m 755 "$BIN_SRC" "$BIN_DEST"
install -m 644 "$ICON_SRC" "$ICON_DEST"

sed \
    -e "s|__POMODORUST_BIN__|$BIN_DEST|g" \
    -e "s|__POMODORUST_ICON__|$ICON_DEST|g" \
    "$DESKTOP_SRC" > "$DESKTOP_DEST"
chmod 644 "$DESKTOP_DEST"

echo "Pomodorust installé :"
echo "  binaire  : $BIN_DEST"
echo "  icône    : $ICON_DEST"
echo "  lanceur  : $DESKTOP_DEST"
echo
echo "Rappel : sur Ubuntu/GNOME Shell standard, l'icône de la barre système"
echo "ne sera visible qu'après installation de l'extension GNOME Shell"
echo "\"AppIndicator and KStatusNotifierItem Support\" (extensions.gnome.org)."
echo
echo "Lancez l'application depuis le menu des applications, ou directement :"
echo "  $BIN_DEST"
