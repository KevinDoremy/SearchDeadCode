#!/bin/sh
# Compare les sorties courantes aux references d'un corpus local.
#
# Valider un detecteur sur un vrai projet Android prenait 4 a 5 minutes par
# run, et il en faut plusieurs (avant, apres, avec le flag). Sur un corpus
# reduit a quelques centaines de fichiers representatifs, la meme validation
# tient en une quinzaine de secondes et se lit dans un diff.
#
# Mise en place, une fois :
#   1. copier des modules representatifs dans un dossier hors de ce repo
#      (un module Kotlin/Compose, un module Java legacy, un module avec DI,
#      leurs proguard-rules.pro et leurs res/)
#   2. export SDC_CORPUS=~/dev/sdc-corpus
#   3. ./scripts/check-corpus.sh --bless   pour figer les references
#
# Ensuite : ./scripts/check-corpus.sh avant chaque commit qui touche un
# detecteur. Sortie vide = aucun changement de comportement.

set -eu

: "${SDC_CORPUS:?exporte SDC_CORPUS vers ton corpus local (voir l'entete)}"
[ -d "$SDC_CORPUS" ] || { echo "SDC_CORPUS n'existe pas : $SDC_CORPUS" >&2; exit 2; }

BLESS=""
[ "${1:-}" = "--bless" ] && BLESS=1

EXPECTED="$SDC_CORPUS/expected"
OUT="${TMPDIR:-/tmp}/sdc-corpus"
mkdir -p "$EXPECTED" "$OUT"

# Le binaire release sert ici : c'est celui dont on mesure le comportement,
# et il tourne assez souvent sur le corpus pour que la compilation soit amortie.
cargo build --release

BIN="./target/release/searchdeadcode"
status=0

for view in standard islands clusters quick-wins; do
  case "$view" in
    standard) flag="" ;;
    *)        flag="--$view" ;;
  esac

  # shellcheck disable=SC2086
  "$BIN" "$SDC_CORPUS" $flag > "$OUT/$view.txt" 2>&1 || true

  # Les chemins absolus et les durees varient d'un run a l'autre : sans ca,
  # chaque diff serait un faux positif.
  sed -e "s|$SDC_CORPUS|CORPUS|g" \
      -e 's/in [0-9][0-9.]*s/in TIME/g' \
      -e 's/[0-9][0-9.]*ms/TIMEms/g' \
      "$OUT/$view.txt" > "$OUT/$view.normalized"

  if [ -n "$BLESS" ]; then
    cp "$OUT/$view.normalized" "$EXPECTED/$view.txt"
    echo "fige : $view"
  elif [ ! -f "$EXPECTED/$view.txt" ]; then
    echo "pas de reference pour $view — lance --bless" >&2
    status=2
  elif ! diff -u "$EXPECTED/$view.txt" "$OUT/$view.normalized"; then
    echo "^^ $view a change" >&2
    status=1
  fi
done

[ -n "$BLESS" ] && echo "references ecrites dans $EXPECTED"
exit $status
