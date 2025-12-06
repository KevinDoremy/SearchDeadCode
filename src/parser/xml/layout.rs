use super::XmlParseResult;
use miette::Result;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::path::Path;
use tracing::debug;

/// Parser for Android layout XML files
pub struct LayoutParser;

impl LayoutParser {
    pub fn new() -> Self {
        Self
    }

    /// Parse a layout XML file and extract class references
    pub fn parse(&self, path: &Path, contents: &str) -> Result<XmlParseResult> {
        let mut result = XmlParseResult::new();
        let mut reader = Reader::from_str(contents);
        reader.trim_text(true);

        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                    let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                    // Check if the tag itself is a custom view class
                    if tag_name.contains('.') {
                        result.class_references.insert(tag_name.clone());
                    }

                    // Extract class attribute for <view> tags
                    if tag_name == "view" || tag_name == "View" {
                        for attr in e.attributes().filter_map(|a| a.ok()) {
                            let key = String::from_utf8_lossy(attr.key.as_ref());
                            if key == "class" {
                                let value = String::from_utf8_lossy(&attr.value).to_string();
                                result.class_references.insert(value);
                            }
                        }
                    }

                    // Extract tools:context for activity association
                    for attr in e.attributes().filter_map(|a| a.ok()) {
                        let key = String::from_utf8_lossy(attr.key.as_ref());

                        // tools:context=".MainActivity"
                        if key == "tools:context" || key.ends_with(":context") {
                            let value = String::from_utf8_lossy(&attr.value).to_string();
                            if value.contains('.') || value.starts_with('.') {
                                // Need package context to resolve relative names
                                result.class_references.insert(value);
                            }
                        }

                        // app:viewModel="@{viewModel}" or similar binding expressions
                        if key.starts_with("app:") || key.starts_with("bind:") {
                            let value = String::from_utf8_lossy(&attr.value).to_string();
                            // Extract class names from binding expressions
                            self.extract_binding_references(&value, &mut result);
                        }

                        // android:onClick="onButtonClick" (method references)
                        if key == "android:onClick" || key.ends_with(":onClick") {
                            // This references a method, but we track it for completeness
                            let value = String::from_utf8_lossy(&attr.value).to_string();
                            // Method references start with a letter, not @
                            if !value.starts_with('@') && !value.is_empty() {
                                // Could track method references here
                            }
                        }
                    }

                    // Handle <fragment> tags
                    if tag_name == "fragment"
                        || tag_name == "androidx.fragment.app.FragmentContainerView"
                    {
                        for attr in e.attributes().filter_map(|a| a.ok()) {
                            let key = String::from_utf8_lossy(attr.key.as_ref());
                            if key == "android:name" || key == "class" || key.ends_with(":name") {
                                let value = String::from_utf8_lossy(&attr.value).to_string();
                                if value.contains('.') {
                                    result.class_references.insert(value);
                                }
                            }
                        }
                    }

                    // Handle navigation graph references
                    if tag_name == "action" || tag_name == "fragment" || tag_name == "dialog" {
                        for attr in e.attributes().filter_map(|a| a.ok()) {
                            let key = String::from_utf8_lossy(attr.key.as_ref());
                            if key == "android:name" || key.ends_with(":name") {
                                let value = String::from_utf8_lossy(&attr.value).to_string();
                                if value.contains('.') {
                                    result.class_references.insert(value);
                                }
                            }
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    debug!("Error parsing layout {}: {:?}", path.display(), e);
                    break;
                }
                _ => {}
            }
            buf.clear();
        }

        debug!(
            "Parsed layout {}: {} class references",
            path.display(),
            result.class_references.len()
        );

        Ok(result)
    }

    /// Extract class references from data binding expressions
    fn extract_binding_references(&self, expression: &str, result: &mut XmlParseResult) {
        // Data binding expressions like "@{viewModel.field}" or "@{com.example.Util.method()}"
        if expression.starts_with("@{") && expression.ends_with('}') {
            let inner = &expression[2..expression.len() - 1];

            // Look for fully qualified class names
            for word in inner.split(|c: char| !c.is_alphanumeric() && c != '.') {
                let word = word.trim();
                if word.contains('.')
                    && word
                        .chars()
                        .next()
                        .map(|c| c.is_uppercase())
                        .unwrap_or(false)
                {
                    // Likely a class reference
                    result.class_references.insert(word.to_string());
                }
            }
        }

        // Also handle type references like "type="com.example.MyType""
        if expression.contains('.') && !expression.contains(' ') {
            let cleaned = expression.trim_matches('"').trim_matches('\'');
            if cleaned
                .chars()
                .next()
                .map(|c| c.is_alphabetic())
                .unwrap_or(false)
            {
                result.class_references.insert(cleaned.to_string());
            }
        }
    }
}

impl Default for LayoutParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_layout_custom_view() {
        let parser = LayoutParser::new();
        let layout = r#"
            <?xml version="1.0" encoding="utf-8"?>
            <LinearLayout xmlns:android="http://schemas.android.com/apk/res/android">
                <com.example.CustomView
                    android:layout_width="match_parent"
                    android:layout_height="wrap_content" />
            </LinearLayout>
        "#;

        let result = parser.parse(Path::new("layout.xml"), layout).unwrap();

        assert!(result.class_references.contains("com.example.CustomView"));
    }

    #[test]
    fn test_parse_layout_fragment() {
        let parser = LayoutParser::new();
        let layout = r#"
            <?xml version="1.0" encoding="utf-8"?>
            <FrameLayout xmlns:android="http://schemas.android.com/apk/res/android">
                <fragment
                    android:name="com.example.MyFragment"
                    android:layout_width="match_parent"
                    android:layout_height="match_parent" />
            </FrameLayout>
        "#;

        let result = parser.parse(Path::new("layout.xml"), layout).unwrap();

        assert!(result.class_references.contains("com.example.MyFragment"));
    }

    #[test]
    fn test_parse_tools_context() {
        let parser = LayoutParser::new();
        let layout = r#"
            <?xml version="1.0" encoding="utf-8"?>
            <LinearLayout
                xmlns:android="http://schemas.android.com/apk/res/android"
                xmlns:tools="http://schemas.android.com/tools"
                tools:context=".MainActivity" />
        "#;

        let result = parser.parse(Path::new("layout.xml"), layout).unwrap();

        assert!(result.class_references.contains(".MainActivity"));
    }
}
