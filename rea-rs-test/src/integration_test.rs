// use anyhow::Result;
use fs_extra::dir::CopyOptions;
use ini::Ini;
use std::error::Error;
use std::fs::File;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime};
use std::{fs, io};
use wait_timeout::ChildExt;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub(crate) const INTEGRATION_RESULT_PATH_ENV: &str =
    "REA_RS_INTEGRATION_RESULT_PATH";

#[derive(Debug, PartialEq, Eq)]
enum IntegrationTestOutcome {
    Passed,
    Failed,
}

fn parse_integration_test_outcome(
    result: &str,
) -> io::Result<IntegrationTestOutcome> {
    match result.trim() {
        "PASS" => Ok(IntegrationTestOutcome::Passed),
        "FAIL" => Ok(IntegrationTestOutcome::Failed),
        other => Err(io::Error::new(
            ErrorKind::InvalidData,
            format!("invalid integration test result: {other:?}"),
        )),
    }
}

fn classify_integration_test_result(
    contents: io::Result<String>,
    termination: HostTermination,
) -> Result<()> {
    let exit_status = termination.describe();
    let contents = contents.map_err(|error| match termination {
        HostTermination::TimedOut => io::Error::new(
            error.kind(),
            format!(
                "REAPER integration test timed out after 300 seconds without producing a result file: {error}"
            ),
        ),
        _ => io::Error::new(
            error.kind(),
            format!(
                "REAPER exited with status {exit_status}, but did not produce an integration test result: {error}"
            ),
        ),
    })?;
    let outcome = parse_integration_test_outcome(&contents).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("invalid plugin result (REAPER status {exit_status}): {error}"),
        )
    })?;
    if let HostTermination::Exited(code) = termination {
        if code == 173 {
            return Err(format!(
                "REAPER test plug-in could not write its PASS result (host exit code: {code})"
            )
            .into());
        }
    }
    if matches!(termination, HostTermination::TimedOut) {
        return Err(format!(
            "REAPER integration test timed out after 300 seconds (plugin outcome: {outcome:?}, host status: {exit_status})"
        ).into());
    }
    match outcome {
        IntegrationTestOutcome::Passed => {
            match termination {
                HostTermination::Exited(0) => {
                    println!("Integration test passed (REAPER exit status: {exit_status})");
                    Ok(())
                }
                _ => Err(format!(
                    "Plugin tests passed, but REAPER terminated abnormally ({exit_status})"
                )
                .into()),
            }
        }
        IntegrationTestOutcome::Failed => Err(format!(
            "Integration test reported failure (REAPER exit status: {exit_status})"
        )
        .into()),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HostTermination {
    Exited(i32),
    Signaled(i32),
    Unknown,
    TimedOut,
}

impl HostTermination {
    fn from_status(status: wait_timeout::ExitStatus) -> Self {
        if let Some(code) = status.code() {
            return Self::Exited(code);
        }
        if let Some(signal) = status.unix_signal() {
            return Self::Signaled(signal);
        }
        Self::Unknown
    }

    fn describe(self) -> String {
        match self {
            Self::Exited(code) => format!("exit code: {code}"),
            Self::Signaled(signal) => format!("signal: {signal}"),
            Self::Unknown => "unknown termination status".to_owned(),
            Self::TimedOut => "timeout".to_owned(),
        }
    }
}

pub enum ReaperVersion {
    V6_71,
    V6_73,
    V7_78,
    V7_82,
}
impl ReaperVersion {
    pub fn latest() -> Self {
        Self::V7_82
    }
    fn version_key(&self) -> &'static str {
        match self {
            Self::V6_71 => "6.71",
            Self::V6_73 => "6.73",
            Self::V7_78 => "7.78",
            Self::V7_82 => "7.82",
        }
    }
    fn linux_download_url(&self) -> &'static str {
        match self {
            Self::V6_71 => {
                "https://www.reaper.fm/files/6.x/reaper671_linux_x86_64.tar.xz"
            }
            Self::V6_73 => {
                "https://www.reaper.fm/files/6.x/reaper673_linux_x86_64.tar.xz"
            }
            Self::V7_78 => {
                "https://www.reaper.fm/files/7.x/reaper778_linux_x86_64.tar.xz"
            }
            Self::V7_82 => {
                "https://www.reaper.fm/files/7.x/reaper782_linux_x86_64.tar.xz"
            }
        }
    }
    fn macos_download_url(&self) -> &'static str {
        match self {
            Self::V6_71 => {
                "https://www.reaper.fm/files/6.x/reaper671_x86_64.dmg"
            }
            Self::V6_73 => {
                "https://www.reaper.fm/files/6.x/reaper673_x86_64.dmg"
            }
            Self::V7_78 => {
                "https://www.reaper.fm/files/7.x/reaper778_x86_64.dmg"
            }
            Self::V7_82 => {
                "https://www.reaper.fm/files/7.x/reaper782_universal.dmg"
            }
        }
    }
    fn windows_download_url(&self) -> &'static str {
        match self {
            Self::V6_71 => {
                "https://www.reaper.fm/files/6.x/reaper671_x64-install.exe"
            }
            Self::V6_73 => {
                "https://www.reaper.fm/files/6.x/reaper673_x64-install.exe"
            }
            Self::V7_78 => {
                "https://www.reaper.fm/files/7.x/reaper778_x64-install.exe"
            }
            Self::V7_82 => {
                "https://www.reaper.fm/files/7.x/reaper782_x64-install.exe"
            }
        }
    }
    fn linux_download_path(&self) -> PathBuf {
        PathBuf::from("reaper_linux_x86_64/REAPER")
    }
    fn macos_download_path(&self) -> PathBuf {
        PathBuf::from("reaper_macos_x86_64")
    }
    fn linux_executable_path(&self) -> PathBuf {
        PathBuf::from("reaper")
    }
    fn macos_executable_path(&self) -> PathBuf {
        PathBuf::from("REAPER.app/Contents/MacOS/REAPER")
    }
}

