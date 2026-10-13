//! The supported languages: grammars, highlight queries and how files map
//! to them.

use std::path::Path;

use tree_sitter_highlight::HighlightConfiguration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Bash,
    Css,
    Dockerfile,
    Html,
    Java,
    JavaScript,
    Markdown,
    /// Inline Markdown (emphasis, links, code spans), injected into blocks.
    MarkdownInline,
    Python,
    Toml,
    Tsx,
    TypeScript,
    Yaml,
}

impl Language {
    pub const ALL: [Self; 13] = [
        Self::Bash,
        Self::Css,
        Self::Dockerfile,
        Self::Html,
        Self::Java,
        Self::JavaScript,
        Self::Markdown,
        Self::MarkdownInline,
        Self::Python,
        Self::Toml,
        Self::Tsx,
        Self::TypeScript,
        Self::Yaml,
    ];

    /// The language of the file at `path`, by file name or extension.
    pub fn for_path(path: &str) -> Option<Self> {
        let file = Path::new(path);
        let name = file.file_name()?.to_str()?.to_ascii_lowercase();
        if name == "dockerfile" || name == "containerfile" || name.starts_with("dockerfile.") {
            return Some(Self::Dockerfile);
        }
        if matches!(
            name.as_str(),
            ".bashrc" | ".zshrc" | ".profile" | ".bash_profile"
        ) {
            return Some(Self::Bash);
        }
        let extension = file.extension()?.to_str()?.to_ascii_lowercase();
        Some(match extension.as_str() {
            "sh" | "bash" | "zsh" => Self::Bash,
            "css" => Self::Css,
            "dockerfile" | "containerfile" => Self::Dockerfile,
            "html" | "htm" => Self::Html,
            "java" => Self::Java,
            "js" | "mjs" | "cjs" | "jsx" => Self::JavaScript,
            "md" | "markdown" => Self::Markdown,
            "py" | "pyi" => Self::Python,
            "toml" => Self::Toml,
            "tsx" => Self::Tsx,
            "ts" | "mts" | "cts" => Self::TypeScript,
            "yml" | "yaml" => Self::Yaml,
            _ => return None,
        })
    }

    /// The language a query injects by name, e.g. `<script>` in HTML.
    pub fn for_injection(name: &str) -> Option<Self> {
        Some(match name.to_ascii_lowercase().as_str() {
            "bash" | "sh" | "shell" | "zsh" => Self::Bash,
            "css" => Self::Css,
            "dockerfile" | "containerfile" => Self::Dockerfile,
            "html" => Self::Html,
            "java" => Self::Java,
            "javascript" | "js" | "jsx" => Self::JavaScript,
            "markdown" | "md" => Self::Markdown,
            "markdown_inline" => Self::MarkdownInline,
            "python" | "py" => Self::Python,
            "toml" => Self::Toml,
            "tsx" => Self::Tsx,
            "typescript" | "ts" => Self::TypeScript,
            "yaml" | "yml" => Self::Yaml,
            _ => return None,
        })
    }

    pub fn configuration(self) -> Result<HighlightConfiguration, tree_sitter::QueryError> {
        use tree_sitter_javascript as js;
        use tree_sitter_typescript as ts;

        let (language, highlights, injections, locals): (
            tree_sitter::Language,
            String,
            &str,
            &str,
        ) = match self {
            Self::Bash => (
                tree_sitter_bash::LANGUAGE.into(),
                tree_sitter_bash::HIGHLIGHT_QUERY.into(),
                "",
                "",
            ),
            Self::Css => (
                tree_sitter_css::LANGUAGE.into(),
                tree_sitter_css::HIGHLIGHTS_QUERY.into(),
                "",
                "",
            ),
            Self::Dockerfile => (
                tree_sitter_containerfile::LANGUAGE.into(),
                tree_sitter_containerfile::HIGHLIGHTS_QUERY.into(),
                tree_sitter_containerfile::INJECTIONS_QUERY,
                "",
            ),
            Self::Html => (
                tree_sitter_html::LANGUAGE.into(),
                tree_sitter_html::HIGHLIGHTS_QUERY.into(),
                tree_sitter_html::INJECTIONS_QUERY,
                "",
            ),
            Self::Java => (
                tree_sitter_java::LANGUAGE.into(),
                tree_sitter_java::HIGHLIGHTS_QUERY.into(),
                "",
                "",
            ),
            Self::JavaScript => (
                js::LANGUAGE.into(),
                format!("{}\n{}", js::HIGHLIGHT_QUERY, js::JSX_HIGHLIGHT_QUERY),
                js::INJECTIONS_QUERY,
                js::LOCALS_QUERY,
            ),
            Self::Markdown => (
                tree_sitter_md::LANGUAGE.into(),
                tree_sitter_md::HIGHLIGHT_QUERY_BLOCK.into(),
                tree_sitter_md::INJECTION_QUERY_BLOCK,
                "",
            ),
            Self::MarkdownInline => (
                tree_sitter_md::INLINE_LANGUAGE.into(),
                tree_sitter_md::HIGHLIGHT_QUERY_INLINE.into(),
                tree_sitter_md::INJECTION_QUERY_INLINE,
                "",
            ),
            Self::Python => (
                tree_sitter_python::LANGUAGE.into(),
                tree_sitter_python::HIGHLIGHTS_QUERY.into(),
                "",
                "",
            ),
            Self::Toml => (
                tree_sitter_toml_ng::LANGUAGE.into(),
                tree_sitter_toml_ng::HIGHLIGHTS_QUERY.into(),
                "",
                "",
            ),
            // TypeScript's queries extend JavaScript's. When several patterns
            // capture the same node, the later one wins, so the specific JSX
            // patterns come after JavaScript's generic `(identifier)` ones.
            Self::Tsx => (
                ts::LANGUAGE_TSX.into(),
                format!(
                    "{}\n{}\n{}",
                    ts::HIGHLIGHTS_QUERY,
                    js::HIGHLIGHT_QUERY,
                    js::JSX_HIGHLIGHT_QUERY
                ),
                js::INJECTIONS_QUERY,
                ts::LOCALS_QUERY,
            ),
            Self::TypeScript => (
                ts::LANGUAGE_TYPESCRIPT.into(),
                format!("{}\n{}", ts::HIGHLIGHTS_QUERY, js::HIGHLIGHT_QUERY),
                js::INJECTIONS_QUERY,
                ts::LOCALS_QUERY,
            ),
            Self::Yaml => (
                tree_sitter_yaml::LANGUAGE.into(),
                tree_sitter_yaml::HIGHLIGHTS_QUERY.into(),
                "",
                "",
            ),
        };
        HighlightConfiguration::new(
            language,
            format!("{self:?}"),
            &highlights,
            injections,
            locals,
        )
    }
}
