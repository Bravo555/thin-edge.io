use crate::command::Command;
use tedge_config::TEdgeConfigLocation;
use tedge_config::WritableKey;

pub struct AddConfigCommand {
    pub key: WritableKey,
    pub value: String,
    pub config_location: TEdgeConfigLocation,
}

impl Command for AddConfigCommand {
    fn description(&self) -> String {
        format!(
            "set the configuration key: '{}' with value: {}.",
            self.key.to_cow_str(),
            self.value
        )
    }

    fn execute(&self) -> anyhow::Result<()> {
        self.config_location.update_toml(&|dto, reader| {
            dto.try_append_str(reader, &self.key, &self.value)
                .map_err(|e| e.into())
        })?;
        Ok(())
    }
}
