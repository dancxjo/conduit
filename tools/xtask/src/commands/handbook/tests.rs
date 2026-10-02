use super::*;

#[test]
fn wiki_links_and_heading_ids_become_static_routes() {
    let prepared = prepare_wiki_markdown(
        "# Hello, world!\nSee [[Forms and flow|Forms-and-flow#cords-and-fore-direction]].\n",
    );
    assert!(prepared.contains("# Hello, world! {#hello-world}"));
    assert!(prepared.contains("[Forms and flow](Forms-and-flow.html#cords-and-fore-direction)"));
}

#[test]
fn conduit_fences_use_the_canonical_highlighter_losslessly() {
    let source = "form hello {\n    \"hi\" >> text/upper\n}.\n";
    let rendered = render_markdown(&format!("```conduit\n{source}```\n")).unwrap();
    assert!(rendered.contains("class=\"syntax-keyword\">form</span>"));
    assert!(rendered.contains("class=\"syntax-identity\">text/upper</span>"));
    assert_eq!(
        rendered
            .matches("class=\"syntax-operator\">&gt;</span>")
            .count(),
        2
    );
    let plain = rendered
        .replace("<pre class=\"syntax-example\" data-application-syntax=\"conduit\" tabindex=\"0\" aria-label=\"Read-only Conduit example\"><code>", "")
        .replace("</code></pre>\n", "");
    assert!(plain.contains("&quot;hi&quot;"));
}

#[test]
fn wiki_links_inside_code_are_not_rewritten() {
    let source = "```conduit\nvalue: [[Text; 4]]\n```\n[[Home]]\n";
    let prepared = prepare_wiki_markdown(source);
    assert!(prepared.contains("value: [[Text; 4]]"));
    assert!(prepared.contains("[Home](index.html)"));
}

#[test]
fn duplicate_headings_receive_stable_suffixes() {
    let prepared = prepare_wiki_markdown("## Same\n## Same\n");
    assert!(prepared.contains("## Same {#same}\n"));
    assert!(prepared.contains("## Same {#same-1}\n"));
}

#[test]
fn sidebar_order_drives_the_reading_journey() {
    let sidebar = "### Learn\n- [[Home]]\n- [[Why Conduit|Why-Conduit]]\n";
    assert_eq!(wiki_link_order(sidebar), ["Home", "Why-Conduit"]);
    let rendered = render_markdown(sidebar).unwrap();
    let current = current_sidebar(&rendered, "Why-Conduit");
    assert!(current.contains("aria-current=\"page\" href=\"Why-Conduit.html\""));
}
