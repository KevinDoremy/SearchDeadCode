// Kotlin parser - some internal methods reserved for future use
#![allow(dead_code)]

use super::common::{node_text, point_to_location, ParseResult, Parser};
use crate::graph::{
    Declaration, DeclarationId, DeclarationKind, Language, Location, ReferenceKind,
    UnresolvedReference, Visibility,
};
use miette::{IntoDiagnostic, Result};
use regex::Regex;
use std::path::Path;
use std::sync::LazyLock;
use tracing::debug;
use tree_sitter::{Node, Parser as TsParser};

/// Matches misparsed function calls like `foo()` inside tree-sitter ERROR regions
static MISPARSED_CALL_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([a-z][a-zA-Z0-9]*)\s*\(\s*\)").expect("Invalid call regex"));

/// Column-0 type header tree-sitter can lose entirely inside an ERROR region
/// (the shape below: the `object` node is never produced at all).
static LOST_TYPE_HEADER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?m)^(?:(?:public|internal|private|protected|abstract|open|final|sealed|data|value)\s+)*(?:object|class|interface)\s+([A-Z][A-Za-z0-9_]*)",
    )
    .expect("Invalid lost type header regex")
});

/// Member headers inside a recovered type extent: functions and nested
/// classes (properties stay out — a local `val` in a lost body is
/// indistinguishable from a member). Modifiers and same-line annotations
/// are absorbed so `private` is visible in the match.
static LOST_MEMBER_HEADER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?m)^[ \t]+(?:(?:@[A-Za-z0-9_.]+|public|internal|private|protected|open|final|override|inline|operator|infix|suspend|tailrec|external|data|sealed|value)\s+)*(?:(fun)\s+([A-Za-z_][A-Za-z0-9_]*)\s*[(<]|(class)\s+([A-Z][A-Za-z0-9_]*))",
    )
    .expect("Invalid lost member header regex")
});

/// Kotlin source code parser using tree-sitter
pub struct KotlinParser {
    parser: TsParser,
}

impl KotlinParser {
    pub fn new() -> Self {
        let mut parser = TsParser::new();
        parser
            .set_language(&tree_sitter_kotlin::language())
            .expect("Failed to load Kotlin grammar");
        Self { parser }
    }

    /// Parse Kotlin source code and extract declarations
    fn parse_internal(&mut self, path: &Path, contents: &str) -> Result<ParseResult> {
        let tree = self
            .parser
            .parse(contents, None)
            .ok_or_else(|| miette::miette!("Failed to parse Kotlin file"))?;

        let root = tree.root_node();
        let mut result = ParseResult::new();

        // Extract package declaration
        result.package = self.extract_package(root, contents);

        // Extract imports
        result.imports = self.extract_imports(root, contents);

        // Clone to avoid borrow issues
        let package = result.package.clone();
        let imports = result.imports.clone();

        // Extract declarations
        self.extract_declarations(path, root, contents, &package, &mut result)?;

        // Extract references
        self.extract_references(path, root, contents, &imports, &mut result)?;

        Ok(result)
    }

