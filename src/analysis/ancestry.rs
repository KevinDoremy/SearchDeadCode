//! Qui a le droit de ressusciter son conteneur.
//!
//! Les trois analyseurs de reachability marquent les ancêtres d'un symbole
//! atteignable : un membre vivant garde sa classe, sinon le rapport
//! proposerait de supprimer un conteneur dont l'intérieur sert encore.
//!
//! Le problème est la provenance de « atteignable ». Quand la résolution par
//! type échoue, `builder.rs` retombe sur le nom simple et lie TOUS les
//! homonymes, en marquant les arêtes `ambiguous`. Mesuré sur un corpus neutre
//! de 196 fichiers Kotlin : sept des huit objets morts manqués portaient un
//! membre nommé `scope`, un nom qui y apparaît 908 fois. Le membre passait
//! pour atteint, le marquage d'ancêtres remontait jusqu'à l'objet, et l'objet
//! ne sortait jamais du rapport — pendant que `--explain` annonçait
//! `Incoming references: 0` sur le même symbole.
//!
//! Le garde ne peut pas être local. Écarter « D dont toutes les arêtes
//! entrantes sont ambiguës » laisse D dans l'ensemble atteignable, donc ses
//! arêtes sortantes sont suivies, donc ses cibles reçoivent une arête entrante
//! NON ambiguë — celle qui vient de D — et récupèrent le droit de marquer
//! leurs ancêtres. La contamination avance d'un saut au lieu de disparaître.
//! Il faut la fermeture transitive, celle que `kill_list::forward_closure`
//! calcule déjà : atteint depuis un point d'entrée sans jamais traverser une
//! devinette.
//!
//! Effet mesuré avant/après sur un projet Kotlin de 325 fichiers : 29
//! conteneurs apparaissent (15 classes, 9 objets, une interface), 99
//! trouvailles disparaissent, dont 94 propriétés et 5 méthodes — toutes des
//! membres de ces mêmes conteneurs, dans les mêmes douze fichiers. Le rapport
//! ne perd rien, il replie « N membres morts » en « un conteneur mort », ce
//! que `should_skip_declaration` annonçait déjà par « report class instead ».

use crate::graph::{DeclarationId, Graph};
use std::collections::HashSet;

/// Le sous-ensemble de `reachable` autorisé à marquer ses ancêtres.
///
/// Deux termes, et les deux sont nécessaires :
///
/// - la fermeture stricte : atteint sans jamais croire une devinette ;
/// - les symboles sans aucune arête entrante : point d'entrée, bénédiction
///   (override, constructeur, sous-type scellé), cible d'un initialiseur
///   suivi. Aucune devinette n'est en cause, ils gardent leur pouvoir de
///   marquage. Les retirer produirait des faux positifs, le seul sens
///   d'erreur qu'un détecteur de code mort n'a pas le droit de prendre.
///
/// Ce que la bénédiction n'a pas besoin de gagner : sa précondition est déjà
/// que le parent soit atteignable, donc un override n'a jamais à ressusciter
/// sa classe, et les grands-parents suivent par le parent. C'est ce qui permet
/// une seule fermeture au lieu de rejouer toute la boucle de point fixe en
/// mode strict.
pub(crate) fn ancestor_seeds(
    graph: &Graph,
    entry_points: &HashSet<DeclarationId>,
    reachable: &HashSet<DeclarationId>,
) -> Vec<DeclarationId> {
    let solid = crate::analysis::kill_list::forward_closure(graph, entry_points);
    reachable
        .iter()
        .filter(|id| solid.contains(*id) || graph.get_references_to(id).is_empty())
        .cloned()
        .collect()
}
