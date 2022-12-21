use super::cli::CreateCertCa;
use super::error::CertError;
use crate::command::Command;
use aws_config::meta::region::RegionProviderChain;
use certificate::KeyCertPair;
use certificate::NewCertificateConfig;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::prelude::*;
use std::path::Path;
use tedge_config::*;
use tedge_utils::paths::set_permission;
use tedge_utils::paths::validate_parent_dir_exists;

/// Create a self-signed device certificate
pub struct CreateCertCmd {
    /// The device identifier
    pub id: String,

    /// The path where the device certificate will be stored
    pub cert_path: FilePath,

    /// The path where the device private key will be stored
    pub key_path: FilePath,

    pub ca: Option<CreateCertCa>,
}

impl Command for CreateCertCmd {
    fn description(&self) -> String {
        format!("create a test certificate for the device {}.", self.id)
    }

    fn execute(&self) -> anyhow::Result<()> {
        let config = NewCertificateConfig::default();
        match self.ca {
            None => {
                self.create_test_certificate(&config).unwrap();
            }
            Some(CreateCertCa::AWS) => {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap()
                    .block_on(async {
                        let region_provider =
                            RegionProviderChain::default_provider().or_else("eu-central-1");
                        let aws_config =
                            aws_config::from_env().region(region_provider).load().await;
                        let client = aws_sdk_iot::Client::new(&aws_config);

                        let self_cert =
                            KeyCertPair::new_selfsigned_certificate(&config, &self.id).unwrap();
                        let csr = self_cert.csr_string().unwrap();

                        let csr_response = client
                            .create_certificate_from_csr()
                            .certificate_signing_request(csr)
                            .send()
                            .await
                            .unwrap();

                        let certificate_pem = csr_response.certificate_pem().unwrap();
                        self.create_certificate_with_pem(&config, certificate_pem)
                            .unwrap();
                    });
            }
        }
        eprintln!("Certificate was successfully created");
        Ok(())
    }
}

impl CreateCertCmd {
    fn create_certificate_with_pem(
        &self,
        config: &NewCertificateConfig,
        cert_pem: &str,
    ) -> Result<(), CertError> {
        let cert = KeyCertPair::new_selfsigned_certificate(config, &self.id)?;
        self.create_certificate_from_parts(&cert, cert_pem)
    }

    fn create_test_certificate(&self, config: &NewCertificateConfig) -> Result<(), CertError> {
        let cert = KeyCertPair::new_selfsigned_certificate(config, &self.id)?;
        let cert_pem = cert.certificate_pem_string().unwrap();
        self.create_certificate_from_parts(&cert, &cert_pem)
    }

    /// Saves a certificate and a private key into the `cert_path` and `key_path` directories. Cert
    /// comes from `cert_pem` parameter and private key comes from `cert` parameter. I've done it
    /// like this because it's scary crypto stuff I didn't want to break, but the readability here
    /// should definitely be improved.
    ///
    /// TODO: clean up this method mess
    fn create_certificate_from_parts(
        &self,
        cert: &KeyCertPair,
        cert_pem: &str,
    ) -> Result<(), CertError> {
        validate_parent_dir_exists(&self.cert_path).map_err(CertError::CertPathError)?;
        validate_parent_dir_exists(&self.key_path).map_err(CertError::KeyPathError)?;

        // Creating files with permission 644 owned by the MQTT broker
        let mut cert_file =
            create_new_file(&self.cert_path, crate::BROKER_USER, crate::BROKER_GROUP)
                .map_err(|err| err.cert_context(self.cert_path.clone()))?;
        let mut key_file = create_new_file(&self.key_path, crate::BROKER_USER, crate::BROKER_GROUP)
            .map_err(|err| err.key_context(self.key_path.clone()))?;

        cert_file.write_all(cert_pem.as_bytes())?;
        cert_file.sync_all()?;

        // Prevent the certificate to be overwritten
        set_permission(&cert_file, 0o444)?;

        {
            // Make sure the key is secret, before write
            set_permission(&key_file, 0o600)?;

            // Zero the private key on drop
            let cert_key = cert.private_key_pem_string()?;
            key_file.write_all(cert_key.as_bytes())?;
            key_file.sync_all()?;

            // Prevent the key to be overwritten
            set_permission(&key_file, 0o400)?;
        }

        Ok(())
    }
}

