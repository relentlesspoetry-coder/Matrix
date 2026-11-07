//! Keep the verified Matrix AppImage's private runtime out of external programs.
use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    os::unix::ffi::{OsStrExt, OsStringExt},
    path::{Component, Path, PathBuf},
    process::Command,
};

type Environment = BTreeMap<OsString, OsString>;

pub struct Context {
    pub image: PathBuf,
    root: PathBuf,
}
impl Context {
    pub fn identify(image: &Path, appdir: &Path, exe: &Path) -> Option<Self> {
        let image = image.canonicalize().ok()?;
        let root = appdir.canonicalize().ok()?;
        let exe = exe.canonicalize().ok()?;
        // Inherited APPIMAGE/APPDIR from another program is not our runtime.
        (image.is_file()
            && root.is_dir()
            && exe.starts_with(&root)
            && exe.file_name()? == "matrix-frontend")
            .then_some(Self { image, root })
    }

    fn from_environment(environment: &Environment, exe: &Path) -> Option<Self> {
        Self::identify(
            Path::new(environment.get(OsStr::new("APPIMAGE"))?),
            Path::new(environment.get(OsStr::new("APPDIR"))?),
            exe,
        )
    }

    fn owns_path(&self, value: &[u8]) -> bool {
        let path = Path::new(OsStr::from_bytes(value));
        if !path.is_absolute() {
            return false;
        }
        // Resolve existing ancestors too: a missing library below a symlink to
        // an outside folder must not be removed by a textual prefix match.
        let mut ancestor = path;
        let mut tail = Vec::new();
        let mut resolved = loop {
            if let Ok(resolved) = ancestor.canonicalize() {
                break resolved;
            }
            let Some(last) = ancestor.components().next_back() else {
                return false;
            };
            tail.push(last.as_os_str().to_owned());
            let Some(parent) = ancestor.parent() else {
                return false;
            };
            ancestor = parent;
        };
        for part in tail.iter().rev() {
            resolved.push(part);
        }
        let mut normalized = PathBuf::new();
        for part in resolved.components() {
            match part {
                Component::ParentDir => {
                    normalized.pop();
                }
                Component::CurDir => (),
                other => normalized.push(other.as_os_str()),
            }
        }
        normalized.starts_with(&self.root)
    }

    fn clean_paths(&self, command: &mut Command, environment: &Environment, name: &str) {
        let Some(value) = environment.get(OsStr::new(name)) else {
            return;
        };
        let paths: Vec<_> = value.as_bytes().split(|byte| *byte == b':').collect();
        let kept: Vec<_> = paths
            .iter()
            .copied()
            .filter(|path| !self.owns_path(path))
            .collect();
        if kept.len() == paths.len() {
            return;
        }
        if kept.is_empty() {
            // Unset is different from an empty search path (current directory).
            command.env_remove(name);
        } else {
            command.env(name, OsString::from_vec(kept.join(&b':')));
        }
    }

    fn clean(&self, command: &mut Command, environment: &Environment) {
        // Audited AppRun.wrapped and linuxdeploy-plugin-gtk.sh from the shipped
        // AppImage. Preserve outside paths, duplicate/empty entries and raw OS
        // bytes. No env_clear, user preload removal, or theme/locale resets.
        // AppRun overwrites PYTHONHOME without saving its original value. An
        // inherited outside value is retained; a lost pre-AppRun value cannot
        // safely be reconstructed from an unrelated *_ORIG variable.
        for name in [
            "PYTHONHOME",
            "PYTHONPATH",
            "PATH",
            "LD_LIBRARY_PATH",
            "PERLLIB",
            "XDG_DATA_DIRS",
            "GSETTINGS_SCHEMA_DIR",
            "QT_PLUGIN_PATH",
            "GST_PLUGIN_SYSTEM_PATH",
            "GST_PLUGIN_SYSTEM_PATH_1_0",
            "GI_TYPELIB_PATH",
            "GTK_PATH",
        ] {
            self.clean_paths(command, environment, name);
        }
        for name in [
            "GTK_DATA_PREFIX",
            "GTK_EXE_PREFIX",
            "GIO_MODULE_DIR",
            "GTK_IM_MODULE_FILE",
            "GDK_PIXBUF_MODULE_FILE",
        ] {
            if environment
                .get(OsStr::new(name))
                .is_some_and(|value| self.owns_path(value.as_bytes()))
            {
                command.env_remove(name);
            }
        }
        // Children are not the old AppImage runtime, especially the coordinator
        // that must remain usable after the old mount is removed.
        for name in ["APPDIR", "APPIMAGE", "ARGV0"] {
            command.env_remove(name);
        }
    }
}

fn clean_external(command: &mut Command, environment: &Environment, exe: &Path) {
    if let Some(context) = Context::from_environment(environment, exe) {
        context.clean(command, environment);
    }
}

