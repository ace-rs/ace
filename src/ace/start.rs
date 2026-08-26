use std::collections::HashMap;
use std::io::Write;

use crate::actions::project::{Prepare, PrepareError};
use crate::backend::{
    BackendError, BackendMode, OneShotOptions, PromptInput, ResumeMode, SessionOptions,
};
use crate::config::ConfigError;
use crate::config::ace_toml::Trust;
use crate::config::resolve::Source;
use crate::school::SchoolError;
use crate::templates::session::{SessionPromptInput, build_session_prompt};

use super::{Ace, IoError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartMode {
    OneShot {
        prompt: String,
    },
    Session {
        resume: ResumeMode,
        backend: BackendMode,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum StartError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Config(#[from] ConfigError),
    #[error("{0}")]
    Backend(#[from] BackendError),
    #[error("{0}")]
    School(#[from] SchoolError),
    #[error("{0}")]
    Prepare(#[from] PrepareError),
    #[error("{0}")]
    Prompt(#[from] IoError),
    #[error("invalid start mode")]
    InvalidStartMode,
}

impl StartError {
    pub fn hint(&self) -> Option<&'static str> {
        match self {
            Self::School(error) => error.hint(),
            Self::Prepare(error) => error.hint(),
            Self::Prompt(error) => error.hint(),
            Self::Io(_) | Self::Config(_) | Self::Backend(_) | Self::InvalidStartMode => None,
        }
    }
}

impl Ace {
    pub fn start(&mut self, mode: StartMode) -> Result<(), StartError> {
        if matches!(
            &mode,
            StartMode::Session {
                backend: BackendMode::WithServer,
                ..
            }
        ) {
            return Err(StartError::InvalidStartMode);
        }

        self.require_config_or_recover()?;

        let (specifier, school_from) = {
            let resolved = self.require_config()?;
            let specifier = resolved
                .school_specifier
                .value
                .clone()
                .ok_or(SchoolError::NoSpecifier)?;
            (specifier, resolved.school_specifier.from)
        };

        if let Some(notice) = school_source_notice(school_from, &specifier) {
            self.info(&notice);
        }

        let prepare_result = (Prepare {
            specifier: &specifier,
        })
        .run(self)?;

        let project_dir = self.project_dir().to_path_buf();
        let school_clone = self.require_linked_school()?.clone_path.clone();
        let (school_name, school_session_prompt) = {
            let school = self.school()?;
            (school.name.clone(), school.session_prompt.clone())
        };
        let backend_dir = project_dir.join(self.backend()?.backend_dir());
        let (resolved_session_prompt, trust, resume_preference, env) = {
            let resolved = self.require_config()?;
            let env: HashMap<String, String> = resolved
                .env
                .iter()
                .map(|(key, value)| (key.clone(), value.value.clone()))
                .collect();
            (
                resolved.session_prompt.value.clone(),
                resolved.trust.value,
                resolved.resume.value,
                env,
            )
        };
        let excluded_skills = self.excluded_skills();
        let session_prompt = build_session_prompt(&SessionPromptInput {
            school_name: &school_name,
            school_session_prompt: &school_session_prompt,
            project_session_prompt: &resolved_session_prompt,
            backend_dir: &backend_dir,
            changes: &prepare_result.changes,
            school_clone: school_clone.as_deref(),
            school_is_dirty: prepare_result.school_is_dirty,
            excluded_skills: &excluded_skills,
        });

        match mode {
            StartMode::OneShot { prompt } => self.start_one_shot(prompt, project_dir, env),
            StartMode::Session { resume, backend } => {
                let resume = match (resume, resume_preference) {
                    (ResumeMode::Latest, true) => ResumeMode::Latest,
                    (ResumeMode::Latest, false) | (ResumeMode::Fresh, _) => ResumeMode::Fresh,
                };

                self.start_session(SessionOptions {
                    trust,
                    session_prompt,
                    project_dir,
                    env,
                    extra_args: self.backend_args.clone(),
                    resume,
                    backend_mode: backend,
                })
            }
        }
    }

    fn start_one_shot(
        &mut self,
        prompt: String,
        project_dir: std::path::PathBuf,
        env: HashMap<String, String>,
    ) -> Result<(), StartError> {
        let output = self.backend()?.exec_one_shot(OneShotOptions {
            prompt: PromptInput::Inline(prompt),
            project_dir,
            env,
            extra_args: self.backend_args.clone(),
        })?;

        std::io::stdout().write_all(&output.stdout)?;
        std::io::stderr().write_all(&output.stderr)?;
        if !output.status.success() {
            std::process::exit(output.status.code().unwrap_or(1));
        }

        Ok(())
    }

    fn start_session(&mut self, options: SessionOptions) -> Result<(), StartError> {
        let backend = self.backend()?.clone();
        if backend.kind.supports_trust(options.trust) {
            match options.trust {
                Trust::Auto => self.info("auto mode — AI decides approvals"),
                Trust::Yolo => self.warn("yolo mode — permission prompts disabled"),
                Trust::Default => {}
            }
        } else {
            self.warn(&format!(
                "{} does not support {} trust — running with its default permissions",
                backend.kind.name(),
                options.trust.label(),
            ));
        }

        if matches!(options.resume, ResumeMode::Latest) {
            self.hint("Resuming previous session. If this fails, run: ace new");
        }
        self.separator();

        backend.exec_session(options)?;

        Ok(())
    }

    fn require_config_or_recover(&mut self) -> Result<(), StartError> {
        self.require_config()?;
        match self.backend() {
            Ok(_) => Ok(()),
            Err(BackendError::Unknown(name)) => self.recover_backend(&name),
            Err(error) => Err(error.into()),
        }
    }

    fn recover_backend(&mut self, attempted: &str) -> Result<(), StartError> {
        if !self.can_ask() {
            self.hint(&format!(
                "to fix: ace config set backend <name> (registry has no `{attempted}`)"
            ));
            return Err(BackendError::Unknown(attempted.to_string()).into());
        }

        let names = self.known_backend_names()?;
        self.warn(&format!("backend `{attempted}` is not in the registry"));
        let pick = self.prompt_select("Pick a backend for this session:", names)?;
        self.override_backend(pick.clone());
        self.backend()?;
        self.hint(&format!("to make permanent: ace config set backend {pick}"));
        Ok(())
    }
}

fn school_source_notice(from: Source, specifier: &str) -> Option<String> {
    match from {
        Source::User => Some(format!("school {specifier} — user config")),
        Source::Local => Some(format!("school {specifier} — local config")),
        Source::Project | Source::Override | Source::School | Source::Default => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::paths::AcePaths;

    #[test]
    fn user_school_is_announced() {
        let notice = school_source_notice(Source::User, "ace-rs/school");
        assert_eq!(
            notice.as_deref(),
            Some("school ace-rs/school — user config")
        );
    }

    #[test]
    fn local_school_is_announced() {
        let notice = school_source_notice(Source::Local, "ace-rs/school");
        assert_eq!(
            notice.as_deref(),
            Some("school ace-rs/school — local config")
        );
    }

    #[test]
    fn project_school_is_silent() {
        assert!(school_source_notice(Source::Project, "ace-rs/school").is_none());
    }

    #[test]
    fn typed_override_is_silent() {
        assert!(school_source_notice(Source::Override, "ace-rs/school").is_none());
    }

    #[test]
    fn unavailable_backend_mode_fails_before_configuration() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let project_dir = temp.path().to_path_buf();
        let paths = AcePaths {
            user: project_dir.join("user.toml"),
            project: project_dir.join("ace.toml"),
            local: project_dir.join("ace.local.toml"),
            cache: project_dir.join("cache"),
        };
        let mut ace = Ace::new(project_dir, paths, super::super::Io::new(false, true));

        let error = ace
            .start(StartMode::Session {
                resume: ResumeMode::Latest,
                backend: BackendMode::WithServer,
            })
            .expect_err("server-backed start should not prepare the project yet");

        assert!(matches!(error, StartError::InvalidStartMode));
    }
}
