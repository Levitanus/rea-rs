use fs_extra::dir::CopyOptions;
use std::error::Error;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use std::{fs, io};
use wait_timeout::ChildExt;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub enum ReaperVersion {
    V7_71,
    V7_73,
}
impl ReaperVersion {
    pub fn latest() -> Self {
        Self::V7_73
    }
    fn linux_download_url(&self) -> &'static str {
        match self {
            Self::V7_71 => {
                "https://www.reaper.fm/files/6.x/reaper671_linux_x86_64.tar.xz"
            }
            Self::V7_73 => {
                "https://www.reaper.fm/files/6.x/reaper673_linux_x86_64.tar.xz"
            }
        }
    }
    fn macos_download_url(&self) -> &'static str {
        match self {
            Self::V7_71 => {
                "https://www.reaper.fm/files/6.x/reaper671_x86_64.dmg"
            }
            Self::V7_73 => {
                "https://www.reaper.fm/files/6.x/reaper673_x86_64.dmg"
            }
        }
    }
    fn linux_download_path(&self) -> PathBuf {
        match self {
            Self::V7_71 => PathBuf::from("reaper_linux_x86_64/REAPER"),
            Self::V7_73 => PathBuf::from("reaper_linux_x86_64/REAPER"),
        }
    }
    fn macos_download_path(&self) -> PathBuf {
        match self {
            Self::V7_71 => PathBuf::from("reaper_macos_x86_64"),
            Self::V7_73 => PathBuf::from("reaper_macos_x86_64"),
        }
    }
    fn linux_executable_path(&self) -> PathBuf {
        match self {
            Self::V7_71 => PathBuf::from("reaper"),
            Self::V7_73 => PathBuf::from("reaper"),
        }
    }
    fn macos_executable_path(&self) -> PathBuf {
        match self {
            Self::V7_71 => PathBuf::from("REAPER.app/Contents/MacOS/REAPER"),
            Self::V7_73 => PathBuf::from("REAPER.app/Contents/MacOS/REAPER"),
        }
    }
    fn macos_install_folder(&self) -> PathBuf {
        match self {
            Self::V7_71 => {
                PathBuf::from("/Volumes/REAPER_INSTALL_INTEL64/REAPER.app")
            }
            Self::V7_73 => {
                PathBuf::from("/Volumes/REAPER_INSTALL_INTEL64/REAPER.app")
            }
        }
    }
}

pub fn run_integration_test(reaper_version: ReaperVersion) {
    env_logger::init();
    let executable_path = match build_integration_test(reaper_version) {
        Some(result) => result.expect("Can not build test environment"),
        None => return (),
    };
    let result = run_integration_test_in_reaper(&executable_path);
    result.expect("Running the integration test in REAPER failed");
}

pub fn build_integration_test(
    reaper_version: ReaperVersion,
) -> Option<Result<PathBuf>> {
    if cfg!(target_family = "windows") {
        println!(
            "REAPER integration tests currently not supported on Windows"
        );
        return None;
    }
    let target_dir_path =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("../target");
    let reaper_download_dir_path = target_dir_path.join("reaper");
    println!("Running integration test");
    let executable_path = if cfg!(target_os = "macos") {
        build_on_macos(
            &reaper_version,
            &target_dir_path,
            &reaper_download_dir_path,
        )
    } else {
        build_on_linux(
            &reaper_version,
            &target_dir_path,
            &reaper_download_dir_path,
        )
    };
    Some(executable_path)
}

fn build_on_linux(
    reaper_version: &ReaperVersion,
    target_dir_path: &Path,
    reaper_download_dir_path: &Path,
) -> Result<PathBuf> {
    let reaper_home_path =
        setup_reaper_for_linux(reaper_version, reaper_download_dir_path)?;
    install_plugin(&target_dir_path, &reaper_home_path)?;
    let reaper_executable =
        reaper_home_path.join(reaper_version.linux_executable_path());
    Ok(reaper_executable)
}