pub fn command(program: impl AsRef<OsStr>) -> Command {
    let mut command = Command::new(program);
    if let Ok(exe) = std::env::current_exe() {
        clean_external(&mut command, &std::env::vars_os().collect(), &exe);
    }
    command.env_remove("MATRIX_INSTANCE_TOKEN");
    command
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn fixture(root: &Path) -> (Context, Environment, PathBuf) {
        let mount = root.join("bundle with spaces");
        fs::create_dir_all(mount.join("usr/bin")).unwrap();
        let exe = mount.join("usr/bin/matrix-frontend");
        fs::write(&exe, b"not executed").unwrap();
        let image = root.join("Matrix.AppImage");
        fs::write(&image, b"not executed").unwrap();
        let environment = [
            (OsString::from("APPDIR"), mount.clone().into_os_string()),
            (OsString::from("APPIMAGE"), image.clone().into_os_string()),
            (OsString::from("ARGV0"), image.clone().into_os_string()),
        ]
        .into_iter()
        .collect();
        (
            Context::identify(&image, &mount, &exe).unwrap(),
            environment,
            exe,
        )
    }

    fn cleaned(environment: &Environment, exe: &Path) -> Environment {
        let mut command = Command::new("/bin/true");
        command.env_clear().envs(environment);
        clean_external(&mut command, environment, exe);
        command
            .get_envs()
            .filter_map(|(key, value)| value.map(|value| (key.to_owned(), value.to_owned())))
            .collect()
    }

    #[test]
    fn audited_paths_keep_outside_entries_empty_components_and_user_settings() {
        let root = tempfile::tempdir().unwrap();
        let (context, mut environment, exe) = fixture(root.path());
        let bundled = context.root.join("missing private libs");
        for name in [
            "PATH",
            "LD_LIBRARY_PATH",
            "PYTHONPATH",
            "PERLLIB",
            "XDG_DATA_DIRS",
            "GSETTINGS_SCHEMA_DIR",
            "QT_PLUGIN_PATH",
            "GST_PLUGIN_SYSTEM_PATH",
            "GST_PLUGIN_SYSTEM_PATH_1_0",
            "GI_TYPELIB_PATH",
            "GTK_PATH",
        ] {
            environment.insert(
                name.into(),
                format!("{}:/outside user path::relative/path:", bundled.display()).into(),
            );
        }
        environment.insert(
            "PYTHONHOME".into(),
            "/user/python-prefix:/user/python-exec-prefix".into(),
        );
        for name in [
            "LD_PRELOAD",
            "LD_LIBRARY_PATH_ORIG",
            "GTK_THEME",
            "PYTHONDONTWRITEBYTECODE",
            "CUSTOM_USER_SETTING",
        ] {
            environment.insert(name.into(), "literal user value".into());
        }
        for name in [
            "GTK_DATA_PREFIX",
            "GTK_EXE_PREFIX",
            "GIO_MODULE_DIR",
            "GTK_IM_MODULE_FILE",
            "GDK_PIXBUF_MODULE_FILE",
        ] {
            environment.insert(name.into(), bundled.clone().into_os_string());
        }
        let result = cleaned(&environment, &exe);
        for (key, value) in environment {
            match key.to_str().unwrap() {
                "APPDIR"
                | "APPIMAGE"
                | "ARGV0"
                | "GTK_DATA_PREFIX"
                | "GTK_EXE_PREFIX"
                | "GIO_MODULE_DIR"
                | "GTK_IM_MODULE_FILE"
                | "GDK_PIXBUF_MODULE_FILE" => assert!(!result.contains_key(&key)),
                _ if value.as_bytes().starts_with(bundled.as_os_str().as_bytes()) => {
                    assert_eq!(result[&key], "/outside user path::relative/path:")
                }
                _ => assert_eq!(result[&key], value),
            }
        }
    }

    #[test]
    fn bundle_only_search_paths_are_unset_without_inventing_current_directory() {
        let root = tempfile::tempdir().unwrap();
        let (context, mut environment, exe) = fixture(root.path());
        environment.insert(
            "PYTHONHOME".into(),
            context.root.join("usr").into_os_string(),
        );
        environment.insert(
            "PYTHONPATH".into(),
            context.root.join("usr/lib").into_os_string(),
        );
        environment.insert("PATH".into(), format!("{}:", context.root.display()).into());
        environment.insert("PERLLIB".into(), "".into());
        let result = cleaned(&environment, &exe);
        assert!(!result.contains_key(OsStr::new("PYTHONHOME")));
        assert!(!result.contains_key(OsStr::new("PYTHONPATH")));
        assert_eq!(result[OsStr::new("PATH")], "");
        assert_eq!(result[OsStr::new("PERLLIB")], "");
        assert!(!result.contains_key(OsStr::new("QT_PLUGIN_PATH")));
    }

    #[test]
    fn literal_path_boundaries_symlinks_and_non_utf8_values_are_preserved() {
        let root = tempfile::tempdir().unwrap();
        let (context, mut environment, exe) = fixture(root.path());
        let outside = root.path().join("bundle with spaces-other");
        fs::create_dir(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, context.root.join("outside-link")).unwrap();
        let alias = root.path().join("bundle-alias");
        std::os::unix::fs::symlink(&context.root, &alias).unwrap();
        let kept = [
            outside.join("missing").into_os_string(),
            context.root.join("outside-link/missing").into_os_string(),
            context
                .root
                .join("../bundle with spaces-other/missing")
                .into_os_string(),
            OsString::from_vec(b"relative-\xff-value".to_vec()),
            OsString::from(""),
        ];
        let expected = OsString::from_vec(
            kept.iter()
                .map(|value| value.as_bytes())
                .collect::<Vec<_>>()
                .join(&b':'),
        );
        let value = OsString::from_vec(
            [
                alias.join("missing").as_os_str().as_bytes(),
                expected.as_bytes(),
                context.root.join("private missing").as_os_str().as_bytes(),
            ]
            .join(&b':'),
        );
        environment.insert("PYTHONPATH".into(), value);
        assert_eq!(
            cleaned(&environment, &exe)[OsStr::new("PYTHONPATH")],
            expected
        );
    }

    #[test]
    fn native_and_unrelated_appimage_environments_are_not_sanitized() {
        let root = tempfile::tempdir().unwrap();
        let (context, mut environment, exe) = fixture(root.path());
        environment.insert(
            "PYTHONHOME".into(),
            context.root.join("usr").into_os_string(),
        );
        environment.insert("GTK_DATA_PREFIX".into(), "/user/gtk".into());
        let outside = root.path().join("matrix-frontend");
        fs::write(&outside, b"not executed").unwrap();
        assert_eq!(cleaned(&environment, &outside), environment);
        let mut native = environment.clone();
        native.remove(OsStr::new("APPIMAGE"));
        assert_eq!(cleaned(&native, &exe), native);
        assert_eq!(
            cleaned(&environment, &exe)[OsStr::new("GTK_DATA_PREFIX")],
            "/user/gtk"
        );
    }

    #[test]
    fn python_launch_fixture() {
        if std::env::var_os("MATRIX_PYTHON_ENV_FIXTURE").is_none() {
            return;
        }
        if let Some(coordinator) = std::env::var_os("MATRIX_PYTHON_COORDINATOR_FIXTURE") {
            let (_, module) = module_path!().split_once("::").unwrap();
            let output = command(coordinator)
                .args([
                    "--exact",
                    &format!("{module}::python_launch_fixture"),
                    "--nocapture",
                ])
                .env_remove("MATRIX_PYTHON_COORDINATOR_FIXTURE")
                .env("MATRIX_PYTHON_REOPEN_FIXTURE", "1")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        if std::env::var_os("MATRIX_PYTHON_REOPEN_FIXTURE").is_some() {
            assert!(std::env::var_os("APPDIR").is_none());
            assert!(std::env::var_os("APPIMAGE").is_none());
            assert!(std::env::var_os("PYTHONHOME").is_none());
        }
        let output = command("/usr/bin/python3")
            .args([
                "-c",
                "import encodings, matrix_user_module; print(matrix_user_module.VALUE)",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "user-module-preserved"
        );
    }

    #[test]
    fn verified_appimage_can_launch_host_python_without_losing_user_modules() {
        let root = tempfile::tempdir().unwrap();
        let mount = root.path().join("bundle with spaces");
        fs::create_dir_all(mount.join("usr/bin")).unwrap();
        let exe = mount.join("usr/bin/matrix-frontend");
        fs::copy(std::env::current_exe().unwrap(), &exe).unwrap();
        let image = root.path().join("Matrix.AppImage");
        fs::write(&image, b"synthetic identity fixture, never executed").unwrap();
        let user_modules = root.path().join("user modules");
        fs::create_dir(&user_modules).unwrap();
        fs::write(
            user_modules.join("matrix_user_module.py"),
            "VALUE='user-module-preserved'\n",
        )
        .unwrap();
        let (_, module) = module_path!().split_once("::").unwrap();
        let coordinator = root.path().join("matrix-update-coordinator");
        fs::copy(std::env::current_exe().unwrap(), &coordinator).unwrap();
        for handoff in [false, true] {
            let mut command = Command::new(&exe);
            if handoff {
                command.env("MATRIX_PYTHON_COORDINATOR_FIXTURE", &coordinator);
            }
            let output = command
                .args([
                    "--exact",
                    &format!("{module}::python_launch_fixture"),
                    "--nocapture",
                ])
                .env("MATRIX_PYTHON_ENV_FIXTURE", "1")
                .env("APPIMAGE", &image)
                .env("APPDIR", &mount)
                .env("PYTHONHOME", mount.join("usr"))
                .env(
                    "PYTHONPATH",
                    format!(
                        "{}:{}:",
                        mount.join("usr/share/pyshared").display(),
                        user_modules.display()
                    ),
                )
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}
