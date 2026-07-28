//! Event-bus orphan detection.
//!
//! Cross-references @Subscribe handlers with post()/postSticky() call sites:
//! an event posted with no subscriber goes into the void, a handler for an
//! event never posted is dead weight. Dynamic posts (`bus.post(variable)`)
//! cannot be enumerated, so their presence adds a caveat to the second
//! verdict instead of silencing it.

use regex::Regex;
use std::collections::BTreeSet;
use std::sync::LazyLock;

/// Kotlin: `@Subscribe ... fun name(param: Type` — tolère une annotation
/// de paramètre (`@Suppress("unused") event: Type`)
static KOTLIN_SUBSCRIBER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"@Subscribe\b[^\n]*\n\s*(?:[a-z]+\s+)*fun\s+\w+\(\s*(?:@\w+(?:\([^)]*\))?\s+)*\w+\s*:\s*([A-Z]\w*(?:\.[A-Z]\w*)*)"#,
    )
    .expect("Invalid Kotlin subscriber regex")
});

/// Java: `@Subscribe ... void name(Type param` — tolère `final` et les
/// annotations de paramètre (`@SuppressWarnings("unused") final Type e`),
/// omniprésents dans les bases Java
static JAVA_SUBSCRIBER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"@Subscribe\b[^\n]*\n\s*(?:[a-z]+\s+)*void\s+\w+\(\s*(?:@\w+(?:\([^)]*\))?\s+)*(?:final\s+)?([A-Z]\w*(?:\.[A-Z]\w*)*)\s+\w+"#,
    )
    .expect("Invalid Java subscriber regex")
});

/// `.post(FooEvent(` / `.post(new FooEvent(` / `.post(Parent.Variant(` /
/// postSticky variants — capture le chemin qualifié complet
static LITERAL_POST: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\.post(?:Sticky)?\(\s*(?:new\s+)?([A-Z]\w*(?:\.[A-Z]\w*)*)\s*\(")
        .expect("Invalid post regex")
});

/// `.post(someVariable)` — a post whose type cannot be known statically
static DYNAMIC_POST: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\.post(?:Sticky)?\(\s*[a-z]\w*\s*\)").expect("Invalid dynamic post regex")
});

/// Kotlin subclassing: `class Open(...) : MediaEvent(` — un handler abonné au
/// parent sealed reçoit les variantes postées ; sans ce lien, chaque paire
/// (variante postée, parent souscrit) produit deux orphelins fantômes
static KOTLIN_SUBTYPE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:class|object)\s+(\w+)\s*(?:\([^)]*\))?\s*:\s*(\w+)\s*\(")
        .expect("Invalid Kotlin subtype regex")
});

/// Java subclassing: `class ClickEvent extends BaseEvent`
static JAVA_SUBTYPE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"class\s+(\w+)\s+extends\s+(\w+)").expect("Invalid Java subtype regex")
});

#[derive(Debug, Default)]
pub struct BusReport {
    /// Event types posted somewhere, subscribed nowhere
    pub posted_never_subscribed: BTreeSet<String>,
    /// Event types with a @Subscribe handler but no literal post
    pub subscribed_never_posted: BTreeSet<String>,
    /// Number of posts whose event type could not be determined
    pub dynamic_posts: usize,
}

impl BusReport {
    pub fn is_empty(&self) -> bool {
        self.posted_never_subscribed.is_empty() && self.subscribed_never_posted.is_empty()
    }
}