fn build_on_macos(
    reaper_version: &ReaperVersion,
    target_dir_path: &Path,
    reaper_download_dir_path: &Path,
) -> Result<PathBuf> {
    let reaper_home_path =
        setup_reaper_for_macos(reaper_version, reaper_download_dir_path)?;
    install_plugin(&target_dir_path, &reaper_home_path)?;
    let reaper_executable =
        reaper_home_path.join(reaper_version.macos_executable_path());
    Ok(reaper_executable)
}

/// Download file only if it is not exists.
// fn download_file(url: impl Into<String>, path: PathBuf) -> Result<()> {
//     if path.exists() {
//         return Ok(());
//     }
//     let resp = reqwest::blocking::get(url.into())?;
//     let mut f = File::create(path.clone())?;
//     let mut content = Cursor::new(resp.bytes()?);
//     std::io::copy(&mut content, &mut f)?;
//     Ok(())
// }

fn install_plugin(
    target_dir_path: &Path,
    reaper_home_path: &Path,
) -> Result<()> {
    let extension = if cfg!(target_os = "macos") {
        "dylib"
    } else {
        "so"
    };
    let source_path = target_dir_path
        .join("debug")
        .join(format!("libreaper_test_extension_plugin.{}", extension));
    let target_path = reaper_home_path
        .join("UserPlugins")
        .join(format!("reaper_test_extension_plugin.{}", extension));
    fs::create_dir_all(target_path.parent().ok_or("no parent")?)?;
    println!("Copying plug-in to {:?}...", &target_path);
    fs::copy(&source_path, &target_path)?;
    // println!("installing ReaImGui Extension...");
    // [
    //     "reaper_imgui-aarch64.so",
    //     "reaper_imgui-armv7l.so",
    //     "reaper_imgui-i386.dylib",
    //     "reaper_imgui-i686.so",
    //     "reaper_imgui-x64.dll",
    //     "reaper_imgui-x86.dll",
    //     "reaper_imgui-x86_64.dylib",
    //     "reaper_imgui-x86_64.so ",
    // ]
    // .into_iter()
    // .map(|name| {
    //     download_file(
    //         "https://github.com/cfillion/reaimgui/releases/latest/download/"
    //             .to_string()
    //             + name,
    //         reaper_home_path.join("UserPlugins").join(name),
    //     )
    //     .expect("Can not download file")
    // })
    // .count();
    Ok(())
}

fn run_integration_test_in_reaper(reaper_executable: &Path) -> Result<()> {
    println!("Starting REAPER ({:?})...", &reaper_executable);
    let mut child = Command::new(reaper_executable)
        .env("RUN_REAPER_INTEGRATION_TEST", "true")
        .env("RUST_LOG", "debug")
        .arg("-newinst")
        .arg("-new")
        // .arg("-splashlog")
        // .arg("splash.log")
        .spawn()?;
    let exit_status = child.wait_timeout(Duration::from_secs(120))?;
    let exit_status = match exit_status {
        None => {
            child.kill()?;
            return Err(
                "REAPER didn't exit in time (maybe integration test has not started at all)",
            )?;
        }
        Some(s) => s,
    };
    if exit_status.success() {
        return Ok(());
    }
    let exit_code = exit_status.code().unwrap_or(101);
    if exit_code == 172 {
        Err("Integration test failed")?
    } else if exit_code == 101 {
        println!("Exited with code 101, which is, probably, normal for linux");
        Ok(())
    } else {
        Err(
            "REAPER exited unsuccessfully but neither because of signal nor because of failed \
            integration test",
        )?
    }
}

/// Returns path of REAPER home
fn setup_reaper_for_linux(
    reaper_version: &ReaperVersion,
    reaper_download_dir_path: &Path,
) -> Result<PathBuf> {
    let reaper_home_path =
        reaper_download_dir_path.join(reaper_version.linux_download_path());
    let reaper_check_path = reaper_download_dir_path.join("/reaper");
    if reaper_check_path.exists() {
        return Ok(reaper_home_path);
    }
    let reaper_tarball_path =
        reaper_download_dir_path.join("reaper-linux.tar.xz");
    if !reaper_tarball_path.exists() {
        println!("Downloading REAPER to ({:?})...", &reaper_tarball_path);
        download(reaper_version.linux_download_url(), &reaper_tarball_path)?;
    }
    println!("Unpacking REAPER tarball...");
    unpack_tar_xz(&reaper_tarball_path, &reaper_download_dir_path)?;
    write_reaper_config(&reaper_home_path)?;
    println!("REAPER home directory is {:?}", &reaper_home_path);
    Ok(reaper_home_path)
}

