//! Static handbook and Pages-root construction from repository-owned sources.
pub(crate) mod static_body;

use clap::Args;
use conduit_plot::{highlight_syntax, SyntaxHighlightKind};
use pulldown_cmark::{html, CodeBlockKind, CowStr, Event, Options, Parser, Tag, TagEnd};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const MAXIMUM_PAGES: usize = 64;
const MAXIMUM_MARKDOWN_BYTES: usize = 2 * 1024 * 1024;
const MAXIMUM_ASSET_FILES: usize = 256;
const MAXIMUM_ASSET_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Args, Debug)]
pub struct HandbookArgs {
    /// New directory that will receive the static handbook.
    pub output: PathBuf,
}

#[derive(Args, Debug)]
pub struct PagesRootArgs {
    /// New directory that will receive the complete Pages carrier root.
    pub output: PathBuf,
}

pub fn run_handbook(args: HandbookArgs) -> Result<(), Box<dyn std::error::Error>> {
    build_handbook(Path::new("wiki"), &args.output)?;
    println!("handbook staged at {}", args.output.display());
    Ok(())
}

pub fn run_pages_root(args: PagesRootArgs) -> Result<(), Box<dyn std::error::Error>> {
    let output = &args.output;
    refuse_existing(output)?;
    fs::create_dir(output)?;
    render_site_page("site/index.html", output.join("index.html"), "home")?;
    copy_file("site/site.css", output.join("site.css"))?;
    copy_file(
        "targets/browser/host/assets/conduit.css",
        output.join("conduit.css"),
    )?;
    render_site_page(
        "site/current-product.html",
        output.join("current-product.html"),
        "status",
    )?;
    copy_file(
        "site/current-product.mjs",
        output.join("current-product.mjs"),
    )?;
    build_handbook(Path::new("wiki"), &output.join("handbook"))?;
    for required in [
        "index.html",
        "site.css",
        "conduit.css",
        "current-product.html",
        "current-product.mjs",
        "handbook/index.html",
        "handbook/handbook.css",
        "handbook/handbook.mjs",
        "handbook/svg-viewport.css",
        "handbook/svg-viewport.js",
    ] {
        if !output.join(required).is_file() {
            return Err(format!("Pages root lacks required {required}").into());
        }
    }
    println!("Pages root staged at {}", output.display());
    Ok(())
}

fn build_handbook(source: &Path, output: &Path) -> Result<(), Box<dyn std::error::Error>> {
    refuse_existing(output)?;
    let pages = markdown_pages(source)?;
    let sidebar = fs::read_to_string(source.join("_Sidebar.md"))?;
    let reading_order = wiki_link_order(&sidebar);
    let sidebar = render_markdown(&sidebar)?;
    fs::create_dir_all(output)?;
    fs::write(
        output.join("handbook.css"),
        include_str!("handbook/handbook.css"),
    )?;
    fs::write(
        output.join("handbook.mjs"),
        include_str!("handbook/handbook.mjs"),
    )?;
    fs::write(
        output.join("svg-viewport.css"),
        include_str!("../../../../plots/patchbay/workbench/browser/svg-viewport.css"),
    )?;
    fs::write(
        output.join("svg-viewport.js"),
        include_str!("../../../../plots/patchbay/workbench/browser/svg-viewport.js"),
    )?;

    for (stem, markdown) in &pages {
        let title = page_title(markdown).unwrap_or_else(|| readable_name(stem));
        let mut article = render_markdown(markdown)?;
        if !article.contains("<h1") {
            let mut heading = String::new();
            escape_html_into(&title, &mut heading);
            article = format!("<h1>{heading}</h1>{article}");
        }
        let position = reading_order.iter().position(|page| page == stem);
        let next = position
            .and_then(|index| reading_order.get(index + 1))
            .and_then(|next_stem| pages.get(next_stem).map(|markdown| (next_stem, markdown)));
        let page = page_shell(
            stem,
            &title,
            &current_sidebar(&sidebar, stem),
            &article,
            position.map(|index| (index + 1, reading_order.len())),
            next,
        );
        fs::write(output.join(format!("{stem}.html")), page.as_bytes())?;
        if stem == "Home" {
            fs::write(output.join("index.html"), page.as_bytes())?;
        }
    }
    if !output.join("index.html").is_file() {
        return Err("wiki/Home.md is required for the handbook entrance".into());
    }
    copy_assets(&source.join("assets"), &output.join("assets"))?;
    Ok(())
}

