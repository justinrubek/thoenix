use crate::AppResult;
use project_base_directory::Project;
use std::{os::unix::prelude::PermissionsExt, path::PathBuf};
use tracing::{debug, error, info};

impl crate::commands::Terraform {
    /// Calls terraform's cli, but with some extra preparation to build the configuration
    /// The configuration's derivation is built using `nix eval --raw .#terraformConfiguration/{configuration_name}`.
    /// The configuration's `config.tf.json` file is copied to the configuration directory in the local repository.
    /// Finally the terraform cli is called with the given arguments. It will detect the
    /// `config.tf.json` file in addition to any HCL files in the configuration directory.
    pub async fn spawn_command(self) -> AppResult<tokio::process::Child> {
        info!(?self.args, ?self.configuration_name);

        info!("building terraform configuration");
        let nix_eval = tokio::process::Command::new("nix")
            .arg("build")
            .arg("--no-link")
            .arg("--print-out-paths")
            .arg(format!(
                ".#terraformConfiguration/{}",
                self.configuration_name
            ))
            .output()
            .await?;

        if !nix_eval.status.success() {
            // TODO: better detect the error that is happening. It is likely that the configuration
            // does not exist in the nix flake if it fails here
            let err = std::str::from_utf8(&nix_eval.stderr)?;
            error!("{:?}", err);
            return Err(crate::error::AppError::Nix(
                nix_eval.status.code().unwrap_or(1),
            ));
        }

        let derivation_path = std::str::from_utf8(&nix_eval.stdout)?.trim();
        debug!(?derivation_path, "built terraform configuration");
        let generated_configuration_path = PathBuf::from(derivation_path).join("config.tf.json");

        let project = Project::discover_and_assume().await?;
        let repo_path = project
            .root_directory
            .expect("failed to determine project root directory");

        // TODO: Support other terraform configuration directories?
        let configuration_directory = repo_path
            .join("terraform")
            .join("configurations")
            .join(self.configuration_name);
        let destination = configuration_directory.join("config.tf.json");

        // Only attempt to copy the nix generated configuration if it exists
        if generated_configuration_path.exists() {
            info!(
                "copying generated config file {:?} -> {:?}",
                generated_configuration_path, destination
            );
            tokio::fs::copy(&generated_configuration_path, &destination).await?;

            // mark the destination file as writeable for the current user as it came from the read only nix store
            let mut perms = tokio::fs::metadata(&destination).await?.permissions();
            perms.set_mode(0o644);
            info!("-> setting permissions to {:?}", perms);
            tokio::fs::set_permissions(&destination, perms).await?;
        }

        // Call the terraform executable with the provided args
        info!(?configuration_directory, ?self.args, "invoking terraform");
        let terraform = tokio::process::Command::new(&self.command)
            .arg(format!("-chdir={}", configuration_directory.display()))
            .args(self.args)
            .spawn()?;

        Ok(terraform)
    }
}
