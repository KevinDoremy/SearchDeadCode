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
/// de paramètre (`@Suppress("unused") event: Type`), les arguments
/// d'annotation, et le handler écrit sur la même ligne que `@Subscribe`
static KOTLIN_SUBSCRIBER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"@Subscribe\b(?:\([^)]*\))?\s*(?:[a-z]+\s+)*fun\s+\w+\(\s*(?:@\w+(?:\([^)]*\))?\s+)*\w+\s*:\s*([A-Z]\w*(?:\.[A-Z]\w*)*)"#,
    )
    .expect("Invalid Kotlin subscriber regex")
});

/// Java: `@Subscribe ... void name(Type param` — tolère `final` et les
/// annotations de paramètre (`@SuppressWarnings("unused") final Type e`),
/// omniprésents dans les bases Java
static JAVA_SUBSCRIBER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"@Subscribe\b(?:\([^)]*\))?\s*(?:[a-z]+\s+)*void\s+\w+\(\s*(?:@\w+(?:\([^)]*\))?\s+)*(?:final\s+)?([A-Z]\w*(?:\.[A-Z]\w*)*)\s+\w+"#,
    )
    .expect("Invalid Java subscriber regex")
});

/// Souscription inline sans `@Subscribe` : `bus.subscribe<FooEvent> { … }`,
/// `bus.register(TapEvent::class) { … }`, `bus.subscribe(E::class.java, l)`
static INLINE_SUBSCRIBER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\.(?:subscribe|register)\s*(?:<\s*([A-Z]\w*(?:\.[A-Z]\w*)*)\s*>|\(\s*([A-Z]\w*(?:\.[A-Z]\w*)*)::class)",
    )
    .expect("Invalid inline subscriber regex")
});

/// `.post(FooEvent(` / `.post(new FooEvent(` / `.post(Parent.Variant(` /
/// postSticky variants — capture le chemin qualifié complet
static LITERAL_POST: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\.post(?:Sticky)?\(\s*(?:new\s+)?([A-Z]\w*(?:\.[A-Z]\w*)*)\s*\(")
        .expect("Invalid post regex")
});

/// `.post(someVariable)` — the variable's declared type is looked up in the
/// corpus; unresolved posts feed the honest caveat count
static DYNAMIC_POST: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\.post(?:Sticky)?\(\s*([a-z]\w*)\s*\)").expect("Invalid dynamic post regex")
});

/// `.post(buildEvent())` / `.post(factory.create(x))` — the callee's declared
/// return type is looked up in the corpus
static FACTORY_POST: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\.post(?:Sticky)?\(\s*(?:[\w.]+\.)?([a-z]\w*)\s*\(")
        .expect("Invalid factory post regex")
});

/// Kotlin supertype list: `class Foo(...) : Bar(), Runnable, Baz<T>` —
/// interfaces carry no parens, so this is broader than KOTLIN_SUBTYPE
static KOTLIN_SUPERTYPES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:class|object)\s+(\w+)(?:\s*\([^)]*\))?\s*:\s*([^{\n]+)")
        .expect("Invalid Kotlin supertypes regex")
});