fn wiki_link_order(markdown: &str) -> Vec<String> {
    let mut pages = Vec::new();
    let mut rest = markdown;
    while let Some(start) = rest.find("[[") {
        let after = &rest[start + 2..];
        let Some(end) = after.find("]]") else { break };
        let contents = &after[..end];
        let destination = contents
            .split_once('|')
            .map_or(contents, |(_, destination)| destination);
        let page = destination
            .split_once('#')
            .map_or(destination, |(page, _)| page);
        pages.push(page.to_owned());
        rest = &after[end + 2..];
    }
    pages
}

fn current_sidebar(sidebar: &str, stem: &str) -> String {
    let href = if stem == "Home" {
        "href=\"index.html\""
    } else {
        return sidebar.replacen(
            &format!("href=\"{stem}.html\""),
            &format!("aria-current=\"page\" href=\"{stem}.html\""),
            1,
        );
    };
    sidebar.replacen(href, "aria-current=\"page\" href=\"index.html\"", 1)
}

fn markdown_pages(source: &Path) -> Result<BTreeMap<String, String>, Box<dyn std::error::Error>> {
    let mut pages = BTreeMap::new();
    let mut total_bytes = 0usize;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("md") {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|value| value.to_str())
            .ok_or("wiki page name is not UTF-8")?;
        if stem.starts_with('_') {
            continue;
        }
        let markdown = fs::read_to_string(&path)?;
        total_bytes = total_bytes
            .checked_add(markdown.len())
            .ok_or("wiki source byte count overflow")?;
        if pages.insert(stem.to_owned(), markdown).is_some() {
            return Err(format!("duplicate wiki page {stem}").into());
        }
    }
    if pages.len() > MAXIMUM_PAGES || total_bytes > MAXIMUM_MARKDOWN_BYTES {
        return Err("wiki source exceeds the admitted handbook bound".into());
    }
    Ok(pages)
}

fn render_markdown(markdown: &str) -> Result<String, Box<dyn std::error::Error>> {
    let prepared = prepare_wiki_markdown(markdown);
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_HEADING_ATTRIBUTES);
    let mut events = Vec::new();
    let mut parser = Parser::new_ext(&prepared, options).peekable();
    while let Some(event) = parser.next() {
        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(language)))
                if language.trim().eq_ignore_ascii_case("conduit") =>
            {
                let mut source = String::new();
                for next in parser.by_ref() {
                    match next {
                        Event::End(TagEnd::CodeBlock) => break,
                        Event::Text(text) | Event::Code(text) => source.push_str(&text),
                        Event::SoftBreak | Event::HardBreak => source.push('\n'),
                        _ => return Err("unexpected Markdown event inside Conduit code".into()),
                    }
                }
                events.push(Event::Html(CowStr::Boxed(
                    highlighted_conduit(&source)?.into_boxed_str(),
                )));
            }
            other => events.push(other),
        }
    }
    let mut rendered = String::new();
    html::push_html(&mut rendered, events.into_iter());
    Ok(rendered)
}

fn highlighted_conduit(source: &str) -> Result<String, Box<dyn std::error::Error>> {
    let spans = highlight_syntax(source)
        .map_err(|reason| format!("Conduit syntax highlighting refused: {reason:?}"))?;
    let mut html = String::from(
        "<pre class=\"syntax-example\" data-application-syntax=\"conduit\" tabindex=\"0\" aria-label=\"Read-only Conduit example\"><code>",
    );
    for span in spans {
        html.push_str("<span class=\"syntax-");
        html.push_str(kind_name(span.kind));
        html.push_str("\">");
        escape_html_into(&source[span.start..span.end], &mut html);
        html.push_str("</span>");
    }
    html.push_str("</code></pre>\n");
    Ok(html)
}

