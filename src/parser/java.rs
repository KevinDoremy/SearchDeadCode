// Java parser - some internal methods reserved for future use
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

/// Java source code parser using tree-sitter
pub struct JavaParser {
    parser: TsParser,
}

impl JavaParser {
    pub fn new() -> Self {
        let mut parser = TsParser::new();
        parser
            .set_language(&tree_sitter_java::language())
            .expect("Failed to load Java grammar");
        Self { parser }
    }

    fn extract_package(&self, root: Node, source: &str) -> Option<String> {
        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            if child.kind() == "package_declaration" {
                // Find the scoped_identifier
                let mut pkg_cursor = child.walk();
                for pkg_child in child.children(&mut pkg_cursor) {
                    if pkg_child.kind() == "scoped_identifier" || pkg_child.kind() == "identifier" {
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
            if child.kind() == "import_declaration" {
                let mut import_cursor = child.walk();
                for import_child in child.children(&mut import_cursor) {
                    if import_child.kind() == "scoped_identifier"
                        || import_child.kind() == "identifier"
                    {
                        let import_text = node_text(import_child, source);
                        // Check for wildcard import
                        if let Some(_asterisk) = child.child_by_field_name("asterisk") {
                            imports.push(format!("{}.*", import_text));
                        } else {
                            imports.push(import_text.to_string());
                        }
                        break;
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
                "interface_declaration" => {
                    self.extract_interface(path, child, source, package, None, result)?;
                }
                "enum_declaration" => {
                    self.extract_enum(path, child, source, package, None, result)?;
                }
                "annotation_type_declaration" => {
                    self.extract_annotation_type(path, child, source, package, result)?;
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
        let name = node
            .child_by_field_name("name")
            .map(|n| node_text(n, source).to_string())
            .unwrap_or_else(|| "<anonymous>".to_string());

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
            DeclarationKind::Class,
            location,
            Language::Java,
        );

        decl.fully_qualified_name = Some(self.build_fqn(package, &name));
        self.extract_modifiers(node, source, &mut decl);
        decl.super_types = self.extract_super_types(node, source);
        decl.annotations = self.extract_annotations(node, source);
        decl.parent = parent.clone();

        result.declarations.push(decl);

        // Extract class body members
        if let Some(body) = node.child_by_field_name("body") {
            self.extract_class_members(path, body, source, package, id, result)?;
        }

        Ok(())
    }

    fn extract_interface(
        &self,
        path: &Path,
        node: Node,
        source: &str,
        package: &Option<String>,
        parent: Option<DeclarationId>,
        result: &mut ParseResult,
    ) -> Result<()> {
        let name = node
            .child_by_field_name("name")
            .map(|n| node_text(n, source).to_string())
            .unwrap_or_else(|| "<anonymous>".to_string());

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
            DeclarationKind::Interface,
            location,
            Language::Java,
        );

        decl.fully_qualified_name = Some(self.build_fqn(package, &name));
        self.extract_modifiers(node, source, &mut decl);
        decl.super_types = self.extract_super_types(node, source);
        decl.annotations = self.extract_annotations(node, source);
        decl.parent = parent.clone();
        decl.is_abstract = true; // Interfaces are implicitly abstract

        result.declarations.push(decl);

        // Extract interface body members
        if let Some(body) = node.child_by_field_name("body") {
            self.extract_class_members(path, body, source, package, id, result)?;
        }

        Ok(())
    }

    fn extract_enum(
        &self,
        path: &Path,
        node: Node,
        source: &str,
        package: &Option<String>,
        parent: Option<DeclarationId>,
        result: &mut ParseResult,
    ) -> Result<()> {
        let name = node
            .child_by_field_name("name")
            .map(|n| node_text(n, source).to_string())
            .unwrap_or_else(|| "<anonymous>".to_string());

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
            DeclarationKind::Enum,
            location,
            Language::Java,
        );

        decl.fully_qualified_name = Some(self.build_fqn(package, &name));
        self.extract_modifiers(node, source, &mut decl);
        decl.annotations = self.extract_annotations(node, source);
        decl.parent = parent.clone();

        result.declarations.push(decl);

        // Extract enum body
        if let Some(body) = node.child_by_field_name("body") {
            self.extract_enum_body(path, body, source, package, id, result)?;
        }

        Ok(())
    }

    fn extract_enum_body(
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
                "enum_constant" => {
                    self.extract_enum_constant(path, child, source, parent.clone(), result)?;
                }
                "method_declaration" => {
                    self.extract_method(
                        path,
                        child,
                        source,
                        package,
                        Some(parent.clone()),
                        result,
                    )?;
                }
                "field_declaration" => {
                    self.extract_field(path, child, source, Some(parent.clone()), result)?;
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn extract_enum_constant(
        &self,
        path: &Path,
        node: Node,
        source: &str,
        parent: DeclarationId,
        result: &mut ParseResult,
    ) -> Result<()> {
        let name = node
            .child_by_field_name("name")
            .map(|n| node_text(n, source).to_string())
            .unwrap_or_else(|| "<unknown>".to_string());

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
            Language::Java,
        );

        decl.parent = Some(parent);
        decl.is_static = true;

        result.declarations.push(decl);

        Ok(())
    }

    fn extract_annotation_type(
        &self,
        path: &Path,
        node: Node,
        source: &str,
        package: &Option<String>,
        result: &mut ParseResult,
    ) -> Result<()> {
        let name = node
            .child_by_field_name("name")
            .map(|n| node_text(n, source).to_string())
            .unwrap_or_else(|| "<anonymous>".to_string());

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
            DeclarationKind::Annotation,
            location,
            Language::Java,
        );

        decl.fully_qualified_name = Some(self.build_fqn(package, &name));
        self.extract_modifiers(node, source, &mut decl);

        result.declarations.push(decl);

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
                "interface_declaration" => {
                    self.extract_interface(
                        path,
                        child,
                        source,
                        package,
                        Some(parent.clone()),
                        result,
                    )?;
                }
                "enum_declaration" => {
                    self.extract_enum(path, child, source, package, Some(parent.clone()), result)?;
                }
                "method_declaration" => {
                    self.extract_method(
                        path,
                        child,
                        source,
                        package,
                        Some(parent.clone()),
                        result,
                    )?;
                }
                "constructor_declaration" => {
                    self.extract_constructor(path, child, source, parent.clone(), result)?;
                }
                "field_declaration" => {
                    self.extract_field(path, child, source, Some(parent.clone()), result)?;
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn extract_method(
        &self,
        path: &Path,
        node: Node,
        source: &str,
        _package: &Option<String>,
        parent: Option<DeclarationId>,
        result: &mut ParseResult,
    ) -> Result<()> {
        let name = node
            .child_by_field_name("name")
            .map(|n| node_text(n, source).to_string())
            .unwrap_or_else(|| "<anonymous>".to_string());

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
            name,
            DeclarationKind::Method,
            location,
            Language::Java,
        );

        self.extract_modifiers(node, source, &mut decl);
        decl.annotations = self.extract_annotations(node, source);
        decl.parent = parent;

        // Extract parameters
        if let Some(params) = node.child_by_field_name("parameters") {
            self.extract_parameters(path, params, source, id, result)?;
        }

        result.declarations.push(decl);

        Ok(())
    }

    fn extract_constructor(
        &self,
        path: &Path,
        node: Node,
        source: &str,
        parent: DeclarationId,
        result: &mut ParseResult,
    ) -> Result<()> {
        let name = node
            .child_by_field_name("name")
            .map(|n| node_text(n, source).to_string())
            .unwrap_or_else(|| "constructor".to_string());

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
            name,
            DeclarationKind::Constructor,
            location,
            Language::Java,
        );

        self.extract_modifiers(node, source, &mut decl);
        decl.annotations = self.extract_annotations(node, source);
        decl.parent = Some(parent);

        // Extract parameters
        if let Some(params) = node.child_by_field_name("parameters") {
            self.extract_parameters(path, params, source, id, result)?;
        }

        result.declarations.push(decl);

        Ok(())
    }

    fn extract_field(
        &self,
        path: &Path,
        node: Node,
        source: &str,
        parent: Option<DeclarationId>,
        result: &mut ParseResult,
    ) -> Result<()> {
        // Extract the type from field_declaration (shared by all declarators)
        let field_type = self.extract_field_type(node, source);

        // Field declaration can have multiple declarators
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "variable_declarator" {
                if let Some(name_node) = child.child_by_field_name("name") {
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
                        DeclarationKind::Field,
                        location,
                        Language::Java,
                    );

                    self.extract_modifiers(node, source, &mut decl);
                    decl.annotations = self.extract_annotations(node, source);
                    decl.parent = parent.clone();
                    decl.type_name = field_type.clone();

                    result.declarations.push(decl);
                }
            }
        }

        Ok(())
    }

    /// Extract the type from a field declaration (e.g., "private String name" -> "String")
    fn extract_field_type(&self, node: Node, source: &str) -> Option<String> {
        // In Java's tree-sitter grammar, the type is a direct child of field_declaration
        // before the variable_declarator(s)
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                // Simple type: String, Integer, etc.
                "type_identifier" => {
                    return Some(node_text(child, source).to_string());
                }
                // Generic type: List<String>, Map<K, V>
                "generic_type" => {
                    return Some(node_text(child, source).to_string());
                }
                // Array type: String[], int[]
                "array_type" => {
                    return Some(node_text(child, source).to_string());
                }
                // Primitive types: int, boolean, etc.
                "integral_type" | "floating_point_type" | "boolean_type" | "void_type" => {
                    return Some(node_text(child, source).to_string());
                }
                // Scoped type: com.example.MyClass
                "scoped_type_identifier" => {
                    return Some(node_text(child, source).to_string());
                }
                _ => {}
            }
        }
        None
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
            if child.kind() == "formal_parameter" || child.kind() == "spread_parameter" {
                if let Some(name_node) = child.child_by_field_name("name") {
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
                        Language::Java,
                    );

                    decl.parent = Some(parent.clone());

                    result.declarations.push(decl);
                }
            }
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
        let mut cursor = node.walk();

