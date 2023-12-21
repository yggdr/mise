use crate::config::{Config, Settings};
use crate::install_context::InstallContext;
use crate::toolset::{ToolVersion, Toolset};
use crate::ui::multi_progress_report::MultiProgressReport;
use crate::{runtime_symlinks, shims};
use console::style;
use eyre::Result;
use itertools::Itertools;
use std::sync::{Arc, Mutex};
use std::thread;

pub struct Installer<'a> {
    pub ts: &'a mut Toolset,
    pub mpr: Option<&'a MultiProgressReport>,
    pub force: bool,
    pub jobs: usize,
    pub raw: bool,
    pub latest_versions: bool,
}

impl<'a> Installer<'a> {
    pub fn new(ts: &'a mut Toolset) -> Self {
        let settings = Settings::try_get()?;
        Self {
            ts,
            mpr: None,
            force: false,
            jobs: settings.jobs,
            raw: settings.raw,
            latest_versions: false,
        }
    }

    pub fn with_mpr(mut self, mpr: &'a MultiProgressReport) -> Self {
        self.mpr = Some(mpr);
        self
    }

    pub fn with_force(mut self, force: bool) -> Self {
        self.force = force;
        self
    }

    pub fn with_jobs(mut self, jobs: usize) -> Self {
        self.jobs = jobs;
        self
    }

    pub fn with_raw(mut self, raw: bool) -> Self {
        self.raw = raw;
        if raw {
            self.jobs = 1;
        }
        self
    }

    pub fn with_latest_versions(mut self, latest_versions: bool) -> Self {
        self.latest_versions = latest_versions;
        self
    }

    pub fn install(mut self, versions: Vec<ToolVersion>) -> Result<()> {
        if versions.is_empty() {
            return Ok(());
        }
        let config = Config::try_get()?;
        let queue: Vec<_> = versions
            .into_iter()
            .group_by(|v| v.plugin_name.clone())
            .into_iter()
            .map(|(pn, v)| (config.get_or_create_plugin(&pn), v.collect_vec()))
            .collect();
        let mut _mpr: Option<MultiProgressReport> = None;
        let mpr = self
            .mpr
            .unwrap_or_else(|| _mpr.insert(MultiProgressReport::new()));
        for (t, _) in &queue {
            if !t.is_installed() {
                t.ensure_installed(mpr, false)?;
            }
        }
        let queue = Arc::new(Mutex::new(queue));
        thread::scope(|s| {
            (0..self.jobs)
                .map(|_| {
                    let queue = queue.clone();
                    let ts = &*self.ts;
                    let mpr = &mpr;
                    s.spawn(move || {
                        let next_job = || queue.lock().unwrap().pop();
                        while let Some((t, versions)) = next_job() {
                            for tv in versions {
                                let prefix = format!("{}", style(&tv).cyan().for_stderr());
                                let ctx = InstallContext {
                                    ts,
                                    tv: tv.request.resolve(
                                        t.as_ref(),
                                        tv.opts.clone(),
                                        opts.latest_versions,
                                    )?,
                                    pr: mpr.add(&prefix),
                                    raw,
                                    force: opts.force,
                                };
                                t.install_version(ctx)?;
                            }
                        }
                        Ok(())
                    })
                })
                .collect_vec()
                .into_iter()
                .map(|t| t.join().unwrap())
                .collect::<eyre::Result<Vec<()>>>()
        })?;
        self.ts.resolve(&config);
        shims::reshim(&config, self.ts)?;
        runtime_symlinks::rebuild(&config)
    }
}