fn kind_name(kind: SyntaxHighlightKind) -> &'static str {
    match kind {
        SyntaxHighlightKind::Whitespace => "whitespace",
        SyntaxHighlightKind::Comment => "comment",
        SyntaxHighlightKind::Keyword => "keyword",
        SyntaxHighlightKind::Name => "name",
        SyntaxHighlightKind::Identity => "identity",
        SyntaxHighlightKind::String => "string",
        SyntaxHighlightKind::Number => "number",
        SyntaxHighlightKind::Literal => "literal",
        SyntaxHighlightKind::Operator => "operator",
        SyntaxHighlightKind::Delimiter => "delimiter",
    }
}

fn prepare_wiki_markdown(markdown: &str) -> String {
    let mut output = String::with_capacity(markdown.len() + 256);
    let mut in_fence = false;
    let mut slugs = BTreeMap::<String, usize>::new();
    for line in markdown.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            in_fence = !in_fence;
            output.push_str(line);
            continue;
        }
        if in_fence {
            output.push_str(line);
            continue;
        }
        let rewritten = rewrite_wiki_links(line);
        if let Some((marks, heading)) = atx_heading(&rewritten) {
            let base = heading_slug(heading);
            let occurrence = slugs.entry(base.clone()).or_default();
            let slug = if *occurrence == 0 {
                base
            } else {
                format!("{base}-{occurrence}")
            };
            *occurrence += 1;
            let ending = if rewritten.ends_with('\n') { "\n" } else { "" };
            output.push_str(marks);
            output.push(' ');
            output.push_str(heading.trim_end());
            output.push_str(" {#");
            output.push_str(&slug);
            output.push('}');
            output.push_str(ending);
        } else {
            output.push_str(&rewritten);
        }
    }
    output
}

fn rewrite_wiki_links(line: &str) -> String {
    let mut output = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(start) = rest.find("[[") {
        output.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("]]") else {
            output.push_str(&rest[start..]);
            return output;
        };
        let contents = &after[..end];
        let (label, destination) = contents
            .split_once('|')
            .map_or((contents, contents), |(label, destination)| {
                (label, destination)
            });
        let (page, anchor) = destination
            .split_once('#')
            .map_or((destination, None), |(page, anchor)| (page, Some(anchor)));
        output.push('[');
        output.push_str(label);
        output.push_str("](");
        output.push_str(if page == "Home" { "index" } else { page });
        output.push_str(".html");
        if let Some(anchor) = anchor {
            output.push('#');
            output.push_str(anchor);
        }
        output.push(')');
        rest = &after[end + 2..];
    }
    output.push_str(rest);
    output
}

fn atx_heading(line: &str) -> Option<(&str, &str)> {
    let marks = line.bytes().take_while(|byte| *byte == b'#').count();
    if !(1..=6).contains(&marks) || line.as_bytes().get(marks) != Some(&b' ') {
        return None;
    }
    Some((&line[..marks], line[marks + 1..].trim_end_matches('\n')))
}

fn heading_slug(heading: &str) -> String {
    let mut slug = String::new();
    let mut separator = false;
    for character in heading.chars() {
        if character.is_alphanumeric() {
            if separator && !slug.is_empty() {
                slug.push('-');
            }
            separator = false;
            for lower in character.to_lowercase() {
                slug.push(lower);
            }
        } else if !matches!(character, '`' | '*' | '_') {
            separator = true;
        }
    }
    slug
}

fn page_title(markdown: &str) -> Option<String> {
    markdown
        .lines()
        .find_map(|line| line.strip_prefix("# ").map(str::to_owned))
}

fn readable_name(stem: &str) -> String {
    stem.replace('-', " ")
}

