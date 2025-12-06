// Kotlin parser - some internal methods reserved for future use
#![allow(dead_code)]

use super::common::{node_text, point_to_location, ParseResult, Parser};
use crate::graph::{
    Declaration, DeclarationId, DeclarationKind, Language, ReferenceKind, UnresolvedReference,
    Visibility,
};
use miette::{IntoDiagnostic, Result};
use std::path::Path;
use tracing::debug;
use tree_sitter::{Node, Parser as TsParser};

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

        decl.parent = parent.clone();

        result.declarations.push(decl);

        // Extract class body members
        // Note: tree-sitter-kotlin doesn't use field names for class_body, so we find by kind
        let mut cursor = node.walk();
        let mut found_class_body = false;
        for child in node.children(&mut cursor) {
            if child.kind() == "class_body" {
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
                name,
                DeclarationKind::EnumCase,
                location,
                Language::Kotlin,
            );

            decl.parent = Some(parent);

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
        let mut cursor = node.walk();
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
            // Indexing and range expressions
            "indexing_expression" | "range_expression" => Some(ReferenceKind::Read),
            // If/when conditions and bodies
            "if_expression"
            | "when_expression"
            | "when_condition"
            | "when_entry"
            | "control_structure_body"
            | "statements" => Some(ReferenceKind::Read),
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

        // Extract references
        temp_parser.extract_references(path, root, contents, &imports, &mut result)?;

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
}
