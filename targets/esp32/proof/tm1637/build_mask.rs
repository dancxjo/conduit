pub fn emit(out: &std::path::Path) {
    let source = conduit_environment_make::SevenSegmentMaskSource::compile()
        .expect("checked ordinary Mask Source");
    let material = conduit_environment_make::SevenSegmentMaskMaterial::prepare(&source)
        .expect("exact ordinary Mask planning material");
    let mut rust = String::new();
    material
        .write_rust(&mut rust)
        .expect("linked ordinary Mask module");
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
    std::fs::write(out.join("mask-source.rs"), rust).unwrap();
    println!("cargo:rerun-if-changed=../../../../plots/seven-segment-mask/main.conduit");
}
