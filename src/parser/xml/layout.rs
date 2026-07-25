use super::{MethodReference, XmlParseResult};
use miette::Result;
use quick_xml::events::Event;
use quick_xml::Reader;
use regex::Regex;
use std::path::Path;
use std::sync::LazyLock;
use tracing::debug;

/// Regex patterns for parsing data binding expressions
static METHOD_CALL_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    // Matches: variable.method() or variable::method or variable.method
    // Examples: viewModel.onClicked(), viewModel::onClicked, vm.doSomething()
    Regex::new(r"([a-z][a-zA-Z0-9_]*)\s*(?:::|\.)([a-z][a-zA-Z0-9_]*)\s*(?:\(\)|$|\s|[,})])")
        .expect("Invalid method call regex")
});

static LAMBDA_METHOD_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    // Matches: (_) -> variable.method() or () -> variable.method()
    // Examples: (_) -> viewModel.onConnectClicked(), () -> vm.onClick()
    Regex::new(r"\([_,\s]*\)\s*->\s*([a-z][a-zA-Z0-9_]*)\s*\.\s*([a-z][a-zA-Z0-9_]*)\s*\(\)")
        .expect("Invalid lambda method regex")
});

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
        reader.config_mut().trim_text(true);

        let mut buf = Vec::new();
        let mut in_data_block = false;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                    let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                    // Track when we enter a <data> block
                    if tag_name == "data" {
                        in_data_block = true;
                    }

                    // Parse <variable> and <import> tags inside <data> block
                    if in_data_block {
                        self.parse_data_block_element(&tag_name, e, &mut result);
                    }

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

                        // Any attribute with binding expression @{...}
                        let value = String::from_utf8_lossy(&attr.value).to_string();
                        if value.starts_with("@{") {
                            self.extract_binding_references(&value, &mut result);
                        }

                        // android:onClick="onButtonClick" (non-binding method references)
                        if (key == "android:onClick" || key.ends_with(":onClick"))
                            && !value.starts_with('@')
                            && !value.is_empty()
                        {
                            // Legacy onClick - these are activity methods, harder to track
                            // but we could add them if we knew the activity from tools:context
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
                Ok(Event::End(ref e)) => {
                    let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    if tag_name == "data" {
                        in_data_block = false;
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

        // After parsing, resolve method references using binding variables
        self.resolve_method_references(&mut result);

        if !result.method_references.is_empty() || !result.binding_variables.is_empty() {
            debug!(
                "Parsed layout {}: {} class refs, {} method refs, {} binding vars",
                path.display(),
                result.class_references.len(),
                result.method_references.len(),
                result.binding_variables.len()
            );
            for method_ref in &result.method_references {
                debug!(
                    "  -> Method: {}.{}",
                    method_ref.class_fqn, method_ref.method_name
                );
            }
        }

        Ok(result)
    }

    /// Parse elements inside a <data> block
    fn parse_data_block_element(
        &self,
        tag_name: &str,
        element: &quick_xml::events::BytesStart<'_>,
        result: &mut XmlParseResult,
    ) {
        // <variable name="viewModel" type="com.example.MyViewModel" />
        if tag_name == "variable" {
            let mut name = None;
            let mut type_fqn = None;

            for attr in element.attributes().filter_map(|a| a.ok()) {
                let key = String::from_utf8_lossy(attr.key.as_ref());
                let value = String::from_utf8_lossy(&attr.value).to_string();

                if key == "name" {
                    name = Some(value);
                } else if key == "type" {
                    type_fqn = Some(value);
                }
            }

            if let (Some(name), Some(type_fqn)) = (name, type_fqn) {
                debug!("Found binding variable: {} -> {}", name, type_fqn);
                // Also add the type as a class reference
                result.class_references.insert(type_fqn.clone());
                result.binding_variables.insert(name, type_fqn);
            }
        }

        // <import type="com.example.Helper" /> or <import type="..." alias="..." />
        if tag_name == "import" {
            for attr in element.attributes().filter_map(|a| a.ok()) {
                let key = String::from_utf8_lossy(attr.key.as_ref());
                if key == "type" {
                    let value = String::from_utf8_lossy(&attr.value).to_string();
                    result.class_references.insert(value.clone());

                    // If there's an alias, map it to the type
                    let mut alias = None;
                    for attr2 in element.attributes().filter_map(|a| a.ok()) {
                        let key2 = String::from_utf8_lossy(attr2.key.as_ref());
                        if key2 == "alias" {
                            alias = Some(String::from_utf8_lossy(&attr2.value).to_string());
                        }
                    }

                    // Use alias or simple name as the variable name
                    let simple_name = alias.unwrap_or_else(|| {
                        value.split('.').next_back().unwrap_or(&value).to_string()
                    });
                    result.binding_variables.insert(simple_name, value);
                }
            }
        }
    }

    /// Extract class and method references from data binding expressions
    fn extract_binding_references(&self, expression: &str, result: &mut XmlParseResult) {
        // Data binding expressions like "@{viewModel.field}" or "@{com.example.Util.method()}"
        if !expression.starts_with("@{") || !expression.ends_with('}') {
            return;
        }

        let inner = &expression[2..expression.len() - 1];

        // Extract method calls like: viewModel.onClicked() or viewModel::onClicked
        // Also handles lambdas: (_) -> viewModel.onClicked()
        self.extract_method_calls(inner, result);

        // Look for fully qualified class names (e.g., com.example.Util.method())
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

    /// Extract method calls from binding expressions
    fn extract_method_calls(&self, expression: &str, result: &mut XmlParseResult) {
        // Store pending method references (variable_name, method_name)
        // These will be resolved to actual class FQNs later

        // Pattern 1: Lambda style - (_) -> viewModel.method()
        for cap in LAMBDA_METHOD_PATTERN.captures_iter(expression) {
            if let (Some(var), Some(method)) = (cap.get(1), cap.get(2)) {
                let var_name = var.as_str().to_string();
                let method_name = method.as_str().to_string();
                debug!("Found lambda method call: {}.{}()", var_name, method_name);
                // Store with placeholder class - will be resolved later
                result.method_references.insert(MethodReference {
                    class_fqn: format!("__var__{}", var_name),
                    method_name,
                });
            }
        }

        // Pattern 2: Direct call style - viewModel.method() or viewModel::method
        for cap in METHOD_CALL_PATTERN.captures_iter(expression) {
            if let (Some(var), Some(method)) = (cap.get(1), cap.get(2)) {
                let var_name = var.as_str().to_string();
                let method_name = method.as_str().to_string();

                // Skip common false positives
                if is_kotlin_keyword(&var_name) || is_common_property(&method_name) {
                    continue;
                }

                debug!("Found method call: {}.{}", var_name, method_name);
                result.method_references.insert(MethodReference {
                    class_fqn: format!("__var__{}", var_name),
                    method_name,
                });
            }
        }
    }

    /// Resolve method references using binding variable types
    fn resolve_method_references(&self, result: &mut XmlParseResult) {
        let resolved: Vec<MethodReference> = result
            .method_references
            .iter()
            .filter_map(|method_ref| {
                if method_ref.class_fqn.starts_with("__var__") {
                    let var_name = &method_ref.class_fqn[7..]; // Strip "__var__" prefix
                    if let Some(class_fqn) = result.binding_variables.get(var_name) {
                        return Some(MethodReference {
                            class_fqn: class_fqn.clone(),
                            method_name: method_ref.method_name.clone(),
                        });
                    }
                    // Variable not found - keep placeholder for debugging
                    debug!(
                        "Unresolved binding variable: {} for method {}",
                        var_name, method_ref.method_name
                    );
                    None
                } else {
                    // Already has a class FQN
                    Some(method_ref.clone())
                }
            })
            .collect();

        // Replace with resolved references
        result.method_references = resolved.into_iter().collect();
    }
}

/// Check if a string is a Kotlin keyword that should be skipped
fn is_kotlin_keyword(s: &str) -> bool {
    matches!(
        s,
        "if" | "else"
            | "when"
            | "for"
            | "while"
            | "do"
            | "try"
            | "catch"
            | "finally"
            | "throw"
            | "return"
            | "break"
            | "continue"
            | "this"
            | "super"
            | "null"
            | "true"
            | "false"
            | "is"
            | "as"
            | "in"
            | "it"
    )
}

/// Check if a string is a common property access (not a custom method)
fn is_common_property(s: &str) -> bool {
    matches!(
        s,
        "text"
            | "visibility"
            | "enabled"
            | "selected"
            | "checked"
            | "value"
            | "size"
            | "length"
            | "isEmpty"
            | "isNotEmpty"
            | "toString"
            | "toInt"
            | "toLong"
            | "toFloat"
            | "toDouble"
            | "equals"
            | "hashCode"
            | "first"
            | "last"
            | "get"
            | "set"
            | "contains"
            | "indexOf"
            | "let"
            | "also"
            | "apply"
            | "run"
            | "with"
            | "takeIf"
            | "takeUnless"
    )
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

    #[test]
    fn test_parse_data_binding_variable() {
        let parser = LayoutParser::new();
        let layout = r#"
            <?xml version="1.0" encoding="utf-8"?>
            <layout xmlns:android="http://schemas.android.com/apk/res/android">
                <data>
                    <variable
                        name="viewModel"
                        type="com.example.MyViewModel" />
                </data>
                <LinearLayout
                    android:layout_width="match_parent"
                    android:layout_height="wrap_content" />
            </layout>
        "#;

        let result = parser.parse(Path::new("layout.xml"), layout).unwrap();

        // Should extract the type as a class reference
        assert!(result.class_references.contains("com.example.MyViewModel"));
        // Should map the variable name to its type
        assert_eq!(
            result.binding_variables.get("viewModel"),
            Some(&"com.example.MyViewModel".to_string())
        );
    }

    #[test]
    fn test_parse_data_binding_import() {
        let parser = LayoutParser::new();
        let layout = r#"
            <?xml version="1.0" encoding="utf-8"?>
            <layout xmlns:android="http://schemas.android.com/apk/res/android">
                <data>
                    <import type="com.example.Utils" />
                    <import type="com.example.Helper" alias="H" />
                </data>
                <LinearLayout
                    android:layout_width="match_parent"
                    android:layout_height="wrap_content" />
            </layout>
        "#;

        let result = parser.parse(Path::new("layout.xml"), layout).unwrap();

        assert!(result.class_references.contains("com.example.Utils"));
        assert!(result.class_references.contains("com.example.Helper"));
        assert_eq!(
            result.binding_variables.get("Utils"),
            Some(&"com.example.Utils".to_string())
        );
        assert_eq!(
            result.binding_variables.get("H"),
            Some(&"com.example.Helper".to_string())
        );
    }

    #[test]
    fn test_parse_data_binding_lambda_method_call() {
        let parser = LayoutParser::new();
        let layout = r#"
            <?xml version="1.0" encoding="utf-8"?>
            <layout xmlns:android="http://schemas.android.com/apk/res/android">
                <data>
                    <variable
                        name="viewModel"
                        type="com.example.WelcomeViewModel" />
                </data>
                <TextView
                    android:layout_width="wrap_content"
                    android:layout_height="wrap_content"
                    android:onClick="@{(_) -> viewModel.onConnectClicked()}" />
            </layout>
        "#;

        let result = parser.parse(Path::new("layout.xml"), layout).unwrap();

        // Should find the method reference
        let method_ref = MethodReference {
            class_fqn: "com.example.WelcomeViewModel".to_string(),
            method_name: "onConnectClicked".to_string(),
        };
        assert!(
            result.method_references.contains(&method_ref),
            "Expected method reference {:?}, got {:?}",
            method_ref,
            result.method_references
        );
    }

    #[test]
    fn test_parse_data_binding_method_reference() {
        let parser = LayoutParser::new();
        let layout = r#"
            <?xml version="1.0" encoding="utf-8"?>
            <layout xmlns:android="http://schemas.android.com/apk/res/android">
                <data>
                    <variable
                        name="vm"
                        type="com.example.TestViewModel" />
                </data>
                <CheckBox
                    android:layout_width="wrap_content"
                    android:layout_height="wrap_content"
                    android:onClick="@{vm::onCheckboxClicked}" />
            </layout>
        "#;

        let result = parser.parse(Path::new("layout.xml"), layout).unwrap();

        let method_ref = MethodReference {
            class_fqn: "com.example.TestViewModel".to_string(),
            method_name: "onCheckboxClicked".to_string(),
        };
        assert!(
            result.method_references.contains(&method_ref),
            "Expected method reference {:?}, got {:?}",
            method_ref,
            result.method_references
        );
    }

    #[test]
    fn test_parse_data_binding_multiple_methods() {
        let parser = LayoutParser::new();
        let layout = r#"
            <?xml version="1.0" encoding="utf-8"?>
            <layout xmlns:android="http://schemas.android.com/apk/res/android">
                <data>
                    <variable name="vm" type="com.example.AdminViewModel" />
                </data>
                <LinearLayout
                    android:layout_width="match_parent"
                    android:layout_height="wrap_content">
                    <Button
                        android:onClick="@{() -> vm.onSaveClicked()}" />
                    <Button
                        android:onClick="@{() -> vm.onCancelClicked()}" />
                    <Button
                        android:onClick="@{vm::onResetClicked}" />
                </LinearLayout>
            </layout>
        "#;

        let result = parser.parse(Path::new("layout.xml"), layout).unwrap();

        assert!(result
            .method_references
            .iter()
            .any(|r| r.method_name == "onSaveClicked"));
        assert!(result
            .method_references
            .iter()
            .any(|r| r.method_name == "onCancelClicked"));
        assert!(result
            .method_references
            .iter()
            .any(|r| r.method_name == "onResetClicked"));
    }

    #[test]
    fn test_parse_real_lapresse_layout() {
        let parser = LayoutParser::new();
        // Actual layout from lapresse project
        let layout = r#"<?xml version="1.0" encoding="utf-8"?>
<layout
        xmlns:android="http://schemas.android.com/apk/res/android"
        xmlns:app="http://schemas.android.com/apk/res-auto">

    <data>
        <variable
                name="viewModel"
                type="ca.lapresse.android.lapresseplus.module.openingscenario.viewmodel.WelcomeViewModel" />
    </data>

    <androidx.constraintlayout.motion.widget.MotionLayout
            android:id="@+id/welcome_motion_layout"
            android:layout_width="match_parent"
            android:layout_height="match_parent">

        <androidx.appcompat.widget.AppCompatTextView
                android:id="@+id/welcome_sign_in_cta"
                android:layout_width="260dp"
                android:onClick="@{(_) -> viewModel.onConnectClicked()}"
                android:text="@string/cta_sign_in" />

        <androidx.appcompat.widget.AppCompatTextView
                android:id="@+id/welcome_not_now_cta"
                android:onClick="@{(_) -> viewModel.onNotNowClicked()}"
                android:text="@string/welcome_login_not_now" />
    </androidx.constraintlayout.motion.widget.MotionLayout>
</layout>"#;

        let result = parser
            .parse(Path::new("activity_welcome.xml"), layout)
            .unwrap();

        // Check variable is extracted
        assert_eq!(
            result.binding_variables.get("viewModel"),
            Some(&"ca.lapresse.android.lapresseplus.module.openingscenario.viewmodel.WelcomeViewModel".to_string())
        );

        // Check class reference is extracted
        assert!(result.class_references.contains(
            "ca.lapresse.android.lapresseplus.module.openingscenario.viewmodel.WelcomeViewModel"
        ));

        // Check method references are extracted
        let connect_ref = MethodReference {
            class_fqn:
                "ca.lapresse.android.lapresseplus.module.openingscenario.viewmodel.WelcomeViewModel"
                    .to_string(),
            method_name: "onConnectClicked".to_string(),
        };
        let not_now_ref = MethodReference {
            class_fqn:
                "ca.lapresse.android.lapresseplus.module.openingscenario.viewmodel.WelcomeViewModel"
                    .to_string(),
            method_name: "onNotNowClicked".to_string(),
        };

        println!("Method references found: {:?}", result.method_references);

        assert!(
            result.method_references.contains(&connect_ref),
            "Expected to find onConnectClicked, got: {:?}",
            result.method_references
        );
        assert!(
            result.method_references.contains(&not_now_ref),
            "Expected to find onNotNowClicked, got: {:?}",
            result.method_references
        );
    }
}
