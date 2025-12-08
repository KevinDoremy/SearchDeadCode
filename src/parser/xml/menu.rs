// Menu XML parser
//
// Parses Android menu XML files (res/menu/*.xml)
// to extract action class references and onClick handlers.

use super::XmlParseResult;
use miette::Result;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::path::Path;
use tracing::debug;

/// Parser for Android Menu XML files
pub struct MenuParser;

impl MenuParser {
    pub fn new() -> Self {
        Self
    }

    /// Parse a menu XML file and extract class references
    pub fn parse(&self, path: &Path, contents: &str) -> Result<XmlParseResult> {
        let mut result = XmlParseResult::new();
        let mut reader = Reader::from_str(contents);
        reader.config_mut().trim_text(true);

        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                    let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                    // Handle <item> menu items
                    if tag_name == "item" {
                        for attr in e.attributes().filter_map(|a| a.ok()) {
                            let key = String::from_utf8_lossy(attr.key.as_ref());

                            // android:onClick="onMenuItemClick"
                            // We track method names for potential reference
                            if key == "android:onClick" || key.ends_with(":onClick") {
                                let _value = String::from_utf8_lossy(&attr.value).to_string();
                                // Could track onClick method references
                            }

                            // app:actionViewClass="com.example.CustomActionView"
                            if key == "app:actionViewClass"
                                || key == "actionViewClass"
                                || key.ends_with(":actionViewClass")
                            {
                                let value = String::from_utf8_lossy(&attr.value).to_string();
                                if value.contains('.') {
                                    debug!("Menu: found actionViewClass {}", value);
                                    result.class_references.insert(value);
                                }
                            }

                            // app:actionProviderClass="androidx.appcompat.widget.ShareActionProvider"
                            if key == "app:actionProviderClass"
                                || key == "actionProviderClass"
                                || key.ends_with(":actionProviderClass")
                            {
                                let value = String::from_utf8_lossy(&attr.value).to_string();
                                if value.contains('.') {
                                    debug!("Menu: found actionProviderClass {}", value);
                                    result.class_references.insert(value);
                                }
                            }

                            // app:actionLayout="@layout/custom_action"
                            // This references a layout, not a class directly
                        }
                    }

                    // Handle custom menu item classes (rare but possible)
                    if tag_name.contains('.') {
                        result.class_references.insert(tag_name.clone());
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    debug!("Error parsing menu {}: {:?}", path.display(), e);
                    break;
                }
                _ => {}
            }
            buf.clear();
        }

        debug!(
            "Parsed menu {}: {} class references",
            path.display(),
            result.class_references.len()
        );

        Ok(result)
    }
}

impl Default for MenuParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_menu_action_view() {
        let parser = MenuParser::new();
        let menu = r#"
            <?xml version="1.0" encoding="utf-8"?>
            <menu xmlns:android="http://schemas.android.com/apk/res/android"
                xmlns:app="http://schemas.android.com/apk/res-auto">

                <item
                    android:id="@+id/action_search"
                    android:title="Search"
                    app:actionViewClass="androidx.appcompat.widget.SearchView"
                    app:showAsAction="ifRoom|collapseActionView" />

                <item
                    android:id="@+id/action_share"
                    android:title="Share"
                    app:actionProviderClass="androidx.appcompat.widget.ShareActionProvider"
                    app:showAsAction="never" />
            </menu>
        "#;

        let result = parser.parse(Path::new("menu_main.xml"), menu).unwrap();

        assert!(result
            .class_references
            .contains("androidx.appcompat.widget.SearchView"));
        assert!(result
            .class_references
            .contains("androidx.appcompat.widget.ShareActionProvider"));
    }
}