fn create_new_file(path: impl AsRef<Path>, user: &str, group: &str) -> Result<File, CertError> {
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path.as_ref())?;

    // Ignore errors - This was the behavior with the now deprecated user manager.
    // - When `tedge cert create` is not run as root, a certificate is created but owned by the user running the command.
    // - A better approach could be to remove this `chown` and run the command as mosquitto.
    let _ = tedge_utils::file::change_user_and_group(path.as_ref(), user, group);

    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_matches::assert_matches;
    use std::fs;
    use tempfile::*;

    #[test]
    fn basic_usage() {
        let dir = tempdir().unwrap();
        let cert_path = temp_file_path(&dir, "my-device-cert.pem");
        let key_path = temp_file_path(&dir, "my-device-key.pem");
        let id = "my-device-id";

        let cmd = CreateCertCmd {
            id: String::from(id),
            cert_path: cert_path.clone(),
            key_path: key_path.clone(),
            ca: None,
        };

        assert_matches!(
            cmd.create_test_certificate(&NewCertificateConfig::default()),
            Ok(())
        );
        assert_eq!(parse_pem_file(&cert_path).unwrap().tag, "CERTIFICATE");
        assert_eq!(parse_pem_file(&key_path).unwrap().tag, "PRIVATE KEY");
    }

    #[test]
    fn check_certificate_is_not_overwritten() {
        let dir = tempdir().unwrap();

        let cert_path = temp_file_path(&dir, "my-device-cert.pem");
        let key_path = temp_file_path(&dir, "my-device-key.pem");

        let cert_content = "some cert content";
        let key_content = "some key content";

        fs::write(&cert_path, cert_content).unwrap();
        fs::write(&key_path, key_content).unwrap();

        let cmd = CreateCertCmd {
            id: "my-device-id".into(),
            cert_path: cert_path.clone(),
            key_path: key_path.clone(),
            ca: None,
        };

        assert!(cmd
            .create_test_certificate(&NewCertificateConfig::default())
            .ok()
            .is_none());

        assert_eq!(fs::read(&cert_path).unwrap(), cert_content.as_bytes());
        assert_eq!(fs::read(&key_path).unwrap(), key_content.as_bytes());
    }

    #[test]
    fn create_certificate_in_non_existent_directory() {
        let dir = tempdir().unwrap();
        let key_path = temp_file_path(&dir, "my-device-key.pem");
        let cert_path = FilePath::from("/non/existent/cert/path");

        let cmd = CreateCertCmd {
            id: "my-device-id".into(),
            cert_path,
            key_path,
            ca: None,
        };

        let cert_error = cmd
            .create_test_certificate(&NewCertificateConfig::default())
            .unwrap_err();
        assert_matches!(cert_error, CertError::CertPathError { .. });
    }

    #[test]
    fn create_key_in_non_existent_directory() {
        let dir = tempdir().unwrap();
        let cert_path = temp_file_path(&dir, "my-device-cert.pem");
        let key_path = FilePath::from("/non/existent/key/path");

        let cmd = CreateCertCmd {
            id: "my-device-id".into(),
            cert_path,
            key_path,
            ca: None,
        };

        let cert_error = cmd
            .create_test_certificate(&NewCertificateConfig::default())
            .unwrap_err();
        assert_matches!(cert_error, CertError::KeyPathError { .. });
    }

    fn temp_file_path(dir: &TempDir, filename: &str) -> FilePath {
        dir.path().join(filename).into()
    }

    fn parse_pem_file(path: impl AsRef<Path>) -> Result<pem::Pem, String> {
        let content = fs::read(path).map_err(|err| err.to_string())?;
        pem::parse(content).map_err(|err| err.to_string())
    }
}
