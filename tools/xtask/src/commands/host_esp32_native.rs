//! Checked native ESP32 Host application build; never writes physical flash.
use crate::cli::GlobalOpts;
use conduit_host_make::HostImage;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
#[path = "host_esp32_native_seed.rs"]
mod seed;
pub(super) fn selected(image: &HostImage) -> bool {
    image.manifest.target == "esp32/xtensa-lx6/hw-463-esp-wroom-32"
        && image
            .manifest
            .base_selections
            .iter()
            .any(|base| base.driver == conduit_host_esp32_make::physical::DHT11)
}
pub(super) fn build(
    profile: &Path,
    output: &Path,
    image: &HostImage,
    description: &[u8],
    private_input: Option<&Path>,
    private_output: Option<&Path>,
    workset: Option<&super::host_workset_source::WorksetSourceInput>,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    let checked = super::load_configuration(profile)?;
    conduit_host_esp32_make::physical::CheckedPhysicalConstruction::from_configuration(&checked)?;
    if opts.dry_run {
        return Ok(());
    }
    let root = crate::workspace::workspace_root()?;
    let revision = if let Some(workset) = workset {
        if workset.identity != image.manifest.source_identity {
            return Err("ESP32 Source/IMAGE identity mismatch".into());
        }
        workset.verify()?;
        workset.git_revision.as_str()
    } else {
        image.manifest.source_identity.as_str()
    };
    clean_source(&root, revision)?;
    let sysroot = Command::new("rustc")
        .args(["+esp-conduit-1.91.1", "--print", "sysroot"])
        .output()?;
    if !sysroot.status.success() {
        return Err("qualified ESP toolchain unavailable".into());
    }
    let bin = PathBuf::from(String::from_utf8(sysroot.stdout)?.trim())
        .join("xtensa-esp-elf/esp-15.2.0_20250920/xtensa-esp-elf/bin");
    let linker = bin.join("xtensa-esp32-elf-gcc");
    if !linker.is_file() {
        return Err("qualified ESP linker unavailable".into());
    }
    let target = root.join("target/esp32-electrical-check");
    let temporary = root.join("target/esp32-electrical-tmp");
    fs::create_dir_all(&temporary)?;
    let image_description =
        super::host_workset_source::ImageDescriptionInput::capture(output, description, image)?;
    let mut command = Command::new("cargo");
    command
        .current_dir(root.join("targets/esp32/firmware/host"))
        .args([
            "+esp-conduit-1.91.1",
            "build",
            "--locked",
            "--release",
            "--features",
            "native-host",
        ])
        .env("CONDUIT_HOST_CONFIGURATION", profile.canonicalize()?)
        .env("CONDUIT_HOST_IMAGE_ID", &image.manifest.image_id)
        .env("CONDUIT_SOURCE_IDENTITY", &image.manifest.source_identity)
        .env("CARGO_TARGET_DIR", &target)
        .env("CARGO_BUILD_JOBS", "2")
        .env("CARGO_INCREMENTAL", "0")
        .env("TMPDIR", &temporary)
        .env("CARGO_TARGET_XTENSA_ESP32_NONE_ELF_LINKER", &linker);
    image_description.configure(&mut command)?;
    if let Some(workset) = workset {
        workset.configure(&mut command)?;
    }
    let build_out = super::host_cargo_build::run(&mut command, &target, "conduit-esp32-host")?;
    clean_source(&root, revision)?;
    image_description.verify()?;
    if let Some(workset) = workset {
        workset.verify()?;
    }
    fs::create_dir_all(output)?;
    let elf = target.join("xtensa-esp32-none-elf/release/conduit-esp32-host");
    let merged = output.join("conversion-only-merged.bin");
    let status = crate::process::command_for("espflash")
        .args([
            "save-image",
            "--chip",
            "esp32",
            "--flash-size",
            "4mb",
            "--flash-mode",
            "dio",
            "--flash-freq",
            "40mhz",
            "--merge",
            "--skip-padding",
            "--skip-update-check",
        ])
        .arg(&elf)
        .arg(&merged)
        .status()?;
    if !status.success() {
        return Err("native ESP32 application conversion failed".into());
    }
    let bytes = fs::read(&merged)?;
    let application = application_only(&bytes)?;
    let ram = ram_bytes(&bin.join("xtensa-esp32-elf-objdump"), &elf)?;
    if ram > checked.configuration().limits.static_memory_bytes {
        return Err(format!("ESP32 static DRAM {ram} exceeds checked construction grant").into());
    }
    fs::write(output.join("host.bin"), application)?;
    fs::copy(&elf, output.join("host.elf"))?;
    // Only the application is deployable. Do not package default bootloader or
    // partition-table bytes that could overwrite existing member/history state.
    fs::remove_file(&merged)?;
    fs::write(output.join("resolved-image.json"), description)?;
    fs::write(
        output.join("build-manifest.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema":"conduit.esp32-host/build@1","source_identity":image.manifest.source_identity,
            "image_id":image.manifest.image_id,"configuration_id":checked.configuration_id(),
            "target":image.manifest.target,"application_flash_address":65536,
            "application_bytes":application.len(),"application_sha256":format!("{:x}",Sha256::digest(application)),
            "static_dram_bytes":ram,"static_dram_grant":checked.configuration().limits.static_memory_bytes,
            "bootstrap_flash_address":conduit_host_esp32_make::NATIVE_SPORE_REGION_START,
            "physical_deployment_performed":false
        }))?,
    )?;
    if let Some(workset) = workset {
        workset.verify()?;
    }
    let mut payloads = vec![("host.bin", "application/octet-stream")];
    if super::host_continuation_recipe::publish(
        &build_out,
        output,
        image,
        checked.configuration_id(),
        workset,
    )? {
        payloads.push((super::host_continuation_recipe::NAME, "application/json"));
    }
    super::host_release::seal(
        output,
        "checked-artifact.json",
        &image.manifest.target,
        &image.manifest.make_package_id,
        "bin",
        &image.manifest.builder_adapter,
        image
            .manifest
            .deployment_adapter
            .as_deref()
            .ok_or("ESP32 deployment adapter absent")?,
        &image.manifest.source_identity,
        &payloads,
    )?;
    if let Some(workset) = workset {
        workset.publish(&output.join("checked-artifact.json"))?;
    }
    match (private_input, private_output) {
        (Some(input), Some(destination)) => seed::compose(
            &checked,
            &image.manifest.image_id,
            input,
            destination,
            output,
        )?,
        (None, None) => (),
        _ => return Err("private bootstrap input and destination must be paired".into()),
    }
    Ok(())
}
fn clean_source(root: &Path, expected: &str) -> Result<(), Box<dyn std::error::Error>> {
    let state = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=normal"])
        .current_dir(root)
        .output()?;
    let head = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()?;
    if !state.status.success()
        || !state.stdout.is_empty()
        || !head.status.success()
        || String::from_utf8(head.stdout)?.trim() != expected
    {
        return Err(
            "ESP32 build requires the exact clean committed Source before and after compilation"
                .into(),
        );
    }
    Ok(())
}
fn application_only(merged: &[u8]) -> Result<&[u8], Box<dyn std::error::Error>> {
    let app = merged.get(0x10000..).ok_or("application segment absent")?;
    if app.first() != Some(&0xe9) || app.len() > 0x337000 {
        return Err("application crosses retained flash state or has invalid header".into());
    }
    Ok(app)
}
fn ram_bytes(objdump: &Path, elf: &Path) -> Result<u64, Box<dyn std::error::Error>> {
    let output = Command::new(objdump).arg("-h").arg(elf).output()?;
    if !output.status.success() {
        return Err("ELF static DRAM inventory failed".into());
    }
    static_dram_bytes(&String::from_utf8(output.stdout)?)
}
fn static_dram_bytes(sections: &str) -> Result<u64, Box<dyn std::error::Error>> {
    let mut total = 0u64;
    for line in sections.lines() {
        let columns = line.split_whitespace().collect::<Vec<_>>();
        if columns.len() < 5 || columns[0].parse::<usize>().is_err() {
            continue;
        }
        let size = u64::from_str_radix(columns[2], 16)?;
        let address = u64::from_str_radix(columns[3], 16)?;
        // HAL's .stack is the unused primary-region remainder, not an owned
        // static allocation. A separate linker assertion reserves >=16KiB.
        // Other NOLOAD stores, including the real secondary heap, are charged.
        if columns[1] != ".stack" && (0x3ff00000..0x40000000).contains(&address) {
            total = total.checked_add(size).ok_or("DRAM inventory overflow")?;
        }
    }
    if total == 0 {
        return Err("ELF has no measured static DRAM".into());
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::{application_only, selected, static_dram_bytes};
    #[test]
    fn physical_base_selects_native_builder_without_a_fictitious_gear_implementation() {
        use conduit_host_make as h;
        let packages =
            h::MakePackageSet::compose(&[&conduit_host_esp32_make::Esp32MakePackage]).unwrap();
        let catalog = h::MakeCatalog::canonical().with_packages(&packages);
        let source = include_str!("../../../../targets/esp32/profiles/blue-wroom32.host.conduit");
        let checked = h::check_host_configuration(
            h::parse_host_configuration_conduit(source).unwrap(),
            &catalog,
            &packages,
        )
        .unwrap();
        let (mut image, _) = h::build_default_host_image(
            checked.into_profile(),
            &catalog,
            &packages,
            &h::BuildInputs {
                source_identity: "test/native-builder-selection".into(),
                toolchain_available: true,
            },
        )
        .unwrap();
        assert!(image.manifest.implementations.is_empty());
        assert!(selected(&image));
        let target = image.manifest.target.clone();
        image.manifest.target = "unsupported/other-board".into();
        assert!(!selected(&image));
        image.manifest.target = target;
        image
            .manifest
            .base_selections
            .retain(|base| base.driver != conduit_host_esp32_make::physical::DHT11);
        assert!(!selected(&image));
    }
    #[test]
    fn static_inventory_excludes_reserved_stack_but_counts_secondary_noload_heap() {
        let sections="0 .data 00001000 3ffb0000 3ffb0000\n1 .bss 00002000 3ffb1000 3ffb1000\n2 .stack 00008000 3ffb3000 3ffb3000\n3 .dram2_uninit 00018000 3ffe7e30 3ffe7e30\n4 .text 00100000 400d0020 400d0020";
        assert_eq!(static_dram_bytes(sections).unwrap(), 110592);
        assert_eq!(
            static_dram_bytes(
                "0 .data 00001000 3ffb0000 3ffb0000\n1 .bss 00002000 3ffb1000 3ffb1000"
            )
            .unwrap(),
            12288
        );
        assert!(static_dram_bytes("0 .stack 00008000 3ffb3000 3ffb3000").is_err());
    }
    #[test]
    fn application_extraction_never_packages_bootloader_or_retained_state() {
        let mut merged = vec![0xff; 0x10001];
        merged[0] = 0xe9;
        assert!(application_only(&merged).is_err());
        merged[0x10000] = 0xe9;
        assert_eq!(application_only(&merged).unwrap(), &[0xe9]);
        merged.resize(0x347001, 0xff);
        assert!(application_only(&merged).is_err());
        assert!(application_only(&merged[..0x10000]).is_err());
    }
}
