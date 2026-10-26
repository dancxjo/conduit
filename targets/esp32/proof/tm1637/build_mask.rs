pub fn emit(out: &std::path::Path) {
    let source = conduit_environment_make::SevenSegmentMaskSource::compile()
        .expect("checked ordinary Mask Source");
    let identity = &source.role.plot_identity;
    let mut rust=format!("pub fn source() -> conduit_core::PlotIdentity {{ conduit_core::PlotIdentity {{source_document_id:{:?}.into(),checked_plot_id:{:?}.into(),expanded_plot_id:{:?}.into()}} }}\n",identity.source_document_id.as_str(),identity.checked_plot_id.as_str(),identity.expanded_plot_id.as_str());
    rust.push_str("pub const CONFIGURATIONS: &[(&str, &str, &str)] = &[\n");
    for gear in &source.plot.expanded.gears {
        let label = conduit_core::face_data_label(&gear.configuration).expect("finite label");
        rust.push_str(&format!(
            "({:?},{:?},{:?}),\n",
            gear.gear_id.as_str(),
            gear.kind_id.as_str(),
            label
        ));
    }
    rust.push_str("];\n");
    rust.push_str("pub const CONNECTIONS: &[(&str,&str,&str,&str)] = &[\n");
    for cord in &source.plot.expanded.connections {
        rust.push_str(&format!(
            "({:?},{:?},{:?},{:?}),\n",
            cord.source_gear_id.as_str(),
            cord.source_port_id.as_str(),
            cord.sink_gear_id.as_str(),
            cord.sink_port_id.as_str()
        ));
    }
    rust.push_str("];\n");
    let role = serde_json::to_vec(&source.role).unwrap();
    rust.push_str(&format!("pub const ROLE: &[u8] = &{:?};\n", role));
    std::fs::write(out.join("mask-source.rs"), rust).unwrap();
    println!("cargo:rerun-if-changed=../../../../plots/seven-segment-mask/main.conduit");
}
