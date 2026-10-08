fn main() {
    println!("cargo:rustc-check-cfg=cfg(conduitos_domain_image)");
    println!("cargo:rustc-check-cfg=cfg(conduitos_protected_execution)");
    println!("cargo:rustc-cfg=conduitos_domain_image");
}
