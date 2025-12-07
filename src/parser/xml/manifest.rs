use super::XmlParseResult;
use miette::Result;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::path::Path;
use tracing::debug;

/// Parser for AndroidManifest.xml files
pub struct ManifestParser;

impl ManifestParser {
    pub fn new() -> Self {
        Self
    }

    /// Parse an AndroidManifest.xml file and extract class references
    pub fn parse(&self, path: &Path, contents: &str) -> Result<XmlParseResult> {
        let mut result = XmlParseResult::new();
        let mut reader = Reader::from_str(contents);
        reader.config_mut().trim_text(true);

        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                    let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                    // Extract package from manifest tag
                    if tag_name == "manifest" {
                        for attr in e.attributes().filter_map(|a| a.ok()) {
                            if attr.key.as_ref() == b"package" {
                                result.package =
                                    Some(String::from_utf8_lossy(&attr.value).to_string());
                            }
                        }
                    }

                    // Extract android:name attributes from component declarations
                    if matches!(
                        tag_name.as_str(),
                        "activity" | "service" | "receiver" | "provider" | "application"
                    ) {
                        for attr in e.attributes().filter_map(|a| a.ok()) {
                            let key = String::from_utf8_lossy(attr.key.as_ref());
                            if key == "android:name" || key.ends_with(":name") {
                                let value = String::from_utf8_lossy(&attr.value).to_string();
                                let class_name = self.resolve_class_name(&value, &result.package);
                                result.class_references.insert(class_name);
                            }
                        }
                    }

                    // Extract meta-data values that might be class names
                    if tag_name == "meta-data" {
                        let mut value_value = None;

                        for attr in e.attributes().filter_map(|a| a.ok()) {
                            let key = String::from_utf8_lossy(attr.key.as_ref());
                            if key == "android:value" || key.ends_with(":value") {
                                value_value =
                                    Some(String::from_utf8_lossy(&attr.value).to_string());
                            }
                        }

                        // Check if value looks like a class name
                        if let Some(value) = value_value {
                            if value.contains('.') && !value.contains(' ') {
                                result.class_references.insert(value);
                            }
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    debug!("Error parsing manifest {}: {:?}", path.display(), e);
                    break;
                }
                _ => {}
            }
            buf.clear();
        }

        debug!(
            "Parsed manifest {}: {} class references",
            path.display(),
            result.class_references.len()
        );

        Ok(result)
    }

    /// Resolve a class name, handling relative names like ".MainActivity"
    fn resolve_class_name(&self, name: &str, package: &Option<String>) -> String {
        if let Some(stripped) = name.strip_prefix('.') {
            // Relative class name
            if let Some(pkg) = package {
                format!("{}.{}", pkg, stripped)
            } else {
                stripped.to_string() // Remove leading dot
            }
        } else if !name.contains('.') {
            // Simple class name, assume same package
            if let Some(pkg) = package {
                format!("{}.{}", pkg, name)
            } else {
                name.to_string()
            }
        } else {
            // Fully qualified name
            name.to_string()
        }
    }
}

impl Default for ManifestParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_manifest() {
        let parser = ManifestParser::new();
        let manifest = r#"
            <?xml version="1.0" encoding="utf-8"?>
            <manifest xmlns:android="http://schemas.android.com/apk/res/android"
                package="com.example.app">
                <application android:name=".MyApplication">
                    <activity android:name=".MainActivity" />
                    <service android:name=".MyService" />
                </application>
            </manifest>
        "#;

        let result = parser
            .parse(Path::new("AndroidManifest.xml"), manifest)
            .unwrap();

        assert_eq!(result.package, Some("com.example.app".to_string()));
        assert!(result
            .class_references
            .contains("com.example.app.MainActivity"));
        assert!(result
            .class_references
            .contains("com.example.app.MyService"));
        assert!(result
            .class_references
            .contains("com.example.app.MyApplication"));
    }

    #[test]
    fn test_resolve_class_name() {
        let parser = ManifestParser::new();
        let package = Some("com.example".to_string());

        assert_eq!(
            parser.resolve_class_name(".MainActivity", &package),
            "com.example.MainActivity"
        );
        assert_eq!(
            parser.resolve_class_name("com.other.Activity", &package),
            "com.other.Activity"
        );
    }
}
