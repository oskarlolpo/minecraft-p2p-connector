use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rustc-check-cfg=cfg(embedded_ygg)");
    println!("cargo:rerun-if-env-changed=MC_EMBED_YGG");
    println!("cargo:rerun-if-env-changed=MC_YGGSTACK_SOURCE_DIR");
    println!("cargo:rerun-if-env-changed=MC_YGGSTACK_BRIDGE_CC");

    tauri_build::build();

    if !should_embed_ygg() {
        return;
    }

    let source_dir = env::var("MC_YGGSTACK_SOURCE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(r"G:\minecraftjava\newrepo\yggstack-develop"));
    let capi_dir = source_dir.join("contrib").join("capi");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap()).join("yggffi");

    println!("cargo:warning=MC_EMBED_YGG=1: preparing embedded yggstack bridge from {}", source_dir.display());
    if !capi_dir.exists() {
        println!(
            "cargo:warning=embedded yggstack bridge skipped: {} not found",
            capi_dir.display()
        );
        return;
    }

    if let Err(error) = std::fs::create_dir_all(&out_dir) {
        println!(
            "cargo:warning=embedded yggstack bridge skipped: cannot create {}: {}",
            out_dir.display(),
            error
        );
        return;
    }

    let archive_path = out_dir.join("yggstackbridge.a");
    let mut command = Command::new("go");
    command
        .arg("build")
        .arg("-buildmode=c-archive")
        .arg("-o")
        .arg(&archive_path)
        .arg("./contrib/capi")
        .current_dir(&source_dir)
        .env("CGO_ENABLED", "1");

    if let Some((cc, extra_path)) = resolve_c_compiler() {
        command.env("CC", cc);
        if let Some(extra_path) = extra_path {
            let existing_path = env::var_os("PATH").unwrap_or_default();
            let mut joined = std::ffi::OsString::new();
            joined.push(extra_path);
            joined.push(";");
            joined.push(existing_path);
            command.env("PATH", joined);
        }
    }

    match command.status() {
        Ok(status) if status.success() => {
            println!("cargo:rustc-cfg=embedded_ygg");
            println!("cargo:rustc-link-search=native={}", out_dir.display());
            println!("cargo:rustc-link-lib=static=yggstackbridge");
            for native_lib in ["ws2_32", "iphlpapi", "bcrypt", "userenv", "crypt32", "advapi32", "ntdll"] {
                println!("cargo:rustc-link-lib={native_lib}");
            }
            println!(
                "cargo:warning=embedded yggstack bridge archive built at {}",
                archive_path.display()
            );
        }
        Ok(status) => {
            println!(
                "cargo:warning=embedded yggstack bridge build failed with exit status {}",
                status
            );
        }
        Err(error) => {
            println!(
                "cargo:warning=embedded yggstack bridge build failed to start: {}",
                error
            );
        }
    }
}

fn should_embed_ygg() -> bool {
    match env::var("MC_EMBED_YGG") {
        Ok(value) if value == "0" => false,
        Ok(value) if value == "1" => true,
        _ => cfg!(target_os = "windows"),
    }
}

fn resolve_c_compiler() -> Option<(String, Option<String>)> {
    if let Ok(cc) = env::var("MC_YGGSTACK_BRIDGE_CC") {
        if !cc.trim().is_empty() {
            return Some((cc, None));
        }
    }

    let mingw_bin = PathBuf::from(r"C:\msys64\mingw64\bin");
    if mingw_bin.join("gcc.exe").exists() {
        return Some(("gcc".into(), Some(mingw_bin.display().to_string())));
    }

    None
}