/// Returns path of REAPER home
fn setup_reaper_for_macos(
    reaper_version: &ReaperVersion,
    reaper_download_dir_path: &Path,
) -> Result<PathBuf> {
    let reaper_home_path =
        reaper_download_dir_path.join(reaper_version.macos_download_path());
    if reaper_home_path.exists() {
        return Ok(reaper_home_path);
    }
    let reaper_dmg_path = reaper_download_dir_path.join("reaper-macos.dmg");
    if !reaper_dmg_path.exists() {
        println!("Downloading REAPER to ({:?})...", &reaper_dmg_path);
        download(reaper_version.macos_download_url(), &reaper_dmg_path)?;
    }
    println!("Unpacking REAPER dmg...");
    mount_dmg(&reaper_dmg_path)?;
    println!("Copying from mount...");
    fs::create_dir_all(&reaper_home_path)?;

    fs_extra::dir::copy(
        reaper_version.macos_install_folder(),
        &reaper_home_path,
        &CopyOptions {
            overwrite: false,
            skip_exist: false,
            buffer_size: 0,
            copy_inside: false,
            depth: 0,
            ..Default::default()
        },
    )?;
    write_reaper_config(&reaper_home_path)?;
    remove_rewire_plugin_macos_bundle(&reaper_home_path)?;
    println!("REAPER home directory is {:?}", &reaper_home_path);
    Ok(reaper_home_path)
}

fn write_reaper_config(reaper_home_path: &Path) -> Result<()> {
    println!("Writing REAPER configuration...");
    let content = r#"
[audioconfig]
; For dummy audio on Windows
mode=4

[REAPER]
; For dummy audio on Linux
linux_audio_mode=2
; For <none> audio on macOS
coreaudiobs=512
coreaudioindevnew=<none>
coreaudiooutdevnew=<none>
"#;
    fs::write(reaper_home_path.join("reaper.ini"), content)?;
    Ok(())
}

fn remove_rewire_plugin_macos_bundle(reaper_home_path: &Path) -> Result<()> {
    println!("Removing Rewire plug-in (because it makes REAPER get stuck on headless macOS)...");
    fs::remove_dir_all(
        reaper_home_path.join("REAPER.app/Contents/Plugins/ReWire.bundle"),
    )?;
    Ok(())
}

fn download(url: &str, dest_file_path: &Path) -> Result<()> {
    let mut response = reqwest::blocking::get(url)?;
    fs::create_dir_all(
        dest_file_path
            .parent()
            .ok_or("download destination path must be absolute")?,
    )?;
    let mut dest_file = fs::File::create(&dest_file_path)?;
    io::copy(&mut response, &mut dest_file)?;
    Ok(())
}

fn unpack_tar_xz(file_path: &Path, dest_dir_path: &Path) -> Result<()> {
    let tar_xz = File::open(file_path)?;
    let tar = xz2::read::XzDecoder::new(tar_xz);
    let mut archive = tar::Archive::new(tar);
    archive.unpack(dest_dir_path)?;
    Ok(())
}

fn mount_dmg(file_path: &Path) -> Result<()> {
    let mut child = Command::new("hdiutil")
        .arg("attach")
        .arg(file_path)
        .stdin(Stdio::piped())
        .spawn()?;
    let stdin = child.stdin.as_mut().ok_or("Failed to open stdin")?;
    // Get rid of displayed license by simulating q and y key presses
    stdin.write_all("q\nq\ny\ny\ny\ny\n".as_bytes())?;
    let status = child.wait()?;
    if !status.success() {
        return Err("mount not successful".into());
    }
    Ok(())
}
