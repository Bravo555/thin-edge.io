use std::fs;
use std::io;

use super::error::CertError;
use crate::command::Command;
use tedge_config::*;

/// Remove the device certificate
pub struct RemoveCertCmd {
    /// The path of the certificate to be removed
    pub cert_path: FilePath,

    /// The path of the private key to be removed
    pub key_path: FilePath,

    /// The path of the certificate signing request to be removed
    pub csr_path: FilePath,
}

impl Command for RemoveCertCmd {
    fn description(&self) -> String {
        "remove the device certificate".into()
    }

    fn execute(&self) -> anyhow::Result<()> {
        match self.remove_certificate()? {
            RemoveCertResult::Removed => eprintln!("Certificate was successfully removed"),
            RemoveCertResult::NotFound => eprintln!("There is no certificate to remove"),
        }
        Ok(())
    }
}

impl RemoveCertCmd {
    fn remove_certificate(&self) -> Result<RemoveCertResult, CertError> {
        match fs::remove_file(&self.cert_path)
            .and_then(|()| fs::remove_file(&self.key_path))
            .and_then(|()| fs::remove_file(&self.csr_path))
        {
            Ok(()) => Ok(RemoveCertResult::Removed),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(RemoveCertResult::NotFound),
            Err(err) => Err(err.into()),
        }
    }
}

enum RemoveCertResult {
    Removed,
    NotFound,
}