pub fn run_integration_test(reaper_version: ReaperVersion) {
    let executable_path = match build_integration_test(reaper_version) {
        Some(result) => result.expect("Can not build test environment"),
        None => return (),
    };
    let result = run_integration_test_in_reaper(&executable_path);
    result.expect("Running the integration test in REAPER failed");
}

/// Build the test extension and its test executable before installing it into
/// the REAPER test environment.
fn build_test_extension() -> Result<()> {
    let manifest_dir = std::env::var_os("CARGO_MANIFEST_DIR")
        .ok_or("CARGO_MANIFEST_DIR is not set")?;
    let workspace_dir = PathBuf::from(manifest_dir).join("..");
    let target_dir = workspace_dir.join("target");
    let status = Command::new("cargo")
        .current_dir(workspace_dir)
        .args([
            "build",
            "-p",
            "reaper-test-extension-plugin",
            "--target-dir",
        ])
        .arg(target_dir)
        .status()?;
    if !status.success() {
        return Err(format!(
            "building the REAPER test extension failed: {status}"
        )
        .into());
    }
    Ok(())
}

pub fn build_integration_test(
    reaper_version: ReaperVersion,
) -> Option<Result<PathBuf>> {
    if let Err(error) = build_test_extension() {
        return Some(Err(error));
    }
    let target_dir_path =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("../target");
    let reaper_download_dir_path = target_dir_path.join("reaper");
    println!("Running integration test");
    let executable_path = if cfg!(target_os = "windows") {
        build_on_windows(
            &reaper_version,
            &target_dir_path,
            &reaper_download_dir_path,
        )
    } else if cfg!(target_os = "macos") {
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
    seed_bundled_vst_cache(&reaper_home_path)?;
    install_plugin(&target_dir_path, &reaper_home_path)?;
    let reaper_executable =
        reaper_home_path.join(reaper_version.linux_executable_path());
    Ok(reaper_executable)
}

/// Seeds REAPER's per-install bundled-VST cache without scanning arbitrary
/// third-party plug-in paths. This makes bundled effects such as ReaEQ
/// immediately addressable when plugin_scan disables first-run scanning.
fn seed_bundled_vst_cache(reaper_home_path: &Path) -> Result<()> {
    let cache_path = reaper_home_path.join("reaper-vstplugins64.ini");
    let mut ini = match Ini::load_from_file(&cache_path) {
        Ok(ini) => ini,
        Err(ini::Error::Io(error)) if error.kind() == ErrorKind::NotFound => {
            Ini::new()
        }
        Err(error) => return Err(error.into()),
    };
    let mut cache = ini.with_section(Some("vstcache"));
    for (file_name, identifier, display_name) in [
        ("reaeq.vst.so", "1919247729", "ReaEQ (Cockos)"),
        ("reacomp.vst.so", "1919247213", "ReaComp (Cockos)"),
    ] {
        let plugin_path = reaper_home_path.join("Plugins/FX").join(file_name);
        if plugin_path.is_file() {
            cache.set(
                file_name,
                format!("00035C13AD2EDD01,{identifier},{display_name}"),
            );
        }
    }
    ini.write_to_file(cache_path)?;
    Ok(())
}

fn build_on_macos(
    reaper_version: &ReaperVersion,
    target_dir_path: &Path,
    reaper_download_dir_path: &Path,
) -> Result<PathBuf> {
    let reaper_home_path =
        setup_reaper_for_macos(reaper_version, reaper_download_dir_path)?;
    let resource_path = macos_resource_path(&reaper_home_path);
    install_plugin(&target_dir_path, &resource_path)?;
    let reaper_executable =
        reaper_home_path.join(reaper_version.macos_executable_path());
    Ok(reaper_executable)
}

fn build_on_windows(
    reaper_version: &ReaperVersion,
    target_dir_path: &Path,
    reaper_download_dir_path: &Path,
) -> Result<PathBuf> {
    let reaper_home_path =
        setup_reaper_for_windows(reaper_version, reaper_download_dir_path)?;
    install_plugin(target_dir_path, &reaper_home_path)?;
    Ok(reaper_home_path.join("reaper.exe"))
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
    let extension = if cfg!(target_os = "windows") {
        "dll"
    } else if cfg!(target_os = "macos") {
        "dylib"
    } else {
        "so"
    };
    let library_prefix = if cfg!(target_os = "windows") {
        ""
    } else {
        "lib"
    };
    let source_path = target_dir_path.join("debug").join(format!(
        "{library_prefix}reaper_test_extension_plugin.{extension}"
    ));
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
    let executable_directory = reaper_executable
        .parent()
        .ok_or("can not find parent dir of reaper executable")?;
    let (resource_path, isolated_home) = if cfg!(target_os = "macos") {
        let app_bundle_directory = executable_directory
            .parent()
            .ok_or("can not find Contents directory of REAPER.app")?;
        let reaper_home_path = app_bundle_directory
            .parent()
            .ok_or("can not find REAPER.app directory")?
            .parent()
            .ok_or("can not find REAPER installation directory")?;
        (
            macos_resource_path(reaper_home_path),
            Some(reaper_home_path),
        )
    } else {
        (executable_directory.to_path_buf(), None)
    };
    write_reaper_config(&resource_path)?;
    println!("Starting REAPER ({:?})...", &reaper_executable);
    let result_path = integration_result_path()?;
    let output_path = result_path.with_extension("log");
    let output_file = fs::File::create(&output_path)?;
    let error_file = output_file.try_clone()?;
    let mut command = Command::new(reaper_executable);
    if let Some(home) = isolated_home {
        // macOS REAPER reads its resources from ~/Library/Application
        // Support/REAPER. Isolate that directory per downloaded REAPER
        // copy so the test plug-in, configuration, and
        // result-producing run all use the same resource path.
        command.env("HOME", home);
    }
    let mut child = command
        .env("RUN_REAPER_INTEGRATION_TEST", "true")
        .env(INTEGRATION_RESULT_PATH_ENV, &result_path)
        // .env("RUST_LOG", "debug")
        .arg("-newinst")
        .arg("-new")
        .stdout(Stdio::from(output_file))
        .stderr(Stdio::from(error_file))
        // .arg("-splashlog")
        // .arg("splash.log")
        .spawn()?;
    let exit_status = child.wait_timeout(Duration::from_secs(300))?;
    let termination = match exit_status {
        None => {
            let _ = child.kill();
            let _ = child.wait()?;
            HostTermination::TimedOut
        }
        Some(status) => HostTermination::from_status(status),
    };

    let outcome = fs::read_to_string(&result_path);
    let _ = fs::remove_file(&result_path);
    let result = classify_integration_test_result(outcome, termination);
    if result.is_err() {
        if let Ok(log) = fs::read_to_string(&output_path) {
            eprintln!("REAPER process output:\n{log}");
        }
    }
    let _ = fs::remove_file(&output_path);
    result
}

fn integration_result_path() -> Result<PathBuf> {
    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_nanos();
    Ok(std::env::temp_dir().join(format!(
        "rea-rs-integration-{}-{timestamp}.result",
        std::process::id()
    )))
}

/// Returns path of REAPER home
fn setup_reaper_for_linux(
    reaper_version: &ReaperVersion,
    reaper_download_dir_path: &Path,
) -> Result<PathBuf> {
    let reaper_home_path =
        reaper_download_dir_path.join(reaper_version.linux_download_path());
    let reaper_check_path = reaper_home_path.join("reaper");
    if reaper_check_path.exists() {
        return Ok(reaper_home_path);
    }
    let reaper_tarball_path = reaper_download_dir_path.join(format!(
        "reaper-{}_linux.tar.xz",
        reaper_version.version_key()
    ));
    if !reaper_tarball_path.exists() {
        println!("Downloading REAPER to ({:?})...", &reaper_tarball_path);
        download(reaper_version.linux_download_url(), &reaper_tarball_path)?;
    }
    println!("Unpacking REAPER tarball...");
    unpack_tar_xz(&reaper_tarball_path, reaper_download_dir_path)?;
    println!("REAPER home directory is {:?}", &reaper_home_path);
    Ok(reaper_home_path)
}

fn setup_reaper_for_windows(
    reaper_version: &ReaperVersion,
    reaper_download_dir_path: &Path,
) -> Result<PathBuf> {
    let reaper_home_path = reaper_download_dir_path.join(format!(
        "reaper-{}-windows-x64",
        reaper_version.version_key()
    ));
    let reaper_executable = reaper_home_path.join("reaper.exe");
    if reaper_executable.exists() {
        return Ok(reaper_home_path);
    }

    let installer_path = reaper_download_dir_path.join(format!(
        "reaper-{}_windows_x64-install.exe",
        reaper_version.version_key()
    ));
    if !installer_path.exists() {
        println!("Downloading REAPER to ({installer_path:?})...");
        download(reaper_version.windows_download_url(), &installer_path)?;
    }

    fs::create_dir_all(&reaper_home_path)?;
    println!("Installing REAPER to ({reaper_home_path:?})...");
    let status = Command::new(&installer_path)
        // GitHub Actions runs without an interactive UAC desktop. Force the
        // installer to use the runner's current token; the destination is
        // inside the workspace and does not require elevation.
        .env("__COMPAT_LAYER", "RunAsInvoker")
        .arg("/S")
        .arg(format!("/D={}", reaper_home_path.display()))
        .status()?;
    if !status.success() || !reaper_executable.exists() {
        return Err(format!(
            "REAPER installer failed ({status}) or did not create {:?}",
            reaper_executable
        )
        .into());
    }

    // REAPER enables portable mode when this marker is next to the executable.
    fs::write(reaper_home_path.join("portable.ini"), "")?;
    write_reaper_config(&reaper_home_path)?;
    println!("REAPER home directory is {reaper_home_path:?}");
    Ok(reaper_home_path)
}

/// Returns path of REAPER home
fn setup_reaper_for_macos(
    reaper_version: &ReaperVersion,
    reaper_download_dir_path: &Path,
) -> Result<PathBuf> {
    let reaper_home_path = reaper_download_dir_path
        .join(format!("reaper-{}", reaper_version.version_key()))
        .join(reaper_version.macos_download_path());
    let reaper_executable =
        reaper_home_path.join(reaper_version.macos_executable_path());
    if reaper_executable.exists() {
        return Ok(reaper_home_path);
    }
    let reaper_dmg_path = reaper_download_dir_path
        .join(format!("reaper-{}_macos.dmg", reaper_version.version_key()));
    if !reaper_dmg_path.exists() {
        println!("Downloading REAPER to ({:?})...", &reaper_dmg_path);
        download(reaper_version.macos_download_url(), &reaper_dmg_path)?;
    }
    println!("Unpacking REAPER dmg...");
    fs::create_dir_all(&reaper_home_path)?;
    let mount_path = reaper_download_dir_path
        .join(format!("reaper-{}-mounted", reaper_version.version_key()));
    fs::create_dir_all(&mount_path)?;
    mount_dmg(&reaper_dmg_path, &mount_path)?;
    let copy_result: Result<()> = (|| {
        let app_bundle = find_reaper_app_bundle(&mount_path)?;
        println!("Copying REAPER bundle from {app_bundle:?}...");
        fs_extra::dir::copy(
            app_bundle,
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
        Ok(())
    })();
    let detach_result = detach_dmg(&mount_path);
    let _ = fs::remove_dir(&mount_path);
    copy_result?;
    detach_result?;
    write_reaper_config(&macos_resource_path(&reaper_home_path))?;
    remove_rewire_plugin_macos_bundle(&reaper_home_path)?;
    println!("REAPER home directory is {:?}", &reaper_home_path);
    Ok(reaper_home_path)
}

fn macos_resource_path(reaper_home_path: &Path) -> PathBuf {
    reaper_home_path.join("Library/Application Support/REAPER")
}

fn write_reaper_config(reaper_home_path: &Path) -> Result<()> {
    println!("Writing REAPER configuration...");
    fs::create_dir_all(reaper_home_path)?;
    let config_path = reaper_home_path.join("reaper.ini");
    let mut ini = match Ini::load_from_file(config_path.clone()) {
        Ok(ini) => ini,
        Err(ini::Error::Io(error)) if error.kind() == ErrorKind::NotFound => {
            Ini::new()
        }
        Err(error) => return Err(error.into()),
    };
    ini.with_section(Some("REAPER"))
        .set("linux_audio_mode", "2")
        .set("coreaudiobs", "512")
        .set("coreaudioindevnew", "<none>")
        .set("coreaudiooutdevnew", "<none>");
    // Plug-in scan preferences belong to REAPER's canonical [reaper]
    // section. Setting them under [REAOER] leaves the first-run scan prompt
    // enabled because REAPER ignores those entries there.
    ini.with_section(Some("reaper"))
        .set("plugin_scan", "2")
        .set("vst_scan", "2")
        .set("vstpath", "")
        .set("lv2path_linux", "")
        .set("clap_path_linux-x86_64", "");
    ini.with_section(Some("audioconfig")).set("mode", "4");
    ini.write_to_file(config_path)?;
    Ok(())
}

fn remove_rewire_plugin_macos_bundle(reaper_home_path: &Path) -> Result<()> {
    println!("Removing Rewire plug-in (because it makes REAPER get stuck on headless macOS)...");
    let rewire_bundle =
        reaper_home_path.join("REAPER.app/Contents/Plugins/ReWire.bundle");
    if rewire_bundle.exists() {
        fs::remove_dir_all(rewire_bundle)?;
    }
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

fn mount_dmg(file_path: &Path, mount_path: &Path) -> Result<()> {
    let mut child = Command::new("hdiutil")
        .arg("attach")
        .arg("-nobrowse")
        .arg("-readonly")
        .arg("-mountpoint")
        .arg(mount_path)
        .arg(file_path)
        .stdin(Stdio::piped())
        .spawn()?;
    // REAPER's DMG includes a software license dialog. hdiutil presents it
    // through its pager before asking for agreement, even in non-interactive
    // CI. Quit the pager and accept the bundled license so attachment works
    // without a TTY.
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(b"q\ny\n")?;
    }
    let status = child.wait()?;
    if !status.success() {
        return Err(
            format!("mounting REAPER disk image failed: {status}").into()
        );
    }
    Ok(())
}

fn detach_dmg(mount_path: &Path) -> Result<()> {
    let status = Command::new("hdiutil")
        .arg("detach")
        .arg(mount_path)
        .status()?;
    if !status.success() {
        return Err(
            format!("detaching REAPER disk image failed: {status}").into()
        );
    }
    Ok(())
}

fn find_reaper_app_bundle(mount_path: &Path) -> Result<PathBuf> {
    let mut pending = vec![mount_path.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let path = entry.path();
            if path.file_name().is_some_and(|name| name == "REAPER.app")
                && path.is_dir()
            {
                return Ok(path);
            }
            if path.is_dir() {
                pending.push(path);
            }
        }
    }
    Err(format!(
        "could not find REAPER.app in mounted disk image at {}",
        mount_path.display()
    )
    .into())
}

#[cfg(test)]
mod integration_result_tests {
    use super::{
        classify_integration_test_result, parse_integration_test_outcome,
        HostTermination, IntegrationTestOutcome,
    };
    use std::io::{Error, ErrorKind};

    #[test]
    fn parses_pass_result() {
        assert_eq!(
            parse_integration_test_outcome("PASS\n").unwrap(),
            IntegrationTestOutcome::Passed
        );
    }

    #[test]
    fn parses_fail_result() {
        assert_eq!(
            parse_integration_test_outcome("FAIL\n").unwrap(),
            IntegrationTestOutcome::Failed
        );
    }

    #[test]
    fn rejects_missing_or_unknown_result() {
        assert!(parse_integration_test_outcome("").is_err());
        assert!(parse_integration_test_outcome("SUCCESS").is_err());
    }

    #[test]
    fn pass_requires_normal_host_exit() {
        classify_integration_test_result(
            Ok("PASS".to_owned()),
            HostTermination::Exited(0),
        )
        .unwrap();
        for status in [101, 172, 23] {
            assert!(classify_integration_test_result(
                Ok("PASS".to_owned()),
                HostTermination::Exited(status),
            )
            .is_err());
        }
    }

    #[test]
    fn fail_result_remains_failure_for_any_host_status() {
        for status in [0, 172, 23] {
            assert!(classify_integration_test_result(
                Ok("FAIL".to_owned()),
                HostTermination::Exited(status),
            )
            .is_err());
        }
    }

    #[test]
    fn missing_result_and_timeout_are_failures() {
        assert!(classify_integration_test_result(
            Err(Error::from(ErrorKind::NotFound)),
            HostTermination::Exited(0),
        )
        .is_err());
        assert!(classify_integration_test_result(
            Ok("PASS".to_owned()),
            HostTermination::TimedOut,
        )
        .is_err());
    }

    #[test]
    fn pass_result_with_success_report_failure_exit_code_is_error() {
        assert!(classify_integration_test_result(
            Ok("PASS".to_owned()),
            HostTermination::Exited(173),
        )
        .is_err());
    }

    #[cfg(unix)]
    #[test]
    fn signal_termination_is_reported_as_signal_not_exit_code() {
        assert_eq!(HostTermination::Signaled(9).describe(), "signal: 9");
        let error = classify_integration_test_result(
            Ok("PASS".to_owned()),
            HostTermination::Signaled(9),
        )
        .unwrap_err();
        assert!(error.to_string().contains("signal: 9"));
    }
}
