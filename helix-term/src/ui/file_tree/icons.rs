use std::{collections::HashMap, path::Path};

use helix_view::editor::FileTreeIconConfig;

const DEFAULT_DIRECTORY: &str = "\u{f07b}";
const DEFAULT_DIRECTORY_OPEN: &str = "\u{f07c}";
const DEFAULT_FILE: &str = "\u{f15b}";

pub struct IconSet {
    directory: String,
    directory_open: String,
    file: String,
    by_extension: HashMap<String, String>,
    by_name: HashMap<String, String>,
}

impl IconSet {
    pub fn new(config: &FileTreeIconConfig) -> Self {
        Self {
            directory: config
                .directory
                .clone()
                .unwrap_or_else(|| DEFAULT_DIRECTORY.to_owned()),
            directory_open: config
                .directory_open
                .clone()
                .unwrap_or_else(|| DEFAULT_DIRECTORY_OPEN.to_owned()),
            file: config
                .file
                .clone()
                .unwrap_or_else(|| DEFAULT_FILE.to_owned()),
            by_extension: config
                .by_extension
                .iter()
                .map(|(extension, icon)| (extension.to_ascii_lowercase(), icon.clone()))
                .collect(),
            by_name: config.by_name.clone(),
        }
    }

    pub fn directory(&self, expanded: bool) -> &str {
        if expanded {
            &self.directory_open
        } else {
            &self.directory
        }
    }

    pub fn file(&self, path: &Path) -> &str {
        if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
            if let Some(icon) = self.by_name.get(name) {
                return icon;
            }
            if let Some(icon) = default_name_icon(name) {
                return icon;
            }
        }

        if let Some(extension) = path.extension().and_then(|extension| extension.to_str()) {
            let extension = extension.to_ascii_lowercase();
            if let Some(icon) = self.by_extension.get(&extension) {
                return icon;
            }
            if let Some(icon) = default_extension_icon(&extension) {
                return icon;
            }
        }

        &self.file
    }
}

fn default_name_icon(name: &str) -> Option<&'static str> {
    Some(match name {
        ".gitignore" | ".gitattributes" | ".gitmodules" | ".gitconfig" => "\u{e702}",
        ".dockerignore" | "Dockerfile" | "docker-compose.yml" | "docker-compose.yaml" => "\u{e7b0}",
        ".bashrc" | ".bash_profile" | ".zshrc" | ".profile" | "profile" => "\u{f489}",
        ".vimrc" | ".gvimrc" => "\u{e62b}",
        ".editorconfig" | ".env" | ".env.local" => "\u{e615}",
        "Cargo.toml" => "\u{e7a8}",
        "Cargo.lock" | "package-lock.json" | "yarn.lock" | "pnpm-lock.yaml" => "\u{f023}",
        "go.mod" => "\u{e627}",
        "go.sum" => "\u{f023}",
        "package.json" | "deno.json" | "deno.jsonc" => "\u{e60b}",
        "tsconfig.json" | "jsconfig.json" => "\u{e628}",
        "pyproject.toml" | "Pipfile" | "requirements.txt" => "\u{e73c}",
        "Gemfile" | "Rakefile" => "\u{e739}",
        "composer.json" => "\u{e73d}",
        "pom.xml" => "\u{e738}",
        "CMakeLists.txt" => "\u{e794}",
        "Makefile" | "makefile" | "Justfile" | "justfile" => "\u{e779}",
        "LICENSE" | "LICENSE.md" | "COPYING" => "\u{f15c}",
        _ => return None,
    })
}

fn default_extension_icon(extension: &str) -> Option<&'static str> {
    Some(match extension {
        // Systems and compiled languages.
        "rs" => "\u{e7a8}",
        "c" | "h" | "m" | "mm" => "\u{e61e}",
        "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" => "\u{e61d}",
        "cs" => "\u{e7b2}",
        "fs" | "fsx" | "fsi" => "\u{e7a7}",
        "go" => "\u{e627}",
        "java" => "\u{e738}",
        "kt" | "kts" => "\u{e634}",
        "scala" | "sc" | "sbt" => "\u{e737}",
        "swift" => "\u{e755}",
        "dart" => "\u{e798}",
        "zig" => "\u{e6a9}",
        "nim" => "\u{e677}",
        "cr" => "\u{e62f}",
        "hs" | "lhs" => "\u{e777}",
        "ml" | "mli" => "\u{e67a}",
        "clj" | "cljs" | "cljc" | "edn" => "\u{e768}",
        "ex" | "exs" => "\u{e62d}",
        "erl" | "hrl" => "\u{e7b1}",
        "pl" | "pm" => "\u{e769}",
        "groovy" | "gradle" => "\u{e775}",
        "asm" | "s" => "\u{e637}",
        "sol" => "\u{f121}",

        // Scripting and web.
        "py" | "pyi" => "\u{e73c}",
        "rb" => "\u{e739}",
        "php" => "\u{e73d}",
        "lua" => "\u{e620}",
        "r" => "\u{e68a}",
        "jl" => "\u{e624}",
        "js" | "mjs" | "cjs" => "\u{e74e}",
        "ts" | "tsx" | "mts" | "cts" => "\u{e628}",
        "jsx" => "\u{e7ba}",
        "vue" => "\u{e6a0}",
        "svelte" => "\u{e697}",
        "html" | "htm" => "\u{e736}",
        "css" | "scss" | "sass" | "less" => "\u{e749}",
        "graphql" | "gql" => "\u{e662}",
        "sh" | "bash" | "zsh" | "fish" | "awk" | "sed" | "ps1" | "psm1" | "psd1" => "\u{f489}",
        "bat" | "cmd" => "\u{e629}",

        // Data, config and markup.
        "md" | "markdown" => "\u{e73e}",
        "toml" | "ini" | "cfg" | "conf" | "properties" | "env" => "\u{e615}",
        "json" | "jsonc" | "json5" => "\u{e60b}",
        "yaml" | "yml" => "\u{e6a8}",
        "xml" | "plist" => "\u{e619}",
        "sql" | "sqlite" | "db" => "\u{e706}",
        "proto" => "\u{e7a9}",
        "tf" | "tfvars" | "hcl" => "\u{e69a}",
        "cmake" => "\u{e794}",
        "tex" | "latex" | "sty" | "cls" => "\u{e69b}",
        "rst" | "adoc" | "asciidoc" | "org" => "\u{f15c}",
        "csv" | "tsv" | "xls" | "xlsx" | "ods" => "\u{f1c3}",
        "doc" | "docx" | "odt" => "\u{f1c2}",
        "ppt" | "pptx" | "odp" => "\u{f1c4}",
        "ipynb" => "\u{e678}",

        // Shell/config-adjacent source files.
        "vim" => "\u{e62b}",
        "nix" => "\u{f313}",

        // Media and binaries.
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "bmp" | "ico" => "\u{f1c5}",
        "pdf" => "\u{f1c1}",
        "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "zst" => "\u{f1c6}",
        "mp3" | "wav" | "flac" | "ogg" => "\u{f1c7}",
        "mp4" | "mkv" | "avi" | "mov" | "webm" => "\u{f1c8}",
        "ttf" | "otf" | "woff" | "woff2" => "\u{f031}",
        "iso" | "dmg" => "\u{f0a0}",
        "pem" | "crt" | "cer" | "key" => "\u{f023}",
        "ics" => "\u{f073}",

        // Plain text and misc.
        "txt" | "log" => "\u{f15c}",
        "lock" => "\u{f023}",
        _ => return None,
    })
}