/// Java implements clause: `class Foo extends Bar implements Runnable, Baz`
static JAVA_IMPLEMENTS: LazyLock<Regex> = LazyLock::new(|| {
    // `record` as well as `class`: a Java 16 record implements interfaces like
    // anything else, and one posted to UIThread is thread dispatch. Asking for
    // the literal `class` let records through as events.
    // The record's component list sits between the name and `implements`, so it
    // is skipped rather than assumed absent.
    Regex::new(
        r"(?:class|record)\s+(\w+)(?:\s*\([^)]*\))?(?:\s+extends\s+\w+)?\s+implements\s+([^{\n]+)",
    )
    .expect("Invalid Java implements regex")
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

/// Declared types of a variable posted dynamically: Kotlin `pending: Type`
/// (val, var ou paramètre), Kotlin `val pending = Type(...)`, Java
/// `Type pending`. Every match counts — ambiguity widens the covering set.
fn variable_types(corpus: &str, name: &str) -> Vec<String> {
    let patterns = [
        format!(r"\b{name}\s*:\s*([A-Z][\w.]*)"),
        format!(r"\b(?:val|var)\s+{name}\s*=\s*([A-Z][\w.]*)\s*\("),
        format!(r"\b([A-Z][\w.]*)\s+{name}\s*[=;,)]"),
    ];
    collect_types(corpus, &patterns)
}

/// Declared return types of a factory posted from: Kotlin
/// `fun make(...): Type`, Java `Type make(`.
fn factory_return_types(corpus: &str, name: &str) -> Vec<String> {
    let patterns = [
        format!(r"\bfun\s+{name}\s*\([^)]*\)\s*:\s*([A-Z][\w.]*)"),
        format!(r"\b([A-Z][\w.]*)\s+{name}\s*\("),
    ];
    collect_types(corpus, &patterns)
}

fn collect_types(corpus: &str, patterns: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for pattern in patterns {
        let Ok(re) = Regex::new(pattern) else {
            continue;
        };
        for cap in re.captures_iter(corpus) {
            let ty = cap[1].trim_end_matches('.').to_string();
            // Any/Object ne bornent rien : le post reste dynamique.
            if ty == "Any" || ty == "Object" {
                continue;
            }
            if !out.contains(&ty) {
                out.push(ty);
            }
        }
    }
    out
}

/// Analyze the concatenated project sources
pub fn analyze(corpus: &str) -> BusReport {
    // Type qualifié (`MainActivity.PausedEvent`) : le type souscrit est le
    // dernier segment — le préfixe est la classe conteneur, pas un event
    let subscribed: BTreeSet<String> = KOTLIN_SUBSCRIBER
        .captures_iter(corpus)
        .chain(JAVA_SUBSCRIBER.captures_iter(corpus))
        .map(|c| c[1].to_string())
        .chain(INLINE_SUBSCRIBER.captures_iter(corpus).map(|c| {
            c.get(1)
                .or_else(|| c.get(2))
                .map(|m| m.as_str().to_string())
                .unwrap_or_default()
        }))
        .filter(|path| !path.is_empty())
        .map(|path| path.rsplit('.').next().unwrap_or(&path).to_string())
        .collect();

    // Toutes les relations de sous-typage, interfaces comprises : c'est ce
    // qui permet de reconnaître un `FlushTask implements Runnable` posté.
    let supertypes: std::collections::HashMap<String, Vec<String>> = {
        let mut map: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        for cap in KOTLIN_SUPERTYPES
            .captures_iter(corpus)
            .chain(JAVA_IMPLEMENTS.captures_iter(corpus))
        {
            let child = cap[1].to_string();
            let parents = cap[2]
                .split(',')
                .filter_map(|entry| {
                    let entry = entry.trim();
                    let end = entry
                        .find(|c: char| !(c.is_alphanumeric() || c == '_'))
                        .unwrap_or(entry.len());
                    let name = &entry[..end];
                    (!name.is_empty()).then(|| name.to_string())
                })
                .collect::<Vec<_>>();
            map.entry(child).or_default().extend(parents);
        }
        map
    };
    let reaches_runnable = |name: &str| -> bool {
        let mut stack = vec![name.to_string()];
        let mut seen = std::collections::HashSet::new();
        while let Some(current) = stack.pop() {
            if current.ends_with("Runnable") {
                return true;
            }
            if !seen.insert(current.clone()) {
                continue;
            }
            if let Some(parents) = supertypes.get(&current) {
                stack.extend(parents.iter().cloned());
            }
        }
        false
    };
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
        // Un Runnable posté — par son nom ou par sa chaîne de supertypes —
        // est du thread dispatch (UIThread/Handler), pas un event
        if reaches_runnable(&last) {
            continue;
        }
        posted_covering.extend(segments.iter().cloned());
        containers_of
            .entry(last.clone())
            .or_default()
            .extend(segments[..segments.len() - 1].iter().cloned());
        posted.insert(last);
    }
    // Un post dynamique dont la variable a un type déclaré dans le corpus, ou
    // dont la factory déclare son type de retour, est borné : le type COUVRE
    // les souscriptions (jamais signalé comme orphelin — la résolution par
    // nom est approximative). Seuls les posts irrésolus comptent au caveat.
    let mut resolved_covering: BTreeSet<String> = BTreeSet::new();
    let mut dynamic_posts = 0usize;
    let mut cache: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for cap in DYNAMIC_POST.captures_iter(corpus) {
        let var = cap[1].to_string();
        let types = cache
            .entry(var.clone())
            .or_insert_with(|| variable_types(corpus, &var))
            .clone();
        if types.is_empty() {
            dynamic_posts += 1;
        } else {
            resolved_covering.extend(types);
        }
    }
    for cap in FACTORY_POST.captures_iter(corpus) {
        let callee = format!("{}()", &cap[1]);
        let types = cache
            .entry(callee)
            .or_insert_with(|| factory_return_types(corpus, &cap[1]))
            .clone();
        if types.is_empty() {
            dynamic_posts += 1;
        } else {
            resolved_covering.extend(types);
        }
    }

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
        .chain(resolved_covering.iter())
        .flat_map(|name| {
            let last = name.rsplit('.').next().unwrap_or(name).to_string();
            let chain = ancestors(&last);
            std::iter::once(last).chain(chain)
        })
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

    /// Java 16 records implement interfaces like anything else, and a record
    /// posted to UIThread is thread dispatch. The supertype regex asked for
    /// the literal `class`, so a record never reached the Runnable check.
    #[test]
    fn a_record_implementing_runnable_is_not_an_event() {
        let corpus = "\
UIThread.post(new HandleRefreshDone(this));
private record HandleRefreshDone(WeakReference<Frag> ref) implements Runnable {
    public void run() {}
}
";
        let report = analyze(corpus);
        assert!(
            !report.posted_never_subscribed.contains("HandleRefreshDone"),
            "a record implementing Runnable is thread dispatch: {report:?}"
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
    fn a_runnable_by_interface_posted_to_a_handler_is_not_an_event() {
        // Cas réel : la classe ne s'appelle pas *Runnable mais implémente
        // Runnable — la poster sur UIThread est du thread dispatch.
        let corpus = concat!(
            "class AdRefreshTask(private val ctx: Context) : Runnable {\n",
            "    override fun run() {}\n",
            "}\n",
            "uiThread.post(AdRefreshTask(ctx))\n",
        );
        let report = analyze(corpus);
        assert!(
            report.is_empty(),
            "une classe qui implémente Runnable n'est pas un event: {report:?}"
        );
    }

    #[test]
    fn a_java_runnable_implementer_posted_is_not_an_event() {
        let corpus = concat!(
            "public class FlushTask implements Runnable {\n",
            "    public void run() {}\n",
            "}\n",
            "handler.post(new FlushTask());\n",
        );
        let report = analyze(corpus);
        assert!(
            report.is_empty(),
            "implements Runnable = thread dispatch, pas un event: {report:?}"
        );
    }

    #[test]
    fn a_same_line_subscribe_fun_is_a_subscription() {
        let corpus = concat!(
            "@Subscribe fun onPing(event: PingEvent) = handle(event)\n",
            "bus.post(PingEvent())\n",
            "@Subscribe public void onPong(PongEvent event) {}\n",
            "bus.post(new PongEvent());\n",
        );
        let report = analyze(corpus);
        assert!(
            report.is_empty(),
            "@Subscribe et fun/void sur la même ligne restent une souscription: {report:?}"
        );
    }

    #[test]
    fn an_inline_generic_subscription_is_seen() {
        let corpus = concat!(
            "bus.subscribe<ScrollEvent> { handle(it) }\n",
            "bus.post(ScrollEvent())\n",
        );
        let report = analyze(corpus);
        assert!(
            report.is_empty(),
            "une souscription inline `subscribe<T> {{ }}` compte: {report:?}"
        );
    }

    #[test]
    fn a_class_literal_subscription_is_seen() {
        let corpus = concat!(
            "bus.register(TapEvent::class) { onTap(it) }\n",
            "bus.post(TapEvent())\n",
            "bus.subscribe(SwipeEvent::class.java, listener)\n",
            "bus.post(SwipeEvent())\n",
        );
        let report = analyze(corpus);
        assert!(
            report.is_empty(),
            "une souscription par littéral de classe compte: {report:?}"
        );
    }

    #[test]
    fn a_post_via_typed_variable_satisfies_the_subscription() {
        let corpus = concat!(
            "@Subscribe\nfun on(event: SyncEvent) {}\n",
            "val pending: SyncEvent = SyncEvent()\n",
            "bus.post(pending)\n",
        );
        let report = analyze(corpus);
        assert!(
            report.subscribed_never_posted.is_empty(),
            "un post via variable typée nourrit la souscription: {report:?}"
        );
        assert_eq!(report.dynamic_posts, 0, "le post est résolu, pas dynamique");
    }

    #[test]
    fn a_post_via_constructor_inferred_variable_satisfies_the_subscription() {
        let corpus = concat!(
            "@Subscribe\nfun on(event: RetryEvent) {}\n",
            "val retry = RetryEvent()\n",
            "bus.post(retry)\n",
        );
        let report = analyze(corpus);
        assert!(
            report.subscribed_never_posted.is_empty(),
            "un post via variable inférée du constructeur nourrit la souscription: {report:?}"
        );
    }

    #[test]
    fn a_post_via_factory_return_type_satisfies_the_subscription() {
        let corpus = concat!(
            "@Subscribe\nfun on(event: BuiltEvent) {}\n",
            "fun buildEvent(): BuiltEvent = BuiltEvent()\n",
            "bus.post(buildEvent())\n",
            "@Subscribe\npublic void onJ(MadeEvent event) {}\n",
            "public MadeEvent makeEvent() { return new MadeEvent(); }\n",
            "bus.post(makeEvent());\n",
        );
        let report = analyze(corpus);
        assert!(
            report.subscribed_never_posted.is_empty(),
            "le type de retour d'une factory borne le post: {report:?}"
        );
    }

    #[test]
    fn a_resolved_variable_post_is_never_reported_as_orphan() {
        // La résolution par nom est approximative : elle COUVRE les
        // souscriptions mais ne fabrique jamais un orphelin signalable.
        let corpus = "val e = LonelyEvent()\nbus.post(e)\n@Subscribe\nfun on(x: OtherEvent) {}\n";
        let report = analyze(corpus);
        assert!(
            !report.posted_never_subscribed.contains("LonelyEvent"),
            "un post résolu par variable ne devient pas un orphelin: {report:?}"
        );
    }

    #[test]
    fn an_unresolvable_variable_still_counts_as_dynamic() {
        let corpus = "@Subscribe\nfun on(event: MaybeEvent) {}\nbus.post(mystery)\n";
        let report = analyze(corpus);
        assert_eq!(report.dynamic_posts, 1);
        assert!(report.subscribed_never_posted.contains("MaybeEvent"));
    }

    #[test]
    fn annotation_arguments_do_not_break_matching() {
        let corpus =
            "@Subscribe(threadMode = ThreadMode.MAIN)\nfun onSticky(event: ConfigEvent) {}\n";
        let report = analyze(corpus);
        assert!(report.subscribed_never_posted.contains("ConfigEvent"));
    }
}
