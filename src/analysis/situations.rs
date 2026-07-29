//! Situations du repo → commandes suggérées, paramètres inclus.
//!
//! Le run par défaut sait déjà détecter (deep, bus, write-only…) ; ce
//! qui manque à l'utilisateur, c'est le ROUTAGE vers les vues
//! spécialisées et leurs paramètres. Ce module regarde la forme du
//! projet (arborescences jumelles main/mainV2, classes X/XV2, modules
//! Gradle déclarés mais morts, façades reliquats) et rend des hints
//! prêts à copier. Tout est bon marché : détecteurs purs sur le graphe
//! déjà construit, plus une lecture de settings.gradle.

use super::{bus::BusReport, dead_modules, middlemen, twins};
use crate::graph::Graph;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Un constat sur le repo et la commande qui l'explore
pub struct Hint {
    pub message: String,
    pub command: String,
}

/// Suffixes de répertoires qui trahissent une migration (miroir de
/// l'esprit de VERSION_SUFFIXES côté classes)
const DIR_SUFFIXES: &[&str] = &["V2", "V3", "v2", "2", "New"];

/// Un monde de migration doit avoir un minimum de matière de chaque
/// côté — un répertoire de 1-2 fichiers n'est pas un monde
const MIN_WORLD_FILES: usize = 3;

const MAX_HINTS: usize = 5;

pub fn detect(graph: &Graph, root: &Path, bus: &BusReport, dead_count: usize) -> Vec<Hint> {
    let mut hints = Vec::new();

    for (old, new) in twin_directories(graph) {
        hints.push(Hint {
            message: format!(
                "Deux arborescences similaires détectées ({} / {}) — migration en cours ?",
                old.display(),
                new.display()
            ),
            command: format!(
                "searchdeadcode . --compare \"{}={}\"    vieux monde: supprimable au flip + bloqueurs",
                old.display(),
                new.display()
            ),
        });
    }

    let twin_classes = twins::version_twins(graph);
    if !twin_classes.is_empty() {
        hints.push(Hint {
            message: format!(
                "{} paire(s) de classes V1/V2 (ex. {} / {})",
                twin_classes.len(),
                twin_classes[0].base.name,
                twin_classes[0].variant.name
            ),
            command: "searchdeadcode . --twins    les paires côte à côte avec leurs références"
                .to_string(),
        });
    }

    if let Some(modules) = dead_modules::dead_modules(root) {
        if !modules.is_empty() {
            hints.push(Hint {
                message: format!(
                    "{} module(s) Gradle déclarés mais jamais consommés",
                    modules.len()
                ),
                command:
                    "searchdeadcode . --dead-modules    modules supprimables de settings.gradle"
                        .to_string(),
            });
        }
    }

    let facades = middlemen::middlemen(graph);
    if !facades.is_empty() {
        hints.push(Hint {
            message: format!(
                "{} façade(s) qui ne font que déléguer (reliquat de migration ?)",
                facades.len()
            ),
            command: "searchdeadcode . --middlemen    les classes à court-circuiter".to_string(),
        });
    }

    if !bus.is_empty() {
        hints.push(Hint {
            message: "Des events de bus orphelins sont listés plus haut".to_string(),
            command: "searchdeadcode . --explain <Event>    pourquoi un event est considéré mort"
                .to_string(),
        });
    }

    hints.truncate(MAX_HINTS);

    // Les gestes génériques de triage, seulement quand il y a des findings
    if dead_count > 0 {
        for command in [
            "searchdeadcode . --interactive    trier les findings au clavier",
            "searchdeadcode . --clusters    grouper les findings en unités supprimables",
            "searchdeadcode . --explain <nom>    pourquoi un symbole est considéré mort",
            "searchdeadcode . --delete --dry-run    prévisualiser le nettoyage",
        ] {
            hints.push(Hint {
                message: String::new(),
                command: command.to_string(),
            });
        }
    }

    hints
}

/// Paires de répertoires frères `X` / `X+suffixe` avec assez de fichiers
/// de chaque côté. Les chemins viennent du graphe (fichiers analysés),
/// rendus relatifs à la racine pour une commande copiable.
fn twin_directories(graph: &Graph) -> Vec<(PathBuf, PathBuf)> {
    let mut file_counts: HashMap<PathBuf, usize> = HashMap::new();
    for decl in graph.declarations() {
        if let Some(dir) = decl.location.file.parent() {
            *file_counts.entry(dir.to_path_buf()).or_default() += 1;
        }
    }

    let mut pairs = Vec::new();
    for dir in file_counts.keys() {
        let Some(name) = dir.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Some(parent) = dir.parent() else {
            continue;
        };
        for suffix in DIR_SUFFIXES {
            let variant = parent.join(format!("{name}{suffix}"));
            if let Some(&variant_count) = file_counts.get(&variant) {
                if file_counts[dir] >= MIN_WORLD_FILES && variant_count >= MIN_WORLD_FILES {
                    pairs.push((dir.clone(), variant));
                }
            }
        }
    }
    pairs.sort();
    pairs.dedup();
    pairs
}
