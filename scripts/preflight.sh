#!/bin/sh
# Rejoue localement ce que la CI verifie sur une PR, en une vingtaine de
# secondes au lieu de plusieurs minutes d'attente et d'un aller-retour.
#
#   ./scripts/preflight.sh          verifie
#   ./scripts/preflight.sh --fix    corrige le formatage au passage
#
# Ce que ce script NE remplace pas, et ne peut pas remplacer :
#   - la matrice Windows / Linux, qui est la raison d'etre de la CI
#   - la signature cosign et l'attestation de provenance, dont la valeur
#     vient de l'OIDC du runner GitHub ; les produire en local donne un
#     artefact que personne ne peut verifier
#   - la publication crates.io, qui demanderait le token sur cette machine

set -e

cd "$(dirname "$0")/.."

FIX=""
[ "${1:-}" = "--fix" ] && FIX=1

step() { printf '\n\033[1m%s\033[0m\n' "$1"; }

step "1/3  format"
if [ -n "$FIX" ]; then
  cargo fmt --all
  echo "formate"
else
  cargo fmt --all -- --check
  echo "ok"
fi

step "2/3  clippy"
cargo clippy --all-targets -- -D warnings

step "3/3  tests"
cargo nextest run --no-fail-fast

# Le corpus repond a une autre question que les tests : est-ce que le
# COMPORTEMENT sur du vrai code a change. Silencieux s'il n'est pas configure.
if [ -n "${SDC_CORPUS:-}" ]; then
  step "bonus  corpus"
  ./scripts/check-corpus.sh
fi

printf '\n\033[32mpreflight vert — la CI ne devrait rien trouver de plus sur ubuntu\033[0m\n'
