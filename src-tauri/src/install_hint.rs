use crate::ffmpeg::Tool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    MacOs,
    Windows,
    Linux(Option<LinuxPm>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinuxPm {
    Apt,
    Dnf,
    Pacman,
    Zypper,
}

impl LinuxPm {
    const ALL: [LinuxPm; 4] = [LinuxPm::Apt, LinuxPm::Dnf, LinuxPm::Pacman, LinuxPm::Zypper];

    fn binary(self) -> &'static str {
        match self {
            Self::Apt => "apt",
            Self::Dnf => "dnf",
            Self::Pacman => "pacman",
            Self::Zypper => "zypper",
        }
    }

    /// The first package manager the system has, in the order distros are most common.
    pub fn detect(has: impl Fn(&str) -> bool) -> Option<LinuxPm> {
        Self::ALL.into_iter().find(|pm| has(pm.binary()))
    }
}

impl Platform {
    pub fn current() -> Platform {
        if cfg!(target_os = "macos") {
            Platform::MacOs
        } else if cfg!(target_os = "windows") {
            Platform::Windows
        } else {
            Platform::Linux(LinuxPm::detect(|b| which::which(b).is_ok()))
        }
    }
}

/// One shell command. `unattended` flags are added when Mosaic runs it but
/// left out of the text shown to the user, who answers any prompts themselves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub program: &'static str,
    pub args: Vec<&'static str>,
    pub unattended: &'static [&'static str],
}

impl Step {
    pub fn display(&self) -> String {
        std::iter::once(self.program).chain(self.args.iter().copied()).collect::<Vec<_>>().join(" ")
    }