        loop {
            let current = cursor.node();

            match current.kind() {
                "identifier" => {
                    if let Some(parent) = current.parent() {
                        if let Some(kind) = self.determine_reference_kind(parent) {
                            let name = node_text(current, source).to_string();
                            let location = point_to_location(
                                path,
                                current.start_position(),
                                current.end_position(),
                                current.start_byte(),
                                current.end_byte(),
                            );

                            // Debug: log when we add the reference
                            if name == "calculateEndTranslation"
                                || name == "getZoomContainerDistanceFromRecyclerViewCenter"
                            {
                                debug!("  -> Added as reference with kind {:?}", kind);
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
                "type_identifier" => {
                    let name = node_text(current, source).to_string();
                    let location = point_to_location(
                        path,
                        current.start_position(),
                        current.end_position(),
                        current.start_byte(),
                        current.end_byte(),
                    );

                    // `new X(...)` : le X est un type_identifier, mais la
                    // référence est une INSTANCIATION — la sortir en Type
                    // prive le constructeur de toute arête entrante (les
                    // règles "classe instanciée" ne voient que Call/
                    // Instantiation) et tout ce que le ctor appelle meurt.
                    let kind = if current
                        .parent()
                        .is_some_and(|p| p.kind() == "object_creation_expression")
                    {
                        ReferenceKind::Instantiation
                    } else {
                        ReferenceKind::Type
                    };

                    result.references.push(UnresolvedReference {
                        name,
                        qualified_name: None,
                        kind,
                        location,
                        imports: imports.to_vec(),
                    });
                }
                "scoped_identifier" | "scoped_type_identifier" => {
                    let name = node_text(current, source).to_string();
                    let location = point_to_location(
                        path,
                        current.start_position(),
                        current.end_position(),
                        current.start_byte(),
                        current.end_byte(),
                    );

                    // même règle que type_identifier : `new a.b.X(...)`
                    let kind = if current
                        .parent()
                        .is_some_and(|p| p.kind() == "object_creation_expression")
                    {
                        ReferenceKind::Instantiation
                    } else {
                        ReferenceKind::Type
                    };

                    result.references.push(UnresolvedReference {
                        name: name.split('.').next_back().unwrap_or(&name).to_string(),
                        qualified_name: Some(name),
                        kind,
                        location,
                        imports: imports.to_vec(),
                    });
                }
                _ => {}
            }

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

    fn extract_modifiers(&self, node: Node, source: &str, decl: &mut Declaration) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "modifiers" {
                let mut mod_cursor = child.walk();
                for modifier in child.children(&mut mod_cursor) {
                    let text = node_text(modifier, source);
                    decl.modifiers.push(text.to_string());

                    match text {
                        "public" => decl.visibility = Visibility::Public,
                        "private" => decl.visibility = Visibility::Private,
                        "protected" => decl.visibility = Visibility::Protected,
                        "static" => decl.is_static = true,
                        "abstract" => decl.is_abstract = true,
                        _ => {}
                    }
                }
            }
        }

        // Java default is package-private
        if !decl
            .modifiers
            .iter()
            .any(|m| m == "public" || m == "private" || m == "protected")
        {
            decl.visibility = Visibility::PackagePrivate;
        }
    }

    fn extract_super_types(&self, node: Node, source: &str) -> Vec<String> {
        let mut super_types = Vec::new();

        // Check superclass
        if let Some(superclass) = node.child_by_field_name("superclass") {
            // The `superclass` node spans `extends Foo` — keep the type only,
            // so consumers can match names exactly instead of by substring.
            let text = match superclass.named_child(0) {
                Some(type_node) => node_text(type_node, source).to_string(),
                None => node_text(superclass, source)
                    .trim_start_matches("extends")
                    .trim_start()
                    .to_string(),
            };
            super_types.push(text);
        }

        // Check interfaces
        if let Some(interfaces) = node.child_by_field_name("interfaces") {
            let mut cursor = interfaces.walk();
            for child in interfaces.children(&mut cursor) {
                if child.kind() == "type_list" {
                    let mut type_cursor = child.walk();
                    for type_node in child.children(&mut type_cursor) {
                        if type_node.kind() != "," {
                            super_types.push(node_text(type_node, source).to_string());
                        }
                    }
                }
            }
        }

        super_types
    }

    fn extract_annotations(&self, node: Node, source: &str) -> Vec<String> {
        let mut annotations = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.kind() == "modifiers" {
                let mut mod_cursor = child.walk();
                for modifier in child.children(&mut mod_cursor) {
                    if modifier.kind() == "marker_annotation" || modifier.kind() == "annotation" {
                        annotations.push(node_text(modifier, source).to_string());
                    }
                }
            }
        }

        annotations
    }

    fn determine_reference_kind(&self, parent: Node) -> Option<ReferenceKind> {
        match parent.kind() {
            // Method/function calls
            "method_invocation" => Some(ReferenceKind::Call),
            "explicit_constructor_invocation" => Some(ReferenceKind::Call),

            // Field/property access
            "field_access" => Some(ReferenceKind::Read),

            // Assignment - identifiers on the right side are reads
            // The left side (target) handling is separate
            "assignment_expression" => Some(ReferenceKind::Read),

            // Type references
            "type_identifier" | "generic_type" | "type_arguments" => Some(ReferenceKind::Type),

            // Inheritance
            "superclass" | "super_interfaces" | "extends_interfaces" => {
                Some(ReferenceKind::Inheritance)
            }

            // Object instantiation
            "object_creation_expression" => Some(ReferenceKind::Instantiation),

            // Annotations
            "annotation" | "marker_annotation" | "annotation_argument_list" => {
                Some(ReferenceKind::Annotation)
            }

            // Cast expression
            "cast_expression" => Some(ReferenceKind::Read),

            // Binary expressions (arithmetic, comparison, logical)
            "binary_expression" => Some(ReferenceKind::Read),

            // Unary expressions (!, -, +, ++, --, ~)
            "unary_expression" | "update_expression" => Some(ReferenceKind::Read),

            // Ternary/conditional expression (condition ? a : b)
            "ternary_expression" | "conditional_expression" => Some(ReferenceKind::Read),

            // Parenthesized expressions
            "parenthesized_expression" => Some(ReferenceKind::Read),

            // Return statements
            "return_statement" => Some(ReferenceKind::Read),

            // Method arguments
            "argument_list" => Some(ReferenceKind::Read),

            // Array access (array[index])
            "array_access" => Some(ReferenceKind::Read),

            // Array creation
            "array_creation_expression" | "array_initializer" => Some(ReferenceKind::Read),

            // Variable declarations and initializers
            "variable_declarator" | "local_variable_declaration" => Some(ReferenceKind::Read),

            // Control flow statements - conditions and bodies
            "if_statement"
            | "while_statement"
            | "do_statement"
            | "for_statement"
            | "enhanced_for_statement" => Some(ReferenceKind::Read),

            // Switch statements
            "switch_expression"
            | "switch_statement"
            | "switch_block"
            | "switch_label"
            | "switch_rule"
            | "switch_block_statement_group" => Some(ReferenceKind::Read),

            // Exception handling
            "throw_statement"
            | "catch_clause"
            | "try_statement"
            | "try_with_resources_statement" => Some(ReferenceKind::Read),

            // Assert statement
            "assert_statement" => Some(ReferenceKind::Read),

            // Synchronized statement
            "synchronized_statement" => Some(ReferenceKind::Read),

            // Lambda expressions
            "lambda_expression" | "lambda_body" => Some(ReferenceKind::Read),

            // Expression statements (standalone expressions)
            "expression_statement" => Some(ReferenceKind::Read),

            // Instanceof check
            "instanceof_expression" => Some(ReferenceKind::Read),

            // Class literal (SomeClass.class)
            "class_literal" => Some(ReferenceKind::Type),

            // Method reference (SomeClass::method)
            "method_reference" => Some(ReferenceKind::Read),

            // String concatenation and templates
            "string_literal" | "template_expression" => Some(ReferenceKind::Read),

            // Block statements (code blocks)
            "block" => Some(ReferenceKind::Read),

            // Dimensions for array types
            "dimensions_expr" => Some(ReferenceKind::Read),

            // Resource in try-with-resources
            "resource" | "resource_specification" => Some(ReferenceKind::Read),

            // Spread in varargs or method references
            "spread_element" => Some(ReferenceKind::Read),

            // For loop parts
            "for_init" | "for_condition" | "for_update" => Some(ReferenceKind::Read),

            _ => None,
        }
    }

    fn build_fqn(&self, package: &Option<String>, name: &str) -> String {
        match package {
            Some(pkg) => format!("{}.{}", pkg, name),
            None => name.to_string(),
        }
    }
}

impl Parser for JavaParser {
    fn parse(&self, path: &Path, contents: &str) -> Result<ParseResult> {
        let mut parser = TsParser::new();
        parser
            .set_language(&tree_sitter_java::language())
            .into_diagnostic()?;

        let tree = parser
            .parse(contents, None)
            .ok_or_else(|| miette::miette!("Failed to parse Java file"))?;

        let root = tree.root_node();
        let mut result = ParseResult::new();

        // Create a temporary instance for parsing
        let temp_parser = Self::new();

        let package = temp_parser.extract_package(root, contents);
        result.package = package.clone();
        let imports = temp_parser.extract_imports(root, contents);
        result.imports = imports.clone();
        temp_parser.extract_declarations(path, root, contents, &package, &mut result)?;
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

impl Default for JavaParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_class() {
        let parser = JavaParser::new();
        let source = r#"
            package com.example;

            public class MyClass {
                public void myMethod() {}
            }
        "#;

        let result = parser.parse(Path::new("Test.java"), source).unwrap();

        assert!(result.package.is_some());
        assert_eq!(result.package.as_ref().unwrap(), "com.example");
        assert!(!result.declarations.is_empty());
    }

    #[test]
    fn test_parse_imports() {
        let parser = JavaParser::new();
        let source = r#"
            import com.example.Foo;
            import com.example.Bar;

            class Test {}
        "#;

        let result = parser.parse(Path::new("Test.java"), source).unwrap();

        assert_eq!(result.imports.len(), 2);
    }

    #[test]
    fn test_super_types_carry_no_extends_keyword() {
        // P3 : le nœud `superclass` couvre `extends Foo` entier ; avant le fix,
        // 631/3196 tableaux super_types du monorepo portaient "extends Foo".
        let parser = JavaParser::new();
        let source = r#"
            package com.example;

            public class Child extends Base implements Runnable, java.io.Closeable {
            }
        "#;

        let result = parser.parse(Path::new("Test.java"), source).unwrap();

        let child = result
            .declarations
            .iter()
            .find(|d| d.name == "Child")
            .unwrap();
        assert_eq!(
            child.super_types,
            vec!["Base", "Runnable", "java.io.Closeable"],
            "super_types must hold bare type names"
        );
    }
}
