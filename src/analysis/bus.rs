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

/// Kotlin: `@Subscribe ... fun name(param: Type`
static KOTLIN_SUBSCRIBER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"@Subscribe\b[^\n]*\n\s*(?:[a-z]+\s+)*fun\s+\w+\(\s*\w+\s*:\s*([A-Z]\w*)")
        .expect("Invalid Kotlin subscriber regex")
});

/// Java: `@Subscribe ... void name(Type param`
static JAVA_SUBSCRIBER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"@Subscribe\b[^\n]*\n\s*(?:[a-z]+\s+)*void\s+\w+\(\s*([A-Z]\w*)\s+\w+")
        .expect("Invalid Java subscriber regex")
});

/// `.post(FooEvent(` / `.post(new FooEvent(` / postSticky variants
static LITERAL_POST: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\.post(?:Sticky)?\(\s*(?:new\s+)?([A-Z]\w*)\s*\(").expect("Invalid post regex")
});

/// `.post(someVariable)` — a post whose type cannot be known statically
static DYNAMIC_POST: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\.post(?:Sticky)?\(\s*[a-z]\w*\s*\)").expect("Invalid dynamic post regex")
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
    let subscribed: BTreeSet<String> = KOTLIN_SUBSCRIBER
        .captures_iter(corpus)
        .chain(JAVA_SUBSCRIBER.captures_iter(corpus))
        .map(|c| c[1].to_string())
        .collect();
    // Handler.post(new Runnable() {...}) is thread dispatch, not an event
    let posted: BTreeSet<String> = LITERAL_POST
        .captures_iter(corpus)
        .map(|c| c[1].to_string())
        .filter(|name| name != "Runnable")
        .collect();
    let dynamic_posts = DYNAMIC_POST.find_iter(corpus).count();

    if subscribed.is_empty() && posted.is_empty() {
        return BusReport::default(); // no bus in this project
    }

    BusReport {
        posted_never_subscribed: posted.difference(&subscribed).cloned().collect(),
        subscribed_never_posted: subscribed.difference(&posted).cloned().collect(),
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
    fn annotation_arguments_do_not_break_matching() {
        let corpus =
            "@Subscribe(threadMode = ThreadMode.MAIN)\nfun onSticky(event: ConfigEvent) {}\n";
        let report = analyze(corpus);
        assert!(report.subscribed_never_posted.contains("ConfigEvent"));
    }
}
