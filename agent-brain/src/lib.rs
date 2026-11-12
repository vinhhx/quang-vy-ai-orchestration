//! agent-brain: lõi điều phối Lead → plan → Dev → QA (Phase 3).

pub mod answers;
pub mod app;
pub mod autopilot;
pub mod config;
pub mod container;
pub mod dashboard;
pub mod departments;
pub mod db;
pub mod doctor;
pub mod dsh;
pub mod engine;
pub mod error;
pub mod fastpath;
pub mod fixbug;
pub mod gitops;
pub mod guard;
pub mod human;
pub mod inputs;
pub mod journal;
pub mod knowledge;
pub mod orchestrator;
pub mod planfile;
pub mod preflight;
pub mod pricing;
pub mod procs;
pub mod profile;
pub mod projects;
pub mod protocol_paths;
pub mod prompts;
pub mod quota;
pub mod report;
pub mod roles;
pub mod routing;
pub mod runner;
pub mod secrets;
pub mod skills;
pub mod staff;
pub mod stats;
pub mod stream;
pub mod tasks;
pub mod teams;
pub mod telegram;
pub mod term;
pub mod workflows;
pub mod workspace;

pub mod tracing {
    #[macro_export]
    macro_rules! tracing_warn {
        ($($arg:tt)*) => {
            eprintln!($($arg)*);
        };
    }
    pub use tracing_warn as warn;
}