/// Analyze the concatenated project sources
pub fn analyze(corpus: &str) -> BusReport {
    // Type qualifié (`MainActivity.PausedEvent`) : le type souscrit est le
    // dernier segment — le préfixe est la classe conteneur, pas un event
    let subscribed: BTreeSet<String> = KOTLIN_SUBSCRIBER
        .captures_iter(corpus)
        .chain(JAVA_SUBSCRIBER.captures_iter(corpus))
        .map(|c| {
            let path = c[1].to_string();
            path.rsplit('.').next().unwrap_or(&path).to_string()
        })
        .collect();
    // Handler.post(new Runnable() {...}) is thread dispatch, not an event
    // Un post qualifié `Parent.Variant(...)` : seul le dernier segment est
    // le type instancié (signalable) ; les préfixes COUVRENT une
    // souscription au conteneur mais ne sont pas des posts signalables.
    let mut posted: BTreeSet<String> = BTreeSet::new();
    let mut posted_covering: BTreeSet<String> = BTreeSet::new();
    let mut containers_of: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for cap in LITERAL_POST.captures_iter(corpus) {
        let path = cap[1].to_string();
        let segments: Vec<String> = path.split('.').map(str::to_string).collect();
        let Some(last) = segments.last().cloned() else {
            continue;
        };
        // Un `SomeRunnable` posté est du thread dispatch (UIThread/Handler),
        // pas un event — même logique que le Runnable anonyme
        if last.ends_with("Runnable") {
            continue;
        }
        posted_covering.extend(segments.iter().cloned());
        containers_of
            .entry(last.clone())
            .or_default()
            .extend(segments[..segments.len() - 1].iter().cloned());
        posted.insert(last);
    }
    let dynamic_posts = DYNAMIC_POST.find_iter(corpus).count();

    if subscribed.is_empty() && posted.is_empty() {
        return BusReport::default(); // no bus in this project
    }

    // child -> parent, pour remonter la hiérarchie des events
    let parent_of: std::collections::HashMap<String, String> = KOTLIN_SUBTYPE
        .captures_iter(corpus)
        .chain(JAVA_SUBTYPE.captures_iter(corpus))
        .map(|c| (c[1].to_string(), c[2].to_string()))
        .collect();

    let ancestors = |name: &str| -> Vec<String> {
        let mut chain = Vec::new();
        let mut current = name;
        // garde-fou contre un cycle A : B, B : A issu du regex
        while let Some(parent) = parent_of.get(current) {
            if chain.contains(parent) || parent == name {
                break;
            }
            chain.push(parent.clone());
            current = parent;
        }
        chain
    };

    // Un post d'une variante nourrit un abonné de n'importe quel ancêtre
    // (héritage) ou conteneur (chemin qualifié) ; symétriquement il compte
    // comme post de ses ancêtres et conteneurs.
    let posted_never_subscribed = posted
        .iter()
        .filter(|name| {
            !subscribed.contains(*name)
                && !ancestors(name).iter().any(|a| subscribed.contains(a))
                && !containers_of
                    .get(*name)
                    .is_some_and(|cs| cs.iter().any(|c| subscribed.contains(c)))
        })
        .cloned()
        .collect();

    let posted_with_ancestors: BTreeSet<String> = posted
        .iter()
        .flat_map(|name| std::iter::once(name.clone()).chain(ancestors(name)))
        .chain(posted_covering.iter().cloned())
        .collect();
    let subscribed_never_posted = subscribed
        .difference(&posted_with_ancestors)
        .cloned()
        .collect();

    BusReport {
        posted_never_subscribed,
        subscribed_never_posted,
        dynamic_posts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kotlin_and_java_subscribers_are_both_parsed() {
        let corpus = concat!(
            "@Subscribe\nfun onA(event: AlphaEvent) {}\n",
            "@Subscribe\npublic void onB(BetaEvent event) {}\n",
        );
        let report = analyze(corpus);
        assert!(report.subscribed_never_posted.contains("AlphaEvent"));
        assert!(report.subscribed_never_posted.contains("BetaEvent"));
    }

    #[test]
    fn matched_pairs_disappear_from_both_lists() {
        let corpus = "@Subscribe\nfun on(event: PingEvent) {}\nbus.post(PingEvent())\n";
        let report = analyze(corpus);
        assert!(report.is_empty());
    }

    #[test]
    fn new_keyword_and_sticky_posts_are_understood() {
        let corpus = "bus.post(new LegacyEvent());\nbus.postSticky(StickyEvent())\n";
        let report = analyze(corpus);
        assert!(report.posted_never_subscribed.contains("LegacyEvent"));
        assert!(report.posted_never_subscribed.contains("StickyEvent"));
    }

    #[test]
    fn dynamic_posts_are_counted_not_guessed() {
        let corpus = "@Subscribe\nfun on(event: MaybeEvent) {}\nbus.post(pending)\n";
        let report = analyze(corpus);
        assert_eq!(report.dynamic_posts, 1);
        assert!(report.subscribed_never_posted.contains("MaybeEvent"));
    }

    #[test]
    fn a_project_without_a_bus_reports_nothing() {
        let report = analyze("fun main() { println(\"hello\") }\n");
        assert!(report.is_empty());
        assert_eq!(report.dynamic_posts, 0);
    }

    #[test]
    fn subscribe_on_annotation_is_not_a_bus_subscriber() {
        let corpus = "@SubscribeOn(Schedulers.IO)\nfun stream(param: FlowEvent) {}\n";
        let report = analyze(corpus);
        assert!(
            !report.subscribed_never_posted.contains("FlowEvent"),
            "@SubscribeOn is RxJava scheduling, not an event-bus handler"
        );
    }

    #[test]
    fn a_named_runnable_subclass_posted_to_a_handler_is_not_an_event() {
        // Cas réel : `UIThread.post(new OnlyRunIfVisibleRunnable(ctx) {...})`
        let corpus = "handler.post(new OnlyRunIfVisibleRunnable(context) {\n});\n";
        let report = analyze(corpus);
        assert!(
            report.is_empty(),
            "un Runnable nommé posté à un Handler n'est pas un event: {report:?}"
        );
    }

    #[test]
    fn handler_post_runnable_is_not_an_event() {
        let corpus = "handler.post(new Runnable() {\n    public void run() {}\n});\n";
        let report = analyze(corpus);
        assert!(
            !report.posted_never_subscribed.contains("Runnable"),
            "Handler.post(Runnable) is thread dispatch, not an event"
        );
    }

    #[test]
    fn post_delayed_is_not_a_bus_post() {
        let corpus = "handler.postDelayed(TickEvent(), 100)\n";
        let report = analyze(corpus);
        assert!(
            !report.posted_never_subscribed.contains("TickEvent"),
            "postDelayed belongs to Handler, not to an event bus"
        );
    }

    #[test]
    fn a_post_of_a_sealed_variant_satisfies_a_parent_subscription() {
        // Cas réel : on poste la variante d'une sealed (`MediaEvent.Open(...)`)
        // et on souscrit au parent (le handler fait `when (event) { is Open ... }`).
        // La comparaison à plat produisait DEUX orphelins fantômes :
        // la variante posté-jamais-souscrit + le parent souscrit-jamais-posté.
        let corpus = concat!(
            "sealed class MediaEvent {\n",
            "    data class Open(val origin: String) : MediaEvent()\n",
            "    class Close(val ok: Boolean) : MediaEvent()\n",
            "}\n",
            "@Subscribe\nfun onMedia(event: MediaEvent) {}\n",
            "bus.post(Open(\"feed\"))\n",
        );
        let report = analyze(corpus);
        assert!(
            !report.posted_never_subscribed.contains("Open"),
            "poster une variante nourrit l'abonné du parent sealed: {report:?}"
        );
        assert!(
            !report.subscribed_never_posted.contains("MediaEvent"),
            "souscrire au parent est nourri par le post d'une variante: {report:?}"
        );
    }

    #[test]
    fn a_java_subclass_event_satisfies_a_base_subscription() {
        let corpus = concat!(
            "public class BaseEvent {}\n",
            "public class ClickEvent extends BaseEvent {}\n",
            "@Subscribe\npublic void onAny(BaseEvent event) {}\n",
            "bus.post(new ClickEvent());\n",
        );
        let report = analyze(corpus);
        assert!(
            report.is_empty(),
            "extends couvre la souscription à la base: {report:?}"
        );
    }

    #[test]
    fn an_unrelated_orphan_is_still_reported_next_to_a_hierarchy() {
        let corpus = concat!(
            "sealed class MediaEvent {\n",
            "    data class Open(val origin: String) : MediaEvent()\n",
            "}\n",
            "@Subscribe\nfun onMedia(event: MediaEvent) {}\n",
            "bus.post(Open(\"feed\"))\n",
            "bus.post(LostEvent())\n",
        );
        let report = analyze(corpus);
        assert!(
            report.posted_never_subscribed.contains("LostEvent"),
            "un orphelin sans lien de parenté reste signalé: {report:?}"
        );
    }

    #[test]
    fn a_qualified_post_container_is_not_reported_as_orphan() {
        // Régression du fix "posts qualifiés" : chaque segment comptait
        // comme posté, donc `post(PuzzleEvents.Completed(...))` faisait
        // apparaître "PuzzleEvents" en posté-jamais-souscrit. Le
        // conteneur sert à COUVRIR les souscriptions, pas à être signalé.
        let corpus = concat!(
            "sealed class PuzzleEvents {\n",
            "    class Completed : PuzzleEvents()\n",
            "}\n",
            "@Subscribe\nfun onCompleted(event: Completed) {}\n",
            "bus.post(PuzzleEvents.Completed())\n",
        );
        let report = analyze(corpus);
        assert!(
            !report.posted_never_subscribed.contains("PuzzleEvents"),
            "le conteneur d'un post qualifié n'est pas un orphelin: {report:?}"
        );
    }

    #[test]
    fn a_qualified_subscriber_param_matches_the_nested_event() {
        // Cas réel : `onBusEvent(final MainActivity.PausedEvent e)` — event nested
        // — la capture s'arrêtait au premier segment : « MainActivity »
        // polluait subscribed-never-posted et le vrai event restait
        // posté-jamais-souscrit.
        let corpus = concat!(
            "@Subscribe\n",
            "public void onBusEvent(final MainActivity.PausedEvent event) {}\n",
            "bus.post(new MainActivity.PausedEvent());\n",
            "@Subscribe\nfun onKt(event: MainActivity.ResumedEvent) {}\n",
            "bus.post(MainActivity.ResumedEvent())\n",
        );
        let report = analyze(corpus);
        assert!(
            report.is_empty(),
            "un type de param qualifié matche sur son dernier segment: {report:?}"
        );
    }

    #[test]
    fn a_final_java_param_is_still_a_subscription() {
        // Cas réel : `public void onBusEvent(final RemovedEvent e)`
        // — le `final` devant le type faisait échouer la capture, l'event
        // sortait posté-jamais-souscrit alors que le handler existe.
        let corpus = concat!(
            "@Subscribe\npublic void onBusEvent(final RemovedEvent event) {}\n",
            "bus.post(new RemovedEvent());\n",
        );
        let report = analyze(corpus);
        assert!(
            report.is_empty(),
            "un param Java `final` reste une souscription: {report:?}"
        );
    }

    #[test]
    fn an_annotated_param_is_still_a_subscription() {
        // Cas réel : `onBusEvent(@SuppressWarnings("unused") final ScrollEvent e)`
        let corpus = concat!(
            "@Subscribe\n",
            "public void onBusEvent(@SuppressWarnings(\"unused\") final ScrollEvent event) {}\n",
            "bus.post(new ScrollEvent());\n",
            "@Subscribe\nfun onOther(@Suppress(\"unused\") event: OtherEvent) {}\n",
            "bus.post(OtherEvent())\n",
        );
        let report = analyze(corpus);
        assert!(
            report.is_empty(),
            "un param annoté reste une souscription (Java et Kotlin): {report:?}"
        );
    }

    #[test]
    fn a_qualified_nested_post_satisfies_the_parent_subscription() {
        // Cas réel : `bus.post(PlaybackEvents.Started(...))` — post qualifié
        // — le regex de post exigeait `Ident(` direct, le chemin qualifié
        // ne matchait pas → le parent souscrit sortait "never posted".
        let corpus = concat!(
            "sealed class PlaybackEvents {\n",
            "    class Started : PlaybackEvents()\n",
            "}\n",
            "@Subscribe\nfun onPlayback(event: PlaybackEvents) {}\n",
            "bus.post(PlaybackEvents.Started())\n",
        );
        let report = analyze(corpus);
        assert!(
            report.is_empty(),
            "un post qualifié Parent.Variant nourrit l'abonné du parent: {report:?}"
        );
    }

    #[test]
    fn annotation_arguments_do_not_break_matching() {
        let corpus =
            "@Subscribe(threadMode = ThreadMode.MAIN)\nfun onSticky(event: ConfigEvent) {}\n";
        let report = analyze(corpus);
        assert!(report.subscribed_never_posted.contains("ConfigEvent"));
    }
}
