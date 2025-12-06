use tree_sitter::Parser;

fn main() {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_kotlin::language())
        .unwrap();

    let source = r#"
sealed class UiState {
    object Loading : UiState()
    data class Success(val data: String) : UiState()
    object Empty : UiState()
}
"#;

    let tree = parser.parse(source, None).unwrap();
    print_tree(&tree.root_node(), source, 0);
}

fn print_tree(node: &tree_sitter::Node, source: &str, indent: usize) {
    let indent_str = "  ".repeat(indent);
    let text = if node.child_count() == 0 {
        format!(" \"{}\"", node.utf8_text(source.as_bytes()).unwrap_or(""))
    } else {
        String::new()
    };
    println!("{}{}{}", indent_str, node.kind(), text);

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        print_tree(&child, source, indent + 1);
    }
}