    pub fn run_args(&self) -> Vec<String> {
        self.args.iter().chain(self.unattended).map(|s| s.to_string()).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallPlan {
    /// Empty when nothing is missing, or on a Linux without a known package manager.
    pub steps: Vec<Step>,
    /// The package manager the steps call, named for a "install it first" message.
    pub manager: Option<&'static str>,
    /// Whether Mosaic may run the steps itself. Linux needs root, which a GUI
    /// can't ask for reliably across distros, so there the user runs them.
    pub runnable: bool,
}

const WINGET_UNATTENDED: &[&str] =
    &["--accept-package-agreements", "--accept-source-agreements", "--disable-interactivity"];

pub fn install_plan(platform: Platform, missing: &[Tool]) -> InstallPlan {
    let needs_ffmpeg = missing.iter().any(|t| matches!(t, Tool::Ffmpeg | Tool::Ffprobe));
    let needs_mediainfo = missing.contains(&Tool::MediaInfo);
    let packages = |ffmpeg: &'static str, mediainfo: &'static str| {
        [needs_ffmpeg.then_some(ffmpeg), needs_mediainfo.then_some(mediainfo)]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
    };
    let one_step = |program, mut args: Vec<&'static str>, pkgs: Vec<&'static str>| {
        if pkgs.is_empty() { return vec![]; }
        args.extend(pkgs);
        vec![Step { program, args, unattended: &[] }]
    };

    match platform {
        Platform::MacOs => InstallPlan {
            steps: one_step("brew", vec!["install"], packages("ffmpeg-full", "media-info")),
            manager: Some("Homebrew"),
            runnable: true,
        },
        // One package per command: older winget versions take only one per call.
        Platform::Windows => InstallPlan {
            steps: packages("Gyan.FFmpeg", "MediaArea.MediaInfo")
                .into_iter()
                .map(|id| Step {
                    program: "winget",
                    args: vec!["install", "--exact", "--id", id, "--source", "winget"],
                    unattended: WINGET_UNATTENDED,
                })
                .collect(),
            manager: Some("winget"),
            runnable: true,
        },
        Platform::Linux(pm) => {
            let pkgs = packages("ffmpeg", "mediainfo");
            let steps = match pm {
                Some(LinuxPm::Apt) => one_step("sudo", vec!["apt", "install"], pkgs),
                Some(LinuxPm::Dnf) => one_step("sudo", vec!["dnf", "install"], pkgs),
                Some(LinuxPm::Pacman) => one_step("sudo", vec!["pacman", "-S"], pkgs),
                Some(LinuxPm::Zypper) => one_step("sudo", vec!["zypper", "install"], pkgs),
                None => vec![],
            };
            InstallPlan { steps, manager: pm.map(LinuxPm::binary), runnable: false }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn displayed(plan: &InstallPlan) -> Vec<String> {
        plan.steps.iter().map(Step::display).collect()
    }

    #[test]
    fn macos_installs_the_full_ffmpeg_build_and_mediainfo_in_one_brew_call() {
        let plan = install_plan(Platform::MacOs, &Tool::ALL);
        assert_eq!(displayed(&plan), ["brew install ffmpeg-full media-info"]);
        assert!(plan.runnable);
        assert_eq!(plan.manager, Some("Homebrew"));
    }

    #[test]
    fn only_missing_packages_are_named() {
        assert_eq!(displayed(&install_plan(Platform::MacOs, &[Tool::MediaInfo])), ["brew install media-info"]);
        assert_eq!(displayed(&install_plan(Platform::MacOs, &[Tool::Ffprobe])), ["brew install ffmpeg-full"]);
    }

    #[test]
    fn missing_ffmpeg_and_ffprobe_need_the_ffmpeg_package_once() {
        let plan = install_plan(Platform::MacOs, &[Tool::Ffmpeg, Tool::Ffprobe]);
        assert_eq!(displayed(&plan), ["brew install ffmpeg-full"]);
    }

    #[test]
    fn nothing_missing_means_no_steps() {
        for platform in [Platform::MacOs, Platform::Windows, Platform::Linux(Some(LinuxPm::Apt))] {
            assert!(install_plan(platform, &[]).steps.is_empty());
        }
    }

    #[test]
    fn windows_runs_one_winget_install_per_package() {
        let plan = install_plan(Platform::Windows, &Tool::ALL);
        assert_eq!(
            displayed(&plan),
            [
                "winget install --exact --id Gyan.FFmpeg --source winget",
                "winget install --exact --id MediaArea.MediaInfo --source winget",
            ],
        );
        assert!(plan.runnable);
    }

    #[test]
    fn winget_runs_without_prompts_but_shows_the_plain_command() {
        let step = &install_plan(Platform::Windows, &[Tool::Ffmpeg]).steps[0];
        assert!(!step.display().contains("--accept"));
        let args = step.run_args();
        assert!(args.contains(&"--accept-package-agreements".to_string()));
        assert!(args.contains(&"--disable-interactivity".to_string()));
        assert_eq!(args[..4], ["install", "--exact", "--id", "Gyan.FFmpeg"]);
    }

    #[test]
    fn linux_shows_the_distro_command_but_never_runs_it() {
        let cases = [
            (LinuxPm::Apt, "sudo apt install ffmpeg mediainfo"),
            (LinuxPm::Dnf, "sudo dnf install ffmpeg mediainfo"),
            (LinuxPm::Pacman, "sudo pacman -S ffmpeg mediainfo"),
            (LinuxPm::Zypper, "sudo zypper install ffmpeg mediainfo"),
        ];
        for (pm, expected) in cases {
            let plan = install_plan(Platform::Linux(Some(pm)), &Tool::ALL);
            assert_eq!(displayed(&plan), [expected]);
            assert!(!plan.runnable);
        }
    }

    #[test]
    fn linux_without_a_known_package_manager_has_no_command() {
        let plan = install_plan(Platform::Linux(None), &Tool::ALL);
        assert!(plan.steps.is_empty());
        assert_eq!(plan.manager, None);
    }

    #[test]
    fn detect_prefers_apt_when_several_package_managers_exist() {
        assert_eq!(LinuxPm::detect(|b| b == "dnf" || b == "apt"), Some(LinuxPm::Apt));
        assert_eq!(LinuxPm::detect(|b| b == "pacman"), Some(LinuxPm::Pacman));
        assert_eq!(LinuxPm::detect(|_| false), None);
    }
}
