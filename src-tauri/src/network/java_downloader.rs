use anyhow::{anyhow, Context, Result};
use std::{
    fs,
    io::{self, Cursor},
    path::{Path, PathBuf},
};
use tauri::{AppHandle, Emitter};
use zip::ZipArchive;

const JRE_VERSION: &str = "21";
const ADOPTIUM_JRE_URL: &str = "https://api.adoptium.net/v3/binary/latest/21/ga/windows/x64/jre/hotspot/normal/eclipse";

pub fn resolve_jre_dir() -> Result<PathBuf> {
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        return Ok(PathBuf::from(local_app_data)
            .join("MinecraftP2PConnector")
            .join("bin")
            .join(format!("jre{}", JRE_VERSION)));
    }

    Ok(std::env::temp_dir()
        .join("MinecraftP2PConnector")
        .join("bin")
        .join(format!("jre{}", JRE_VERSION)))
}

pub fn resolve_java_binary() -> String {
    // 1. Check if we have a portable JRE installed
    if let Ok(jre_dir) = resolve_jre_dir() {
        let java_exe = jre_dir.join("bin").join("java.exe");
        if java_exe.exists() {
            return java_exe.display().to_string();
        }
        
        // Sometimes extracted as eclipse-temurin-jre-21.../bin/java.exe
        // Search one level deep
        if let Ok(entries) = fs::read_dir(&jre_dir) {
            for entry in entries.filter_map(Result::ok) {
                if entry.path().is_dir() {
                    let sub_java = entry.path().join("bin").join("java.exe");
                    if sub_java.exists() {
                        return sub_java.display().to_string();
                    }
                }
            }
        }
    }

    // 2. Check JAVA_HOME
    if let Some(java_home) = std::env::var_os("JAVA_HOME") {
        let candidate = PathBuf::from(java_home).join("bin").join("java.exe");
        if candidate.exists() {
            return candidate.display().to_string();
        }
    }

    // 3. Fallback to system java
    "java".into()
}

pub async fn ensure_java_ready(app: &AppHandle) -> Result<String> {
    let java_path = resolve_java_binary();
    
    // Check if java -version works
    let output = std::process::Command::new(&java_path)
        .arg("-version")
        .output();

    let is_valid = if let Ok(output) = output {
        let version_str = String::from_utf8_lossy(&output.stderr);
        version_str.contains("version \"21") || version_str.contains(" 21.")
    } else {
        false
    };

    if is_valid {
        return Ok(java_path);
    }

    // If not valid Java 21, download it
    download_and_extract_jre(app).await?;
    
    Ok(resolve_java_binary())
}

async fn download_and_extract_jre(app: &AppHandle) -> Result<()> {
    let jre_dir = resolve_jre_dir()?;
    if jre_dir.exists() {
        let _ = fs::remove_dir_all(&jre_dir);
    }
    fs::create_dir_all(&jre_dir)?;

    let _ = app.emit("jre-download-status", "Downloading JRE 21... (Portable)");

    let response = reqwest::get(ADOPTIUM_JRE_URL)
        .await
        .context("failed to download JRE from Adoptium")?;
    
    if !response.status().is_success() {
        return Err(anyhow!("Adoptium API returned status {}", response.status()));
    }

    let bytes = response.bytes().await.context("failed to read JRE response bytes")?;
    let _ = app.emit("jre-download-status", "Extracting JRE...");

    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .context("failed to open JRE zip archive")?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = match file.enclosed_name() {
            Some(path) => jre_dir.join(path),
            None => continue,
        };

        if file.name().ends_with('/') {
            fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs::create_dir_all(p)?;
                }
            }
            let mut outfile = fs::File::create(&outpath)?;
            io::copy(&mut file, &mut outfile)?;
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Some(mode) = file.unix_mode() {
                fs::set_permissions(&outpath, fs::Permissions::from_mode(mode))?;
            }
        }
    }

    let _ = app.emit("jre-download-status", "JRE 21 is ready.");
    Ok(())
}