    fn extract_package(&self, root: Node, source: &str) -> Option<String> {
        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            if child.kind() == "package_header" {
                // Find the identifier within package_header
                let mut pkg_cursor = child.walk();
                for pkg_child in child.children(&mut pkg_cursor) {
                    if pkg_child.kind() == "identifier" {
                        return Some(node_text(pkg_child, source).to_string());
                    }
                }
            }
        }
        None
    }

    fn extract_imports(&self, root: Node, source: &str) -> Vec<String> {
        let mut imports = Vec::new();
        let mut cursor = root.walk();

        for child in root.children(&mut cursor) {
            if child.kind() == "import_list" {
                let mut import_cursor = child.walk();
                for import in child.children(&mut import_cursor) {
                    if import.kind() == "import_header" {
                        // Find identifier by kind (not field name) since tree-sitter-kotlin
                        // doesn't use field names for import identifiers
                        let mut header_cursor = import.walk();
                        for header_child in import.children(&mut header_cursor) {
                            if header_child.kind() == "identifier" {
                                let import_text = node_text(header_child, source);
                                imports.push(import_text.to_string());
                                break;
                            }
                        }
                    }
                }
            }
        }

        imports
    }

    fn extract_declarations(
        &self,
        path: &Path,
        node: Node,
        source: &str,
        package: &Option<String>,
        result: &mut ParseResult,
    ) -> Result<()> {
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            match child.kind() {
                "class_declaration" => {
                    self.extract_class(path, child, source, package, None, result)?;
                }
                "object_declaration" => {
                    self.extract_object(path, child, source, package, None, result)?;
                }
                "function_declaration" => {
                    self.extract_function(path, child, source, package, None, result)?;
                }
                "property_declaration" => {
                    self.extract_property(path, child, source, package, None, result)?;
                }
                "type_alias" => {
                    self.extract_type_alias(path, child, source, package, result)?;
                }
                // Skip class_body and related nodes - they are already handled by extract_class_members
                // If we recurse into them, methods get extracted twice (once with parent, once without)
                "class_body" | "enum_class_body" | "companion_object" => {
                    // Don't recurse - already handled by extract_class/extract_object
                }
                _ => {
                    // Recurse into other nodes
                    self.extract_declarations(path, child, source, package, result)?;
                }
            }
        }

        Ok(())
    }

    fn extract_class(
        &self,
        path: &Path,
        node: Node,
        source: &str,
        package: &Option<String>,
        parent: Option<DeclarationId>,
        result: &mut ParseResult,
    ) -> Result<()> {
        let name = self.get_type_name(node, source)?;
        let location = point_to_location(
            path,
            node.start_position(),
            node.end_position(),
            node.start_byte(),
            node.end_byte(),
        );

        let id = DeclarationId::new(path.to_path_buf(), node.start_byte(), node.end_byte());

        // Determine kind (class, interface, enum, annotation)
        let kind = self.determine_class_kind(node, source);

        let mut decl = Declaration::new(id.clone(), name.clone(), kind, location, Language::Kotlin);

        // Set fully qualified name
        decl.fully_qualified_name = Some(self.build_fqn(package, &name));

        // Extract modifiers and visibility
        self.extract_modifiers(node, source, &mut decl);

        // Extract super types
        decl.super_types = self.extract_super_types(node, source);

        // Extract class delegation (e.g., class Foo : Bar by delegate)
        let imports_clone = result.imports.clone();
        self.extract_class_delegates(node, source, path, &imports_clone, result);

        // Extract annotations
        decl.annotations = self.extract_annotations(node, source);

        // Annotations on the primary constructor belong to the class for
        // retention purposes: `class Foo @Inject constructor()` is how DI
        // marks the CLASS as injectable
        let mut ctor_cursor = node.walk();
        for child in node.children(&mut ctor_cursor) {
            if child.kind() == "primary_constructor" {
                decl.annotations
                    .extend(self.extract_annotations(child, source));
            }
        }

        decl.parent = parent.clone();

        result.declarations.push(decl);

        // Extract class body members
        // Note: tree-sitter-kotlin doesn't use field names for class_body, so we find by kind
        let mut cursor = node.walk();
        let mut found_class_body = false;
        for child in node.children(&mut cursor) {
            // enums put their entries in an enum_class_body, not a class_body
            if child.kind() == "class_body" || child.kind() == "enum_class_body" {
                self.extract_class_members(path, child, source, package, id.clone(), result)?;
                found_class_body = true;
                break;
            }
        }

        // WORKAROUND: tree-sitter-kotlin grammar bug
        // When a class uses delegation (e.g., `class Foo : SomeInterface by delegate { ... }`),
        // the grammar incorrectly parses the class body `{...}` as a trailing lambda attached
        // to the delegation expression. We need to look for class members inside these
        // misplaced lambda_literal nodes.
        if !found_class_body {
            self.extract_class_members_from_misplaced_lambda(
                path, node, source, package, id, result,
            )?;
        }

        Ok(())
    }

    /// WORKAROUND for tree-sitter-kotlin grammar bug with class delegation.
    /// When class uses `by` delegation, the class body may be incorrectly parsed as a lambda.
    /// This method traverses the delegation_specifier nodes to find misplaced class members.
    fn extract_class_members_from_misplaced_lambda(
        &self,
        path: &Path,
        class_node: Node,
        source: &str,
        package: &Option<String>,
        parent: DeclarationId,
        result: &mut ParseResult,
    ) -> Result<()> {
        let mut cursor = class_node.walk();
        for child in class_node.children(&mut cursor) {
            if child.kind() == "delegation_specifier" {
                self.find_lambda_class_members(
                    path,
                    child,
                    source,
                    package,
                    parent.clone(),
                    result,
                )?;
            }
        }
        Ok(())
    }

    /// Recursively search for lambda_literal nodes that might contain misplaced class members
    fn find_lambda_class_members(
        &self,
        path: &Path,
        node: Node,
        source: &str,
        package: &Option<String>,
        parent: DeclarationId,
        result: &mut ParseResult,
    ) -> Result<()> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "lambda_literal" => {
                    // Look for statements inside the lambda
                    let mut lambda_cursor = child.walk();
                    for lambda_child in child.children(&mut lambda_cursor) {
                        if lambda_child.kind() == "statements" {
                            // This is likely the misplaced class body - extract members from it
                            self.extract_class_members(
                                path,
                                lambda_child,
                                source,
                                package,
                                parent.clone(),
                                result,
                            )?;
                        }
                    }
                }
                // Recurse into nested structures
                "call_expression" | "call_suffix" | "annotated_lambda" | "explicit_delegation" => {
                    self.find_lambda_class_members(
                        path,
                        child,
                        source,
                        package,
                        parent.clone(),
                        result,
                    )?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn extract_object(
        &self,
        path: &Path,
        node: Node,
        source: &str,
        package: &Option<String>,
        parent: Option<DeclarationId>,
        result: &mut ParseResult,
    ) -> Result<()> {
        let name = self.get_type_name(node, source)?;
        let location = point_to_location(
            path,
            node.start_position(),
            node.end_position(),
            node.start_byte(),
            node.end_byte(),
        );

        let id = DeclarationId::new(path.to_path_buf(), node.start_byte(), node.end_byte());

        let mut decl = Declaration::new(
            id.clone(),
            name.clone(),
            DeclarationKind::Object,
            location,
            Language::Kotlin,
        );

        decl.fully_qualified_name = Some(self.build_fqn(package, &name));
        self.extract_modifiers(node, source, &mut decl);
        decl.super_types = self.extract_super_types(node, source);
        decl.annotations = self.extract_annotations(node, source);
        decl.parent = parent.clone();

        result.declarations.push(decl);

        // Extract object body members
        // Note: tree-sitter-kotlin doesn't use field names for class_body, so we find by kind
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "class_body" {
                self.extract_class_members(path, child, source, package, id, result)?;
                break;
            }
        }

        Ok(())
    }

    fn extract_class_members(
        &self,
        path: &Path,
        body: Node,
        source: &str,
        package: &Option<String>,
        parent: DeclarationId,
        result: &mut ParseResult,
    ) -> Result<()> {
        let mut cursor = body.walk();

        for child in body.children(&mut cursor) {
            match child.kind() {
                "class_declaration" => {
                    self.extract_class(path, child, source, package, Some(parent.clone()), result)?;
                }
                "object_declaration" => {
                    self.extract_object(
                        path,
                        child,
                        source,
                        package,
                        Some(parent.clone()),
                        result,
                    )?;
                }
                "function_declaration" => {
                    self.extract_function(
                        path,
                        child,
                        source,
                        package,
                        Some(parent.clone()),
                        result,
                    )?;
                }
                "property_declaration" => {
                    self.extract_property(
                        path,
                        child,
                        source,
                        package,
                        Some(parent.clone()),
                        result,
                    )?;
                }
                "secondary_constructor" | "primary_constructor" => {
                    self.extract_constructor(path, child, source, parent.clone(), result)?;
                }
                "companion_object" => {
                    self.extract_companion_object(
                        path,
                        child,
                        source,
                        package,
                        parent.clone(),
                        result,
                    )?;
                }
                "enum_entry" => {
                    self.extract_enum_entry(path, child, source, parent.clone(), result)?;
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn extract_function(
        &self,
        path: &Path,
        node: Node,
        source: &str,
        package: &Option<String>,
        parent: Option<DeclarationId>,
        result: &mut ParseResult,
    ) -> Result<()> {
        // Extract function name - handle both regular and extension functions
        let name = self.extract_function_name(node, source);

        let location = point_to_location(
            path,
            node.start_position(),
            node.end_position(),
            node.start_byte(),
            node.end_byte(),
        );

        let id = DeclarationId::new(path.to_path_buf(), node.start_byte(), node.end_byte());

        let kind = if parent.is_some() {
            DeclarationKind::Method
        } else {
            DeclarationKind::Function
        };

        let mut decl = Declaration::new(id, name.clone(), kind, location.clone(), Language::Kotlin);

        if parent.is_none() {
            decl.fully_qualified_name = Some(self.build_fqn(package, &name));
        }

        self.extract_modifiers(node, source, &mut decl);
        decl.annotations = self.extract_annotations(node, source);
        decl.parent = parent;

        // Return type (e.g., "fun provide(): Engine" -> "Engine"); same node
        // shapes as property types
        decl.type_name = self.extract_property_type(node, source);

        // Extract extension receiver type (e.g., fun String.myExtension())
        if let Some(receiver_type) = self.extract_extension_receiver(node, source) {
            // Add a reference to the receiver type so it's not marked as dead code
            result.references.push(UnresolvedReference {
                name: receiver_type,
                qualified_name: None,
                kind: ReferenceKind::ExtensionReceiver,
                location: location.clone(),
                imports: result.imports.clone(),
            });
        }

        // Extract parameters
        if let Some(params) = node.child_by_field_name("function_value_parameters") {
            self.extract_parameters(path, params, source, decl.id.clone(), result)?;
        }

        result.declarations.push(decl);

        Ok(())
    }

    /// Extract the receiver type from an extension function (e.g., "String" from "fun String.myExtension()")
    fn extract_extension_receiver(&self, node: Node, source: &str) -> Option<String> {
        let mut cursor = node.walk();
        let mut found_fun = false;

        for child in node.children(&mut cursor) {
            let kind = child.kind();

            // Track when we see 'fun' keyword
            if kind == "fun" {
                found_fun = true;
                continue;
            }

            // After 'fun', look for receiver_type or user_type before the dot
            if found_fun {
                if kind == "receiver_type" || kind == "type_reference" {
                    let type_text = node_text(child, source);
                    // Strip generic parameters if present
                    let name = type_text.split('<').next().unwrap_or(type_text);
                    // Take the last component of qualified names
                    let simple_name = name.split('.').next_back().unwrap_or(name);
                    return Some(simple_name.to_string());
                }
                // For simple user types
                if kind == "user_type" {
                    let type_text = node_text(child, source);
                    let name = type_text.split('<').next().unwrap_or(type_text);
                    let simple_name = name.split('.').next_back().unwrap_or(name);
                    return Some(simple_name.to_string());
                }
                // Once we hit the function name (simple_identifier after receiver), stop
                if kind == "simple_identifier" {
                    break;
                }
            }
        }
        None
    }

    fn extract_property(
        &self,
        path: &Path,
        node: Node,
        source: &str,
        package: &Option<String>,
        parent: Option<DeclarationId>,
        result: &mut ParseResult,
    ) -> Result<()> {
        // Property can have multiple variable declarations
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "variable_declaration" {
                // Find the simple_identifier child (not a named field, just a child node)
                let mut var_cursor = child.walk();
                let name_node = child
                    .children(&mut var_cursor)
                    .find(|c| c.kind() == "simple_identifier");

                if let Some(name_node) = name_node {
                    let name = node_text(name_node, source).to_string();

                    // Determine the end byte: check if there's a following getter/setter
                    // In Kotlin's tree-sitter grammar, getter/setter are SIBLINGS of property_declaration
                    let end_byte = self.find_property_end_byte(node);

                    // Use the property_declaration node bounds for the declaration ID,
                    // extended to include any getter/setter.
                    let location = point_to_location(
                        path,
                        node.start_position(),
                        node.end_position(),
                        node.start_byte(),
                        end_byte,
                    );

                    let id = DeclarationId::new(path.to_path_buf(), node.start_byte(), end_byte);

                    let mut decl = Declaration::new(
                        id,
                        name.clone(),
                        DeclarationKind::Property,
                        location.clone(),
                        Language::Kotlin,
                    );

                    if parent.is_none() {
                        decl.fully_qualified_name = Some(self.build_fqn(package, &name));
                    }

                    self.extract_modifiers(node, source, &mut decl);
                    decl.annotations = self.extract_annotations(node, source);
                    decl.parent = parent.clone();

                    // Check for val/var keyword - in tree-sitter-kotlin grammar,
                    // val/var is inside binding_pattern_kind which is a child of property_declaration
                    let mut val_var_cursor = node.walk();
                    for child in node.children(&mut val_var_cursor) {
                        if child.kind() == "binding_pattern_kind" {
                            let mut inner_cursor = child.walk();
                            for inner_child in child.children(&mut inner_cursor) {
                                match inner_child.kind() {
                                    "val" => {
                                        decl.modifiers.push("val".to_string());
                                    }
                                    "var" => {
                                        decl.modifiers.push("var".to_string());
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }

                    // Extract property type (e.g., "val name: String" -> "String")
                    decl.type_name = self.extract_property_type(node, source);

                    // Check for property delegation (by lazy, by Delegates, etc.)
                    if let Some(delegate_type) = self.extract_property_delegate(node, source) {
                        // Add delegation reference
                        result.references.push(UnresolvedReference {
                            name: delegate_type,
                            qualified_name: None,
                            kind: ReferenceKind::Delegation,
                            location: location.clone(),
                            imports: result.imports.clone(),
                        });
                        // Mark property as delegated
                        decl.modifiers.push("delegated".to_string());
                    }

                    // Check for private setter (var with private set)
                    if self.has_private_setter(node, source) {
                        decl.modifiers.push("private_set".to_string());
                    }

                    result.declarations.push(decl);
                }
            }
        }

        Ok(())
    }

    /// Extract delegation type from a property (e.g., "lazy" from "by lazy { }")
    fn extract_property_delegate(&self, node: Node, source: &str) -> Option<String> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "property_delegate" {
                // Look for the delegate expression
                let mut delegate_cursor = child.walk();
                for delegate_child in child.children(&mut delegate_cursor) {
                    match delegate_child.kind() {
                        "call_expression" => {
                            // e.g., `by lazy { }`, `by Delegates.observable(...)`
                            if let Some(callee) = self.extract_callee_name(delegate_child, source) {
                                return Some(callee);
                            }
                        }
                        "simple_identifier" => {
                            // e.g., `by myDelegate`
                            return Some(node_text(delegate_child, source).to_string());
                        }
                        "navigation_expression" => {
                            // e.g., `by Delegates.observable`
                            let text = node_text(delegate_child, source);
                            // Get the first component (e.g., "Delegates" from "Delegates.observable")
                            if let Some(first) = text.split('.').next() {
                                return Some(first.to_string());
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        None
    }

    /// Extract the callee name from a call expression
    fn extract_callee_name(&self, node: Node, source: &str) -> Option<String> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "simple_identifier" => {
                    return Some(node_text(child, source).to_string());
                }
                "navigation_expression" => {
                    let text = node_text(child, source);
                    if let Some(first) = text.split('.').next() {
                        return Some(first.to_string());
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// Extract the type of a property declaration (e.g., "val name: String" -> "String")
    fn extract_property_type(&self, node: Node, source: &str) -> Option<String> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                // Direct type reference: `val name: String`
                "user_type" | "type_reference" => {
                    return Some(node_text(child, source).to_string());
                }
                // Nullable type: `val name: String?`
                "nullable_type" => {
                    let type_text = node_text(child, source);
                    // Include the ? for nullable types
                    return Some(type_text.to_string());
                }
                // Function type: `val callback: () -> Unit`
                "function_type" => {
                    return Some(node_text(child, source).to_string());
                }
                _ => {}
            }
        }
        None
    }

    /// Extract generic type arguments from a type (e.g., List<MyClass, OtherClass>)
    fn extract_generic_type_arguments(
        node: Node,
        source: &str,
        path: &Path,
        imports: &[String],
        result: &mut ParseResult,
    ) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "type_arguments" {
                // Iterate through type_argument children
                let mut arg_cursor = child.walk();
                for arg_child in child.children(&mut arg_cursor) {
                    if arg_child.kind() == "type_projection" || arg_child.kind() == "user_type" {
                        // Extract the type name
                        let type_text = node_text(arg_child, source);
                        // Strip variance annotations (in, out) and generic arguments
                        let cleaned = type_text
                            .trim_start_matches("in ")
                            .trim_start_matches("out ")
                            .split('<')
                            .next()
                            .unwrap_or(type_text);

                        // Skip wildcards and primitive types
                        if cleaned == "*" || cleaned.is_empty() {
                            continue;
                        }

                        // Skip common built-in types
                        let builtins = [
                            "String", "Int", "Long", "Boolean", "Float", "Double", "Unit", "Any",
                            "Nothing",
                        ];
                        if builtins.contains(&cleaned) {
                            continue;
                        }

                        let location = point_to_location(
                            path,
                            arg_child.start_position(),
                            arg_child.end_position(),
                            arg_child.start_byte(),
                            arg_child.end_byte(),
                        );

                        result.references.push(UnresolvedReference {
                            name: cleaned.to_string(),
                            qualified_name: None,
                            kind: ReferenceKind::GenericArgument,
                            location,
                            imports: imports.to_vec(),
                        });

                        // Recursively extract nested generics (e.g., Map<String, List<MyClass>>)
                        Self::extract_generic_type_arguments(
                            arg_child, source, path, imports, result,
                        );
                    }
                }
            }
        }
    }

    /// Check if a property has a private setter sibling.
    /// In Kotlin, `var x: String = "" private set` makes the getter public but setter private.
    fn has_private_setter(&self, node: Node, source: &str) -> bool {
        // Check following siblings for a setter with private visibility
        let mut next = node.next_sibling();
        while let Some(sibling) = next {
            match sibling.kind() {
                "setter" => {
                    // Check if the setter has a private visibility modifier
                    let mut cursor = sibling.walk();
                    for child in sibling.children(&mut cursor) {
                        if child.kind() == "visibility_modifier" {
                            let visibility_text = node_text(child, source);
                            if visibility_text == "private" {
                                return true;
                            }
                        }
                    }
                    // Only check the first setter we find
                    return false;
                }
                "getter" => {
                    // Continue looking, there might be a setter after the getter
                    next = sibling.next_sibling();
                }
                _ => break,
            }
        }
        false
    }

    /// Find the end byte of a property declaration, including any getter/setter siblings.
    /// In Kotlin's tree-sitter grammar, getter/setter nodes are siblings of property_declaration,
    /// not children. We need to extend the property's byte range to include them.
    fn find_property_end_byte(&self, node: Node) -> usize {
        let mut end_byte = node.end_byte();

        // Check following siblings for getter/setter
        let mut next = node.next_sibling();
        while let Some(sibling) = next {
            match sibling.kind() {
                "getter" | "setter" => {
                    // Extend the byte range to include this getter/setter
                    end_byte = sibling.end_byte();
                    next = sibling.next_sibling();
                }
                _ => break,
            }
        }

        end_byte
    }

    fn extract_constructor(
        &self,
        path: &Path,
        node: Node,
        source: &str,
        parent: DeclarationId,
        result: &mut ParseResult,
    ) -> Result<()> {
        let location = point_to_location(
            path,
            node.start_position(),
            node.end_position(),
            node.start_byte(),
            node.end_byte(),
        );

        let id = DeclarationId::new(path.to_path_buf(), node.start_byte(), node.end_byte());

        let mut decl = Declaration::new(
            id.clone(),
            "constructor".to_string(),
            DeclarationKind::Constructor,
            location,
            Language::Kotlin,
        );

        self.extract_modifiers(node, source, &mut decl);
        decl.parent = Some(parent);

        // Extract parameters
        if let Some(params) = node.child_by_field_name("class_parameters") {
            self.extract_parameters(path, params, source, id, result)?;
        }

        result.declarations.push(decl);

        Ok(())
    }

    fn extract_parameters(
        &self,
        path: &Path,
        node: Node,
        source: &str,
        parent: DeclarationId,
        result: &mut ParseResult,
    ) -> Result<()> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "parameter" || child.kind() == "class_parameter" {
                if let Some(name_node) = child.child_by_field_name("simple_identifier") {
                    let name = node_text(name_node, source).to_string();
                    let location = point_to_location(
                        path,
                        child.start_position(),
                        child.end_position(),
                        child.start_byte(),
                        child.end_byte(),
                    );

                    let id = DeclarationId::new(
                        path.to_path_buf(),
                        child.start_byte(),
                        child.end_byte(),
                    );

                    let mut decl = Declaration::new(
                        id,
                        name,
                        DeclarationKind::Parameter,
                        location,
                        Language::Kotlin,
                    );

                    decl.parent = Some(parent.clone());

                    result.declarations.push(decl);
                }
            }
        }

        Ok(())
    }

    fn extract_companion_object(
        &self,
        path: &Path,
        node: Node,
        source: &str,
        package: &Option<String>,
        parent: DeclarationId,
        result: &mut ParseResult,
    ) -> Result<()> {
        let location = point_to_location(
            path,
            node.start_position(),
            node.end_position(),
            node.start_byte(),
            node.end_byte(),
        );

        let id = DeclarationId::new(path.to_path_buf(), node.start_byte(), node.end_byte());

        // Companion objects may have a name, otherwise use "Companion"
        let name = self.get_companion_name(node, source);

        let mut decl = Declaration::new(
            id.clone(),
            name,
            DeclarationKind::Object,
            location,
            Language::Kotlin,
        );

        // Mark as companion object via modifiers
        decl.modifiers.push("companion".to_string());
        decl.parent = Some(parent);

        result.declarations.push(decl);

        // Extract companion object body members
        // Find class_body by kind since tree-sitter-kotlin doesn't use field names
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "class_body" {
                self.extract_class_members(path, child, source, package, id, result)?;
                break;
            }
        }

        Ok(())
    }

    /// Get the name of a companion object (may be named or default "Companion")
    fn get_companion_name(&self, node: Node, source: &str) -> String {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "simple_identifier" || child.kind() == "type_identifier" {
                return node_text(child, source).to_string();
            }
        }
        "Companion".to_string()
    }

    fn extract_enum_entry(
        &self,
        path: &Path,
        node: Node,
        source: &str,
        parent: DeclarationId,
        result: &mut ParseResult,
    ) -> Result<()> {
        // "simple_identifier" is the child's kind, not a grammar field name
        let mut cursor = node.walk();
        let name_node = node
            .children(&mut cursor)
            .find(|c| c.kind() == "simple_identifier");
        if let Some(name_node) = name_node {
            let name = node_text(name_node, source).to_string();
            let location = point_to_location(
                path,
                node.start_position(),
                node.end_position(),
                node.start_byte(),
                node.end_byte(),
            );

            let id = DeclarationId::new(path.to_path_buf(), node.start_byte(), node.end_byte());

            let mut decl = Declaration::new(
                id,
                name,
                DeclarationKind::EnumCase,
                location,
                Language::Kotlin,
            );

            decl.parent = Some(parent);
            // Les annotations d'entry (@SerializedName…) vivent dans le
            // child `modifiers` — sans elles, le détecteur d'enum cases
            // condamne des cas instanciés par désérialisation
            self.extract_modifiers(node, source, &mut decl);
            decl.annotations = self.extract_annotations(node, source);

            result.declarations.push(decl);
        }

        Ok(())
    }

    fn extract_type_alias(
        &self,
        path: &Path,
        node: Node,
        source: &str,
        package: &Option<String>,
        result: &mut ParseResult,
    ) -> Result<()> {
        if let Some(name_node) = node.child_by_field_name("simple_identifier") {
            let name = node_text(name_node, source).to_string();
            let location = point_to_location(
                path,
                node.start_position(),
                node.end_position(),
                node.start_byte(),
                node.end_byte(),
            );

            let id = DeclarationId::new(path.to_path_buf(), node.start_byte(), node.end_byte());

            let mut decl = Declaration::new(
                id,
                name.clone(),
                DeclarationKind::TypeAlias,
                location,
                Language::Kotlin,
            );

            decl.fully_qualified_name = Some(self.build_fqn(package, &name));
            self.extract_modifiers(node, source, &mut decl);

            result.declarations.push(decl);
        }

        Ok(())
    }

    fn extract_references(
        &self,
        path: &Path,
        node: Node,
        source: &str,
        imports: &[String],
        result: &mut ParseResult,
    ) -> Result<()> {
        // Create implicit references for parent classes in enum constant imports
        // e.g., "import com.example.MyEnum.CONSTANT" creates a reference to "MyEnum"
        self.extract_enum_parent_references(path, imports, result);

        let mut cursor = node.walk();

        // Walk through all nodes looking for identifiers
        loop {
            let current = cursor.node();

            match current.kind() {
                "simple_identifier" => {
                    // Determine reference kind based on parent context
                    if let Some(parent) = current.parent() {
                        // Skip parameter names in named arguments (left side of =)
                        // e.g., in "primary = primaryLight", skip "primary" but keep "primaryLight"
                        if parent.kind() == "value_argument" {
                            let is_param_name = self.is_named_argument_param_name(parent, current);
                            if is_param_name {
                                // This is the parameter name, not a value reference
                                // Continue to next node
                                if cursor.goto_first_child() {
                                    continue;
                                }
                                while !cursor.goto_next_sibling() {
                                    if !cursor.goto_parent() {
                                        return Ok(());
                                    }
                                }
                                continue;
                            }
                        }

                        // Special handling for infix expressions: "a until b"
                        // The middle element (index 1) is the infix function name -> Call
                        // The operands (indices 0 and 2) are values -> Read
                        let kind = if parent.kind() == "infix_expression" {
                            if self.is_infix_function_name(parent, current) {
                                Some(ReferenceKind::Call)
                            } else {
                                Some(ReferenceKind::Read)
                            }
                        } else {
                            self.determine_reference_kind(parent)
                        };

                        if let Some(kind) = kind {
                            let name = node_text(current, source).to_string();
                            let location = point_to_location(
                                path,
                                current.start_position(),
                                current.end_position(),
                                current.start_byte(),
                                current.end_byte(),
                            );

                            // Kotlin sees a Java class's `getX()`/`setX()` as
                            // the synthetic property `x`, so `button.count` IS
                            // a call to `getCount()`. The bridge belongs here,
                            // where the syntax says this is an access through a
                            // receiver: at resolution time a bare `count` local
                            // is indistinguishable from it, and bridging there
                            // resurrected every Java getter of that name.
                            if parent.kind() == "navigation_suffix" {
                                for accessor in
                                    crate::graph::java_accessors_behind_property(&name, kind)
                                {
                                    result.references.push(UnresolvedReference {
                                        name: accessor,
                                        qualified_name: None,
                                        kind: ReferenceKind::Call,
                                        location: location.clone(),
                                        imports: imports.to_vec(),
                                    });
                                }
                            }

                            result.references.push(UnresolvedReference {
                                name,
                                qualified_name: None,
                                kind,
                                location,
                                imports: imports.to_vec(),
                            });
                        }
                    }
                }
                // "$name" dans un template de chaîne : la grammaire aliase
                // l'identifiant en interpolated_identifier, il n'apparaît
                // jamais comme simple_identifier — sans ce bras, la lecture
                // est invisible et la propriété sort en assign-only.
                // "${x}" nu : même alias, vers interpolated_expression (leaf).
                "interpolated_identifier" => {
                    let name = node_text(current, source).to_string();
                    let location = point_to_location(
                        path,
                        current.start_position(),
                        current.end_position(),
                        current.start_byte(),
                        current.end_byte(),
                    );
                    result.references.push(UnresolvedReference {
                        name,
                        qualified_name: None,
                        kind: ReferenceKind::Read,
                        location,
                        imports: imports.to_vec(),
                    });
                }
                "interpolated_expression" if current.child_count() == 0 => {
                    let name = node_text(current, source).to_string();
                    let location = point_to_location(
                        path,
                        current.start_position(),
                        current.end_position(),
                        current.start_byte(),
                        current.end_byte(),
                    );
                    result.references.push(UnresolvedReference {
                        name,
                        qualified_name: None,
                        kind: ReferenceKind::Read,
                        location,
                        imports: imports.to_vec(),
                    });
                }
                "user_type" => {
                    // Extract just the base type name, stripping generic arguments
                    let full_name = node_text(current, source).to_string();
                    // Strip generic arguments: "Focusable<FeedState>" -> "Focusable"
                    let name = full_name
                        .split('<')
                        .next()
                        .unwrap_or(&full_name)
                        .to_string();

                    let location = point_to_location(
                        path,
                        current.start_position(),
                        current.end_position(),
                        current.start_byte(),
                        current.end_byte(),
                    );

                    result.references.push(UnresolvedReference {
                        name: name.clone(),
                        qualified_name: None,
                        kind: ReferenceKind::Type,
                        location: location.clone(),
                        imports: imports.to_vec(),
                    });

                    // A nested type is usually written through its parent, since
                    // that is what the compiler accepts without an import:
                    // `is Action.Toggled ->`, `Outer.Inner()`. Resolution matches
                    // on the declared name, which is the last segment, so the
                    // qualified form alone never binds and the declaration reads
                    // as unreferenced.
                    //
                    // Emitting the tail as well costs a reference that resolves to
                    // nothing when the type is external (kotlin.collections.List),
                    // and rescues the nested case when it is ours.
                    if let Some((_, tail)) = name.rsplit_once('.') {
                        if !tail.is_empty() {
                            result.references.push(UnresolvedReference {
                                name: tail.to_string(),
                                qualified_name: Some(name.clone()),
                                kind: ReferenceKind::Type,
                                location: location.clone(),
                                imports: imports.to_vec(),
                            });
                        }
                    }

                    // Extract generic type arguments (e.g., FeedState from List<FeedState>)
                    Self::extract_generic_type_arguments(current, source, path, imports, result);
                }
                // Handle type_arguments directly for better coverage
                "type_arguments" => {
                    Self::extract_generic_type_arguments(current, source, path, imports, result);
                }
                // Handle callable references like SomeClass::class or viewModel::method
                // Used in @PreviewParameter(SomeClass::class), method references, etc.
                "callable_reference" => {
                    // Check if this is a ::class reference (reflection)
                    let is_class_literal = self.is_class_literal(current, source);

                    // Extract the type reference from the left side of ::
                    if let Some(type_ref) = self.extract_callable_reference_type(current, source) {
                        let location = point_to_location(
                            path,
                            current.start_position(),
                            current.end_position(),
                            current.start_byte(),
                            current.end_byte(),
                        );

                        // Use Reflection kind for ::class references (more important for dead code detection)
                        let ref_kind = if is_class_literal {
                            ReferenceKind::Reflection
                        } else {
                            ReferenceKind::Type
                        };

                        result.references.push(UnresolvedReference {
                            name: type_ref,
                            qualified_name: None,
                            kind: ref_kind,
                            location,
                            imports: imports.to_vec(),
                        });
                    }

                    // Also extract the method name from the right side of ::
                    // For patterns like viewModel::gameArchiveProgressChanged
                    if !is_class_literal {
                        let mut ref_cursor = current.walk();
                        for child in current.children(&mut ref_cursor) {
                            if child.kind() == "simple_identifier" {
                                let method_name = node_text(child, source).to_string();
                                // Skip "class" which is a keyword, not a method reference
                                if method_name != "class" {
                                    let location = point_to_location(
                                        path,
                                        child.start_position(),
                                        child.end_position(),
                                        child.start_byte(),
                                        child.end_byte(),
                                    );

                                    result.references.push(UnresolvedReference {
                                        name: method_name,
                                        qualified_name: None,
                                        kind: ReferenceKind::Call,
                                        location,
                                        imports: imports.to_vec(),
                                    });
                                }
                            }
                        }
                    }
                }
                // Workaround for tree-sitter-kotlin grammar bug:
                // When a when condition contains `!isXxx()`, the grammar incorrectly
                // parses `!is` as the "is not" type-check operator, resulting in:
                // - type_test containing !is
                // - ERROR node with the identifier (e.g., "Enabled" from "isEnabled")
                // - function_type for the `() -> result` part
                // We detect this pattern and extract the intended function call.
                "type_test" => {
                    let mut has_not_is = false;
                    let mut error_identifier: Option<String> = None;
                    let mut error_location: Option<(usize, usize, usize, usize)> = None;
                    let mut has_function_type = false;

                    let mut test_cursor = current.walk();
                    for child in current.children(&mut test_cursor) {
                        match child.kind() {
                            "!is" => {
                                has_not_is = true;
                            }
                            "ERROR" => {
                                // Extract identifier from ERROR node
                                let mut err_cursor = child.walk();
                                for err_child in child.children(&mut err_cursor) {
                                    if err_child.kind() == "simple_identifier" {
                                        error_identifier =
                                            Some(node_text(err_child, source).to_string());
                                        error_location = Some((
                                            err_child.start_position().row,
                                            err_child.start_position().column,
                                            err_child.start_byte(),
                                            err_child.end_byte(),
                                        ));
                                        break;
                                    }
                                }
                            }
                            "function_type" => {
                                has_function_type = true;
                            }
                            _ => {}
                        }
                    }

                    // If we detected the bug pattern, extract the function call
                    if let (true, true, Some(ident)) =
                        (has_not_is, has_function_type, error_identifier)
                    {
                        // Reconstruct the function name: "is" + error_identifier
                        let func_name = format!("is{}", ident);
                        let (row, col, start, end) = error_location.unwrap();

                        let location = point_to_location(
                            path,
                            tree_sitter::Point {
                                row,
                                column: col.saturating_sub(2),
                            }, // Adjust for "is" prefix
                            tree_sitter::Point {
                                row,
                                column: col + ident.len(),
                            },
                            start.saturating_sub(2),
                            end,
                        );

                        result.references.push(UnresolvedReference {
                            name: func_name.clone(),
                            qualified_name: None,
                            kind: ReferenceKind::Call,
                            location,
                            imports: imports.to_vec(),
                        });
                    }

                    // Also scan the entire type_test text for additional misparsed function calls
                    // Since the parse error can cascade and absorb multiple when entries
                    let type_test_text = node_text(current, source);
                    for cap in MISPARSED_CALL_PATTERN.captures_iter(type_test_text) {
                        if let Some(m) = cap.get(1) {
                            let func_name = m.as_str().to_string();
                            // Skip keywords and already-handled isXxx patterns
                            if func_name != "if"
                                && func_name != "when"
                                && func_name != "for"
                                && !func_name.starts_with("is")
                            {
                                let offset = current.start_byte() + m.start();
                                let end = current.start_byte() + m.end();

                                let location = point_to_location(
                                    path,
                                    current.start_position(),
                                    current.start_position(),
                                    offset,
                                    end,
                                );

                                result.references.push(UnresolvedReference {
                                    name: func_name,
                                    qualified_name: None,
                                    kind: ReferenceKind::Call,
                                    location,
                                    imports: imports.to_vec(),
                                });
                            }
                        }
                    }
                }
                // Also scan when_entry nodes that might have absorbed misparsed content
                "when_entry" => {
                    // Check if this when_entry has parse errors by looking for unusual content
                    // (when entries with errors often contain multiple "-> " patterns)
                    let entry_text = node_text(current, source);
                    if entry_text.matches("->").count() > 1 {
                        // This entry likely contains absorbed misparsed entries
                        for cap in MISPARSED_CALL_PATTERN.captures_iter(entry_text) {
                            if let Some(m) = cap.get(1) {
                                let func_name = m.as_str().to_string();
                                // Skip keywords
                                if func_name != "if" && func_name != "when" && func_name != "for" {
                                    let offset = current.start_byte() + m.start();
                                    let end = current.start_byte() + m.end();

                                    let location = point_to_location(
                                        path,
                                        current.start_position(),
                                        current.start_position(),
                                        offset,
                                        end,
                                    );

                                    result.references.push(UnresolvedReference {
                                        name: func_name,
                                        qualified_name: None,
                                        kind: ReferenceKind::Call,
                                        location,
                                        imports: imports.to_vec(),
                                    });
                                }
                            }
                        }
                    }
                }
                // Workaround for tree-sitter-kotlin grammar bug (continued):
                // After `!isXxx()` parse errors, the when conditions get misparsed.
                // Look for ERROR nodes that contain identifiers followed by () in source.
                "ERROR" => {
                    // Check if this ERROR node is inside a when expression context
                    // by looking for function-call-like patterns in the source text
                    let error_text = node_text(current, source);

                    // Look for patterns like "identifier()" in the error text
                    // These are likely misparsed function calls
                    for cap in MISPARSED_CALL_PATTERN.captures_iter(error_text) {
                        if let Some(m) = cap.get(1) {
                            let func_name = m.as_str().to_string();
                            // Skip common keywords
                            if func_name != "if" && func_name != "when" && func_name != "for" {
                                let offset = current.start_byte() + m.start();
                                let end = current.start_byte() + m.end();

                                let location = point_to_location(
                                    path,
                                    current.start_position(),
                                    current.start_position(),
                                    offset,
                                    end,
                                );

                                result.references.push(UnresolvedReference {
                                    name: func_name,
                                    qualified_name: None,
                                    kind: ReferenceKind::Call,
                                    location,
                                    imports: imports.to_vec(),
                                });
                            }
                        }
                    }
                }
                _ => {}
            }

            // Move to next node
            if cursor.goto_first_child() {
                continue;
            }
            while !cursor.goto_next_sibling() {
                if !cursor.goto_parent() {
                    return Ok(());
                }
            }
        }
    }

    // Helper methods

    /// Extract references to parent classes from enum constant imports
    /// For imports like "import com.example.MyEnum.CONSTANT", this creates
    /// a reference to "MyEnum" so the enum class isn't marked as dead code.
    fn extract_enum_parent_references(
        &self,
        path: &Path,
        imports: &[String],
        result: &mut ParseResult,
    ) {
        for import in imports {
            // Split import path: "com.example.MyEnum.CONSTANT" -> ["com", "example", "MyEnum", "CONSTANT"]
            let parts: Vec<&str> = import.split('.').collect();

            // We need at least 2 parts for a potential enum constant import
            if parts.len() >= 2 {
                let last = parts[parts.len() - 1];
                let second_last = parts[parts.len() - 2];

                // Check if this looks like an enum constant import:
                // - Last segment should be ALL_CAPS or PascalCase (enum constant)
                // - Second-to-last should be PascalCase (class name)
                let last_is_constant = last
                    .chars()
                    .next()
                    .map(|c| c.is_uppercase())
                    .unwrap_or(false);
                let second_last_is_class = second_last
                    .chars()
                    .next()
                    .map(|c| c.is_uppercase())
                    .unwrap_or(false);

                if last_is_constant && second_last_is_class {
                    // Create a synthetic reference to the parent class
                    // Use a zero-position location since this is an implicit reference
                    let location = point_to_location(
                        path,
                        tree_sitter::Point { row: 0, column: 0 },
                        tree_sitter::Point { row: 0, column: 0 },
                        0,
                        0,
                    );

                    result.references.push(UnresolvedReference {
                        name: second_last.to_string(),
                        qualified_name: None,
                        kind: ReferenceKind::Type,
                        location,
                        imports: imports.to_vec(),
                    });
                }
            }
        }
    }

    fn get_type_name(&self, node: Node, source: &str) -> Result<String> {
        // Try common field names first
        if let Some(name_node) = node.child_by_field_name("name") {
            return Ok(node_text(name_node, source).to_string());
        }
        if let Some(name_node) = node.child_by_field_name("simple_identifier") {
            return Ok(node_text(name_node, source).to_string());
        }
        if let Some(name_node) = node.child_by_field_name("type_identifier") {
            return Ok(node_text(name_node, source).to_string());
        }

        // Search for identifier nodes in children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "simple_identifier" | "type_identifier" | "identifier" => {
                    return Ok(node_text(child, source).to_string());
                }
                _ => {}
            }
        }

        // Last resort: try to extract name from node text (first word after keywords)
        let text = node_text(node, source);
        for keyword in ["class", "interface", "object", "enum"] {
            if let Some(pos) = text.find(keyword) {
                let after_keyword = &text[pos + keyword.len()..].trim_start();
                if let Some(name) = after_keyword
                    .split(|c: char| !c.is_alphanumeric() && c != '_')
                    .next()
                {
                    if !name.is_empty() {
                        return Ok(name.to_string());
                    }
                }
            }
        }

        Err(miette::miette!(
            "Could not find type name in node: {}",
            node.kind()
        ))
    }

    /// Extract function name, handling both regular and extension functions
    /// For regular: `fun name(...)` -> name is simple_identifier
    /// For extension: `fun Type.name(...)` -> name is simple_identifier AFTER receiver_type
    fn extract_function_name(&self, node: Node, source: &str) -> String {
        // Try direct field names first
        if let Some(name_node) = node.child_by_field_name("name") {
            return node_text(name_node, source).to_string();
        }

        // For extension functions, we need to find the simple_identifier AFTER the receiver
        let mut cursor = node.walk();
        let mut found_fun = false;

        for child in node.children(&mut cursor) {
            let kind = child.kind();

            // Track when we see 'fun' keyword
            if kind == "fun" {
                found_fun = true;
                continue;
            }

            // Skip receiver types and dots
            if kind == "receiver_type" || kind == "user_type" || kind == "type_reference" {
                continue;
            }

            // Skip the dot after receiver
            if kind == "." {
                continue;
            }

            // If we've seen 'fun' (and optionally a receiver), the next simple_identifier is the name
            if found_fun && kind == "simple_identifier" {
                return node_text(child, source).to_string();
            }
        }

        // Fallback: look for any simple_identifier child that looks like a function name
        // (not a type name - those usually start with uppercase)
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "simple_identifier" {
                let text = node_text(child, source);
                // Return first identifier that starts with lowercase (likely function name)
                // or any identifier if we haven't found one yet
                if text
                    .chars()
                    .next()
                    .map(|c| c.is_lowercase())
                    .unwrap_or(false)
                {
                    return text.to_string();
                }
            }
        }

        // Last resort: return <anonymous>
        "<anonymous>".to_string()
    }

    fn determine_class_kind(&self, node: Node, source: &str) -> DeclarationKind {
        // `enum class` / `interface` keywords are direct child tokens of
        // class_declaration, not part of a modifiers node
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "enum" => return DeclarationKind::Enum,
                "interface" => return DeclarationKind::Interface,
                _ => {}
            }
        }
        for child in node.children(&mut cursor) {
            if child.kind() == "modifiers" {
                let modifiers_text = node_text(child, source);
                if modifiers_text.contains("interface") {
                    return DeclarationKind::Interface;
                }
                if modifiers_text.contains("enum") {
                    return DeclarationKind::Enum;
                }
                if modifiers_text.contains("annotation") {
                    return DeclarationKind::Annotation;
                }
            }
        }
        DeclarationKind::Class
    }

    fn extract_modifiers(&self, node: Node, source: &str, decl: &mut Declaration) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "modifiers" {
                self.extract_modifiers_from_node(child, source, decl);
            }
        }
    }

    fn extract_modifiers_from_node(&self, node: Node, source: &str, decl: &mut Declaration) {
        let mut mod_cursor = node.walk();
        for modifier in node.children(&mut mod_cursor) {
            let kind = modifier.kind();
            let text = node_text(modifier, source).trim();

            // Handle specific modifier types
            match kind {
                "visibility_modifier"
                | "inheritance_modifier"
                | "member_modifier"
                | "class_modifier"
                | "function_modifier"
                | "property_modifier"
                | "parameter_modifier"
                | "type_parameter_modifier" => {
                    // Extract the actual modifier keyword
                    let mut inner_cursor = modifier.walk();
                    for inner_child in modifier.children(&mut inner_cursor) {
                        let inner_text = node_text(inner_child, source).trim();
                        if !inner_text.is_empty() {
                            decl.modifiers.push(inner_text.to_string());
                            self.apply_modifier(inner_text, decl);
                        }
                    }
                    // Also add the text itself if no children
                    if modifier.child_count() == 0 && !text.is_empty() {
                        decl.modifiers.push(text.to_string());
                        self.apply_modifier(text, decl);
                    }
                }
                "annotation" => {
                    // Skip annotations, handled separately
                }
                _ => {
                    // For simple modifiers, add the text directly
                    if !text.is_empty() && !text.starts_with('@') {
                        decl.modifiers.push(text.to_string());
                        self.apply_modifier(text, decl);
                    }
                }
            }
        }
    }

    fn apply_modifier(&self, text: &str, decl: &mut Declaration) {
        match text {
            "public" => decl.visibility = Visibility::Public,
            "private" => decl.visibility = Visibility::Private,
            "protected" => decl.visibility = Visibility::Protected,
            "internal" => decl.visibility = Visibility::Internal,
            "abstract" => decl.is_abstract = true,
            _ => {}
        }
    }

    fn extract_super_types(&self, node: Node, source: &str) -> Vec<String> {
        let mut super_types = Vec::new();

        // Method 1: Try with field name (works for some class declarations)
        if let Some(delegation) = node.child_by_field_name("delegation_specifiers") {
            let mut cursor = delegation.walk();
            for child in delegation.children(&mut cursor) {
                if child.kind() == "delegation_specifier" {
                    // Get full text and strip "by ..." delegation part
                    let text = node_text(child, source);
                    // Take only the type part before "by"
                    let type_part = text.split(" by ").next().unwrap_or(text);
                    super_types.push(type_part.to_string());
                }
            }
        }

        // Method 2: Direct child lookup (works for objects and nested classes)
        // tree-sitter-kotlin doesn't always use field names
        if super_types.is_empty() {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "delegation_specifier" {
                    // Get full text and strip "by ..." delegation part
                    let text = node_text(child, source);
                    // Take only the type part before "by"
                    let type_part = text.split(" by ").next().unwrap_or(text);
                    super_types.push(type_part.to_string());
                }
            }
        }

        super_types
    }

    /// Extract class delegation references (e.g., "delegate" from "class Foo : Bar by delegate")
    fn extract_class_delegates(
        &self,
        node: Node,
        source: &str,
        path: &Path,
        imports: &[String],
        result: &mut ParseResult,
    ) {
        if let Some(delegation) = node.child_by_field_name("delegation_specifiers") {
            let mut cursor = delegation.walk();
            for child in delegation.children(&mut cursor) {
                if child.kind() == "delegation_specifier" {
                    let text = node_text(child, source);
                    // Check if this has "by" delegation
                    if let Some(by_pos) = text.find(" by ") {
                        let delegate_expr = &text[by_pos + 4..].trim();
                        // Extract the delegate identifier (first word)
                        if let Some(delegate_name) = delegate_expr
                            .split(|c: char| !c.is_alphanumeric() && c != '_')
                            .next()
                        {
                            if !delegate_name.is_empty() {
                                let location = point_to_location(
                                    path,
                                    child.start_position(),
                                    child.end_position(),
                                    child.start_byte(),
                                    child.end_byte(),
                                );

                                result.references.push(UnresolvedReference {
                                    name: delegate_name.to_string(),
                                    qualified_name: None,
                                    kind: ReferenceKind::Delegation,
                                    location,
                                    imports: imports.to_vec(),
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    fn extract_annotations(&self, node: Node, source: &str) -> Vec<String> {
        let mut annotations = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.kind() == "modifiers" {
                let mut mod_cursor = child.walk();
                for modifier in child.children(&mut mod_cursor) {
                    if modifier.kind() == "annotation" {
                        annotations.push(node_text(modifier, source).to_string());
                    }
                }
            }
        }

        // Also check for annotations in preceding prefix_expression siblings
        // (tree-sitter-kotlin sometimes places annotations there instead of in modifiers)
        if let Some(prev) = node.prev_sibling() {
            if prev.kind() == "prefix_expression" {
                let mut prefix_cursor = prev.walk();
                for child in prev.children(&mut prefix_cursor) {
                    if child.kind() == "annotation" {
                        annotations.push(node_text(child, source).to_string());
                    }
                }
            }
        }

        annotations
    }

    fn determine_reference_kind(&self, parent: Node) -> Option<ReferenceKind> {
        match parent.kind() {
            "call_expression" => Some(ReferenceKind::Call),
            // navigation_expression and navigation_suffix can be property access OR method calls
            // - For navigation_suffix: check if its parent navigation_expression is being called
            // - For navigation_expression (identifier as direct child): always Read (it's the receiver)
            // Examples:
            // - this.property → Read (no call)
            // - this.method() → method is in navigation_suffix, parent has call_suffix → Call
            // - DEFAULT_HEIGHT.dpToPx() → DEFAULT_HEIGHT is direct child → Read, dpToPx is Call
            "navigation_suffix" => {
                if self.is_navigation_method_call(parent) {
                    Some(ReferenceKind::Call)
                } else {
                    // Check if this navigation_suffix is part of an assignment target
                    // e.g., in `obj.prop = value`, `prop` in the navigation_suffix is being written to
                    if let Some(grandparent) = parent.parent() {
                        if grandparent.kind() == "directly_assignable_expression" {
                            return Some(ReferenceKind::Write);
                        }
                    }
                    Some(ReferenceKind::Read)
                }
            }
            "navigation_expression" => {
                // Direct child of navigation_expression (e.g., DEFAULT_HEIGHT in DEFAULT_HEIGHT.method())
                // This is always the receiver, so it's a Read
                Some(ReferenceKind::Read)
            }
            // For assignment, check if this identifier is the target (left side) or value (right side)
            // The left side is wrapped in directly_assignable_expression
            // The right side is directly under assignment → should be Read
            "assignment" | "augmented_assignment" => Some(ReferenceKind::Read),
            // directly_assignable_expression is the parent for left side of assignments
            // But only if this identifier has NO navigation_suffix sibling
            // e.g., `myProp = true` → myProp is Write
            // e.g., `obj.prop = true` → obj is Read (receiver), prop is Write (in navigation_suffix)
            "directly_assignable_expression" => {
                // Check if there's a navigation_suffix sibling - if so, this identifier is the receiver (Read)
                let mut cursor = parent.walk();
                for child in parent.children(&mut cursor) {
                    if child.kind() == "navigation_suffix" {
                        return Some(ReferenceKind::Read);
                    }
                }
                Some(ReferenceKind::Write)
            }
            "user_type" | "type_reference" => Some(ReferenceKind::Type),
            // Inheritance - when a class extends another
            "delegation_specifier" | "delegation_specifiers" => Some(ReferenceKind::Inheritance),
            "constructor_invocation" => Some(ReferenceKind::Instantiation),
            "annotation" => Some(ReferenceKind::Annotation),
            // Value expressions - identifiers used as values (function arguments, return values, etc.)
            "value_argument" | "value_arguments" => Some(ReferenceKind::Read),
            // Property/variable access
            "property_declaration" | "variable_declaration" => Some(ReferenceKind::Read),
            // Default parameter values: fun test(x: Int = MY_CONST)
            // The default value is a sibling of parameter node, parented by function_value_parameters
            "parameter" | "class_parameter" | "function_value_parameters" => {
                Some(ReferenceKind::Read)
            }
            // Return statements and expression bodies
            "jump_expression" | "function_body" => Some(ReferenceKind::Read),
            // Binary/unary expressions (comparisons, arithmetic, infix, etc.)
            // Note: tree-sitter-kotlin uses _expression suffix for these
            "comparison_expression"
            | "equality_expression"
            | "additive_expression"
            | "multiplicative_expression"
            | "conjunction_expression"
            | "disjunction_expression"
            | "prefix_expression"
            | "postfix_expression"
            | "infix_expression"
            | "check_expression"
            | "elvis_expression"
            | "as_expression"
            | "spread_expression"
            | "parenthesized_expression" => Some(ReferenceKind::Read),
            // Indexing and range expressions — the index of a write
            // (`arr[CONST] = x`) is still a read of CONST
            "indexing_expression" | "indexing_suffix" | "range_expression" => {
                Some(ReferenceKind::Read)
            }
            // If/when conditions and bodies
            "if_expression"
            | "when_expression"
            | "when_subject"
            | "when_condition"
            | "when_entry"
            | "range_test"
            | "control_structure_body"
            | "statements" => Some(ReferenceKind::Read),
            // Loops and exception handling
            "for_statement" | "while_statement" | "do_while_statement" | "catch_block" => {
                Some(ReferenceKind::Read)
            }
            // Delegation: `val x by impl`, `class Foo : Bar by impl`
            "property_delegate" | "explicit_delegation" => Some(ReferenceKind::Read),
            // Annotation collection literals `[A, B]`, enum entries, lambda
            // parameters carrying an explicit type
            "collection_literal" | "enum_entry" | "parameter_with_optional_type" => {
                Some(ReferenceKind::Read)
            }
            // Lambda and anonymous function bodies
            "lambda_literal" | "anonymous_function" => Some(ReferenceKind::Read),
            // String templates
            "string_literal" | "interpolated_expression" => Some(ReferenceKind::Read),
            _ => None,
        }
    }

    /// Check if a simple_identifier is the infix function name in an infix_expression.
    /// In "a until b", "until" is the function name (middle element).
    fn is_infix_function_name(&self, infix_expr: Node, identifier: Node) -> bool {
        let mut cursor = infix_expr.walk();
        let mut index = 0;
        for child in infix_expr.children(&mut cursor) {
            if child.kind() == "simple_identifier" {
                if child.id() == identifier.id() {
                    // The function name is the second simple_identifier (index 1)
                    // In "a until b": a=0, until=1, b=2
                    return index == 1;
                }
                index += 1;
            }
        }
        false
    }

    /// Check if a navigation_expression or navigation_suffix represents a method call.
    /// This distinguishes property access from method calls:
    /// - this.prop → Read (property access)
    /// - this.method() → Call (method call)
    /// - this.prop.method() → prop is Read, method is Call
    ///
    /// A navigation_suffix is a method call if its parent navigation_expression
    /// has a sibling call_suffix (the () part of the call).
    fn is_navigation_method_call(&self, node: Node) -> bool {
        // For navigation_suffix: check if parent navigation_expression has a call_suffix sibling
        // For navigation_expression: check if it has a call_suffix sibling
        let nav_expr = if node.kind() == "navigation_suffix" {
            node.parent()
        } else {
            Some(node)
        };

        if let Some(nav_expr) = nav_expr {
            if nav_expr.kind() == "navigation_expression" {
                // Check if the parent is a call_expression
                if let Some(parent) = nav_expr.parent() {
                    if parent.kind() == "call_expression" {
                        // The navigation_expression is a direct child of call_expression
                        // Check if it has a call_suffix sibling
                        let mut cursor = parent.walk();
                        for sibling in parent.children(&mut cursor) {
                            if sibling.kind() == "call_suffix" {
                                // This navigation_expression is being called
                                return true;
                            }
                        }
                    }
                }
            }
        }
        false
    }

    /// Check if an identifier in a value_argument is the parameter name (left of =)
    /// vs the value (right of =). Returns true if it's the parameter name.
    ///
    /// Example: `someFunc(primary = primaryLight)`
    /// - `primary` is the parameter name -> return true
    /// - `primaryLight` is the value -> return false
    fn is_named_argument_param_name(&self, value_arg: Node, identifier: Node) -> bool {
        let mut cursor = value_arg.walk();
        let identifier_byte = identifier.start_byte();

        for child in value_arg.children(&mut cursor) {
            if child.kind() == "=" {
                // The identifier is the parameter name if it appears before the =
                return identifier_byte < child.start_byte();
            }
        }

        // If no = found, it's a positional argument, so the identifier is a value
        false
    }

    /// Check if a callable_reference is a class literal (::class)
    /// as opposed to a method reference (::method)
    fn is_class_literal(&self, node: Node, source: &str) -> bool {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            // Look for the "class" keyword on the right side of ::
            if child.kind() == "class" {
                return true;
            }
            // Also check for simple_identifier with text "class"
            if child.kind() == "simple_identifier" {
                let text = node_text(child, source);
                if text == "class" {
                    return true;
                }
            }
        }
        false
    }

    /// Extract the type name from a callable reference like `SomeClass::class` or `SomeClass::method`
    /// Returns the type name (e.g., "SomeClass") from the left side of ::
    ///
    /// AST structure for `MyProvider::class`:
    /// ```text
    /// callable_reference
    ///   type_identifier "MyProvider"
    ///   :: "::"
    ///   class "class"
    /// ```
    fn extract_callable_reference_type(&self, node: Node, source: &str) -> Option<String> {
        let mut cursor = node.walk();

        // Look for the type on the left side of ::
        for child in node.children(&mut cursor) {
            match child.kind() {
                // Type identifier (e.g., MyProvider in MyProvider::class)
                // This is the most common case for class literals
                "type_identifier" => {
                    let type_text = node_text(child, source);
                    // Strip generic parameters if present
                    let name = type_text.split('<').next().unwrap_or(type_text);
                    return Some(name.to_string());
                }
                // Direct type reference (e.g., SomeClass::class)
                "user_type" | "type_reference" => {
                    let type_text = node_text(child, source);
                    // Strip generic parameters if present
                    let name = type_text.split('<').next().unwrap_or(type_text);
                    // Also strip trailing dots for qualified names, take last component
                    let simple_name = name.split('.').next_back().unwrap_or(name);
                    return Some(simple_name.to_string());
                }
                // Simple identifier reference
                "simple_identifier" => {
                    let text = node_text(child, source);
                    // Skip the "class" keyword on the right side of ::
                    if text != "class" {
                        return Some(text.to_string());
                    }
                }
                // Parenthesized expression (e.g., (SomeClass)::class)
                "parenthesized_expression" => {
                    // Recursively look for type inside
                    let mut inner_cursor = child.walk();
                    for inner in child.children(&mut inner_cursor) {
                        if inner.kind() == "user_type"
                            || inner.kind() == "type_identifier"
                            || inner.kind() == "simple_identifier"
                        {
                            let text = node_text(inner, source);
                            let name = text.split('<').next().unwrap_or(text);
                            return Some(name.to_string());
                        }
                    }
                }
                _ => {}
            }
        }

        None
    }

    fn build_fqn(&self, package: &Option<String>, name: &str) -> String {
        match package {
            Some(pkg) => format!("{}.{}", pkg, name),
            None => name.to_string(),
        }
    }

    /// WORKAROUND for tree-sitter-kotlin grammar bug
    ///
    /// When certain syntax patterns occur (e.g., `else if` with function calls),
    /// tree-sitter-kotlin may incorrectly end the class body early, causing
    /// subsequent method declarations to be parsed as top-level functions.
    ///
    /// This function fixes orphaned declarations by:
    /// 1. Finding all class/object declarations and their ACTUAL byte ranges (by scanning for matching braces)
    /// 2. Finding orphaned functions (parent=None) that fall within those ranges
    /// 3. Setting the correct parent and updating kind to Method
    fn fix_orphaned_declarations(
        &self,
        path: &Path,
        source: &str,
        has_error: bool,
        result: &mut ParseResult,
    ) {
        // Collect class/object declarations - we'll compute actual end positions
        let mut type_decls: Vec<(DeclarationId, usize)> = result
            .declarations
            .iter()
            .filter(|d| d.kind.is_type() && d.parent.is_none())
            .map(|d| (d.id.clone(), d.id.start))
            .collect();

        // R2b: an ERROR can swallow the enclosing `object` node WHOLE —
        // orphans with no type to adopt them, and every reference after the
        // ERROR falling to the first declaration of the file. Rebuild the
        // type (and the members the same ERROR took) from the source text
        // before giving up.
        //
        // `has_error` is the gate that keeps this off healthy files: a file of
        // top-level functions (Utils.kt, Extensions.kt) legitimately has no
        // type, and text-scanning it turned a commented-out class into a real
        // declaration — with an FQN, competing with the live class of that name
        // for its manifest entry point.
        if has_error && type_decls.is_empty() {
            self.recover_lost_enclosing_types(path, source, result);
            type_decls = result
                .declarations
                .iter()
                .filter(|d| d.kind.is_type() && d.parent.is_none())
                .map(|d| (d.id.clone(), d.id.start))
                .collect();
        }

        if type_decls.is_empty() {
            return;
        }

        // Compute actual end positions by scanning for matching braces
        let type_ranges: Vec<(DeclarationId, usize, usize)> = type_decls
            .into_iter()
            .filter_map(|(id, start)| {
                let actual_end = self.find_matching_brace(source, start)?;
                Some((id, start, actual_end))
            })
            .collect();

        if type_ranges.is_empty() {
            return;
        }

        // Find orphaned declarations and fix their parent
        let mut adopting_types: Vec<(DeclarationId, usize, usize)> = Vec::new();
        for decl in result.declarations.iter_mut() {
            // Only fix top-level functions (not already class members)
            if decl.parent.is_some() {
                continue;
            }

            // Only fix functions, properties, and objects (not types themselves)
            if decl.kind == DeclarationKind::Class
                || decl.kind == DeclarationKind::Interface
                || decl.kind == DeclarationKind::Enum
                || decl.kind == DeclarationKind::Annotation
            {
                continue;
            }

            let decl_start = decl.id.start;
            let decl_end = decl.id.end;

            // Find the innermost containing type (smallest range that contains
            // this declaration). `>=` on the end: the ERROR that orphaned the
            // declaration can inflate its node to the type's own last byte.
            let containing_type = type_ranges
                .iter()
                .filter(|(_, start, end)| *start < decl_start && *end >= decl_end)
                .min_by_key(|(_, start, end)| end - start);

            if let Some((type_id, type_start, type_end)) = containing_type {
                // This declaration is inside a type but wasn't parsed as a member
                decl.parent = Some(type_id.clone());

                // Update kind: Function -> Method
                if decl.kind == DeclarationKind::Function {
                    decl.kind = DeclarationKind::Method;
                }

                // Clear FQN for methods (they don't have their own FQN)
                decl.fully_qualified_name = None;

                if !adopting_types.iter().any(|(id, _, _)| id == type_id) {
                    adopting_types.push((type_id.clone(), *type_start, *type_end));
                }

                debug!(
                    "Fixed orphaned {}: '{}' -> parent {:?}",
                    decl.kind.display_name(),
                    decl.name,
                    type_id
                );
            }
        }

        // A type that had to adopt orphans sits on an ERROR region: the same
        // ERROR usually swallowed member declarations whole (the partial
        // partial shape — the object survives, everything after the
        // broken function is missing). Rebuild them from the source text.
        for (type_id, type_start, type_end) in adopting_types {
            self.recover_lost_members(path, source, type_start, type_end, &type_id, result);
        }
    }

    /// R2b recovery: tree-sitter lost an enclosing type node entirely (the
    /// total-loss shape — fifteen orphans in the parse, the `object`
    /// itself absent, nothing after the ERROR). Rebuild the type from its
    /// column-0 header with the same brace scan the orphan fix trusts, then
    /// rebuild the member functions and nested classes the ERROR swallowed,
    /// so references get real extents to attribute to.
    /// A copy of `source` with every comment and string literal blanked to
    /// spaces, byte offsets preserved. The recovery below scans raw text, and
    /// text inside a comment or a `"""…"""` block is not code: a commented-out
    /// class became a declaration carrying a real FQN, and a raw string
    /// holding a code template produced members out of nothing.
    fn blank_inert_regions(source: &str) -> String {
        let bytes = source.as_bytes();
        let mut out = bytes.to_vec();
        let len = bytes.len();
        let mut i = 0;
        // Blanks a byte range, keeping newlines so line numbers still hold.
        let blank = |out: &mut Vec<u8>, from: usize, to: usize| {
            for b in out.iter_mut().take(to.min(len)).skip(from) {
                if *b != b'\n' {
                    *b = b' ';
                }
            }
        };
        while i < len {
            match bytes[i] {
                b'/' if i + 1 < len && bytes[i + 1] == b'/' => {
                    let start = i;
                    while i < len && bytes[i] != b'\n' {
                        i += 1;
                    }
                    blank(&mut out, start, i);
                }
                b'/' if i + 1 < len && bytes[i + 1] == b'*' => {
                    let start = i;
                    i += 2;
                    while i + 1 < len && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                        i += 1;
                    }
                    i = (i + 2).min(len);
                    blank(&mut out, start, i);
                }
                b'"' if i + 2 < len && bytes[i + 1] == b'"' && bytes[i + 2] == b'"' => {
                    let start = i;
                    i += 3;
                    // A raw string ends at the LAST quote of its closing run,
                    // and has no escapes.
                    while i + 2 < len
                        && !(bytes[i] == b'"' && bytes[i + 1] == b'"' && bytes[i + 2] == b'"')
                    {
                        i += 1;
                    }
                    i = (i + 3).min(len);
                    while i < len && bytes[i] == b'"' {
                        i += 1;
                    }
                    blank(&mut out, start, i);
                }
                b'"' => {
                    let start = i;
                    i += 1;
                    while i < len && bytes[i] != b'"' {
                        if bytes[i] == b'\\' {
                            i += 1;
                        }
                        i += 1;
                    }
                    i = (i + 1).min(len);
                    blank(&mut out, start, i);
                }
                // Char literal, blanked so `'{'` and `'\\'` stop counting as
                // braces. A lone apostrophe in prose has no close nearby and
                // is left alone.
                b'\'' => {
                    let close = (i + 1..(i + 5).min(len))
                        .find(|&j| bytes[j] == b'\'' && bytes[j - 1] != b'\\');
                    match close {
                        Some(close) => {
                            blank(&mut out, i, close + 1);
                            i = close + 1;
                        }
                        None => i += 1,
                    }
                }
                _ => i += 1,
            }
        }
        // Blanking only ever replaces whole ASCII-delimited regions with ASCII
        // spaces, so multi-byte characters inside them keep their length.
        String::from_utf8(out).unwrap_or_else(|_| source.to_string())
    }

    fn recover_lost_enclosing_types(&self, path: &Path, source: &str, result: &mut ParseResult) {
        // Offsets préservés : les spans restent valables sur la source réelle.
        let scan = Self::blank_inert_regions(source);
        let source = scan.as_str();
        let package = result.package.clone();
        let existing: Vec<(usize, usize)> = result
            .declarations
            .iter()
            .map(|d| (d.id.start, d.id.end))
            .collect();
        let covered = |byte: usize| existing.iter().any(|(s, e)| *s <= byte && byte < *e);
        let line_of = |byte: usize| source[..byte].matches('\n').count() + 1;

        let mut recovered: Vec<(Declaration, usize, usize)> = Vec::new();
        for caps in LOST_TYPE_HEADER.captures_iter(source) {
            let header = caps.get(0).expect("whole match");
            let name = caps.get(1).expect("type name").as_str();
            if covered(header.start()) {
                continue;
            }
            // The body brace must sit on the header line: a body-less
            // `data class X(...)` must not steal the next block's braces.
            let brace_on_header_line = source[header.end()..]
                .bytes()
                .take_while(|b| *b != b'\n')
                .any(|b| b == b'{');
            if !brace_on_header_line {
                continue;
            }
            let Some(type_end) = self.find_matching_brace(source, header.start()) else {
                continue;
            };

            let kind = if header.as_str().contains("interface") {
                DeclarationKind::Interface
            } else if header.as_str().contains("object") {
                DeclarationKind::Object
            } else {
                DeclarationKind::Class
            };
            let type_id = DeclarationId::new(path.to_path_buf(), header.start(), type_end);
            let location = Location::new(
                path.to_path_buf(),
                line_of(header.start()),
                1,
                header.start(),
                type_end,
            );
            let mut decl = Declaration::new(
                type_id.clone(),
                name.to_string(),
                kind,
                location,
                Language::Kotlin,
            );
            decl.fully_qualified_name = Some(self.build_fqn(&package, name));
            if header.as_str().contains("private") {
                decl.visibility = Visibility::Private;
            }
            debug!("Recovered lost enclosing type '{}' from source text", name);
            recovered.push((decl, header.start(), type_end));
        }

        for (decl, type_start, type_end) in recovered {
            let type_id = decl.id.clone();
            result.declarations.push(decl);
            self.recover_lost_members(path, source, type_start, type_end, &type_id, result);
        }
    }

    /// Rebuild member functions and nested classes an ERROR swallowed from a
    /// type's brace-scanned extent. A member is lost when no declaration OF
    /// THAT NAME holds its name byte — plain span coverage lies here, since
    /// the ERROR inflates the preceding orphan's node over everything it ate.
    /// Spans run to the next lost member (or the closing brace): over-wide,
    /// but attribution only needs the innermost extent to be the right one.
    fn recover_lost_members(
        &self,
        path: &Path,
        source: &str,
        type_start: usize,
        type_end: usize,
        type_id: &DeclarationId,
        result: &mut ParseResult,
    ) {
        let scan = Self::blank_inert_regions(source);
        let source = scan.as_str();
        // Offsets des retours à la ligne, calculés une fois : la version
        // naïve re-scannait le fichier depuis l'octet 0 pour CHAQUE membre
        // récupéré, soit un coût quadratique sur un gros fichier cassé.
        let newlines: Vec<usize> = source
            .bytes()
            .enumerate()
            .filter(|(_, b)| *b == b'\n')
            .map(|(i, _)| i)
            .collect();
        let line_of = |byte: usize| newlines.partition_point(|nl| *nl < byte) + 1;
        let body = &source[type_start..type_end];
        let mut members: Vec<(usize, String, DeclarationKind, bool)> = Vec::new();
        for mcaps in LOST_MEMBER_HEADER.captures_iter(body) {
            let m = mcaps.get(0).expect("whole member match");
            let abs = type_start + m.start();
            let (name_cap, kind) = if let Some(name) = mcaps.get(2) {
                (name, DeclarationKind::Method)
            } else if let Some(name) = mcaps.get(4) {
                (name, DeclarationKind::Class)
            } else {
                continue;
            };
            let name_abs = type_start + name_cap.start();
            let already_declared = result.declarations.iter().any(|d| {
                d.name == name_cap.as_str() && d.id.start <= name_abs && name_abs < d.id.end
            });
            if already_declared {
                continue;
            }
            let is_private = m.as_str().contains("private");
            members.push((abs, name_cap.as_str().to_string(), kind, is_private));
        }
        members.sort_by_key(|(start, _, _, _)| *start);
        let ends: Vec<usize> = members
            .iter()
            .skip(1)
            .map(|(start, _, _, _)| *start)
            .chain(std::iter::once(type_end.saturating_sub(1)))
            .collect();
        for ((start, member_name, kind, is_private), end) in members.into_iter().zip(ends) {
            let end = end.max(start + 1);
            debug!("Recovered lost member '{}' from source text", member_name);
            let id = DeclarationId::new(path.to_path_buf(), start, end);
            let location = Location::new(path.to_path_buf(), line_of(start), 1, start, end);
            let mut member = Declaration::new(id, member_name, kind, location, Language::Kotlin);
            member.parent = Some(type_id.clone());
            if is_private {
                member.visibility = Visibility::Private;
            }
            result.declarations.push(member);
        }
    }

    /// WORKAROUND: Scan source text for function calls that tree-sitter may have missed
    /// due to grammar bugs in certain contexts (e.g., else-if blocks).
    fn scan_missed_function_calls(
        &self,
        path: &Path,
        source: &str,
        imports: &[String],
        result: &mut ParseResult,
    ) {
        use std::collections::HashSet;

        // Build a set of already-captured call references (by byte position)
        let existing_calls: HashSet<usize> = result
            .references
            .iter()
            .filter(|r| r.kind == ReferenceKind::Call)
            .map(|r| r.location.start_byte)
            .collect();

        // Regex to find simple function calls: identifier followed by (
        // We use captures to extract just the identifier
        let call_pattern = regex::Regex::new(r"\b([a-z][a-zA-Z0-9_]*)\s*\(").ok();

        let keywords: HashSet<&str> = [
            "if",
            "when",
            "for",
            "while",
            "try",
            "catch",
            "finally",
            "return",
            "throw",
            "do",
            "class",
            "fun",
            "val",
            "var",
            "object",
            "interface",
            "enum",
            "annotation",
            "println",
            "print",
            "require",
            "check",
            "error",
            "assert",
            "apply",
            "also",
            "let",
            "run",
            "with",
        ]
        .iter()
        .cloned()
        .collect();

        if let Some(re) = call_pattern {
            for cap in re.captures_iter(source) {
                if let Some(func_match) = cap.get(1) {
                    let func_name = func_match.as_str();
                    let match_start = func_match.start();

                    // Skip if we already have a reference at this position
                    if existing_calls.contains(&match_start) {
                        continue;
                    }

                    // Skip Kotlin keywords and common functions
                    if keywords.contains(func_name) {
                        continue;
                    }

                    // Skip type constructors (PascalCase)
                    if func_name
                        .chars()
                        .next()
                        .map(|c| c.is_uppercase())
                        .unwrap_or(true)
                    {
                        continue;
                    }

                    // Create location
                    let (line, col) = self.byte_to_line_col(source, match_start);
                    let location = Location::new(
                        path.to_path_buf(),
                        line,
                        col,
                        match_start,
                        match_start + func_name.len(),
                    );

                    result.references.push(UnresolvedReference {
                        name: func_name.to_string(),
                        qualified_name: None,
                        kind: ReferenceKind::Call,
                        location,
                        imports: imports.to_vec(),
                    });
                }
            }
        }
    }

    /// Convert byte offset to line and column (1-indexed)
    fn byte_to_line_col(&self, source: &str, byte_offset: usize) -> (usize, usize) {
        let mut line = 1;
        let mut col = 1;

        for (i, ch) in source.bytes().enumerate() {
            if i >= byte_offset {
                break;
            }
            if ch == b'\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
        }

        (line, col)
    }

    /// Find the matching closing brace for a class/object declaration
    /// Returns the byte position of the closing brace, or None if not found
    /// Byte just past the `}` matching the first `{` at or after
    /// `start_byte`. Comments are skipped whole — a KDoc apostrophe
    /// ("the section's first page") used to open a phantom char literal
    /// that swallowed every brace to end of file. A real char literal is
    /// always short (`'a'`, `'\n'`, `'￿'`): a lone apostrophe with
    /// no close nearby is prose, not code.
    fn find_matching_brace(&self, source: &str, start_byte: usize) -> Option<usize> {
        let bytes = source.as_bytes();
        let len = bytes.len();
        let mut pos = start_byte;

        // Find the opening brace
        while pos < len && bytes[pos] != b'{' {
            pos += 1;
        }
        if pos >= len {
            return None;
        }
        pos += 1;
        let mut depth = 1i32;

        while pos < len && depth > 0 {
            match bytes[pos] {
                // Line comment: skip to end of line
                b'/' if pos + 1 < len && bytes[pos + 1] == b'/' => {
                    while pos < len && bytes[pos] != b'\n' {
                        pos += 1;
                    }
                }
                // Block comment (KDoc included): skip past `*/`
                b'/' if pos + 1 < len && bytes[pos + 1] == b'*' => {
                    pos += 2;
                    while pos + 1 < len && !(bytes[pos] == b'*' && bytes[pos + 1] == b'/') {
                        pos += 1;
                    }
                    pos = (pos + 2).min(len);
                    continue;
                }
                // String literal: skip whole, escapes included
                b'"' => {
                    pos += 1;
                    while pos < len && bytes[pos] != b'"' {
                        if bytes[pos] == b'\\' {
                            pos += 1;
                        }
                        pos += 1;
                    }
                    pos = (pos + 1).min(len);
                    continue;
                }
                // Char literal: jump to its close when one exists nearby
                b'\'' => {
                    let close = (pos + 1..(pos + 8).min(len))
                        .find(|&i| bytes[i] == b'\'' && bytes[i - 1] != b'\\');
                    if let Some(close) = close {
                        pos = close;
                    }
                }
                b'{' => depth += 1,
                b'}' => depth -= 1,
                _ => {}
            }
            pos += 1;
        }

        if depth == 0 {
            Some(pos)
        } else {
            None
        }
    }
}

impl Parser for KotlinParser {
    fn parse(&self, path: &Path, contents: &str) -> Result<ParseResult> {
        // We need interior mutability for the parser
        let mut parser = TsParser::new();
        parser
            .set_language(&tree_sitter_kotlin::language())
            .into_diagnostic()?;

        let tree = parser
            .parse(contents, None)
            .ok_or_else(|| miette::miette!("Failed to parse Kotlin file"))?;

        let root = tree.root_node();
        let mut result = ParseResult::new();

        // Create a temporary instance for parsing
        let temp_parser = Self::new();

        // Extract package declaration
        let package = temp_parser.extract_package(root, contents);
        result.package = package.clone();

        // Extract imports
        let imports = temp_parser.extract_imports(root, contents);
        result.imports = imports.clone();

        // Extract declarations
        temp_parser.extract_declarations(path, root, contents, &package, &mut result)?;

        // WORKAROUND: tree-sitter-kotlin sometimes parses class members as top-level
        // due to grammar bugs (e.g., when parsing else-if in certain contexts).
        // Fix orphaned functions by checking if they fall within a class's byte range.
        temp_parser.fix_orphaned_declarations(path, contents, root.has_error(), &mut result);

        // Extract references
        temp_parser.extract_references(path, root, contents, &imports, &mut result)?;

        // WORKAROUND: tree-sitter-kotlin may miss function calls in misparsed blocks
        // (e.g., else-if blocks that get absorbed into weird AST structures).
        // Scan the source text for function call patterns that weren't captured.
        temp_parser.scan_missed_function_calls(path, contents, &imports, &mut result);

        debug!(
            "Parsed {}: {} declarations, {} references",
            path.display(),
            result.declarations.len(),
            result.references.len()
        );

        Ok(result)
    }
}

impl Default for KotlinParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_class() {
        let parser = KotlinParser::new();
        let source = r#"
            package com.example

            class MyClass {
                fun myMethod() {}
            }
        "#;

        let result = parser.parse(Path::new("test.kt"), source).unwrap();

        assert!(result.package.is_some());
        assert_eq!(result.package.as_ref().unwrap(), "com.example");
        assert!(!result.declarations.is_empty());
    }

    #[test]
    fn test_parse_imports() {
        let parser = KotlinParser::new();
        let source = r#"
            import com.example.Foo
            import com.example.Bar

            class Test {}
        "#;

        let result = parser.parse(Path::new("test.kt"), source).unwrap();

        assert_eq!(result.imports.len(), 2);
    }

    #[test]
    fn test_recovery_leaves_healthy_files_and_inert_text_alone() {
        // La récupération ne doit tourner QUE sur un parse cassé, et ne
        // jamais lire commentaires ni chaînes : un fichier de fonctions
        // top-level (Utils.kt) n'a légitimement aucun type, et une classe
        // en commentaire y devenait une déclaration avec un vrai FQN, qui
        // disputait ensuite son point d'entrée à la vraie classe du manifeste.
        let parser = KotlinParser::new();

        let healthy = concat!(
            "package com.ex\n",
            "/*\n",
            "class LegacyParser {\n",
            "    fun parseOldFormat(raw: String): Int = raw.length\n",
            "}\n",
            "*/\n",
            "// class CommentedOut\n",
            "fun realHelper(x: Int): Int = x + 1\n",
        );
        let result = parser.parse(Path::new("Utils.kt"), healthy).unwrap();
        let names: Vec<&str> = result
            .declarations
            .iter()
            .map(|d| d.name.as_str())
            .collect();
        assert!(
            !names.contains(&"LegacyParser") && !names.contains(&"CommentedOut"),
            "aucune déclaration ne sort d'un commentaire, obtenu : {names:?}"
        );

        // Même exigence sur le chemin cassé : l'ERROR est réel (virgule
        // terminale), mais le corps de la raw string reste du texte.
        let broken = concat!(
            "package com.ex\n\n",
            "data class Holder(val a: String, val b: String)\n\n",
            "object Broken {\n",
            "    val TEMPLATE = \"\"\"\n",
            "    fun generatedFromTemplate(): Int = 0\n",
            "    class TemplateHolder\n",
            "    \"\"\"\n",
            "    fun build(x: String): Holder = Holder(\n",
            "        a = x,\n",
            "        b = x,\n",
            "    )\n",
            "    /*\n",
            "    fun deletedLongAgo(): Int = 1\n",
            "    */\n",
            "}\n",
        );
        let result = parser.parse(Path::new("Broken.kt"), broken).unwrap();
        let names: Vec<&str> = result
            .declarations
            .iter()
            .map(|d| d.name.as_str())
            .collect();
        for phantom in ["generatedFromTemplate", "TemplateHolder", "deletedLongAgo"] {
            assert!(
                !names.contains(&phantom),
                "`{phantom}` vit dans une chaîne ou un commentaire, obtenu : {names:?}"
            );
        }
    }

    #[test]
    fn test_error_swallowed_object_members_are_recovered() {
        // P8 (île 7) : les virgules terminales dans les appels nommés font
        // produire un ERROR à tree-sitter qui avale des membres entiers —
        // parfois l'object lui-même. Un fichier réel donnait 15
        // déclarations orphelines, rien après la ligne 62, et le repli
        // fichier du builder attribuait toutes les références du reste à la
        // PREMIÈRE déclaration du fichier. La fixture est son clone
        // structurel anonymisé.
        let source = include_str!("../../tests/fixtures/kotlin/RouteParserUtils.kt");
        let parser = KotlinParser::new();
        let result = parser
            .parse(Path::new("RouteParserUtils.kt"), source)
            .unwrap();

        let object = result
            .declarations
            .iter()
            .find(|d| d.name == "RouteParserUtils" && d.kind == DeclarationKind::Object)
            .expect("l'object englobant doit exister, perdu ou pas");
        for lost in [
            "isValidUriParts",
            "isActionCommandValid",
            "isSubpagePartValid",
            "RoutePageUidHolder",
        ] {
            let member = result
                .declarations
                .iter()
                .find(|d| d.name == lost)
                .unwrap_or_else(|| panic!("membre avalé par l'ERROR non récupéré : {lost}"));
            assert_eq!(
                member.parent.as_ref(),
                Some(&object.id),
                "{lost} doit être re-parenté sous l'object"
            );
        }
        let orphans: Vec<&str> = result
            .declarations
            .iter()
            .filter(|d| d.parent.is_none() && d.id != object.id)
            .map(|d| d.name.as_str())
            .collect();
        assert!(
            orphans.is_empty(),
            "aucune déclaration ne doit rester orpheline, restent : {orphans:?}"
        );
        assert!(
            result.declarations.len() >= 20,
            "la récupération doit rendre l'essentiel des ~25 déclarations, obtenu {}",
            result.declarations.len()
        );
    }

    #[test]
    fn test_matching_brace_survives_a_comment_apostrophe() {
        // L'apostrophe d'un KDoc (« the section's first page ») ouvrait un
        // char literal fantôme qui avalait toutes les accolades jusqu'à la
        // fin du fichier — le brace-scan rendait None et le re-parentage
        // des orphelins était silencieusement abandonné.
        let parser = KotlinParser::new();
        let source = "object X {\n    /** the section's first page */\n    fun f() {}\n    // don't\n    val c = '{'\n}\n";
        let end = parser
            .find_matching_brace(source, 0)
            .expect("l'accolade fermante doit être trouvée malgré l'apostrophe");
        assert_eq!(&source[end - 1..end], "}");
        assert_eq!(end, source.len() - 1);
    }

    #[test]
    fn test_reference_emitted_for_every_identifier_parent() {
        // P5 (île 7) : `uriParts[ACTION_COMMAND_INDEX]` n'émettait que le
        // receveur — indexing_suffix et onze autres parents de
        // simple_identifier tombaient dans le bras `_ => None`.
        let cases: &[(&str, &str, &str)] = &[
            (
                "indexing_suffix",
                "fun f(arr: List<Int>) = arr[MAGIC_INDEX]",
                "MAGIC_INDEX",
            ),
            (
                "when_subject",
                "fun f() = when (CURRENT_MODE) { else -> 0 }",
                "CURRENT_MODE",
            ),
            (
                "for_statement",
                "fun f() { for (item in ALL_ENTRIES) { println(item) } }",
                "ALL_ENTRIES",
            ),
            (
                "while_statement",
                "fun f() { while (KEEP_RUNNING) { } }",
                "KEEP_RUNNING",
            ),
            (
                "do_while_statement",
                "fun f() { do { } while (KEEP_RUNNING) }",
                "KEEP_RUNNING",
            ),
            (
                "range_test",
                "fun f(x: Int) = when (x) { in UPPER_BOUND -> 1; else -> 0 }",
                "UPPER_BOUND",
            ),
            (
                "collection_literal",
                "@Ann(value = [LINKED_CLASS])\nfun f() {}",
                "LINKED_CLASS",
            ),
            (
                "property_delegate",
                "class C { val x by SHARED_DELEGATE }",
                "SHARED_DELEGATE",
            ),
            (
                "explicit_delegation",
                "class C(impl: Runnable) : Runnable by GLOBAL_IMPL",
                "GLOBAL_IMPL",
            ),
        ];

        let parser = KotlinParser::new();
        for (parent, snippet, expected) in cases {
            let source = format!("package sample\n\n{snippet}\n");
            let result = parser.parse(Path::new("test.kt"), &source).unwrap();
            assert!(
                result.references.iter().any(|r| r.name == *expected),
                "parent `{parent}` : `{expected}` doit produire une référence, refs: {:?}",
                result
                    .references
                    .iter()
                    .map(|r| r.name.as_str())
                    .collect::<Vec<_>>()
            );
        }
    }
}