fn page_shell(
    stem: &str,
    title: &str,
    sidebar: &str,
    article: &str,
    progress: Option<(usize, usize)>,
    next: Option<(&String, &String)>,
) -> String {
    let styles = crate::site::styles();
    let navigation = crate::site::navigation("handbook");
    let mut escaped_title = String::new();
    escape_html_into(title, &mut escaped_title);
    let progress = progress.map_or_else(String::new, |(current, total)| {
        format!("<p class=\"handbook-progress\">Section {current} of {total}</p>")
    });
    let continuation = next.map_or_else(String::new, |(next_stem, markdown)| {
        let next_title = page_title(markdown).unwrap_or_else(|| readable_name(next_stem));
        let mut escaped_next = String::new();
        escape_html_into(&next_title, &mut escaped_next);
        format!("<nav class=\"handbook-continue\" aria-label=\"Continue reading\"><span>Continue reading</span><a href=\"{next_stem}.html\">{escaped_next}<b aria-hidden=\"true\">→</b></a></nav>")
    });
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><meta name=\"color-scheme\" content=\"dark light\"><title>{escaped_title} · Conduit handbook</title><style>{styles}</style><link rel=\"stylesheet\" href=\"handbook.css\"><link rel=\"stylesheet\" href=\"svg-viewport.css\"><script type=\"module\" src=\"handbook.mjs\"></script></head><body class=\"handbook-page handbook-page-{stem}\" data-application-theme=\"conduit.presentation/phosphor@1\"><a class=\"conduit-skip-link\" href=\"#handbook-content\">Skip to content</a>{navigation}<div class=\"handbook-layout\"><aside class=\"handbook-sidebar\" aria-label=\"Handbook navigation\">{sidebar}</aside><main id=\"handbook-content\" class=\"handbook-article\" tabindex=\"-1\">{progress}{article}{continuation}</main></div></body></html>"
    )
}

fn copy_assets(source: &Path, output: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if !source.exists() {
        return Ok(());
    }
    let mut files = 0usize;
    let mut bytes = 0u64;
    copy_tree(source, output, &mut files, &mut bytes)?;
    if files > MAXIMUM_ASSET_FILES || bytes > MAXIMUM_ASSET_BYTES {
        return Err("wiki assets exceed the admitted handbook bound".into());
    }
    Ok(())
}

fn copy_tree(
    source: &Path,
    output: &Path,
    files: &mut usize,
    bytes: &mut u64,
) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(output)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let destination = output.join(entry.file_name());
        if kind.is_dir() {
            copy_tree(&entry.path(), &destination, files, bytes)?;
        } else if kind.is_file() {
            *files += 1;
            *bytes = bytes
                .checked_add(entry.metadata()?.len())
                .ok_or("wiki asset byte count overflow")?;
            if *files > MAXIMUM_ASSET_FILES || *bytes > MAXIMUM_ASSET_BYTES {
                return Err("wiki assets exceed the admitted handbook bound".into());
            }
            fs::copy(entry.path(), destination)?;
        } else {
            return Err(format!(
                "wiki asset is not a regular file: {}",
                entry.path().display()
            )
            .into());
        }
    }
    Ok(())
}

fn render_site_page(
    source: &str,
    output: PathBuf,
    current: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let source = fs::read_to_string(source)?;
    let marker = "<!-- conduit-site-navigation -->";
    if source.matches(marker).count() != 1 {
        return Err("site source must contain exactly one navigation marker".into());
    }
    let page = source
        .replace(marker, &crate::site::navigation(current))
        .replace("<link rel=\"stylesheet\" href=\"./conduit.css\">", "");
    let page = page.replace(
        "</head>",
        &format!("<style>{}</style></head>", crate::site::styles()),
    );
    fs::write(output, page)?;
    Ok(())
}

fn copy_file(source: impl AsRef<Path>, output: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    fs::copy(source, output)?;
    Ok(())
}

fn refuse_existing(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if path.exists() {
        return Err(format!("destination already exists: {}", path.display()).into());
    }
    Ok(())
}

fn escape_html_into(text: &str, output: &mut String) {
    for character in text.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&quot;"),
            _ => output.push(character),
        }
    }
}

#[cfg(test)]
mod tests;
