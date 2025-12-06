// Navigation graph XML parser
//
// Parses Android Navigation Component XML files (res/navigation/*.xml)
// to extract fragment, dialog, and activity references.

use super::XmlParseResult;
use miette::Result;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::path::Path;
use tracing::debug;

/// Parser for Android Navigation XML files
pub struct NavigationParser;

impl NavigationParser {
    pub fn new() -> Self {
        Self
    }

    /// Parse a navigation XML file and extract class references
    pub fn parse(&self, path: &Path, contents: &str) -> Result<XmlParseResult> {
        let mut result = XmlParseResult::new();
        let mut reader = Reader::from_str(contents);
        reader.trim_text(true);

        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                    let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                    // Handle <fragment>, <dialog>, <activity> destinations
                    if tag_name == "fragment"
                        || tag_name == "dialog"
                        || tag_name == "activity"
                        || tag_name == "navigation"
                    {
                        for attr in e.attributes().filter_map(|a| a.ok()) {
                            let key = String::from_utf8_lossy(attr.key.as_ref());

                            // android:name="com.example.MyFragment"
                            if key == "android:name" || key == "name" || key.ends_with(":name") {
                                let value = String::from_utf8_lossy(&attr.value).to_string();
                                if value.contains('.') {
                                    debug!("Navigation: found destination {}", value);
                                    result.class_references.insert(value);
                                }
                            }
                        }
                    }

                    // Handle <action> destinations
                    if tag_name == "action" {
                        for attr in e.attributes().filter_map(|a| a.ok()) {
                            let key = String::from_utf8_lossy(attr.key.as_ref());

                            // app:destination="@id/myFragment"
                            // We can't resolve @id references here, but track them
                            if key == "app:destination" || key.ends_with(":destination") {
                                let value = String::from_utf8_lossy(&attr.value).to_string();
                                // Store action destinations for potential resolution
                                if !value.starts_with("@id") && value.contains('.') {
                                    result.class_references.insert(value);
                                }
                            }
                        }
                    }

                    // Handle <argument> types
                    if tag_name == "argument" {
                        for attr in e.attributes().filter_map(|a| a.ok()) {
                            let key = String::from_utf8_lossy(attr.key.as_ref());

                            // app:argType="com.example.MyParcelable"
                            if key == "app:argType" || key.ends_with(":argType") {
                                let value = String::from_utf8_lossy(&attr.value).to_string();
                                // Skip primitive types
                                if value.contains('.') && !value.starts_with("android.") {
                                    result.class_references.insert(value);
                                }
                            }
                        }
                    }

                    // Handle <deepLink> app references
                    if tag_name == "deepLink" {
                        for attr in e.attributes().filter_map(|a| a.ok()) {
                            let key = String::from_utf8_lossy(attr.key.as_ref());
                            if key == "app:uri" || key.ends_with(":uri") {
                                // Deep links might reference activities
                                // We track them for completeness
                            }
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    debug!("Error parsing navigation {}: {:?}", path.display(), e);
                    break;
                }
                _ => {}
            }
            buf.clear();
        }

        debug!(
            "Parsed navigation {}: {} class references",
            path.display(),
            result.class_references.len()
        );

        Ok(result)
    }
}

impl Default for NavigationParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_navigation_fragment() {
        let parser = NavigationParser::new();
        let nav = r#"
            <?xml version="1.0" encoding="utf-8"?>
            <navigation xmlns:android="http://schemas.android.com/apk/res/android"
                xmlns:app="http://schemas.android.com/apk/res-auto"
                app:startDestination="@id/homeFragment">

                <fragment
                    android:id="@+id/homeFragment"
                    android:name="com.example.HomeFragment"
                    android:label="Home" />

                <fragment
                    android:id="@+id/detailFragment"
                    android:name="com.example.DetailFragment"
                    android:label="Detail" />

                <dialog
                    android:id="@+id/confirmDialog"
                    android:name="com.example.ConfirmDialogFragment" />
            </navigation>
        "#;

        let result = parser.parse(Path::new("nav_main.xml"), nav).unwrap();

        assert!(result.class_references.contains("com.example.HomeFragment"));
        assert!(result
            .class_references
            .contains("com.example.DetailFragment"));
        assert!(result
            .class_references
            .contains("com.example.ConfirmDialogFragment"));
    }

    #[test]
    fn test_parse_navigation_argument() {
        let parser = NavigationParser::new();
        let nav = r#"
            <?xml version="1.0" encoding="utf-8"?>
            <navigation xmlns:android="http://schemas.android.com/apk/res/android"
                xmlns:app="http://schemas.android.com/apk/res-auto">

                <fragment
                    android:id="@+id/detailFragment"
                    android:name="com.example.DetailFragment">
                    <argument
                        android:name="item"
                        app:argType="com.example.model.Item" />
                </fragment>
            </navigation>
        "#;

        let result = parser.parse(Path::new("nav_main.xml"), nav).unwrap();

        assert!(result
            .class_references
            .contains("com.example.DetailFragment"));
        assert!(result.class_references.contains("com.example.model.Item"));
    }
}
