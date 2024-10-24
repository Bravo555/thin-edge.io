//! Handles cloud-agnostic operations.
//!
//! The Tedge Agent addresses cloud-agnostic software management operations e.g.
//! listing current installed software list, software update, software removal.
//! Also, the Tedge Agent calls an SM Plugin(s) to execute an action defined by
//! a received operation.
//!
//! It also has following capabilities:
//!
//! - File transfer HTTP server
//! - Restart management
//! - Software management

use std::sync::Arc;

use agent::AgentConfig;
use camino::Utf8PathBuf;
use tedge_config::get_config_dir;
use tedge_config::system_services::get_log_level;
use tedge_config::system_services::set_log_level;
use tracing::log::warn;

mod agent;
mod device_profile_manager;
mod file_transfer_server;
mod operation_file_cache;
mod operation_workflows;
mod restart_manager;
mod software_manager;
mod state_repository;
mod tedge_to_te_converter;

#[derive(Debug, Clone, clap::Parser)]
#[clap(
name = clap::crate_name!(),
version = clap::crate_version!(),
about = clap::crate_description!()
)]
pub struct AgentOpt {
    /// Turn-on the debug log level.
    ///
    /// If off only reports ERROR, WARN, and INFO
    /// If on also reports DEBUG
    #[clap(long)]
    pub debug: bool,

    /// Logging level.
    ///
    /// One of error/warn/info/debug/trace. Takes precedence over `--debug`
    #[clap(long)]
    pub log_level: Option<tracing::Level>,

    /// Start the agent with clean session off, subscribe to the topics, so that no messages are lost
    #[clap(short, long)]
    pub init: bool,

    /// Start the agent from custom path
    ///
    /// [env: TEDGE_CONFIG_DIR, default: /etc/tedge]
    #[clap(
        long = "config-dir",
        default_value = get_config_dir().into_os_string(),
        hide_env_values = true,
        hide_default_value = true,
    )]
    pub config_dir: Utf8PathBuf,

    /// The device MQTT topic identifier
    #[clap(long)]
    pub mqtt_device_topic_id: Option<Arc<str>>,

    /// MQTT root prefix
    #[clap(long)]
    pub mqtt_topic_root: Option<Arc<str>>,
}

pub async fn run(agent_opt: AgentOpt) -> Result<(), anyhow::Error> {
    let tedge_config_location =
        tedge_config::TEdgeConfigLocation::from_custom_root(agent_opt.config_dir.clone());

    // If `--level` was provided, use that log level.
    // If `debug` is `false` then only `error!`, `warn!` and `info!` are reported.
    // If `debug` is `true` then also `debug!` is reported.
    // If neither was provided, use a log level from a config file.
    let log_level = agent_opt
        .log_level
        .or(agent_opt.debug.then_some(tracing::Level::DEBUG));

    let log_level = match log_level {
        Some(log_level) => log_level,
        None => get_log_level("tedge-agent", &tedge_config_location.tedge_config_root_path)?,
    };

    set_log_level(log_level);

    let init = agent_opt.init;

    let agent = agent::Agent::try_new(
        "tedge-agent",
        AgentConfig::from_config_and_cliopts(&tedge_config_location, agent_opt)?,
    )?;

    if init {
        warn!("This --init option has been deprecated and will be removed in a future release");
        return Ok(());
    } else {
        agent.start().await?;
    }
    Ok(())
}

#[derive(Debug, serde::Deserialize, Clone)]
pub struct Capabilities {
    config_update: bool,
    config_snapshot: bool,
    log_upload: bool,
}

#[cfg(test)]
impl Default for Capabilities {
    fn default() -> Self {
        Capabilities {
            config_update: true,
            config_snapshot: true,
            log_upload: true,
        }
    }
}
